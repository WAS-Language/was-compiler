#[path = "../../../tests/support/process.rs"]
mod test_process;

use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use was_compiler::{
    ArrayElementType, Compiler, ExportOptions, OptimizationLevel, Type, diagnostic,
};
use wasmparser::{Parser, Payload, Validator};

const LEVELS: [(&str, OptimizationLevel); 4] = [
    ("-O0", OptimizationLevel::None),
    ("-O1", OptimizationLevel::Basic),
    ("-O2", OptimizationLevel::Standard),
    ("-O3", OptimizationLevel::Aggressive),
];
const HOSTS: [&str; 6] = [
    "--host",
    "host_numbers=resizable.resizable(number[])->number[]",
    "--host",
    "host_flags=env.resizable(bool[])->bool[]",
    "--host",
    "host_texts=resizable.texts(string[])->string[]",
];
const EXPORTS: [&str; 4] = ["main", "numbers", "flags", "texts"];
const SOURCE: &str = r#"
fn numbers() -> number[] {
    let source = [1, 2, 3];
    let a = resizable(source);
    a.reserve({ a.push(4); 16 });
    a.push(a.pop());
    let first = a.remove(-0);
    let last = a.pop();
    a.push(first + last);
    if source != [1, 2, 3] { return []; }
    return host_numbers(a);
}
fn flags() -> bool[] {
    let a: bool[] = resizable([]);
    a.reserve(16);
    a.push(true);
    a.push(false);
    a.push(true);
    let first = a.remove(0);
    let last = a.pop();
    a.push(first && last);
    return host_flags(a);
}
fn texts() -> string[] {
    let a = resizable(["é", "λ", "終"]);
    a.reserve(16);
    let first = a.remove(0);
    let last = a.pop();
    a.push(first);
    if last != "終" { return []; }
    return host_texts(a);
}
fn private_range() -> range { return 0..2; }
fn main() -> bool {
    let a = resizable([10, 20, 30]);
    let alias = a;
    let mutate = () => {
        let nested = () => {
            alias.reserve({ alias.push(40); 64 });
            return alias.remove(0);
        };
        return nested();
    };
    a[0] += mutate();
    if a != [20, 30, 40] { return false; }
    a[1] = { alias.remove(0); 7 };
    let last = a.pop();
    let first = alias.remove(-0);
    return last == 7 && first == 30 && len(a) == 0;
}
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
        ("host_flags", "env", "resizable", ArrayElementType::Bool),
        ("host_texts", "resizable", "texts", ArrayElementType::String),
    ] {
        let ty = Type::Array(element);
        compiler
            .register_host_function(name, module, field, vec![ty], ty)
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
            std::env::temp_dir().join(format!("was-resizable-cli-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn cli(&self, args: &[&str]) -> Output {
        test_process::output(
            Command::new(env!("CARGO_BIN_EXE_was-compiler"))
                .current_dir(&self.0)
                .args(args),
            None,
            Duration::from_secs(20),
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
fn cli_resizable_bytes_wat_and_export_selection_match_core_at_each_preset() {
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
            Validator::new().validate_all(&expected).unwrap();
            let bytes = scratch.compile(level, selected, "wasm");
            assert_eq!(bytes, expected, "{level}, selected={selected}");
            if optimization != OptimizationLevel::None {
                if let Some(enabled) = &enabled_bytes {
                    assert_eq!(&bytes, enabled);
                } else {
                    enabled_bytes = Some(bytes.clone());
                }
            }
            let mut exports = Vec::new();
            for payload in Parser::new(0).parse_all(&bytes) {
                if let Payload::ExportSection(section) = payload.unwrap() {
                    for export in section {
                        exports.push(export.unwrap().name.to_owned());
                    }
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
                scratch.compile(level, selected, "wat"),
                wasmprinter::print_bytes(&expected).unwrap().into_bytes()
            );
        }
    }
}

#[test]
fn cli_resizable_diagnostics_match_core_and_preserve_outputs_at_each_preset() {
    let scratch = Scratch::new();
    let mut sources = vec![
        "fn resizable() {}".to_owned(),
        "fn main() { resizable(); }".to_owned(),
        "fn main() { resizable([1], [2]); }".to_owned(),
        "fn main() { resizable(source: [1]); }".to_owned(),
        "fn main() { resizable(true); }".to_owned(),
        "fn main() { let a: bool[] = resizable([1]); }".to_owned(),
        "fn main() { let resizable = 1; resizable([1]); }".to_owned(),
        "fn main() { [1] |> resizable; }".to_owned(),
    ];
    for (method, args, bad_arity, bad_type) in [
        ("reserve", "1", "", "true"),
        ("push", "1", "1, 2", "true"),
        ("pop", "", "1", ""),
        ("remove", "0", "", "false"),
    ] {
        for expression in [
            format!("(a).{method}({args})"),
            format!("a.{method}({args}).pop()"),
            format!("a.{method}({bad_arity})"),
            format!("a.{method}(item: 1)"),
        ] {
            sources.push(format!("fn main(a: number[]) {{ {expression}; }}"));
        }
        sources.push(format!("fn main(a: bool) {{ a.{method}({args}); }}"));
        sources.push(format!("fn main() {{ missing.{method}({args}); }}"));
        if method != "pop" {
            sources.push(format!(
                "fn main(a: number[]) {{ a.{method}({bad_type}); }}"
            ));
        }
    }
    sources.extend([
        "fn main(a: bool[]) { a.push(1); }".into(),
        "fn main(a: string[]) { a.push(false); }".into(),
        "fn main(a: bool[]) -> number { return a.pop(); }".into(),
        "fn main(a: string[]) -> bool { return a.remove(0); }".into(),
        "fn main(a: number[] = resizable([])) {}".into(),
        "fn main(a: number[]) requires len(resizable(a)) == 0 {}".into(),
    ]);
    for source in sources {
        fs::write(scratch.0.join("input.was"), &source).unwrap();
        let mut baseline = None;
        for (level, optimization) in LEVELS {
            let mut native = Compiler::new();
            native.set_optimization_level(optimization);
            let error = native.compile_source(&source).unwrap_err();
            let expected = format!("{}\n", diagnostic::render("input.was", &source, &error));
            if let Some(baseline) = &baseline {
                assert_eq!(&expected, baseline, "{source}");
            } else {
                baseline = Some(expected.clone());
            }
            for format in ["wasm", "wat"] {
                fs::write(scratch.0.join("output"), b"unchanged").unwrap();
                let output = scratch.cli(&["input.was", "-o", "output", level, "--emit", format]);
                assert_eq!(output.status.code(), Some(1), "{level}: {source}");
                assert!(output.stdout.is_empty());
                assert_eq!(
                    String::from_utf8(output.stderr).unwrap(),
                    expected,
                    "{source}"
                );
                assert_eq!(fs::read(scratch.0.join("output")).unwrap(), b"unchanged");
            }
        }
    }
}

#[test]
fn cli_resizable_host_language_name_rejection_matches_core_without_output_changes() {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("input.was"), "fn main() {}").unwrap();
    let error = Compiler::new()
        .register_host_function("resizable", "resizable", "resizable", vec![], Type::Void)
        .unwrap_err();
    let declaration = "resizable=resizable.resizable()->void";
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
                declaration,
            ]);
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert_eq!(String::from_utf8(output.stderr).unwrap(), expected);
            assert_eq!(fs::read(scratch.0.join("output")).unwrap(), b"unchanged");
        }
    }
}

#[test]
#[ignore = "requires Node.js"]
fn cli_resizable_typed_results_captures_and_nested_effects_execute_at_each_preset() {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("input.was"), SOURCE).unwrap();
    let script = r#"
        const assert = require('node:assert/strict');
        const module = new WebAssembly.Module(require('node:fs').readFileSync('output'));
        assert.deepEqual(WebAssembly.Module.imports(module), [
            {module:'resizable', name:'resizable', kind:'function'},
            {module:'env', name:'resizable', kind:'function'},
            {module:'resizable', name:'texts', kind:'function'}
        ]);
        const calls = [];
        const echo = kind => h => { calls.push(kind); return h; };
        const e = new WebAssembly.Instance(module, {
            resizable: {resizable: echo('number'), texts: echo('string')},
            env: {resizable: echo('bool')}
        }).exports;
        assert.equal(e.main(), 1);
        const view = () => new DataView(e.memory.buffer);
        const word = p => view().getUint32(p, true);
        const text = h => Buffer.from(e.memory.buffer, word(h), word(h+4)).toString('utf8');
        for (const [name, width, expected] of [
            ['numbers', 8, [2,3,5]], ['flags', 1, [0,1]], ['texts', 4, ['λ','é']]
        ]) {
            const h = e[name]();
            const p = word(h), n = word(h+4);
            assert.equal(h % 8, 0);
            const actual = Array.from({length:n}, (_, i) => name === 'numbers'
                ? view().getFloat64(p + i*width, true)
                : name === 'flags' ? view().getUint8(p+i) : text(word(p+i*width)));
            assert.deepEqual(actual, expected);
        }
        assert.deepEqual(calls, ['number','bool','string']);
        assert.ok(!('private_range' in e));
        assert.deepEqual(Object.keys(e).sort(),
            ['__alloc','flags','main','memory','numbers','texts'].sort());
    "#;
    for (level, _) in LEVELS {
        scratch.compile(level, true, "wasm");
        let output = test_process::output(
            Command::new("node")
                .args(["-e", script])
                .current_dir(&scratch.0),
            None,
            Duration::from_secs(20),
        );
        assert!(
            output.status.success(),
            "{level}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
