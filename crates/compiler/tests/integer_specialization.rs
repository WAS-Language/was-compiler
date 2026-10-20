use was_compiler::{Compiler, CompilerOptions, OptimizationLevel, codegen};
use wasmparser::{Parser, Payload, ValType, Validator};

#[path = "../../../tests/support/process.rs"]
mod test_process;

const ENABLED: [OptimizationLevel; 3] = [
    OptimizationLevel::Basic,
    OptimizationLevel::Standard,
    OptimizationLevel::Aggressive,
];

fn compiler(level: OptimizationLevel) -> Compiler {
    Compiler::with_options(CompilerOptions {
        optimization_level: level,
    })
}

fn instructions(bytes: &[u8]) -> Vec<String> {
    Validator::new().validate_all(bytes).unwrap();
    let mut result = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::CodeSectionEntry(body) = payload.unwrap() {
            for op in body.get_operators_reader().unwrap() {
                result.push(format!("{:?}", op.unwrap()));
            }
        }
    }
    result
}

#[test]
fn proven_subtrees_use_integer_operations_without_folding_or_changing_the_abi() {
    let source = "fn main() -> number { return (10 + 20) - 3; }";
    let baseline = Compiler::new().compile_source(source).unwrap();
    let program = Compiler::new().analyze(source).unwrap();
    assert_eq!(baseline, codegen::compile(&program).unwrap());
    assert!(instructions(&baseline).contains(&"F64Add".into()));

    for level in ENABLED {
        let selected = compiler(level);
        let bytes = selected.compile(&program).unwrap();
        assert_eq!(bytes, selected.compile_source(source).unwrap());
        assert_ne!(bytes, baseline);
        assert_eq!(
            instructions(&bytes),
            [
                "I32Const { value: 10 }",
                "I32Const { value: 20 }",
                "I32Add",
                "I32Const { value: 3 }",
                "I32Sub",
                "F64ConvertI32S",
                "Return",
                "End",
            ]
        );
        for payload in Parser::new(0).parse_all(&bytes) {
            match payload.unwrap() {
                Payload::TypeSection(types) => {
                    let types: Vec<_> = types
                        .into_iter_err_on_gc_types()
                        .map(Result::unwrap)
                        .collect();
                    assert_eq!(types.len(), 1);
                    assert!(types[0].params().is_empty());
                    assert_eq!(types[0].results(), &[ValType::F64]);
                }
                Payload::CodeSectionEntry(body) => {
                    assert_eq!(body.get_locals_reader().unwrap().get_count(), 0)
                }
                _ => {}
            }
        }
    }
}

#[test]
fn proven_numeric_comparisons_use_signed_integer_operands_without_result_conversion() {
    for (operator, integer, floating) in [
        ("<", "I32LtS", "F64Lt"),
        ("<=", "I32LeS", "F64Le"),
        (">", "I32GtS", "F64Gt"),
        (">=", "I32GeS", "F64Ge"),
        ("==", "I32Eq", "F64Eq"),
        ("!=", "I32Ne", "F64Ne"),
    ] {
        let source = format!(
            "fn main() -> bool {{ return (0 - 2147483647 - 1) {operator} (2147483646 + 1); }}"
        );
        let baseline = Compiler::new().compile_source(&source).unwrap();
        assert!(instructions(&baseline).contains(&floating.into()));
        let expected = compiler(OptimizationLevel::Basic)
            .compile_source(&source)
            .unwrap();
        for level in ENABLED {
            let bytes = compiler(level).compile_source(&source).unwrap();
            assert_eq!(bytes, expected);
            assert_eq!(
                instructions(&bytes),
                [
                    "I32Const { value: 0 }",
                    "I32Const { value: 2147483647 }",
                    "I32Sub",
                    "I32Const { value: 1 }",
                    "I32Sub",
                    "I32Const { value: 2147483646 }",
                    "I32Const { value: 1 }",
                    "I32Add",
                    integer,
                    "Return",
                    "End",
                ]
            );
        }
    }
}

#[test]
fn safe_negation_uses_integer_subtraction_and_propagates_bounds() {
    for (expr, expected) in [
        (
            "-(2 + 3)",
            vec![
                "I32Const { value: 0 }",
                "I32Const { value: 2 }",
                "I32Const { value: 3 }",
                "I32Add",
                "I32Sub",
                "F64ConvertI32S",
                "Return",
                "End",
            ],
        ),
        (
            "-(-2147483647)",
            vec![
                "I32Const { value: 0 }",
                "I32Const { value: 0 }",
                "I32Const { value: 2147483647 }",
                "I32Sub",
                "I32Sub",
                "F64ConvertI32S",
                "Return",
                "End",
            ],
        ),
        (
            "-(1 - 2) + 3",
            vec![
                "I32Const { value: 0 }",
                "I32Const { value: 1 }",
                "I32Const { value: 2 }",
                "I32Sub",
                "I32Sub",
                "I32Const { value: 3 }",
                "I32Add",
                "F64ConvertI32S",
                "Return",
                "End",
            ],
        ),
    ] {
        let source = format!("fn main() -> number {{ return {expr}; }}");
        assert!(
            instructions(&Compiler::new().compile_source(&source).unwrap())
                .contains(&"F64Neg".into())
        );
        for level in ENABLED {
            assert_eq!(
                instructions(&compiler(level).compile_source(&source).unwrap()),
                expected,
                "{expr}"
            );
        }
    }
}

#[test]
fn never_reassigned_local_chains_use_i32_slots_and_one_return_conversion() {
    let source = "fn main() -> number { let a = 10 + 20; let b = a - 3; return b + a; }";
    let baseline = Compiler::new().compile_source(source).unwrap();
    assert_eq!(
        baseline,
        codegen::compile(&Compiler::new().analyze(source).unwrap()).unwrap()
    );
    let expected = compiler(OptimizationLevel::Basic)
        .compile_source(source)
        .unwrap();
    for level in ENABLED {
        let bytes = compiler(level).compile_source(source).unwrap();
        assert_eq!(bytes, expected);
        assert_eq!(
            instructions(&bytes),
            [
                "I32Const { value: 10 }",
                "I32Const { value: 20 }",
                "I32Add",
                "LocalSet { local_index: 0 }",
                "LocalGet { local_index: 0 }",
                "I32Const { value: 3 }",
                "I32Sub",
                "LocalSet { local_index: 1 }",
                "LocalGet { local_index: 1 }",
                "LocalGet { local_index: 0 }",
                "I32Add",
                "F64ConvertI32S",
                "Return",
                "End",
            ]
        );
        for payload in Parser::new(0).parse_all(&bytes) {
            if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                assert_eq!(
                    body.get_locals_reader()
                        .unwrap()
                        .into_iter()
                        .map(Result::unwrap)
                        .collect::<Vec<_>>(),
                    [(1, ValType::I32), (1, ValType::I32)]
                );
            }
        }
    }
}

#[test]
fn parsed_values_and_every_intermediate_bound_control_specialization() {
    // Counts are independent expected opcodes, not values computed by the proof.
    for (expr, integer_adds, integer_subs, float_adds, float_subs) in [
        ("2147483647 + 0", 1, 0, 0, 0),
        ("0 - 2147483647 - 1", 0, 2, 0, 0),
        ("(0 - 2147483647 - 1) - (0 - 1)", 0, 4, 0, 0),
        ("(2147483647 + 1) - 1", 0, 0, 1, 1),
        ("(0 - 2147483647 - 1) - 1", 0, 2, 0, 1),
        ("2147483648 - 1", 0, 0, 0, 1),
        ("1.00000000000000001 + 2.000", 1, 0, 0, 0),
        ("2147483647.00000001 + 0", 1, 0, 0, 0),
        ("2147483647.99999999 - 1", 0, 0, 0, 1),
        ("0 - 0", 0, 1, 0, 0),
        ("-0 + 0", 0, 0, 1, 0),
        ("0 - -0", 0, 0, 0, 1),
        ("-0 - 0", 0, 0, 0, 1),
        ("1.5 + (2 + 3)", 1, 0, 1, 0),
        ("(2 + 3) * (4 - 1)", 1, 1, 0, 0),
        ("-(2 + 3)", 1, 1, 0, 0),
    ] {
        let source = format!("fn main() -> number {{ return {expr}; }}");
        let expected = compiler(OptimizationLevel::Basic)
            .compile_source(&source)
            .unwrap();
        for level in ENABLED {
            let bytes = compiler(level).compile_source(&source).unwrap();
            assert_eq!(bytes, expected, "enabled preset identity: {expr}");
            let ops = instructions(&bytes);
            for (op, count) in [
                ("I32Add", integer_adds),
                ("I32Sub", integer_subs),
                ("F64Add", float_adds),
                ("F64Sub", float_subs),
            ] {
                assert_eq!(
                    ops.iter().filter(|actual| actual.as_str() == op).count(),
                    count,
                    "{level:?}: {expr}: {op}"
                );
            }
        }
    }
}

#[test]
fn unknown_inputs_mutable_storage_and_unspecialized_consumers_remain_floating() {
    for source in [
        "fn main(x: number) -> number { return x; }",
        "fn main(x: number) -> number { return x * 1.5; }",
        "fn main() -> number { return 2147483648; }",
        "fn main() -> number { return 1.5; }",
    ] {
        let baseline = Compiler::new().compile_source(source).unwrap();
        for level in ENABLED {
            assert_eq!(compiler(level).compile_source(source).unwrap(), baseline);
        }
    }

    for (source, required) in [
        (
            "fn main() -> number { let x = 1 + 2; x = 4; return x + 3; }",
            "F64Add",
        ),
        (
            "fn main() -> number { return (1 + 2) * (3 + 4); }",
            "F64Mul",
        ),
        (
            "fn main() -> number { return (1 + 2) / (3 + 4); }",
            "F64Div",
        ),
        (
            "fn main(x: number) -> bool { return (1 + 2) < x; }",
            "F64Lt",
        ),
        ("fn main() -> number { return -(1 + 2 - 3); }", "F64Neg"),
        (
            "fn main() -> number { let a = [1 + 2]; return a[0] + 3; }",
            "F64Add",
        ),
        (
            "fn main() -> number { let x = 1 + 2; let read = () => x + 3; return read(); }",
            "F64Add",
        ),
        (
            "fn main() -> number { return get() + (1 + 2); } fn get() -> number { return 4; }",
            "F64Add",
        ),
    ] {
        for level in ENABLED {
            let bytes = compiler(level).compile_source(source).unwrap();
            let ops = instructions(&bytes);
            assert!(ops.contains(&"I32Add".into()), "{source}");
            assert!(ops.contains(&required.into()), "{source}: {required}");
            for payload in Parser::new(0).parse_all(&bytes) {
                if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                    // Mutable numeric locals remain f64. Handle/scratch/capture
                    // locals may still use their existing i32 types.
                    if source.contains("let x =") && !source.contains("let read") {
                        let locals: Vec<_> = body
                            .get_locals_reader()
                            .unwrap()
                            .into_iter()
                            .map(Result::unwrap)
                            .collect();
                        assert_eq!(locals, [(1, ValType::F64)]);
                    }
                }
            }
        }
    }
}

#[test]
fn proof_cache_preserves_forged_tree_errors_and_does_not_survive_compilation() {
    use was_compiler::{
        Type,
        typed_ast::{ExprResult, TypedExprKind, TypedStatementKind},
    };

    let baseline = Compiler::new();
    let original = baseline
        .analyze("fn main() -> number { return 1 + 2; }")
        .unwrap();
    for corruption in 0..6 {
        let mut program = original.clone();
        let TypedStatementKind::Return(Some(expr)) = &mut program.functions[0].body[0].kind else {
            panic!()
        };
        let TypedExprKind::Binary { left, right, .. } = &mut expr.kind else {
            panic!()
        };
        match corruption {
            0 => expr.ty = ExprResult::Never,
            1 => left.ty = Type::Bool.into(),
            2 => right.kind = TypedExprKind::Number("-0".into()),
            3 => right.kind = TypedExprKind::Number("9".repeat(400)),
            4 => right.kind = TypedExprKind::Number("1e3".into()),
            5 => right.ty = ExprResult::Never,
            _ => unreachable!(),
        }
        let error = baseline.compile(&program).unwrap_err();
        for level in ENABLED {
            assert_eq!(
                compiler(level).compile(&program).unwrap_err(),
                error,
                "corruption {corruption}"
            );
        }
    }
    let mut program = original.clone();
    let selected = compiler(OptimizationLevel::Basic);
    let before = selected.compile(&program).unwrap();
    let TypedStatementKind::Return(Some(expr)) = &mut program.functions[0].body[0].kind else {
        panic!()
    };
    let TypedExprKind::Binary { right, .. } = &mut expr.kind else {
        panic!()
    };
    right.kind = TypedExprKind::Number("2147483648".into());
    let after = selected.compile(&program).unwrap();
    assert_ne!(before, after);
    assert!(instructions(&after).contains(&"F64Add".into()));

    for source in [
        "fn main() -> number { return 1; return 2 + 3; }",
        "fn main() -> bool { return false && (1 + 2 == 3); }",
        "fn main() -> number { return 1; } fn unused() -> number { return 2 + 3; }",
    ] {
        let mut program = baseline.analyze(source).unwrap();
        let target = if program.functions.len() > 1 {
            &mut program.functions[1].body[0]
        } else {
            program.functions[0].body.last_mut().unwrap()
        };
        let TypedStatementKind::Return(Some(expr)) = &mut target.kind else {
            panic!()
        };
        // Corrupt a child behind the optimized candidate / skipped RHS.
        let TypedExprKind::Binary { right, .. } = &mut expr.kind else {
            panic!()
        };
        right.ty = Type::String.into();
        let error = baseline.compile(&program).unwrap_err();
        for level in ENABLED {
            assert_eq!(compiler(level).compile(&program).unwrap_err(), error);
        }
    }
}

#[test]
fn insufficient_comparison_proofs_keep_floating_consumers_and_independent_children() {
    for (operator, floating) in [
        ("<", "F64Lt"),
        ("<=", "F64Le"),
        (">", "F64Gt"),
        (">=", "F64Ge"),
        ("==", "F64Eq"),
        ("!=", "F64Ne"),
    ] {
        for unknown in [
            "x",
            "-0",
            "1.5",
            "2147483648",
            "-2147483649",
            "0 / 0",
            "1 / 0",
            "-1 / 0",
            "2 * 3",
            "9 % 4",
            "a[0]",
            "len(a)",
            "{ 3 }",
            "if true { 3 } else { 4 }",
        ] {
            for (left, right) in [(unknown, "(2 + 3)"), ("(2 + 3)", unknown)] {
                let source = format!(
                    "fn main(x: number) -> bool {{ let a = [3]; return ({left}) {operator} ({right}); }}"
                );
                for level in ENABLED {
                    let ops = instructions(&compiler(level).compile_source(&source).unwrap());
                    assert!(ops.contains(&floating.into()), "{level:?}: {source}");
                    assert!(ops.contains(&"I32Add".into()), "{source}");
                    assert!(ops.contains(&"F64ConvertI32S".into()), "{source}");
                }
            }
        }
    }
    // A guard/contract/call-site does not provide a proof for a parameter.
    for source in [
        "fn main(x: number) -> bool requires x == 3 { return x < (2 + 3); }",
        "fn main(x: number) -> bool { if x == 3 { return x < (2 + 3); } return false; }",
        "fn test(x: number) -> bool { return x < (2 + 3); } fn main() -> bool { return test(3); }",
    ] {
        for level in ENABLED {
            let ops = instructions(&compiler(level).compile_source(source).unwrap());
            assert!(ops.contains(&"F64Lt".into()), "{source}");
        }
    }
}

#[test]
fn unsafe_negation_keeps_float_and_does_not_hide_supported_children() {
    for (expr, float_negations, integer_subtractions) in [
        ("-0", 1, 0),
        ("-(-0)", 2, 0),
        ("-(1 - 1)", 1, 1),
        ("-(0 - 2147483647 - 1)", 1, 2),
        ("-2147483648", 1, 0),
        ("-2147483649", 1, 0),
        ("-1.5", 1, 0),
        ("-(0 / 0)", 1, 0),
        ("-(1 / 0)", 1, 0),
        ("-x", 1, 0),
        ("-((2 + 3) * 4)", 1, 0),
    ] {
        let source = format!("fn main(x: number) -> number {{ return {expr}; }}");
        for level in ENABLED {
            let ops = instructions(&compiler(level).compile_source(&source).unwrap());
            assert_eq!(
                ops.iter().filter(|op| *op == "F64Neg").count(),
                float_negations,
                "{expr}"
            );
            assert_eq!(
                ops.iter().filter(|op| *op == "I32Sub").count(),
                integer_subtractions,
                "{expr}"
            );
            if expr.contains("2 + 3") {
                assert!(ops.contains(&"I32Add".into()));
                assert!(ops.contains(&"F64ConvertI32S".into()));
            }
        }
    }
}

#[test]
fn nonnumeric_comparisons_and_unaffected_unknown_programs_preserve_baseline_bytes() {
    for source in [
        "fn main() -> bool { return true == false; }",
        "fn main() -> bool { return true != false; }",
        "fn main() -> bool { return !true; }",
        "fn main() -> bool { return \"a\" == \"b\"; }",
        "fn main() -> bool { return \"a\" != \"b\"; }",
        "fn main() -> bool { return [true] == [false]; }",
        "fn main() -> bool { return [\"a\"] != [\"b\"]; }",
        "fn main() -> bool { return [1.5] == [2.5]; }",
        "fn main(x: number, y: number) -> bool { return x <= y; }",
        "fn main(x: number) -> number { return -x; }",
    ] {
        let program = Compiler::new().analyze(source).unwrap();
        let baseline = codegen::compile(&program).unwrap();
        assert_eq!(Compiler::new().compile_source(source).unwrap(), baseline);
        for level in ENABLED {
            let bytes = compiler(level).compile_source(source).unwrap();
            instructions(&bytes);
            assert_eq!(bytes, baseline, "{source}");
        }
    }
}

#[test]
fn comparison_and_negation_candidates_preserve_forged_and_skipped_errors() {
    use was_compiler::{
        Type,
        typed_ast::{ExprResult, TypedExprKind, TypedStatementKind},
    };
    for source in [
        "fn main() -> bool { return -(2 + 3) < (4 + 5); }",
        "fn main() -> bool { return false && (-(2 + 3) < (4 + 5)); }",
        "fn main() -> bool { return true; return -(2 + 3) < (4 + 5); }",
        "fn main() -> number { return -(2 + 3); }",
    ] {
        let original = Compiler::new().analyze(source).unwrap();
        for corruption in 0..4 {
            let mut program = original.clone();
            let TypedStatementKind::Return(Some(expr)) =
                &mut program.functions[0].body.last_mut().unwrap().kind
            else {
                panic!()
            };
            // Descend through skipped logical RHS to the actual candidate.
            let expr = if let TypedExprKind::Binary {
                op: was_compiler::ast::BinaryOp::And,
                right,
                ..
            } = &mut expr.kind
            {
                right.as_mut()
            } else {
                expr
            };
            match corruption {
                0 => expr.ty = ExprResult::Never,
                1 => expr.ty = Type::String.into(),
                2 | 3 => {
                    let operand = match &mut expr.kind {
                        TypedExprKind::Binary { left, .. } => left.as_mut(),
                        TypedExprKind::Unary { expr, .. } => expr.as_mut(),
                        _ => panic!(),
                    };
                    if corruption == 2 {
                        operand.ty = Type::Bool.into();
                    } else {
                        operand.kind = TypedExprKind::Number("9".repeat(400));
                    }
                }
                _ => unreachable!(),
            }
            let error = Compiler::new().compile(&program).unwrap_err();
            for level in ENABLED {
                assert_eq!(
                    compiler(level).compile(&program).unwrap_err(),
                    error,
                    "{source}: corruption {corruption}"
                );
            }
        }
    }
}

#[test]
#[ignore = "requires Node.js 24+ on PATH"]
fn comparisons_and_negation_execute_against_independent_edges_effects_and_traps() {
    let mut cases = Vec::new();
    // Columns are <, <=, >, >=, ==, !=. Expectations are worked outcomes.
    for (left, right, expected) in [
        ("0 - 2147483647 - 1", "2147483647", [1, 1, 0, 0, 0, 1]),
        ("2147483647", "0 - 2147483647 - 1", [0, 0, 1, 1, 0, 1]),
        (
            "0 - 2147483647 - 1",
            "0 - 2147483647 - 1",
            [0, 1, 0, 1, 1, 0],
        ),
        ("2147483647", "2147483647", [0, 1, 0, 1, 1, 0]),
        ("-0", "0", [0, 1, 0, 1, 1, 0]),
        ("0", "-0", [0, 1, 0, 1, 1, 0]),
        ("0", "0", [0, 1, 0, 1, 1, 0]),
        ("0 / 0", "0", [0, 0, 0, 0, 0, 1]),
        ("0", "0 / 0", [0, 0, 0, 0, 0, 1]),
        ("1 / 0", "2147483647", [0, 0, 1, 1, 0, 1]),
        ("-1 / 0", "0 - 2147483647 - 1", [1, 1, 0, 0, 0, 1]),
        ("1 / 0", "1 / 0", [0, 1, 0, 1, 1, 0]),
        ("1.5", "2", [1, 1, 0, 0, 0, 1]),
        ("2147483648", "2147483647", [0, 0, 1, 1, 0, 1]),
        ("-2147483649", "0 - 2147483647 - 1", [1, 1, 0, 0, 0, 1]),
        ("-(2 + 3)", "-(4 + 2)", [0, 0, 1, 1, 0, 1]),
        ("(2 + 3) * 2", "10", [0, 1, 0, 1, 1, 0]),
    ] {
        for (operator, expected) in ["<", "<=", ">", ">=", "==", "!="].into_iter().zip(expected) {
            cases.push((
                format!("({left}) {operator} ({right})"),
                "bool",
                expected.to_string(),
            ));
        }
    }
    for (expr, expected) in [
        ("-(2 + 3)", "-5"),
        ("-(-2147483647)", "2147483647"),
        ("-(0 - 2147483647 - 1)", "2147483648"),
        ("-2147483648", "-2147483648"),
        ("-2147483649", "-2147483649"),
        ("-0", "-0"),
        ("-(-0)", "0"),
        ("-(1 - 1)", "-0"),
        ("-(0 / 0)", "NaN"),
        ("-(1 / 0)", "-Infinity"),
        ("-(-1 / 0)", "Infinity"),
        ("-1.5", "-1.5"),
        ("-(1 - 2) + 3", "4"),
        ("-0.0000000000000000000000000000000000000001", "-1e-40"),
    ] {
        cases.push((expr.into(), "number", expected.into()));
    }
    let mut records = Vec::new();
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        for (expr, ty, expected) in &cases {
            let source = format!("fn main() -> {ty} {{ return {expr}; }}");
            let bytes = compiler(level).compile_source(&source).unwrap();
            instructions(&bytes);
            records.push(format!("[{bytes:?}, {expected:?}]"));
        }
    }
    let input = format!("[{}]", records.join(","));
    let script = r#"
        const assert = require('node:assert/strict');
        const cases = JSON.parse(require('node:fs').readFileSync(0, 'utf8'));
        for (const [bytes, expected] of cases) {
            const e = new WebAssembly.Instance(new WebAssembly.Module(new Uint8Array(bytes))).exports;
            assert.ok(Object.is(e.main(), Number(expected)), `expected ${expected}`);
        }
    "#;
    let output = test_process::output(
        std::process::Command::new("node").args(["-e", script]),
        Some(input.as_bytes()),
        std::time::Duration::from_secs(30),
    );
    assert!(output.status.success(), "{output:?}");

    let source = r#"
        fn effects() -> bool { return mark(-(2 + 3)) < mark(-4); }
        fn negative_zero() -> number { return -mark(0); }
        fn mixed(x: number) -> bool { return -(2 + 3) < x; }
        fn unknown_negate(x: number) -> number { return -x; }
        fn trap_left() -> bool { let a = [1]; return a[mark(2)] < mark(-(2 + 3)); }
        fn trap_right() -> bool { let a = [1]; return mark(-(2 + 3)) < a[mark(2)]; }
        fn negate_trap() -> number { let a = [1]; return -a[mark(2)]; }
        fn skipped() -> bool { return false && (mark(-(2 + 3)) < mark(4)); }
    "#;
    let mut modules = Vec::new();
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        let mut compiler = compiler(level);
        compiler
            .register_host_function(
                "mark",
                "env",
                "mark",
                vec![was_compiler::Type::Number],
                was_compiler::Type::Number,
            )
            .unwrap();
        let bytes = compiler.compile_source(source).unwrap();
        instructions(&bytes);
        modules.push(format!("{bytes:?}"));
    }
    let input = format!("[{}]", modules.join(","));
    let script = r#"
        const assert = require('node:assert/strict');
        const modules = JSON.parse(require('node:fs').readFileSync(0, 'utf8'));
        for (const bytes of modules) {
            const events = [];
            const e = new WebAssembly.Instance(new WebAssembly.Module(new Uint8Array(bytes)), {env: {mark(x) { events.push(x); return x; }}}).exports;
            assert.equal(e.effects(), 1); assert.deepEqual(events.splice(0), [-5, -4]);
            assert.ok(Object.is(e.negative_zero(), -0)); assert.deepEqual(events.splice(0), [0]);
            for (const [x, expected] of [[NaN, 0], [Infinity, 1], [-Infinity, 0], [-0, 1], [0, 1], [-5, 0], [-4.5, 1], [-6, 0]]) {
                assert.equal(e.mixed(x), expected);
                assert.ok(Object.is(e.unknown_negate(x), -x));
            }
            assert.throws(() => e.trap_left(), WebAssembly.RuntimeError); assert.deepEqual(events.splice(0), [2]);
            assert.throws(() => e.trap_right(), WebAssembly.RuntimeError); assert.deepEqual(events.splice(0), [-5, 2]);
            assert.throws(() => e.negate_trap(), WebAssembly.RuntimeError); assert.deepEqual(events.splice(0), [2]);
            assert.equal(e.skipped(), 0); assert.deepEqual(events, []);
        }
    "#;
    let output = test_process::output(
        std::process::Command::new("node").args(["-e", script]),
        Some(input.as_bytes()),
        std::time::Duration::from_secs(30),
    );
    assert!(output.status.success(), "{output:?}");
}

#[test]
#[ignore = "requires Node.js 24+ on PATH"]
fn bounded_numeric_edges_execute_against_independent_expected_results() {
    // Expected JS literals are worked outcomes, never the production algorithm.
    let cases = [
        ("2147483647 + 0", "2147483647"),
        ("0 - 2147483647 - 1", "-2147483648"),
        ("(2147483647 + 1) - 1", "2147483647"),
        ("((0 - 2147483647) - 1) - 1", "-2147483649"),
        ("(0 - 2147483647 - 1) - (0 - 1)", "-2147483647"),
        ("1.00000000000000001 + 2.000", "3"),
        ("2147483647.00000001 + 0", "2147483647"),
        ("2147483647.99999999 - 1", "2147483647"),
        ("0 - 0", "0"),
        ("-0 + 0", "0"),
        ("0 - -0", "0"),
        ("-0 - 0", "-0"),
        ("-0 + -0", "-0"),
        ("0 / (0 - 0)", "NaN"),
        ("1 / (0 - 0)", "Infinity"),
        ("1 / (-0 - 0)", "-Infinity"),
        ("(2 + 3) * (4 - 1)", "15"),
        ("(7 + 2) / (4 - 1)", "3"),
        ("(7 + 2) % (4 - 1)", "0"),
        ("-(2 + 3)", "-5"),
    ];
    let mut records = Vec::new();
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        for (expr, expected) in cases {
            let source = format!("fn main() -> number {{ return {expr}; }}");
            let bytes = compiler(level).compile_source(&source).unwrap();
            Validator::new().validate_all(&bytes).unwrap();
            records.push(format!("[{bytes:?}, {expected:?}]"));
        }
    }
    let input = format!("[{}]", records.join(","));
    let script = r#"
        const assert = require('node:assert/strict');
        const fs = require('node:fs');
        const cases = JSON.parse(fs.readFileSync(0, 'utf8'));
        for (const [bytes, literal] of cases) {
            const expected = Number(literal);
            const instance = new WebAssembly.Instance(new WebAssembly.Module(new Uint8Array(bytes)));
            const actual = instance.exports.main();
            assert.ok(Object.is(actual, expected), `${actual} !== ${expected}`);
        }
    "#;
    let output = test_process::output(
        std::process::Command::new("node").args(["-e", script]),
        Some(input.as_bytes()),
        std::time::Duration::from_secs(30),
    );
    assert!(output.status.success(), "{output:?}");
}
