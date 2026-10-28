use super::*;
use crate::Compiler;
use wasmparser::{Operator, Parser, Payload, Validator};

const SOURCE: &str = "fn retained() -> number[] { return [17, 29]; }";

fn modules() -> [Vec<u8>; 3] {
    let program = Compiler::new().analyze(SOURCE).unwrap();
    let modules = [
        super::super::compile(&program).unwrap(),
        compile(&program, &ExportOptions::All, Config::default()).unwrap(),
        compile_instrumented(&program, &ExportOptions::All, Config::default()).unwrap(),
    ];
    for bytes in &modules {
        Validator::new().validate_all(bytes).unwrap();
    }
    modules
}

fn record(mode: &str) {
    use std::{path::PathBuf, process::Command, time::Duration};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output_dir = root.join("target/m3-allocator");
    std::fs::create_dir_all(&output_dir).unwrap();
    let [ordinary, prototype, instrumented] = modules();
    let input = format!(
        "{{\"mode\":{mode:?},\"source\":{SOURCE:?},\"prototype\":{prototype:?},\"instrumented\":{instrumented:?}}}"
    );
    let output = crate::test_process::output(
        Command::new("node").arg(root.join("tests/support/m3_allocator_measure.cjs")),
        Some(input.as_bytes()),
        Duration::from_secs(60),
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for (name, bytes) in [
        ("ordinary", ordinary),
        ("prototype", prototype),
        ("instrumented", instrumented),
    ] {
        std::fs::write(output_dir.join(format!("{name}.wasm")), bytes).unwrap();
    }
    std::fs::write(output_dir.join("workload.was"), SOURCE).unwrap();
    std::fs::write(output_dir.join(format!("{mode}.json")), &output.stdout).unwrap();
    println!("{}", String::from_utf8_lossy(&output.stdout));
}

#[test]
#[ignore = "requires Node.js; explicit isolated measurement"]
fn record_m3_accounting() {
    record("accounting");
}

#[test]
#[ignore = "requires Node.js; explicit isolated measurement"]
fn record_m3_registry_search() {
    record("search");
}

#[test]
#[ignore = "requires Node.js; explicit isolated noninstrumented timing"]
fn record_m3_guest_timing() {
    record("timing");
}

#[test]
fn instrumentation_is_explicit_and_preserves_calls_and_memory_operations() {
    let [ordinary, prototype, instrumented] = modules();
    let operations = |bytes: &[u8]| {
        let mut bodies = Vec::new();
        for payload in Parser::new(0).parse_all(bytes) {
            if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                let mut operations = Vec::new();
                for op in body.get_operators_reader().unwrap() {
                    let op = op.unwrap();
                    if matches!(
                        op,
                        Operator::Call { .. }
                            | Operator::I32Load { .. }
                            | Operator::I64Load { .. }
                            | Operator::I32Store { .. }
                            | Operator::I64Store { .. }
                            | Operator::MemoryGrow { .. }
                    ) {
                        operations.push(format!("{op:?}"));
                    }
                }
                bodies.push(operations);
            }
        }
        bodies
    };
    assert_eq!(operations(&prototype), operations(&instrumented));
    let bodies = |bytes: &[u8]| {
        Parser::new(0)
            .parse_all(bytes)
            .filter_map(|payload| {
                if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                    Some(
                        body.get_operators_reader()
                            .unwrap()
                            .into_iter()
                            .map(|op| format!("{:?}", op.unwrap()))
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    let prototype_bodies = bodies(&prototype);
    let mut observed_bodies = bodies(&instrumented);
    let globals = |bytes: &[u8]| {
        Parser::new(0)
            .parse_all(bytes)
            .filter_map(|payload| {
                if let Payload::GlobalSection(section) = payload.unwrap() {
                    Some(section.count())
                } else {
                    None
                }
            })
            .sum::<u32>()
    };
    let counter = globals(&prototype);
    assert_eq!(globals(&instrumented), counter + 1);
    // FIND is the third-last private helper. Exactly one counter increment is
    // inserted after the null-record guard, before the exact-payload comparison.
    let find = observed_bodies.len() - 3;
    let expected = [
        format!("GlobalGet {{ global_index: {counter} }}"),
        "I64Const { value: 1 }".to_owned(),
        "I64Add".to_owned(),
        format!("GlobalSet {{ global_index: {counter} }}"),
    ];
    assert_eq!(observed_bodies[find][7..11], expected);
    observed_bodies[find].drain(7..11);
    assert_eq!(observed_bodies, prototype_bodies);
    let counter_exports = |bytes: &[u8]| {
        Parser::new(0)
            .parse_all(bytes)
            .filter_map(|p| {
                if let Payload::ExportSection(exports) = p.unwrap() {
                    Some(
                        exports
                            .into_iter()
                            .map(Result::unwrap)
                            .filter(|e| e.name == "__m3_registry_comparisons")
                            .count(),
                    )
                } else {
                    None
                }
            })
            .sum::<usize>()
    };
    assert_eq!(counter_exports(&ordinary), 0);
    assert_eq!(counter_exports(&prototype), 0);
    assert_eq!(counter_exports(&instrumented), 1);
    assert_eq!(modules(), [ordinary, prototype, instrumented]);
}

#[test]
fn instrumentation_export_collision_is_private_to_the_requested_variant() {
    let program = Compiler::new()
        .analyze("fn __m3_registry_comparisons() -> number[] { return [1]; }")
        .unwrap();
    compile(&program, &ExportOptions::All, Config::default()).unwrap();
    assert!(matches!(
        compile_instrumented(&program, &ExportOptions::All, Config::default()),
        Err(Error::Checked(CodegenError::DuplicateExportName { name }))
            if name == "__m3_registry_comparisons"
    ));
}
