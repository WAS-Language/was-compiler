#[path = "../../../tests/support/process.rs"]
mod test_process;

use std::{path::PathBuf, process::Command, time::Duration};
use was_compiler::{Compiler, ExportOptions, OptimizationLevel, Type, diagnostic};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef, Validator};

const LEVELS: [OptimizationLevel; 4] = [
    OptimizationLevel::None,
    OptimizationLevel::Basic,
    OptimizationLevel::Standard,
    OptimizationLevel::Aggressive,
];
const BUILTINS: [&str; 5] = [
    "string_byte_len",
    "string_concat",
    "string_slice_bytes",
    "string_replace_all",
    "string_join",
];
const EXPORTS: [&str; 26] = [
    "bytes",
    "concat",
    "slice",
    "replace",
    "literal",
    "aliases",
    "slot",
    "lexical",
    "ordered_bytes",
    "ordered_concat",
    "ordered_slice",
    "ordered_replace",
    "left_trap",
    "right_trap",
    "publish",
    "fail",
    "outer",
    "stages",
    "join",
    "join_empty",
    "join_build",
    "join_captured",
    "join_ordered",
    "join_fail",
    "join_shadow",
    "join_stages",
];
const SOURCE: &str = r#"
fn bytes(s: string) { return string_byte_len(s); }
fn concat(a: string, b: string) { return string_concat(a, b); }
fn slice(s: string, a: number, b: number) { return string_slice_bytes(s, a, b); }
fn replace(s: string, n: string, r: string) { return string_replace_all(s, n, r); }
fn literal() { return "é\0🙂"; }
fn aliases() -> string[] {
    let s = "é\0🙂";
    let alias = s;
    let saved = [s, alias, "é\0🙂"];
    s = string_concat(s, "!");
    return [saved[0], saved[1], saved[2], alias, s,
        string_slice_bytes(alias, 2, 3), string_replace_all(alias, "é", "λ"), "é\0🙂"];
}
fn slot(a: string[]) { a[0] = string_replace_all(a[0], "é", "!"); }
fn lexical() -> bool {
    let string_byte_len = (s: string) => 7;
    let string_concat = (s: string) => s;
    let string_slice_bytes = (s: string) => s;
    let string_replace_all = (s: string) => s;
    return ("x" |> string_byte_len) == 7 && ("x" |> string_concat) == "x"
        && ("x" |> string_slice_bytes) == "x" && ("x" |> string_replace_all) == "x";
}
fn ordered_bytes(s: string) { return string_byte_len({ effect(1); s }); }
fn ordered_concat(a: string, b: string) {
    return string_concat({ effect(1); a }, { effect(2); b });
}
fn ordered_slice(s: string, a: number, b: number) {
    return string_slice_bytes({ effect(1); s }, { effect(2); a }, { effect(3); b });
}
fn ordered_replace(s: string, n: string, r: string) {
    return string_replace_all({ effect(1); s }, { effect(2); n }, { effect(3); r });
}
fn trap() -> string { let a: number[] = []; let n = a[0]; return "unused"; }
fn left_trap() {
    return string_concat({ effect(4); trap() }, { effect(5); "later" });
}
fn right_trap() {
    return string_replace_all({ effect(6); let s = string_concat("saved", "!"); retain(s); s },
        { effect(7); trap() }, { effect(8); "later" });
}
fn publish(s: string) {
    let a = string_concat(s, "!"); retain(a);
    let b = string_slice_bytes(a, 0, string_byte_len(s)); retain(b);
    let c = string_replace_all(a, "!", "?"); retain(c);
    return c;
}
fn fail(s: string) { publish(s); trap(); }
fn outer(s: string) {
    return string_concat(string_replace_all(s, "é", "λ"),
        { effect(9); string_slice_bytes(s, 0, string_byte_len(s)) });
}
fn stages() {
    let suffix = "!";
    let f = (s: string) => string_concat(s, suffix);
    return "é" |> f |> (string_slice_bytes(_, 0, 2))
        |> (string_replace_all(_, "é", "λ")) |> (string_byte_len(_));
}
fn join(parts: string[], separator: string) -> string { return string_join(parts, separator); }
fn join_empty() -> string { return string_join([], "unused"); }
fn join_build(tail: string) -> string {
    let parts: string[] = resizable([]);
    parts.push("é"); parts.push(""); parts.push(tail);
    return string_join(parts, "λ");
}
fn join_captured() -> string[] {
    let parts = resizable(["old", ""]);
    let alias = parts;
    let separator = () => {
        alias.reserve(100); alias[0] = "雪"; alias.push("🙂");
        parts = ["wrong"];
        return "|";
    };
    let result = string_join(parts, separator());
    alias[0] = "changed";
    return [result, alias[0], string_join(parts, "")];
}
fn join_ordered(parts: string[], separator: string) -> string {
    return string_join({ effect(20); parts }, { effect(21); separator });
}
fn join_fail(parts: string[], separator: string) -> string {
    let result = string_join(parts, separator); retain(result);
    let empty: number[] = []; let bad = empty[0];
    return result;
}
fn join_shadow() -> number {
    let string_join = (n: number) => n + 5;
    return 0 |> string_join;
}
fn join_stages(parts: string[]) -> string {
    return parts |> (string_join(_, "λ"));
}
fn hidden() -> range { return 0..2; }
"#;

fn source() -> String {
    SOURCE.replace("\\0", "\0")
}

fn compiler() -> Compiler {
    let mut compiler = Compiler::new();
    compiler
        .register_host_function(
            "effect",
            "string_concat",
            "\0外 module",
            vec![Type::Number],
            Type::Void,
        )
        .unwrap();
    compiler
        .register_host_function(
            "retain",
            "string_byte_len",
            "string_replace_all",
            vec![Type::String],
            Type::Void,
        )
        .unwrap();
    // An unused import remains required, and external names are not language identifiers.
    compiler
        .register_host_function("unused", "string_join", "string_join", vec![], Type::Number)
        .unwrap();
    compiler
}

fn invalid_sources() -> Vec<String> {
    let mut cases = Vec::new();
    for (name, args, result) in [
        (BUILTINS[0], "\"x\"", "number"),
        (BUILTINS[1], "\"x\", \"y\"", "string"),
        (BUILTINS[2], "\"x\", 0, 1", "string"),
        (BUILTINS[3], "\"x\", \"x\", \"!\"", "string"),
        (BUILTINS[4], "[\"x\"], \"!\"", "string"),
    ] {
        cases.extend([
            format!("fn {name}() {{}} fn main() {{}}"),
            format!("fn main() {{ let {name} = 1; {name}({args}); }}"),
            format!("fn main() {{ {name}(); }}"),
            format!("fn main() {{ {name}({args}, missing); }}"),
            format!("fn main() {{ {name}(source: missing); }}"),
            format!("fn main() {{ return \"x\" |> {name}; }}"),
            format!("fn main() {{ return \"x\" |> {name}({args}); }}"),
            format!("fn main(s: {result} = {name}({args})) {{}}"),
            format!("fn main() requires true || {name}({args}) == {name}({args}) {{}}"),
            format!("fn main() {{ return; {name}(false); }}"),
        ]);
    }
    cases.extend(
        [
            "fn main() { string_concat(\"x\"); }",
            "fn main() { string_join([1], \"!\"); }",
            "fn main() { string_join([true], \"!\"); }",
            "fn main() { string_join(\"x\", \"!\"); }",
            "fn main() { string_join([\"x\"], false); }",
            "fn main() { return; string_join([\"x\"], false); }",
            "fn main() { return false && string_join([1], \"!\") == \"\"; }",
            "fn main() -> string { return string_join(loop {}, missing); }",
            "// é🙂\r\nfn main() { string_join(first); }",
            "// é🙂\r\nfn main() { string_join(left: first, right: second); }",
            "// é🙂\r\nfn main() { string_join(first, second); }",
            "fn main() { string_join([\"x\"], \"!\"; }",
            "fn main() { string_slice_bytes(\"x\", 0); }",
            "fn main() { string_replace_all(\"x\", \"x\"); }",
            "fn main() { string_concat(\"x\", false); }",
            "fn main() { string_slice_bytes(\"x\", false, 1); }",
            "fn main() { string_slice_bytes(\"x\", 0, false); }",
            "fn main() { string_replace_all(\"x\", false, \"!\"); }",
            "fn main() { string_replace_all(\"x\", \"x\", false); }",
            "fn main() { return false && string_byte_len(false) == 0; }",
            "fn main() -> string { return string_concat(loop {}, missing); }",
            "fn main() { string_concat(\"x\"; }",
        ]
        .map(String::from),
    );
    cases
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
            ("string_concat".into(), "\0外 module".into()),
            ("string_byte_len".into(), "string_replace_all".into()),
            ("string_join".into(), "string_join".into()),
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
    assert_eq!(exports, expected, "no new string or management API exports");
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].initial, 1);
    assert_eq!(memories[0].maximum, Some(65_535));
    assert!(!memories[0].memory64 && !memories[0].shared);
}

fn field(input: &mut Vec<u8>, bytes: &[u8]) {
    input.extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_le_bytes());
    input.extend_from_slice(bytes);
}

fn browser_input() -> Vec<u8> {
    let source = source();
    let mut input = Vec::new();
    field(&mut input, source.as_bytes());
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
            let bytes = native.compile_source(&source).unwrap();
            assert_surface(&bytes, selected);
            assert_eq!(bytes, native.compile_source(&source).unwrap());
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
    for name in BUILTINS {
        let error = native
            .register_host_function(name, "env", "bad", vec![], Type::String)
            .unwrap_err();
        field(
            &mut input,
            format!("host registration error: {error}").as_bytes(),
        );
    }
    input
}

#[test]
fn browser_string_native_fixtures_validate_surface_diagnostics_and_preset_parity() {
    browser_input();
}

fn run_browser(execution: &str) {
    let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let package = root.join("pkg");
    assert!(package.join("was_compiler_bg.wasm").is_file());
    let input = browser_input();
    let script = format!(
        r#"
        const assert = require('node:assert/strict'), fs = require('node:fs'), path = require('node:path');
        const {{pathToFileURL}} = require('node:url');
        const {{createAbi}} = require('./runtime/abi.cjs');
        assert.ok(Number(process.versions.node.split('.')[0]) >= 24, 'requires Node.js 24+');
        async function main() {{
            const input = fs.readFileSync(0); let offset = 0;
            function field() {{
                const n = input.readUInt32LE(offset); offset += 4;
                const b = input.subarray(offset, offset+n); offset += n; return b;
            }}
            const source = field().toString();
            const native = Array.from({{length:4}}, () => Array.from({{length:2}},
                () => ({{bytes:field(), wat:field().toString()}})));
            const count = Number(field().toString());
            const diagnostics = Array.from({{length:count}},
                () => ({{source:field().toString(), message:field().toString()}}));
            const registrationErrors = Array.from({{length:5}}, () => field().toString());
            assert.equal(offset, input.length);
            const dir = process.argv[1];
            const {{default:init, Compiler}} = await import(pathToFileURL(path.join(dir,'was_compiler.js')).href);
            await init({{module_or_path:fs.readFileSync(path.join(dir,'was_compiler_bg.wasm'))}});
            const c = new Compiler();
            const builtins = ['string_byte_len','string_concat','string_slice_bytes','string_replace_all','string_join'];
            const names = ['bytes','concat','slice','replace','literal','aliases','slot','lexical',
                'ordered_bytes','ordered_concat','ordered_slice','ordered_replace','left_trap','right_trap',
                'publish','fail','outer','stages',
                'join','join_empty','join_build','join_captured','join_ordered','join_fail',
                'join_shadow','join_stages'];
            const imports = [
                {{module:'string_concat',name:'\0外 module',kind:'function'}},
                {{module:'string_byte_len',name:'string_replace_all',kind:'function'}},
                {{module:'string_join',name:'string_join',kind:'function'}}];
            const enabled = [];
            try {{
                for (let i=0; i<builtins.length; i++) assert.throws(
                    () => c.registerHostFunction(builtins[i],'env','bad',[],'string'),
                    e => e instanceof Error && e.message === registrationErrors[i]);
                c.registerHostFunction('effect','string_concat','\0外 module',['number'],'void');
                c.registerHostFunction('retain','string_byte_len','string_replace_all',['string'],'void');
                c.registerHostFunction('unused','string_join','string_join',[],'number');
                for (let level=0; level<4; level++) {{
                    c.setOptimizationLevel(level);
                    for (let index=0; index<2; index++) {{
                        if (index === 0) c.setExports(names); else c.exportNone();
                        const bytes = c.compile(source);
                        assert.deepEqual(Buffer.from(bytes), native[level][index].bytes, `O${{level}} exports ${{index}}`);
                        assert.equal(c.compileWat(source), native[level][index].wat);
                        if (level === 1) enabled[index] = Buffer.from(bytes);
                        if (level > 1) assert.deepEqual(Buffer.from(bytes), enabled[index]);
                        assert.ok(WebAssembly.validate(bytes));
                        const m = new WebAssembly.Module(bytes);
                        assert.deepEqual(WebAssembly.Module.imports(m), imports);
                        const expected = [{{name:'memory',kind:'memory'}},{{name:'__alloc',kind:'function'}}];
                        if (index === 0) expected.push(...names.map(name => ({{name,kind:'function'}})));
                        const byName = (a,b) => a.name.localeCompare(b.name);
                        assert.deepEqual(WebAssembly.Module.exports(m).sort(byName), expected.sort(byName));
                        assert.throws(() => new WebAssembly.Instance(m, {{}}), TypeError);
                        let effect = () => {{}}, retain = () => {{}};
                        const host = {{
                            string_concat:{{'\0外 module': n => effect(n)}},
                            string_byte_len:{{string_replace_all: h => retain(h)}},
                            string_join:{{string_join: () => {{throw new Error('unused import called');}}}}
                        }};
                        assert.throws(() => new WebAssembly.Instance(m, {{
                            string_concat:host.string_concat, string_byte_len:host.string_byte_len
                        }}), TypeError, 'unused registered import is still required');
                        const instance = new WebAssembly.Instance(m, host);
                        const e = instance.exports, abi = createAbi(() => instance);
                        if (index !== 0) continue;
                        const text = h => abi.decode('string',h);
                        const encode = s => abi.encode('string',s);
                        const trap = f => assert.throws(f,WebAssembly.RuntimeError);
                        {execution}
                    }}
                    for (const d of diagnostics) for (const method of ['compile','compileWat']) {{
                        assert.throws(() => c[method](d.source),
                            e => e instanceof Error && e.message === d.message,
                            `O${{level}} ${{method}}: ${{d.source}}`);
                    }}
                }}
            }} finally {{ c.free(); }}
        }}
        main().catch(e => {{ console.error(e); process.exitCode=1; }});
    "#
    );
    let output = test_process::output(
        Command::new("node")
            .current_dir(root)
            .args(["-e", &script])
            .arg(package),
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

#[test]
#[ignore = "requires Node.js 24+ and pkg freshly rebuilt from current source"]
fn rebuilt_browser_string_join_fixed_resizable_utf8_copies_and_captures_match_native() {
    run_browser(
        r#"
        const array = parts => abi.encode('string[]',parts);
        for (const [parts,separator,expected] of [
            [[],',',''], [[''],',',''], [['',''],',',','],
            [['a','','b'],',','a,,b'], [['a','','b'],'','ab'],
            [['é','\0','🙂'],'雪','é雪\0雪🙂'],
            [['e\u0301','é'],'λ','e\u0301λé'], [['only'],'unused','only']
        ]) {
            const input=parts.slice(), h=array(input), sep=encode(separator);
            input.push('host-only');
            const result=e.join(h,sep);
            assert.equal(text(result),expected);
            assert.deepEqual(abi.decode('string[]',h),parts);
            assert.equal(text(sep),separator);
            const copy=abi.decode('string[]',h); copy.push('copy-only');
            assert.deepEqual(abi.decode('string[]',h),parts);
            if (parts.length===1) {
                const view=new DataView(e.memory.buffer), payload=view.getUint32(h,true);
                assert.equal(result,view.getUint32(payload,true),'singleton returns its entry');
            }
        }
        assert.equal(text(e.join_empty()),'');
        assert.equal(text(e.join_build(encode('\0🙂'))),'éλλ\0🙂');
        assert.deepEqual(abi.decode('string[]',e.join_captured()),['雪||🙂','changed','wrong']);
        assert.equal(e.join_shadow(),5);
        assert.equal(text(e.join_stages(array(['é','','🙂']))),'éλλ🙂');
        const h=array(['a','b']), separator=encode(','), saved=e.join(h,separator);
        const replacement=encode('changed'), view=new DataView(e.memory.buffer);
        view.setUint32(view.getUint32(h,true),replacement,true);
        assert.equal(text(saved),'a,b','joined payload is independent of mutable slots');
        assert.equal(text(e.join(h,separator)),'changed,b');
    "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+ and pkg freshly rebuilt from current source"]
fn rebuilt_browser_string_join_validation_order_reentry_growth_and_retention_match_native() {
    run_browser(
        r#"
        const array=parts => abi.encode('string[]',parts), events=[], retained=[];
        retain=h => retained.push(h);
        for (const parts of [[],[''],['é'],['','']]) {
            const h=array(parts), sep=encode('!');
            const original=new DataView(e.memory.buffer).getUint32(sep,true);
            effect=n => {
                events.push(n);
                if (n===21) new DataView(e.memory.buffer).setUint32(sep,0,true);
            };
            trap(() => e.join_ordered(h,sep));
            assert.deepEqual(events.splice(0),[20,21]);
            new DataView(e.memory.buffer).setUint32(sep,original,true);
        }
        for (const target of ['outer','first','last']) {
            const h=array(['','']), sep=encode('');
            const view=new DataView(e.memory.buffer), payload=view.getUint32(h,true);
            const address=target==='outer' ? h : payload+(target==='last' ? 4 : 0);
            const original=view.getUint32(address,true);
            effect=n => {
                events.push(n);
                if (n===21) new DataView(e.memory.buffer).setUint32(address,0,true);
            };
            trap(() => e.join_ordered(h,sep));
            assert.deepEqual(events.splice(0),[20,21]);
            new DataView(e.memory.buffer).setUint32(address,original,true);
        }
        const h=array(['a','b']), sep=encode('|'), changed=encode('雪');
        const retainedResult=e.join(h,sep);
        let reentries=0;
        effect=n => {
            events.push(n);
            if (n!==21) return;
            reentries++;
            trap(() => e.join_fail(h,sep));
            const large=array(['界'.repeat(70000),'界'.repeat(70000)]), empty=encode('');
            const before=e.memory.buffer;
            const grown=e.join(large,empty);
            assert.notEqual(e.memory.buffer,before,'guest join actually grows memory');
            assert.equal(text(grown),'界'.repeat(140000));
            retained.push(grown);
            const view=new DataView(e.memory.buffer);
            view.setUint32(view.getUint32(h,true),changed,true);
        };
        assert.equal(text(e.join_ordered(h,sep)),'雪|b');
        assert.equal(reentries,1); assert.deepEqual(events.splice(0),[20,21]);
        assert.equal(text(retainedResult),'a|b');
        assert.equal(text(retained[0]),'a|b');
        assert.equal(text(retained[1]),'界'.repeat(140000));
        effect=n => events.push(n);
        trap(() => e.join_ordered(0,sep)); assert.deepEqual(events,[]);
        effect=n => { events.push(n); if (n===20) e.slice(encode('é'),1,1); };
        trap(() => e.join_ordered(h,sep)); assert.deepEqual(events,[20]);
        assert.equal(text(retainedResult),'a|b');
        assert.equal(text(retained[0]),'a|b');
    "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+ and pkg freshly rebuilt from current source"]
fn rebuilt_browser_string_values_strict_slicing_and_replacement_match_native() {
    run_browser(
        r#"
        for (const [s,n] of [['',0],['ASCII',5],['a\0b',3],['éλ🙂',8],['e\u0301',3]]) {
            const h = encode(s);
            assert.equal(e.bytes(h),n);
            assert.equal(text(h),s);
        }
        const empty = encode(''), otherEmpty = encode('');
        assert.equal(e.concat(empty,otherEmpty),otherEmpty);
        for (const [a,b,expected] of [['','é','é'],['é','','é'],['a\0','λ🙂','a\0λ🙂']]) {
            const x = encode(a), y = encode(b), result = e.concat(x,y);
            assert.equal(text(result),expected);
            assert.equal(text(x),a); assert.equal(text(y),b);
            if (!a) assert.equal(result,y); else if (!b) assert.equal(result,x);
            else { assert.notEqual(result,x); assert.notEqual(result,y); }
        }
        const s = encode('aé\0🙂z');
        const boundaries = [0,1,3,4,8,9];
        for (let a=0; a<=9; a++) for (let b=0; b<=9; b++) {
            if (a<=b && boundaries.includes(a) && boundaries.includes(b)) {
                const h = e.slice(s,a,b);
                assert.equal(text(h),Buffer.from('aé\0🙂z').subarray(a,b).toString());
                if (a===0 && b===9) assert.equal(h,s); else assert.notEqual(h,s);
            } else trap(() => e.slice(s,a,b));
        }
        assert.equal(e.slice(s,-0,9),s);
        assert.equal(e.slice(empty,-0,0),empty);
        for (const bad of [-1,0.5,NaN,Infinity,-Infinity,10,2**53,Number.MAX_VALUE]) {
            trap(() => e.slice(s,bad,9)); trap(() => e.slice(s,0,bad));
        }
        for (const [src,needle,replacement,expected,reuse] of [
            ['aaa','aa','b','ba',false], ['aaaa','aa','aaa','aaaaaa',false],
            ['a','a','aa','aa',false], ['é\0🙂é','é','λ','λ\0🙂λ',false],
            ['a\0b\0','\0','','ab',false], ['🙂🙂','🙂','','',false],
            ['abc','z','!','abc',true], ['abc','b','b','abc',true],
            ['','x','y','',true], ['e\u0301','é','!','e\u0301',true],
            ['AaA','a','$&','A$&A',false]
        ]) {
            const a=encode(src), n=encode(needle), r=encode(replacement), h=e.replace(a,n,r);
            assert.equal(text(h),expected);
            assert.equal(text(a),src); assert.equal(text(n),needle); assert.equal(text(r),replacement);
            if (reuse) assert.equal(h,a); else assert.notEqual(h,a);
        }
        trap(() => e.replace(empty,empty,empty));
        trap(() => e.replace(s,empty,encode('!')));
        const literal=e.literal(), values=e.aliases();
        assert.equal(text(literal),'é\0🙂');
        assert.deepEqual(abi.decode('string[]',values),
            ['é\0🙂','é\0🙂','é\0🙂','é\0🙂','é\0🙂!','\0','λ\0🙂','é\0🙂']);
        const v=new DataView(e.memory.buffer), p=v.getUint32(values,true);
        for (const i of [0,1,2,3,7]) assert.equal(v.getUint32(p+4*i,true),literal);
        e.slot(values);
        assert.equal(abi.decode('string[]',values)[0],'!\0🙂');
        assert.equal(text(literal),'é\0🙂');
        const hostCopy=['é','🙂'], array=abi.encode('string[]',hostCopy);
        hostCopy[0]='changed'; e.slot(array);
        const decoded=abi.decode('string[]',array); decoded[0]='changed again';
        assert.deepEqual(abi.decode('string[]',array),['!','🙂']);
        assert.deepEqual(hostCopy,['changed','🙂']);
        assert.equal(e.lexical(),1);
        assert.equal(e.stages(),2);
    "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+ and pkg freshly rebuilt from current source"]
fn rebuilt_browser_string_order_validation_retention_growth_and_reentry_match_native() {
    run_browser(
        r#"
        const events=[], retained=[];
        effect=n => events.push(n);
        retain=h => retained.push([h,text(h)]);
        const a=encode('é🙂'), b=encode('!'), n=encode('é'), r=encode('λ');
        assert.equal(e.ordered_bytes(a),6); assert.deepEqual(events.splice(0),[1]);
        assert.equal(text(e.ordered_concat(a,b)),'é🙂!'); assert.deepEqual(events.splice(0),[1,2]);
        assert.equal(text(e.ordered_slice(a,0,2)),'é'); assert.deepEqual(events.splice(0),[1,2,3]);
        assert.equal(text(e.ordered_replace(a,n,r)),'λ🙂'); assert.deepEqual(events.splice(0),[1,2,3]);
        trap(() => e.ordered_slice(a,1,1)); assert.deepEqual(events.splice(0),[1,2,3]);
        trap(() => e.ordered_replace(a,encode(''),r)); assert.deepEqual(events.splice(0),[1,2,3]);
        trap(() => e.left_trap()); assert.deepEqual(events.splice(0),[4]);
        trap(() => e.right_trap()); assert.deepEqual(events.splice(0),[6,7]);
        assert.deepEqual(retained,[[retained[0][0],'saved!']]);
        assert.equal(text(retained[0][0]),'saved!'); // publication/allocation survives the trap

        // Corrupt descriptors only after entry validation, isolating builtin validation before reuse.
        for (const [method,strings,args,targets,order] of [
            ['ordered_bytes',['é'],[],[0],[1]],
            ['ordered_concat',['','right'],[],[0,1],[1,2]],
            ['ordered_concat',['left',''],[],[0,1],[1,2]],
            ['ordered_concat',['',''],[],[0,1],[1,2]],
            ['ordered_slice',['é'],[0,2],[0],[1,2,3]],
            ['ordered_replace',['abc','z','!'],[],[0,1,2],[1,2,3]],
            ['ordered_replace',['abc','b','b'],[],[0,1,2],[1,2,3]],
            ['ordered_replace',['','','!'],[],[0,1,2],[1,2,3]]
        ]) for (const target of targets) for (const [field,bad] of [[0,0],[4,0xffffffff]]) {
            const handles=strings.map(encode), address=handles[target]+field;
            const original=new DataView(e.memory.buffer).getUint32(address,true);
            effect=k => {
                events.push(k);
                if (k===1) new DataView(e.memory.buffer).setUint32(address,bad,true);
            };
            trap(() => e[method](...handles,...args));
            assert.deepEqual(events.splice(0),order);
            new DataView(e.memory.buffer).setUint32(address,original,true);
            for (let i=0;i<handles.length;i++) assert.equal(text(handles[i]),strings[i]);
        }
        effect=k => events.push(k);
        trap(() => e.ordered_bytes(0)); trap(() => e.ordered_concat(a,0));
        trap(() => e.ordered_slice(0,0,0)); trap(() => e.ordered_replace(a,n,0));
        assert.deepEqual(events,[],'entry validation precedes body effects');
        assert.equal(text(e.publish(a)),'é🙂?');
        const literal=e.literal(), array=e.aliases();
        const arrayCopy=abi.decode('string[]',array), oldBuffer=e.memory.buffer;
        let reentries=0;
        effect=k => {
            events.push(k); assert.equal(k,9); reentries++;
            trap(() => e.fail(a));
            const large=encode('界'.repeat(70000));
            const beforeGuestGrowth=e.memory.buffer;
            const grown=e.concat(large,large);
            assert.notEqual(e.memory.buffer,beforeGuestGrowth,'guest concat must actually grow memory');
            assert.notEqual(e.memory.buffer,oldBuffer);
            assert.equal(beforeGuestGrowth.byteLength,0,'old views are detached');
            assert.equal(text(grown),'界'.repeat(140000));
            retained.push([grown,'界'.repeat(140000)]);
            assert.equal(text(e.publish(encode('inner'))),'inner?');
        };
        assert.equal(text(e.outer(a)),'λ🙂é🙂');
        assert.equal(reentries,1); assert.deepEqual(events,[9]);
        effect=() => {};
        for (const [h,expected] of retained) assert.equal(text(h),expected);
        assert.equal(text(a),'é🙂'); assert.equal(text(literal),'é\0🙂');
        assert.deepEqual(abi.decode('string[]',array),arrayCopy);
        assert.equal(e.bytes(a),6);
        assert.equal(text(e.slice(a,2,6)),'🙂');
        assert.equal(text(e.replace(a,n,r)),'λ🙂');
        trap(() => e.fail(a));
        for (const [h,expected] of retained) assert.equal(text(h),expected);
        assert.equal(text(e.concat(a,b)),'é🙂!');
    "#,
    );
}
