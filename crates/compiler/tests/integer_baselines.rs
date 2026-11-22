#[path = "../../../tests/support/integer_corpus.rs"]
mod corpus;

#[path = "../../../tests/support/process.rs"]
mod test_process;

use was_compiler::{Compiler, CompilerOptions, OptimizationLevel, codegen};
use wasmparser::{Operator, Parser, Payload, ValType, Validator};

#[test]
fn integer_workloads_preserve_the_unoptimized_public_compilation_contract() {
    for case in corpus::cases() {
        let compiler = corpus::compiler(OptimizationLevel::None);
        let typed = compiler.analyze(&case.source).unwrap();
        let bytes = compiler.compile(&typed).unwrap();
        Validator::new().validate_all(&bytes).unwrap();
        assert_eq!(bytes, compiler.compile_source(&case.source).unwrap());
        assert_eq!(
            case.source,
            corpus::cases()
                .into_iter()
                .find(|c| c.name == case.name)
                .unwrap()
                .source
        );
        let mut floating_constants = 0;
        let mut floating_arithmetic = 0;
        for payload in Parser::new(0).parse_all(&bytes) {
            match payload.unwrap() {
                Payload::StartSection { .. } => panic!("unexpected startup effects"),
                Payload::CodeSectionEntry(body) => {
                    for op in body.get_operators_reader().unwrap() {
                        match op.unwrap() {
                            Operator::F64Const { .. } => floating_constants += 1,
                            Operator::F64Add | Operator::F64Sub => floating_arithmetic += 1,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        assert!(
            floating_constants > 0,
            "{} retains f64 constants",
            case.name
        );
        if ["literal_", "locals_", "compare_", "negate_"]
            .iter()
            .any(|p| {
                case.name.starts_with(p)
                    && (case.name.ends_with("_1") || case.name.ends_with("_32"))
            })
        {
            assert!(
                floating_arithmetic > 0,
                "{} has baseline f64 arithmetic",
                case.name
            );
        }
        for optimization_level in [
            OptimizationLevel::None,
            OptimizationLevel::Basic,
            OptimizationLevel::Standard,
            OptimizationLevel::Aggressive,
        ] {
            let selected = corpus::compiler(optimization_level);
            let selected_bytes = selected.compile_source(&case.source).unwrap();
            Validator::new().validate_all(&selected_bytes).unwrap();
            if optimization_level == OptimizationLevel::None {
                assert_eq!(selected_bytes, bytes, "{}", case.name);
            }
            assert_eq!(selected.analyze(&case.source).unwrap(), typed);
            assert_eq!(
                selected.compile(&typed).unwrap(),
                selected_bytes,
                "{}",
                case.name
            );
            assert_eq!(
                selected.compile_source(&case.source).unwrap(),
                selected_bytes
            );
        }
        assert_eq!(*Compiler::new().options(), CompilerOptions::default());
    }
}

#[test]
fn loop_local_composes_integer_storage_with_direct_range_record_elision() {
    let case = corpus::cases()
        .into_iter()
        .find(|case| case.name == "loop_local")
        .unwrap();
    let program = corpus::compiler(OptimizationLevel::None)
        .analyze(&case.source)
        .unwrap();
    let baseline = codegen::compile(&program).unwrap();
    let mut first_enabled_bytes = None;

    // execute_integer_workloads independently checks this same case returns 4576.
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        let compiler = corpus::compiler(level);
        let bytes = compiler.compile_source(&case.source).unwrap();
        assert_eq!(bytes, compiler.compile(&program).unwrap(), "{level:?}");
        Validator::new().validate_all(&bytes).unwrap();
        let main = Parser::new(0)
            .parse_all(&bytes)
            .find_map(|payload| match payload.unwrap() {
                Payload::CodeSectionEntry(body) => Some(body),
                _ => None,
            })
            .unwrap();
        let locals = main
            .get_locals_reader()
            .unwrap()
            .into_iter()
            .map(Result::unwrap)
            .flat_map(|(count, ty)| std::iter::repeat_n(ty, usize::try_from(count).unwrap()))
            .collect::<Vec<_>>();
        let ops = main
            .get_operators_reader()
            .unwrap()
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let calls = ops
            .iter()
            .filter(|op| matches!(op, Operator::Call { .. }))
            .count();
        let loads = ops
            .iter()
            .filter(|op| matches!(op, Operator::I64Load { .. } | Operator::I32Load { .. }))
            .count();
        let int_adds = ops
            .iter()
            .filter(|op| matches!(op, Operator::I32Add))
            .count();
        let float_adds = ops
            .iter()
            .filter(|op| matches!(op, Operator::F64Add))
            .count();

        // Range iteration remains i64 in both layouts. The record path additionally
        // constructs and validates a handle, then loads its fields in main.
        assert!(
            ops.iter().any(|op| matches!(op, Operator::I64Add)),
            "{level:?}"
        );
        if level == OptimizationLevel::None {
            assert_eq!(bytes, baseline);
            assert_eq!(locals[..4], [ValType::F64; 4]);
            assert_eq!((int_adds, float_adds), (0, 2));
            assert_eq!(calls, 2);
            assert_eq!(loads, 4);
        } else {
            assert_ne!(bytes, baseline, "{level:?}");
            assert_eq!(
                &bytes,
                first_enabled_bytes.get_or_insert_with(|| bytes.clone()),
                "{level:?}"
            );
            // The accumulator and iteration binding stay f64; only a and b narrow.
            assert_eq!(
                locals[..4],
                [ValType::F64, ValType::F64, ValType::I32, ValType::I32]
            );
            assert_eq!((int_adds, float_adds), (1, 1));
            assert!(ops.iter().any(|op| matches!(op, Operator::F64ConvertI32S)));
            assert_eq!((calls, loads), (0, 0));
        }
    }
}

fn execute(mode: &str, levels: &[OptimizationLevel]) {
    let mut records = Vec::new();
    for &level in levels {
        for case in corpus::cases() {
            let bytes = corpus::compiler(level)
                .compile_source(&case.source)
                .unwrap();
            records.push(format!(
                r#"["{}","{level:?}",{:?},{bytes:?}]"#,
                case.name,
                case.source.as_bytes()
            ));
        }
    }
    let input = format!("[{}]", records.join(","));
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/integer_measure.cjs"
    );
    let output = test_process::output(
        std::process::Command::new("node").args([script, mode]),
        Some(input.as_bytes()),
        std::time::Duration::from_secs(60),
    );
    assert!(output.status.success(), "{output:?}");
    println!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(String::from_utf8_lossy(&output.stdout).contains("integer workloads passed"));
}

#[test]
#[ignore = "requires Node.js 24+ on PATH"]
fn execute_integer_workloads() {
    execute(
        "verify",
        &[
            OptimizationLevel::None,
            OptimizationLevel::Basic,
            OptimizationLevel::Standard,
            OptimizationLevel::Aggressive,
        ],
    );
}

#[test]
#[ignore = "manual bounded baseline; requires Node.js 24+"]
fn record_integer_guest_timing() {
    execute("timing", &[corpus::measurement_level()]);
}
