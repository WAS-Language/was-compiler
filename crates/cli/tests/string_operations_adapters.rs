#[path = "../../../tests/support/process.rs"]
mod test_process;

use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use was_compiler::semantic::SemanticError;
use was_compiler::{CompileError, Compiler, ExportOptions, OptimizationLevel, Type, diagnostic};
use wasmparser::{Parser, Payload, Validator};

const LEVELS: [(&str, OptimizationLevel); 4] = [
    ("-O0", OptimizationLevel::None),
    ("-O1", OptimizationLevel::Basic),
    ("-O2", OptimizationLevel::Standard),
    ("-O3", OptimizationLevel::Aggressive),
];
const BUILTINS: [&str; 4] = [
    "string_byte_len",
    "string_concat",
    "string_slice_bytes",
    "string_replace_all",
];
const HOSTS: [&str; 6] = [
    "--host",
    "effect=string_concat.string_byte_len(number)->void",
    "--host",
    "retain=string_slice_bytes.string_replace_all(string)->void",
    "--host",
    "unused=外部 module.任意 field!?()->void",
];
const EXPORTS: [&str; 9] = [
    "bytes", "concat", "slice", "replace", "aliases", "shadow", "ordered", "early", "grow",
];
const SOURCE: &str = r#"
fn bytes(s: string) -> number { return string_byte_len(s); }
fn concat(a: string, b: string) -> string { return string_concat(a, b); }
fn slice(s: string, start: number, end: number) -> string {
    return string_slice_bytes(s, start, end);
}
fn replace(s: string, needle: string, replacement: string) -> string {
    return string_replace_all(s, needle, replacement);
}
fn aliases() -> string[] {
    let s = "éλ🙂";
    let alias = s;
    let values = [s, alias];
    let transform = (x: string) => string_replace_all(string_concat(x, "!"), "λ", "雪");
    s = "changed";
    return [values[0], values[1], transform(alias),
        alias |> (string_slice_bytes(_, 2, 4)),
        string_concat("", ""), string_replace_all("aaa", "aa", "b")];
}
fn shadow() -> number {
    let string_byte_len = (n: number) => n + 1;
    let string_concat = (n: number) => n + 2;
    let string_slice_bytes = (n: number) => n + 3;
    let string_replace_all = (n: number) => n + 4;
    return 0 |> string_byte_len |> string_concat |> string_slice_bytes |> string_replace_all;
}
fn ordered(s: string, needle: string, replacement: string, start: number, end: number) -> string {
    let joined = string_concat({ effect(1); s }, { effect(2); "!" });
    retain(joined);
    let part = string_slice_bytes({ effect(3); joined }, { effect(4); start }, { effect(5); end });
    retain(part);
    let result = string_replace_all({ effect(6); part }, { effect(7); needle }, { effect(8); replacement });
    effect(string_byte_len(result));
    return result;
}
fn early(s: string) -> string {
    return string_concat({ effect(9); string_slice_bytes(s, 1, 1) }, { effect(10); "later" });
}
fn grow(s: string) -> string {
    let value = s;
    let i = 0;
    while i < 17 { value = string_concat(value, value); i += 1; }
    return value;
}
fn private_range() -> range { return 0..2; }
"#;

fn compiler() -> Compiler {
    let mut compiler = Compiler::new();
    for (name, module, field, params) in [
        (
            "effect",
            "string_concat",
            "string_byte_len",
            vec![Type::Number],
        ),
        (
            "retain",
            "string_slice_bytes",
            "string_replace_all",
            vec![Type::String],
        ),
        ("unused", "外部 module", "任意 field!?", vec![]),
    ] {
        compiler
            .register_host_function(name, module, field, params, Type::Void)
            .unwrap();
    }
    compiler
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("was-string-cli-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn cli(&self, args: &[&str]) -> Output {
        test_process::output(
            Command::new(env!("CARGO_BIN_EXE_was-compiler"))
                .current_dir(&self.0)
                .args(args),
            None,
            Duration::from_secs(30),
        )
    }

    fn compile(&self, level: &str, selected: bool, format: &str) -> Vec<u8> {
        let mut args = vec!["input.was", "-o", "output", level, "--emit", format];
        args.extend(HOSTS);
        if selected {
            for name in EXPORTS {
                args.extend(["--export", name]);
            }
        } else {
            args.push("--no-exports");
        }
        let output = self.cli(&args);
        assert!(
            output.status.success(),
            "{level} {format}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        fs::read(self.0.join("output")).unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_string_bytes_wat_imports_and_selected_or_no_exports_match_core_at_each_preset() {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("input.was"), SOURCE).unwrap();
    let mut native = compiler();
    for selected in [true, false] {
        native.set_exports(if selected {
            ExportOptions::Only(EXPORTS.iter().map(|name| (*name).into()).collect())
        } else {
            ExportOptions::None
        });
        let mut enabled_bytes = None;
        for (level, optimization) in LEVELS {
            native.set_optimization_level(optimization);
            let expected = native.compile_source(SOURCE).unwrap();
            let bytes = scratch.compile(level, selected, "wasm");
            Validator::new().validate_all(&bytes).unwrap();
            assert_eq!(bytes, expected, "{level}, selected={selected}");
            assert_eq!(bytes, scratch.compile(level, selected, "wasm"));
            if optimization != OptimizationLevel::None {
                if let Some(enabled) = &enabled_bytes {
                    assert_eq!(&bytes, enabled);
                } else {
                    enabled_bytes = Some(bytes.clone());
                }
            }
            let mut exports = Vec::new();
            let mut imports = Vec::new();
            for payload in Parser::new(0).parse_all(&bytes) {
                match payload.unwrap() {
                    Payload::ExportSection(section) => {
                        for export in section {
                            exports.push(export.unwrap().name.to_owned());
                        }
                    }
                    Payload::ImportSection(section) => {
                        for import in section.into_imports() {
                            let import = import.unwrap();
                            imports.push((import.module.to_owned(), import.name.to_owned()));
                        }
                    }
                    _ => {}
                }
            }
            let mut expected_exports = vec!["memory", "__alloc"];
            if selected {
                expected_exports.extend(EXPORTS);
            }
            exports.sort();
            expected_exports.sort();
            assert_eq!(exports, expected_exports);
            assert_eq!(
                imports,
                [
                    ("string_concat".into(), "string_byte_len".into()),
                    ("string_slice_bytes".into(), "string_replace_all".into()),
                    ("外部 module".into(), "任意 field!?".into()),
                ]
            );
            assert_eq!(
                scratch.compile(level, selected, "wat"),
                wasmprinter::print_bytes(&expected).unwrap().into_bytes()
            );
        }
    }
}

#[test]
fn cli_string_diagnostics_and_unchanged_artifacts_match_core_at_each_preset() {
    let scratch = Scratch::new();
    let mut sources = Vec::new();
    for (name, args, result) in [
        ("string_byte_len", "\"x\"", "number"),
        ("string_concat", "\"x\", \"y\"", "string"),
        ("string_slice_bytes", "\"x\", 0, 1", "string"),
        ("string_replace_all", "\"x\", \"x\", \"!\"", "string"),
    ] {
        sources.extend([
            format!("fn {name}() {{}}"),
            format!("fn main() {{ {name}(); }}"),
            format!("fn main() {{ {name}({args}, missing); }}"),
            format!("fn main() {{ {name}(source: missing); }}"),
            format!("fn main() {{ let {name} = 1; {name}({args}); }}"),
            format!("fn main({name}: string) {{ {name}({args}); }}"),
            format!("fn main() {{ return \"x\" |> {name}; }}"),
            format!("fn main(x: {result} = {name}({args})) {{}}"),
            format!("fn main() requires true || {name}({args}) == {name}({args}) {{}}"),
        ]);
        let args: Vec<_> = args.split(", ").collect();
        for position in 0..args.len() {
            for bad in ["true", "[]", "0..1", "empty()"] {
                let mut wrong = args.clone();
                wrong[position] = bad;
                let call = format!("{name}({})", wrong.join(", "));
                sources.push(format!("fn empty() {{}} fn main() {{ {call}; }}"));
            }
        }
    }
    sources.extend([
        "fn main() { return; string_concat(\"x\", 1); }".into(),
        "fn main() { return false && string_byte_len(true) == 0; }".into(),
        "fn main() { return string_slice_bytes(loop {}, 0, false); }".into(),
        "fn main() { return string_replace_all(loop {}, \"x\", missing); }".into(),
        "fn main() { return string_byte_len(1); }".into(),
        "fn main() { return string_concat(1, \"x\"); }".into(),
        "fn main() { return string_concat(\"x\", 1); }".into(),
        "fn main() { return string_slice_bytes(1, 0, 1); }".into(),
        "fn main() { return string_replace_all(1, \"x\", \"!\"); }".into(),
        "fn main() { return string_replace_all(\"x\", 1, \"!\"); }".into(),
        "fn main() { return string_replace_all(\"x\", \"x\", 1); }".into(),
        "fn main() { return len(\"x\"); }".into(),
        "fn main() { return \"a\" + \"b\"; }".into(),
        "fn main() { return string_slice_bytes(\"x\", \"0\", 1); }".into(),
        "fn main() { return string_slice_bytes(\"x\", 0, \"1\"); }".into(),
    ]);
    for source in sources {
        fs::write(scratch.0.join("input.was"), &source).unwrap();
        let mut baseline = None;
        for (level, optimization) in LEVELS {
            let mut native = Compiler::new();
            native.set_optimization_level(optimization);
            let error = native.compile_source(&source).unwrap_err();
            if source.contains("let string_") || source.starts_with("fn main(string_") {
                assert!(
                    matches!(
                        &error,
                        CompileError::Semantic(SemanticError::NotCallable { .. })
                    ),
                    "a lexical value must not fall back to a builtin: {error:?}"
                );
            }
            let expected = format!("{}\n", diagnostic::render("input.was", &source, &error));
            let identity = (format!("{error:?}"), error.span(), expected.clone());
            if let Some(baseline) = &baseline {
                assert_eq!(&identity, baseline, "{source}");
            } else {
                baseline = Some(identity);
            }
            for format in ["wasm", "wat"] {
                fs::write(scratch.0.join("output"), b"unchanged").unwrap();
                let output = scratch.cli(&["input.was", "-o", "output", level, "--emit", format]);
                assert_eq!(output.status.code(), Some(1), "{source}");
                assert!(output.stdout.is_empty());
                assert_eq!(
                    String::from_utf8(output.stderr).unwrap(),
                    expected,
                    "{level}: {source}"
                );
                assert_eq!(fs::read(scratch.0.join("output")).unwrap(), b"unchanged");
            }
        }
    }
}

#[test]
fn cli_string_errors_preserve_raw_utf8_spans_and_structure_before_child_order() {
    #[derive(Clone, Copy)]
    enum ExpectedError {
        Arity,
        NamedArgument,
        FirstUndefinedVariable,
    }

    use ExpectedError::{Arity, FirstUndefinedVariable, NamedArgument};

    let scratch = Scratch::new();
    for (call, kind) in [
        ("string_byte_len(first, second)", Arity),
        ("string_concat(first)", Arity),
        ("string_slice_bytes(first, second)", Arity),
        ("string_replace_all(first, second)", Arity),
        ("string_concat(left: first, right: second)", NamedArgument),
        ("string_byte_len(first)", FirstUndefinedVariable),
        ("string_concat(first, second)", FirstUndefinedVariable),
        (
            "string_replace_all(first, second, third)",
            FirstUndefinedVariable,
        ),
        (
            "string_slice_bytes(first, second, third)",
            FirstUndefinedVariable,
        ),
    ] {
        let source = format!("// é🙂\r\nfn main() {{\r\n\t{call};\r\n}}");
        fs::write(scratch.0.join("input.was"), &source).unwrap();
        for (level, optimization) in LEVELS {
            let mut native = Compiler::new();
            native.set_optimization_level(optimization);
            let error = native.compile_source(&source).unwrap_err();
            assert!(
                match (&error, kind) {
                    (
                        CompileError::Semantic(SemanticError::ArgumentCountMismatch { .. }),
                        Arity,
                    ) => true,
                    (
                        CompileError::Semantic(SemanticError::NamedArgumentsNotSupported {
                            ..
                        }),
                        NamedArgument,
                    ) => true,
                    (
                        CompileError::Semantic(SemanticError::UndefinedVariable { name, .. }),
                        FirstUndefinedVariable,
                    ) => name == "first",
                    _ => false,
                },
                "{error:?}"
            );
            let span = error.span().unwrap();
            let (start, length) = match kind {
                NamedArgument => (source.find("left: first").unwrap(), "left: first".len()),
                Arity | FirstUndefinedVariable => (source.find(call).unwrap(), call.len() + 1),
            };
            assert_eq!((span.start(), span.end()), (start, start + length));
            let output = scratch.cli(&["input.was", level]);
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert_eq!(
                String::from_utf8(output.stderr).unwrap(),
                format!("{}\n", diagnostic::render("input.was", &source, &error))
            );
        }
    }
}

#[test]
fn cli_string_reserved_host_names_fail_before_source_loading_without_changing_output() {
    let scratch = Scratch::new();
    // There is deliberately no input file: registration must fail first.
    for name in BUILTINS {
        let error = Compiler::new()
            .register_host_function(name, name, name, vec![], Type::Void)
            .unwrap_err();
        let declaration = format!("{name}={name}.{name}()->void");
        let expected = format!("host registration error in {declaration:?}: {error}\n");
        for (level, _) in LEVELS {
            for format in ["wasm", "wat"] {
                fs::write(scratch.0.join("output"), b"unchanged").unwrap();
                let output = scratch.cli(&[
                    "input.was",
                    "-o",
                    "output",
                    level,
                    "--emit",
                    format,
                    "--host",
                    &declaration,
                ]);
                assert_eq!(output.status.code(), Some(1));
                assert!(output.stdout.is_empty());
                assert_eq!(String::from_utf8(output.stderr).unwrap(), expected);
                assert_eq!(fs::read(scratch.0.join("output")).unwrap(), b"unchanged");
            }
        }
    }
}

fn execute(script: &str) {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("input.was"), SOURCE).unwrap();
    let script = format!(
        r#"
        const assert = require('node:assert/strict');
        assert.ok(Number(process.versions.node.split('.')[0]) >= 24, 'requires Node.js 24+');
        const {{createAbi}} = require('./runtime/abi.cjs');
        const module = new WebAssembly.Module(require('node:fs').readFileSync(0));
        const effects = [], retained = [];
        const instance = new WebAssembly.Instance(module, {{
            string_concat: {{string_byte_len: n => effects.push(n)}},
            string_slice_bytes: {{string_replace_all: h => retained.push(h)}},
            '外部 module': {{'任意 field!?': () => {{throw new Error('unused import called');}}}}
        }});
        const e = instance.exports, abi = createAbi(() => instance);
        const encode = text => abi.encode('string', text);
        const decode = h => abi.decode('string', h);
        {script}
    "#
    );
    for (level, _) in LEVELS {
        let bytes = scratch.compile(level, true, "wasm");
        let output = test_process::output(
            Command::new("node")
                .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
                .args(["-e", &script]),
            Some(&bytes),
            Duration::from_secs(30),
        );
        assert!(
            output.status.success(),
            "{level}:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "requires Node.js 24+"]
fn cli_string_copy_marshaling_utf8_empty_nul_aliases_and_edge_results_execute_at_each_preset() {
    execute(
        r#"
        for (const [text, length] of [['',0], ['ASCII',5], ['a\0b',3], ['éλ🙂',8], ['e\u0301',3]]) {
            const h = encode(text);
            assert.equal(e.bytes(h), length);
            assert.equal(decode(h), text);
        }
        const empty = encode(''), original = encode('é\0🙂');
        const joined = e.concat(original, encode('雪'));
        assert.equal(decode(joined), 'é\0🙂雪');
        assert.equal(decode(original), 'é\0🙂');
        assert.equal(e.concat(empty, original), original);
        assert.equal(e.concat(original, empty), original);
        assert.equal(e.concat(empty, empty), empty);
        assert.equal(e.slice(original, -0, 7), original);
        assert.equal(decode(e.slice(original, 2, 3)), '\0');
        assert.equal(decode(e.slice(original, 3, 7)), '🙂');
        assert.equal(decode(e.slice(original, 2, 2)), '');
        assert.equal(decode(e.slice(empty, 0, 0)), '');
        for (const [source, needle, replacement, expected] of [
            ['aaa','aa','b','ba'], ['aaaa','aa','aaa','aaaaaa'],
            ['abc','b','','ac'], ['xx','x','',''], ['', 'x','!', ''],
            ['é\0é','é','🙂','🙂\0🙂'], ['é','e\u0301','x','é'],
            ['AaA','a','x','AxA'], ['$x','$','$$','$$x']
        ]) {
            const h = encode(source);
            assert.equal(decode(e.replace(h, encode(needle), encode(replacement))), expected);
            assert.equal(decode(h), source);
        }
        assert.equal(e.replace(original, encode('absent'), encode('!')), original);
        assert.equal(e.replace(original, encode('é'), encode('é')), original);
        const arrayHandle = e.aliases();
        const copy = abi.decode('string[]', arrayHandle);
        assert.deepEqual(copy, ['éλ🙂','éλ🙂','é雪🙂!','λ','','ba']);
        copy[0] = 'host-only';
        assert.equal(abi.decode('string[]', arrayHandle)[0], 'éλ🙂');
        const input = ['é', '🙂'];
        const encodedArray = abi.encode('string[]', input);
        input[0] = 'host-only';
        assert.deepEqual(abi.decode('string[]', encodedArray), ['é','🙂']);
        assert.equal(e.shadow(), 10);
        assert.deepEqual(effects, []);
        assert.deepEqual(retained, []);
        assert.ok(!('private_range' in e));
    "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn cli_string_ordered_effects_traps_growth_and_retained_handles_execute_at_each_preset() {
    execute(
        r#"
        const s = encode('éλ🙂'), needle = encode('λ'), replacement = encode('雪');
        let result = e.ordered(s, needle, replacement, 0, 4);
        assert.equal(decode(result), 'é雪');
        assert.deepEqual(effects, [1,2,3,4,5,6,7,8,5]);
        assert.deepEqual(retained.map(decode), ['éλ🙂!', 'éλ']);
        const successful = result;
        effects.length = 0;
        assert.throws(() => e.ordered(s, encode(''), replacement, 0, 4), WebAssembly.RuntimeError);
        assert.deepEqual(effects, [1,2,3,4,5,6,7,8]);
        assert.equal(decode(successful), 'é雪');
        assert.deepEqual(retained.map(decode), ['éλ🙂!', 'éλ', 'éλ🙂!', 'éλ']);
        effects.length = 0;
        assert.throws(() => e.ordered(s, needle, replacement, 1, 1), WebAssembly.RuntimeError);
        assert.deepEqual(effects, [1,2,3,4,5]);
        assert.equal(decode(retained.at(-1)), 'éλ🙂!');
        effects.length = 0;
        assert.throws(() => e.early(s), WebAssembly.RuntimeError);
        assert.deepEqual(effects, [9]);
        for (const [start, end] of [[1,1], [0,1], [3,4], [-1,2], [4,2], [0,9], [0.5,2], [NaN,2], [0,Infinity]]) {
            assert.throws(() => e.slice(s, start, end), WebAssembly.RuntimeError);
        }
        assert.throws(() => e.replace(encode(''), encode(''), encode('')), WebAssembly.RuntimeError);
        const oldBuffer = e.memory.buffer;
        const grown = e.grow(encode('x'));
        assert.notEqual(e.memory.buffer, oldBuffer);
        assert.equal(e.bytes(grown), 131072);
        assert.equal(decode(grown), 'x'.repeat(131072));
        assert.equal(decode(s), 'éλ🙂');
        assert.equal(decode(successful), 'é雪');
        assert.deepEqual(retained.map(decode), ['éλ🙂!', 'éλ', 'éλ🙂!', 'éλ', 'éλ🙂!']);
        assert.equal(decode(e.replace(s, needle, replacement)), 'é雪🙂');
        assert.equal(decode(e.slice(s, 4, 8)), '🙂');
    "#,
    );
}
