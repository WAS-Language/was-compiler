#[path = "../../../tests/support/process.rs"]
mod test_process;

use std::{path::Path, process::Command, time::Duration};
use was_compiler::{Compiler, OptimizationLevel};
use wasmparser::Validator;

#[test]
#[ignore = "requires Node.js and freshly rebuilt browser bindings in pkg"]
fn rebuilt_browser_integer_specialization_bytes_and_wat_match_library_at_every_level() {
    for (name, source, expected_result) in [
        (
            "literal",
            "fn main() -> number { return (10 + 20) - 3; }",
            27.0,
        ),
        (
            "chain",
            "fn main() -> number { let a = 10; let b = a + 20; let c = b - 3; return c; }",
            27.0,
        ),
        (
            "writes",
            "fn main() -> number { let a = 10; a = 2.5; let b = 20; b += a; b -= 1; b *= 2; b /= 4; b %= 3; return a + b; }",
            4.25,
        ),
        (
            "skipped_write",
            "fn main() -> number { let a = 10; false && { a = 2.5; true }; if false { a += 0.5; } let b = a + 20; return b; }",
            30.0,
        ),
        (
            "unreachable_write",
            "fn main() -> number { let a = 10; let b = a + 20; return b; a = 2.5; a += 0.5; }",
            30.0,
        ),
        (
            "captures",
            "fn main() -> number { let a = 10; let read = () => { let b = 20; let c = b + 3; return a + c; }; let before = read(); a += 0.5; return before + read(); }",
            66.5,
        ),
        (
            "readonly_capture",
            "fn main() -> number { let a = 10; let read = () => a; let b = 20; return read() + b; }",
            30.0,
        ),
        (
            "loop_scope",
            "fn main() -> number { let a = 10; let total = 0; for i in 0..3 { let a = 20; let b = a + 3; if i == 1 { continue; } total += b; } return total + a; }",
            56.0,
        ),
        (
            "calls",
            "fn identity(x: number) -> number { return x; } fn main() -> number { let a = 10; let b = a + 20; let c = identity(b); return c + a; }",
            40.0,
        ),
    ] {
        assert_rebuilt_browser_parity(name, source, expected_result);
    }
}

#[test]
#[ignore = "requires Node.js and freshly rebuilt browser bindings in pkg"]
fn rebuilt_browser_integer_comparisons_and_negation_match_library_at_every_level() {
    for (name, source, expected_result) in [
        (
            "expression_comparisons",
            "fn main() -> number { return if -7 < -3 && -3 <= -3 && 7 > -3 && 7 >= 7 && -7 == -7 && -7 != 7 { 1 } else { 0 }; }",
            1.0,
        ),
        (
            "local_composition",
            "fn main() -> number { let a = 10; let b = -(a + 20); let c = -b; return if b < a && b <= -30 && c > a && c >= 30 && c == 30 && b != c { c + 3 } else { 0 }; }",
            33.0,
        ),
        (
            "signed_endpoints",
            "fn main() -> number { return if (0 - 2147483647 - 1) < 2147483647 && -(0 - 2147483647 - 1) == 2147483648 && -2147483647 < 0 { 1 } else { 0 }; }",
            1.0,
        ),
        (
            "negated_zero",
            "fn main() -> number { return -(4 - 4); }",
            -0.0,
        ),
        (
            "double_negated_zero",
            "fn main() -> number { return -(-0); }",
            0.0,
        ),
        (
            "negated_minimum",
            "fn main() -> number { let minimum = 0 - 2147483647 - 1; return -minimum; }",
            2147483648.0,
        ),
        (
            "negated_maximum",
            "fn main() -> number { return -(-2147483647); }",
            2147483647.0,
        ),
        (
            "mixed_unknown",
            "fn identity(x: number) -> number { return x; } fn main() -> number { let a = 10; let b = a + 20; return if identity(-0) == 0 && identity(b) > a && a <= identity(b) && identity(0 / 0) != b && !(identity(0 / 0) < b) { -identity(b) } else { 0 }; }",
            -30.0,
        ),
    ] {
        assert_rebuilt_browser_parity(name, source, expected_result);
    }
}

#[test]
#[ignore = "requires Node.js and freshly rebuilt browser bindings in pkg"]
fn rebuilt_browser_integer_comparisons_and_negation_diagnostics_match_library() {
    let mut cases = Vec::new();
    for (source, message) in [
        (
            "fn main() -> bool {\n    return false && (1 < true);\n}",
            "Operator '<' cannot be applied to Number and Bool",
        ),
        (
            "fn main() -> number {\n    return 1;\n    -true;\n}",
            "Type mismatch: expected Number, found Bool",
        ),
    ] {
        let mut compiler = Compiler::new();
        let baseline = compiler.compile_source(source).unwrap_err();
        assert!(baseline.span().is_some());
        let expected = was_compiler::diagnostic::render("<browser>", source, &baseline);
        assert!(expected.contains(message), "{expected}");
        for level in [
            OptimizationLevel::None,
            OptimizationLevel::Basic,
            OptimizationLevel::Standard,
            OptimizationLevel::Aggressive,
        ] {
            compiler.set_optimization_level(level);
            let error = compiler.compile_source(source).unwrap_err();
            assert_eq!(error.span(), baseline.span());
            assert_eq!(
                was_compiler::diagnostic::render("<browser>", source, &error),
                expected
            );
        }
        cases.push((source, expected));
    }

    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let package = root.join("pkg");
    for name in ["was_compiler.js", "was_compiler_bg.wasm"] {
        assert!(
            package.join(name).is_file(),
            "rebuild browser bindings before this test"
        );
    }
    let payload = format!(
        "[{}]",
        cases
            .iter()
            .map(|(source, expected)| format!("[{source:?}, {expected:?}]"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let script = r#"
        const assert = require('node:assert/strict');
        const path = require('node:path');
        const fs = require('node:fs');
        const { pathToFileURL } = require('node:url');
        const cases = JSON.parse(fs.readFileSync(0, 'utf8'));
        (async () => {
            const directory = process.argv[1];
            const { default: init, Compiler } = await import(pathToFileURL(path.join(directory, 'was_compiler.js')).href);
            await init({ module_or_path: await fs.promises.readFile(path.join(directory, 'was_compiler_bg.wasm')) });
            const compiler = new Compiler();
            try {
                for (const level of [0, 1, 2, 3, 0]) {
                    compiler.setOptimizationLevel(level);
                    for (const [source, expected] of cases) {
                        for (const method of ['compile', 'compileWat']) {
                            assert.throws(() => compiler[method](source), error => {
                                assert.ok(error instanceof Error);
                                assert.equal(error.message, expected, `${method}: level ${level}`);
                                return true;
                            });
                        }
                    }
                }
            } finally { compiler.free(); }
        })().catch(error => { console.error(error); process.exitCode = 1; });
    "#;
    let output = test_process::output(
        Command::new("node")
            .arg("-e")
            .arg(script)
            .arg(package)
            .current_dir(root),
        Some(payload.as_bytes()),
        Duration::from_secs(30),
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_rebuilt_browser_parity(name: &str, source: &str, expected_result: f64) {
    let mut compiler = Compiler::new();
    let baseline = compiler.compile_source(source).unwrap();
    let mut native = Vec::new();
    let mut native_wat = Vec::new();
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        compiler.set_optimization_level(level);
        let bytes = compiler.compile_source(source).unwrap();
        Validator::new().validate_all(&bytes).unwrap();
        if level == OptimizationLevel::None {
            assert_eq!(bytes, baseline);
        } else {
            assert_ne!(
                bytes, baseline,
                "{name}: {level:?} must specialize eligible literals or locals"
            );
        }
        native_wat.push(wasmprinter::print_bytes(&bytes).unwrap());
        native.push(bytes);
    }

    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let package = root.join("pkg");
    for name in ["was_compiler.js", "was_compiler_bg.wasm"] {
        assert!(
            package.join(name).is_file(),
            "rebuild browser bindings before this test"
        );
    }
    let payload = format!(
        r#"{{"source": {source:?}, "native": {native:?}, "nativeWat": {native_wat:?}, "expectedResult": {expected_result:?}}}"#
    );
    let script = r#"
        const assert = require('node:assert/strict');
        const path = require('node:path');
        const fs = require('node:fs');
        const { pathToFileURL } = require('node:url');
        const { source, native, nativeWat, expectedResult } = JSON.parse(fs.readFileSync(0, 'utf8'));
        (async () => {
            const directory = process.argv[1];
            const { default: init, Compiler } = await import(pathToFileURL(path.join(directory, 'was_compiler.js')).href);
            await init({ module_or_path: await fs.promises.readFile(path.join(directory, 'was_compiler_bg.wasm')) });
            const compiler = new Compiler();
            try {
                assert.deepEqual(Array.from(compiler.compile(source)), native[0]);
                for (const level of [0, 1, 2, 3, 0]) {
                    compiler.setOptimizationLevel(level);
                    const bytes = compiler.compile(source);
                    assert.deepEqual(Array.from(bytes), native[level], `level ${level}`);
                    assert.equal(compiler.compileWat(source), nativeWat[level]);
                    assert.ok(WebAssembly.validate(bytes));
                    const instance = new WebAssembly.Instance(new WebAssembly.Module(bytes));
                    assert.ok(Object.is(instance.exports.main(), expectedResult), `result: level ${level}`);
                    if (level > 0) {
                        assert.notDeepEqual(Array.from(bytes), native[0]);
                        assert.deepEqual(Array.from(bytes), native[1]);
                    }
                    assert.deepEqual(Array.from(compiler.compile(source)), native[level]);
                }
            } finally { compiler.free(); }
        })().catch(error => { console.error(error); process.exitCode = 1; });
        "#;
    let output = test_process::output(
        Command::new("node")
            .arg("-e")
            .arg(script)
            .arg(package)
            .current_dir(root),
        Some(payload.as_bytes()),
        Duration::from_secs(30),
    );
    assert!(
        output.status.success(),
        "{name}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
