use super::*;
use crate::optimization::{OptimizationLevel, optimize};
use std::{path::PathBuf, process::Command, time::Duration};
use wasmparser::{ExternalKind, Operator, Parser, Payload, Validator};

const SOURCE: &str = r#"
fn numbers(n: number, i: number) -> number {
    let a = [n, n + 1];
    a[0] += 5;
    return a[i];
}
fn aligned(n: number) -> number {
    let e: number[] = [];
    let b: bool[] = [true, false, true, false, true, false, true, false, true];
    let a = [n];
    return len(e) + len(b) + a[0];
}
fn lazy() -> number { return 7; let a = [1]; }
"#;

fn levels() -> [OptimizationLevel; 4] {
    [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ]
}

fn measured_modules(program: &TypedProgram, level: OptimizationLevel) -> [Vec<u8>; 3] {
    let optimized = optimize(program, level);
    assert!(matches!(optimized, std::borrow::Cow::Borrowed(_)));
    let policy = level.numeric_policy();
    let ordinary =
        super::super::compile_with_policy(&optimized, &ExportOptions::All, policy).unwrap();
    let prototype = compile(&optimized, &ExportOptions::All, policy).unwrap();
    let instrumented = compile_instrumented(&optimized, &ExportOptions::All, policy).unwrap();
    assert_eq!(
        ordinary,
        super::super::compile_with_policy(&optimized, &ExportOptions::All, policy).unwrap()
    );
    assert_eq!(
        prototype,
        compile(&optimized, &ExportOptions::All, policy).unwrap()
    );
    assert_eq!(
        instrumented,
        compile_instrumented(&optimized, &ExportOptions::All, policy).unwrap()
    );
    for bytes in [&ordinary, &prototype, &instrumented] {
        Validator::new().validate_all(bytes).unwrap();
    }
    [ordinary, prototype, instrumented]
}

#[test]
fn measured_presets_are_deterministic_and_enabled_bytes_match() {
    let program = crate::Compiler::new().analyze(SOURCE).unwrap();
    let mut enabled = None;
    for level in levels() {
        let modules = measured_modules(&program, level);
        if level != OptimizationLevel::None {
            if let Some(previous) = &enabled {
                assert_eq!(&modules, previous);
            } else {
                enabled = Some(modules);
            }
        }
    }
}

fn record(mode: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output_dir = root.join("target/m2-regions");
    std::fs::create_dir_all(&output_dir).unwrap();
    std::fs::write(output_dir.join("workload.was"), SOURCE).unwrap();
    let program = crate::Compiler::new().analyze(SOURCE).unwrap();
    for level in levels() {
        let [ordinary, prototype, mut instrumented] = measured_modules(&program, level);
        if mode == "failure" {
            instrumented = super::runtime_tests::cap_memory(&instrumented, 1);
            Validator::new().validate_all(&instrumented).unwrap();
        }
        let input = format!(
            "{{\"source\":{SOURCE:?},\"preset\":\"{level:?}\",\"mode\":\"{mode}\",\"baseline\":{ordinary:?},\"prototype\":{prototype:?},\"instrumented\":{instrumented:?},\"capacities\":[{{\"name\":\"numbers\",\"capacity\":24}},{{\"name\":\"aligned\",\"capacity\":48}},{{\"name\":\"lazy\",\"capacity\":16}}]}}"
        );
        let output = crate::test_process::output(
            Command::new("node").arg(root.join("tests/support/m2_region_measure.cjs")),
            Some(input.as_bytes()),
            Duration::from_secs(60),
        );
        assert!(
            output.status.success(),
            "{mode}/{level:?}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        for (name, bytes) in [
            ("baseline", &ordinary),
            ("prototype", &prototype),
            ("instrumented", &instrumented),
        ] {
            std::fs::write(
                output_dir.join(format!("{mode}-{level:?}-{name}.wasm")),
                bytes,
            )
            .unwrap();
        }
        std::fs::write(
            output_dir.join(format!("{mode}-{level:?}.json")),
            &output.stdout,
        )
        .unwrap();
        println!("{}", String::from_utf8_lossy(&output.stdout));
    }
}

#[test]
#[ignore = "requires Node.js; explicit isolated measurement"]
fn record_m2_accounting() {
    record("accounting");
}

#[test]
#[ignore = "requires Node.js; explicit isolated measurement"]
fn record_m2_capacity() {
    record("capacity");
}

#[test]
#[ignore = "requires Node.js; explicit isolated measurement"]
fn record_m2_guest_timing() {
    record("timing");
}

#[test]
#[ignore = "requires Node.js; explicit isolated measurement"]
fn record_m2_failed_provisioning() {
    record("failure");
}

#[test]
fn instrumentation_is_explicit_and_adds_no_calls_or_storage_selection_changes() {
    let program = crate::Compiler::new().analyze(SOURCE).unwrap();
    let ordinary = super::super::compile(&program).unwrap();
    let capacities: Vec<_> = select(&program)
        .iter()
        .map(|selection| {
            let Selection::Region(plan) = selection else {
                panic!("workload must remain eligible")
            };
            plan.capacity
        })
        .collect();
    assert_eq!(capacities, [24, 48, 16]);
    let prototype = compile(&program, &ExportOptions::All, NumericPolicy::Baseline).unwrap();
    let instrumented =
        compile_instrumented(&program, &ExportOptions::All, NumericPolicy::Baseline).unwrap();
    let calls = |bytes: &[u8]| {
        let mut calls = Vec::new();
        for payload in Parser::new(0).parse_all(bytes) {
            if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                calls.push(
                    body.get_operators_reader()
                        .unwrap()
                        .into_iter()
                        .filter_map(|op| match op.unwrap() {
                            Operator::Call { function_index } => Some(function_index),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                );
            }
        }
        calls
    };
    assert_eq!(calls(&prototype), calls(&instrumented));
    for (bytes, expected, expected_globals) in [
        (&ordinary, 0, 1),
        (&prototype, 0, 4),
        (&instrumented, 12, 16),
    ] {
        Validator::new().validate_all(bytes).unwrap();
        let count: usize = Parser::new(0)
            .parse_all(bytes)
            .filter_map(|payload| match payload.unwrap() {
                Payload::ExportSection(exports) => Some(
                    exports
                        .into_iter()
                        .filter(|export| {
                            let export = export.as_ref().unwrap();
                            export.name.starts_with("__m2_") && export.kind == ExternalKind::Global
                        })
                        .count(),
                ),
                Payload::GlobalSection(globals) => {
                    assert_eq!(globals.count(), expected_globals);
                    None
                }
                Payload::ImportSection(imports) => {
                    assert_eq!(imports.count(), 0);
                    None
                }
                _ => None,
            })
            .sum();
        assert_eq!(count, expected);
    }
    assert_eq!(ordinary, super::super::compile(&program).unwrap());
    assert_eq!(
        prototype,
        compile(&program, &ExportOptions::All, NumericPolicy::Baseline).unwrap()
    );
}
