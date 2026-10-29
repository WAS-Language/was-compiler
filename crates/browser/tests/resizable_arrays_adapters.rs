#[path = "../../../tests/support/process.rs"]
mod test_process;

use std::{path::PathBuf, process::Command, time::Duration};
use was_compiler::{
    ArrayElementType, Compiler, ExportOptions, OptimizationLevel, Type, diagnostic,
};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef, Validator};

const LEVELS: [OptimizationLevel; 4] = [
    OptimizationLevel::None,
    OptimizationLevel::Basic,
    OptimizationLevel::Standard,
    OptimizationLevel::Aggressive,
];
const EXPORTS: [&str; 6] = [
    "numbers",
    "flags",
    "texts",
    "main",
    "fixed_trap",
    "empty_trap",
];
const SOURCE: &str = r#"
fn numbers() -> number[] {
    let source = host_numbers([10, 20, 30]);
    let a = resizable(source);
    let alias = a;
    let mutate = () => {
        alias.reserve(64);
        alias.push(40);
        return alias.remove(1);
    };
    a[0] += mutate();
    a.pop();
    a.push(-0);
    return a;
}
fn flags() -> bool[] {
    let a = resizable(host_flags([true, false, true]));
    let mutate = () => {
        let nested = () => { a.reserve(64); a.push(false); };
        nested();
        return a.remove(-0);
    };
    mutate();
    a.pop();
    a.push(true);
    return a;
}
fn texts() -> string[] {
    let a = resizable(host_texts(["é", "λ", "tail"]));
    let alias = a;
    let mutate = () => {
        alias.push({ alias.reserve(100); alias.push("inner"); "outer" });
        return alias.remove({ alias.pop(); 1 });
    };
    mutate();
    a.pop();
    return a;
}
fn main() -> bool {
    let source = [1];
    let copy = resizable(source);
    copy[0] = 2;
    let flags: bool[] = resizable(if true { [] } else { [] });
    flags.push(true);
    let words: string[] = resizable(resizable([]));
    words.push("λ");
    let resizable = (n: number) => n + 1;
    return source[0] == 1 && copy[0] == 2 && flags.pop()
        && words.remove(0) == "λ" && resizable(2) == 3;
}
fn fixed_trap() -> number { let a = [1]; return a.remove(mark(9)); }
fn empty_trap() -> number { let a = resizable([]); a.push(mark(7)); a.pop(); return a.pop(); }
fn hidden() -> range { return 0..2; }
"#;

fn compiler() -> Compiler {
    let mut compiler = Compiler::new();
    for (name, module, field, element) in [
        (
            "host_numbers",
            "resizable",
            "resizable",
            ArrayElementType::Number,
        ),
        ("host_flags", "env", "flags", ArrayElementType::Bool),
        ("host_texts", "env", "texts", ArrayElementType::String),
    ] {
        let ty = Type::Array(element);
        compiler
            .register_host_function(name, module, field, vec![ty], ty)
            .unwrap();
    }
    compiler
        .register_host_function("mark", "env", "mark", vec![Type::Number], Type::Number)
        .unwrap();
    compiler
}

fn invalid_sources() -> Vec<String> {
    let mut sources = [
        "fn main() { resizable([1]; }",
        "fn main() { resizable(); }",
        "fn main() { resizable([], []); }",
        "fn main() { resizable(source: []); }",
        "fn main() { resizable(true); }",
        "fn resizable() {} fn main() {}",
        "fn main() { let resizable = 1; resizable([]); }",
        "fn main() { [1] |> resizable; }",
        "fn main() { let a = resizable([true]); a.push(1); }",
        "fn main() { let a = resizable([1]); a.push(true); }",
        "fn main() { let a = resizable([\"λ\"]); a.push(false); }",
        "fn main() { let a = resizable([]); a.reserve(true); }",
        "fn main() { let a = resizable([]); a.remove(\"λ\"); }",
        "fn main() { let a = resizable([true]); let n: number = a.pop(); }",
        "fn main() { let a = resizable([\"λ\"]); let b: bool = a.remove(0); }",
        "fn main(a: bool[] = resizable([])) {}",
        "fn main(a: number[]) requires len(resizable(a)) == 0 {}",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    for (method, args, wrong_arity) in [
        ("reserve", "0", ""),
        ("push", "1", "1, 2"),
        ("pop", "", "1"),
        ("remove", "0", ""),
    ] {
        sources.extend([
            format!("fn main(a: number[]) {{ (a).{method}({args}); }}"),
            format!("fn main(a: number[]) {{ a.{method}({wrong_arity}); }}"),
            format!("fn main(a: number[]) {{ a.{method}(item: 0); }}"),
            format!("fn main(a: number) {{ a.{method}({args}); }}"),
            format!("fn main() {{ missing.{method}({args}); }}"),
            format!("fn main(a: number[]) {{ a.{method}({args}).pop(); }}"),
        ]);
    }
    sources
}

fn assert_surface(bytes: &[u8], selected: bool) {
    Validator::new().validate_all(bytes).unwrap();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut memories = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.unwrap() {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.unwrap();
                    assert!(matches!(import.ty, TypeRef::Func(_)));
                    imports.push((import.module.to_owned(), import.name.to_owned()));
                }
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export.unwrap();
                    if export.kind == ExternalKind::Memory {
                        assert_eq!(export.index, 0);
                    }
                    exports.push((export.name.to_owned(), export.kind));
                }
            }
            Payload::MemorySection(section) => {
                memories.extend(section.into_iter().map(Result::unwrap));
            }
            Payload::StartSection { .. } => panic!("no implicit guest start"),
            _ => {}
        }
    }
    assert_eq!(
        imports,
        [
            ("resizable".into(), "resizable".into()),
            ("env".into(), "flags".into()),
            ("env".into(), "texts".into()),
            ("env".into(), "mark".into()),
        ]
    );
    let mut expected = vec![
        ("memory".to_owned(), ExternalKind::Memory),
        ("__alloc".to_owned(), ExternalKind::Func),
    ];
    if selected {
        expected.extend(EXPORTS.map(|name| (name.to_owned(), ExternalKind::Func)));
    }
    exports.sort_by(|a, b| a.0.cmp(&b.0));
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(exports, expected, "no allocator-management exports");
    assert_eq!(memories.len(), 1);
    let memory = memories[0];
    assert_eq!(memory.initial, 1);
    assert_eq!(memory.maximum, Some(65_535));
    assert!(!memory.memory64 && !memory.shared);
}

fn field(input: &mut Vec<u8>, bytes: &[u8]) {
    input.extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_le_bytes());
    input.extend_from_slice(bytes);
}

// Build the same native oracle for the default fixture check and the ignored browser run.
fn browser_input() -> Vec<u8> {
    let mut input = Vec::new();
    field(&mut input, SOURCE.as_bytes());
    let mut native = compiler();
    let mut enabled: [Option<Vec<u8>>; 2] = [None, None];
    for level in LEVELS {
        native.set_optimization_level(level);
        for (index, selected) in [true, false].into_iter().enumerate() {
            native.set_exports(if selected {
                ExportOptions::Only(EXPORTS.map(String::from).to_vec())
            } else {
                ExportOptions::None
            });
            let bytes = native.compile_source(SOURCE).unwrap();
            assert_surface(&bytes, selected);
            if level != OptimizationLevel::None {
                if let Some(expected) = &enabled[index] {
                    assert_eq!(&bytes, expected, "enabled presets must be byte-identical");
                } else {
                    enabled[index] = Some(bytes.clone());
                }
            }
            field(&mut input, &bytes);
            field(
                &mut input,
                wasmprinter::print_bytes(&bytes).unwrap().as_bytes(),
            );
        }
    }
    let cases = invalid_sources();
    field(&mut input, cases.len().to_string().as_bytes());
    for source in cases {
        let mut expected = None;
        for level in LEVELS {
            native.set_optimization_level(level);
            let error = native.compile_source(&source).unwrap_err();
            let message = diagnostic::render("<browser>", &source, &error);
            if let Some(expected) = &expected {
                assert_eq!(&message, expected, "diagnostic parity: {source}");
            } else {
                expected = Some(message);
            }
        }
        field(&mut input, source.as_bytes());
        field(&mut input, expected.unwrap().as_bytes());
    }
    let error = native
        .register_host_function("resizable", "env", "bad", vec![], Type::Number)
        .unwrap_err();
    field(
        &mut input,
        format!("host registration error: {error}").as_bytes(),
    );
    input
}

#[test]
fn browser_resizable_native_fixtures_validate_surface_and_preset_parity() {
    browser_input();
}

#[test]
#[ignore = "requires Node.js and pkg freshly rebuilt from current source"]
fn rebuilt_browser_resizable_bytes_wat_diagnostics_and_execution_match_native() {
    let package = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../pkg"));
    assert!(package.join("was_compiler_bg.wasm").is_file());
    let input = browser_input();
    let script = r#"
        const assert = require('node:assert/strict'), fs = require('node:fs'), path = require('node:path');
        const {pathToFileURL} = require('node:url');
        async function main() {
            const input = fs.readFileSync(0); let offset = 0;
            function field() {
                const n = input.readUInt32LE(offset); offset += 4;
                const b = input.subarray(offset, offset+n); offset += n; return b;
            }
            const source = field().toString();
            const native = Array.from({length:4}, () => Array.from({length:2},
                () => ({bytes:field(), wat:field().toString()})));
            const count = Number(field().toString());
            const diagnostics = Array.from({length:count},
                () => ({source:field().toString(), message:field().toString()}));
            const registrationError = field().toString();
            assert.equal(offset, input.length);
            const dir = process.argv[1];
            const {default:init, Compiler} = await import(pathToFileURL(path.join(dir,'was_compiler.js')).href);
            await init({module_or_path:fs.readFileSync(path.join(dir,'was_compiler_bg.wasm'))});
            const c = new Compiler();
            const names = ['numbers','flags','texts','main','fixed_trap','empty_trap'];
            const imports = [
                {module:'resizable',name:'resizable',kind:'function'},
                {module:'env',name:'flags',kind:'function'},
                {module:'env',name:'texts',kind:'function'},
                {module:'env',name:'mark',kind:'function'}];
            const enabled = [];
            try {
                assert.throws(() => c.registerHostFunction('resizable','env','bad',[],'number'),
                    e => e instanceof Error && e.message === registrationError);
                c.registerHostFunction('host_numbers','resizable','resizable',['number[]'],'number[]');
                c.registerHostFunction('host_flags','env','flags',['bool[]'],'bool[]');
                c.registerHostFunction('host_texts','env','texts',['string[]'],'string[]');
                c.registerHostFunction('mark','env','mark',['number'],'number');
                for (let level=0; level<4; level++) {
                    c.setOptimizationLevel(level);
                    for (let index=0; index<2; index++) {
                        if (index === 0) c.setExports(names); else c.exportNone();
                        const bytes = c.compile(source);
                        assert.deepEqual(Buffer.from(bytes), native[level][index].bytes, `O${level} exports ${index}`);
                        assert.equal(c.compileWat(source), native[level][index].wat);
                        if (level === 1) enabled[index] = Buffer.from(bytes);
                        if (level > 1) assert.deepEqual(Buffer.from(bytes), enabled[index]);
                        assert.ok(WebAssembly.validate(bytes));
                        const m = new WebAssembly.Module(bytes);
                        assert.deepEqual(WebAssembly.Module.imports(m), imports);
                        const expected = [{name:'memory',kind:'memory'},{name:'__alloc',kind:'function'}];
                        if (index === 0) expected.push(...names.map(name => ({name,kind:'function'})));
                        const byName = (a,b) => a.name.localeCompare(b.name);
                        assert.deepEqual(WebAssembly.Module.exports(m).sort(byName), expected.sort(byName));
                        const calls = [], originals = [];
                        const keep = kind => h => { calls.push(kind); originals.push([kind,h]); return h; };
                        const e = new WebAssembly.Instance(m, {
                            resizable:{resizable:keep('number')},
                            env:{flags:keep('bool'),texts:keep('string'),mark(n) {calls.push(n); return n;}}
                        }).exports;
                        assert.ok(e.memory instanceof WebAssembly.Memory);
                        if (index !== 0) continue;
                        // Reacquire views and payload/length for each observation, never retain a movable borrow.
                        function text(h) {
                            const v = new DataView(e.memory.buffer);
                            const p = v.getUint32(h,true), n = v.getUint32(h+4,true);
                            return new TextDecoder('utf-8',{fatal:true}).decode(new Uint8Array(e.memory.buffer,p,n));
                        }
                        function array(kind,h) {
                            const v = new DataView(e.memory.buffer);
                            const p = v.getUint32(h,true), n = v.getUint32(h+4,true);
                            return Array.from({length:n}, (_,i) => kind === 'number' ? v.getFloat64(p+8*i,true)
                                : kind === 'bool' ? v.getUint8(p+i) : text(v.getUint32(p+4*i,true)));
                        }
                        const n = e.numbers();
                        assert.deepEqual(array('number',n), [30,30,-0]);
                        assert.ok(Object.is(array('number',n)[2],-0));
                        const b = e.flags();
                        assert.deepEqual(array('bool',b), [0,1,1]);
                        const s = e.texts();
                        assert.deepEqual(array('string',s), ['é','tail']);
                        assert.deepEqual(calls, ['number','bool','string']);
                        for (const [[kind,h],copy,values] of [
                            [originals[0],n,[10,20,30]],
                            [originals[1],b,[1,0,1]],
                            [originals[2],s,['é','λ','tail']]]) {
                            assert.notEqual(h,copy);
                            assert.deepEqual(array(kind,h),values);
                        }
                        assert.equal(e.main(),1);
                        assert.throws(() => e.fixed_trap(),WebAssembly.RuntimeError);
                        assert.deepEqual(calls,['number','bool','string']); // authority precedes mark(9)
                        assert.throws(() => e.empty_trap(),WebAssembly.RuntimeError);
                        assert.deepEqual(calls,['number','bool','string',7]);
                        assert.equal(e.main(),1); // caught guest traps do not poison later invocations
                        assert.deepEqual(array('number',n),[30,30,-0]);
                        assert.deepEqual(array('bool',b),[0,1,1]);
                        assert.deepEqual(array('string',s),['é','tail']);
                    }
                    for (const d of diagnostics) for (const method of ['compile','compileWat']) {
                        assert.throws(() => c[method](d.source),
                            e => e instanceof Error && e.message === d.message,
                            `O${level} ${method}: ${d.source}`);
                    }
                }
            } finally { c.free(); }
        }
        main().catch(e => { console.error(e); process.exitCode=1; });
    "#;
    let output = test_process::output(
        Command::new("node").args(["-e", script]).arg(package),
        Some(&input),
        Duration::from_secs(60),
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
