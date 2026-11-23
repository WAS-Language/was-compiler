#[path = "../../../tests/support/process.rs"]
mod test_process;

use std::{process::Command, time::Duration};
use was_compiler::{Compiler, CompilerOptions, OptimizationLevel};
use wasmparser::Validator;

const SOURCE: &str = include_str!("../../../tests/support/m5_string_timing.was");

#[test]
fn runtime_corpus_is_ordinary_deterministic_and_preset_compatible() {
    let baseline = Compiler::with_options(CompilerOptions {
        optimization_level: OptimizationLevel::None,
    })
    .compile_source(SOURCE)
    .unwrap();
    let mut enabled = None;
    for optimization_level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        let compiler = Compiler::with_options(CompilerOptions { optimization_level });
        let bytes = compiler.compile_source(SOURCE).unwrap();
        Validator::new().validate_all(&bytes).unwrap();
        assert_eq!(bytes, compiler.compile_source(SOURCE).unwrap());
        assert_eq!(
            bytes,
            compiler
                .compile(&compiler.analyze(SOURCE).unwrap())
                .unwrap()
        );
        if optimization_level != OptimizationLevel::None {
            if let Some(expected) = &enabled {
                assert_eq!(&bytes, expected);
            } else {
                enabled = Some(bytes.clone());
            }
        } else {
            assert_eq!(bytes, baseline);
        }
        let module = wasmparser::Parser::new(0).parse_all(&bytes);
        let mut exports = Vec::new();
        for payload in module {
            if let wasmparser::Payload::ExportSection(section) = payload.unwrap() {
                exports.extend(section.into_iter().map(|e| e.unwrap().name.to_owned()));
            }
        }
        exports.sort();
        assert_eq!(
            exports,
            [
                "__alloc",
                "accumulate",
                "join_existing",
                "memory",
                "repeated",
                "replace"
            ]
        );
    }
}

#[test]
#[ignore = "requires Node.js 24+; checks all runtime cases without collecting timings"]
fn runtime_cases_have_independent_oracles_and_no_observation_exports() {
    let bytes = Compiler::new().compile_source(SOURCE).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = test_process::output(
        Command::new("node")
            .arg(root.join("tests/support/m5_string_timing.cjs"))
            .args(["check", "-"]),
        Some(&bytes),
        Duration::from_secs(120),
    );
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"checkedCases\":123"));
}
