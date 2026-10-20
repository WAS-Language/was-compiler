use std::{process::Command, time::Duration};

use was_compiler::{
    CodegenError, Compiler, CompilerOptions, OptimizationLevel, Type, codegen,
    typed_ast::{
        ExprResult, TypedExpr, TypedExprKind, TypedProgram, TypedStatementKind, VariableId,
    },
};
use wasmparser::{Parser, Payload, ValType, Validator};

#[path = "../../../tests/support/process.rs"]
mod test_process;

const ENABLED: [OptimizationLevel; 3] = [
    OptimizationLevel::Basic,
    OptimizationLevel::Standard,
    OptimizationLevel::Aggressive,
];
const LEVELS: [OptimizationLevel; 4] = [
    OptimizationLevel::None,
    OptimizationLevel::Basic,
    OptimizationLevel::Standard,
    OptimizationLevel::Aggressive,
];

fn compiler(level: OptimizationLevel) -> Compiler {
    Compiler::with_options(CompilerOptions {
        optimization_level: level,
    })
}

#[derive(Debug)]
struct Body {
    locals: Vec<ValType>,
    ops: Vec<String>,
}

fn decode(bytes: &[u8]) -> Vec<Body> {
    Validator::new().validate_all(bytes).unwrap();
    Parser::new(0)
        .parse_all(bytes)
        .filter_map(|payload| match payload.unwrap() {
            Payload::CodeSectionEntry(body) => Some(Body {
                locals: body
                    .get_locals_reader()
                    .unwrap()
                    .into_iter()
                    .map(Result::unwrap)
                    .flat_map(|(count, ty)| std::iter::repeat_n(ty, count as usize))
                    .collect(),
                ops: body
                    .get_operators_reader()
                    .unwrap()
                    .into_iter()
                    .map(|op| format!("{:?}", op.unwrap()))
                    .collect(),
            }),
            _ => None,
        })
        .collect()
}

fn count(body: &Body, op: &str) -> usize {
    body.ops
        .iter()
        .filter(|actual| actual.as_str() == op)
        .count()
}

fn identifier(id: usize) -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::Identifier {
            id: VariableId(id),
            name: "forged".into(),
        },
        ty: Type::Number.into(),
    }
}

fn initializer(program: &mut TypedProgram, statement: usize) -> &mut TypedExpr {
    let TypedStatementKind::Let { value, .. } = &mut program.functions[0].body[statement].kind
    else {
        panic!("let fixture")
    };
    value
}

fn remap_expression(expr: &mut TypedExpr) {
    match &mut expr.kind {
        TypedExprKind::Identifier { id, .. } => {
            *id = VariableId([9001, 17, 60000][id.0]);
        }
        TypedExprKind::Binary { left, right, .. } => {
            remap_expression(left);
            remap_expression(right);
        }
        TypedExprKind::Number(_) => {}
        _ => panic!("unexpected sparse fixture expression"),
    }
}

fn sparse_chain() -> TypedProgram {
    let mut program = Compiler::new()
        .analyze("fn main() -> number { let a = 20; let b = a + 3; let c = b - a; return c + b; }")
        .unwrap();
    for statement in &mut program.functions[0].body {
        match &mut statement.kind {
            TypedStatementKind::Let { id, value, .. } => {
                *id = VariableId([9001, 17, 60000][id.0]);
                remap_expression(value);
            }
            TypedStatementKind::Return(Some(value)) => remap_expression(value),
            _ => panic!("unexpected sparse fixture statement"),
        }
    }
    program
}

fn error_parity(program: &TypedProgram) -> CodegenError {
    let snapshot = program.clone();
    let expected = compiler(OptimizationLevel::None)
        .compile(program)
        .unwrap_err();
    for level in ENABLED {
        let actual = compiler(level).compile(program).unwrap_err();
        assert_eq!(actual, expected, "{level:?}");
        assert_eq!(actual.to_string(), expected.to_string(), "{level:?}");
        assert_eq!(actual.span(), expected.span(), "{level:?}");
    }
    assert_eq!(*program, snapshot);
    expected
}

#[test]
fn sparse_ids_preserve_declaration_slots_proofs_abi_and_borrowed_input() {
    let source = "fn main() -> number { let a = 20; let b = a + 3; let c = b - a; return c + b; }";
    let program = sparse_chain();
    let snapshot = program.clone();
    let baseline = Compiler::new().compile(&program).unwrap();
    assert_eq!(baseline, codegen::compile(&program).unwrap());
    assert_eq!(baseline, Compiler::new().compile_source(source).unwrap());
    assert_eq!(decode(&baseline)[0].locals, [ValType::F64; 3]);
    let expected = compiler(OptimizationLevel::Basic)
        .compile(&program)
        .unwrap();
    assert_ne!(expected, baseline);
    for level in ENABLED {
        let selected = compiler(level);
        let bytes = selected.compile(&program).unwrap();
        assert_eq!(bytes, expected);
        assert_eq!(bytes, selected.compile_source(source).unwrap());
        assert_eq!(bytes, selected.compile(&program).unwrap());
        let bodies = decode(&bytes);
        assert_eq!(bodies.len(), 1);
        assert_eq!(bodies[0].locals, [ValType::I32; 3]);
        assert_eq!(
            bodies[0].ops,
            [
                "I32Const { value: 20 }",
                "LocalSet { local_index: 0 }",
                "LocalGet { local_index: 0 }",
                "I32Const { value: 3 }",
                "I32Add",
                "LocalSet { local_index: 1 }",
                "LocalGet { local_index: 1 }",
                "LocalGet { local_index: 0 }",
                "I32Sub",
                "LocalSet { local_index: 2 }",
                "LocalGet { local_index: 2 }",
                "LocalGet { local_index: 1 }",
                "I32Add",
                "F64ConvertI32S",
                "Return",
                "End",
            ]
        );
        for payload in Parser::new(0).parse_all(&bytes) {
            if let Payload::TypeSection(types) = payload.unwrap() {
                let types: Vec<_> = types
                    .into_iter_err_on_gc_types()
                    .map(Result::unwrap)
                    .collect();
                assert_eq!(types.len(), 1);
                assert!(types[0].params().is_empty());
                assert_eq!(types[0].results(), [ValType::F64]);
            }
        }
    }
    assert_eq!(program, snapshot);
}

#[test]
fn local_bounds_require_every_intermediate_and_exclude_unsupported_initializers() {
    for (declarations, locals, required) in [
        (
            "let a = 2147483647; let b = a + 0; let c = b - a;",
            vec![ValType::I32; 3],
            "I32Sub",
        ),
        (
            "let a = 0 - 2147483647 - 1; let b = a - 1; let c = b + 1;",
            vec![ValType::I32, ValType::F64, ValType::F64],
            "F64Sub",
        ),
        (
            "let a = 2147483647; let b = (a + 1) - 1; let c = b - 1;",
            vec![ValType::I32, ValType::F64, ValType::F64],
            "F64Add",
        ),
        (
            "let a = 1.5; let b = a + 1; let c = b - 1;",
            vec![ValType::F64; 3],
            "F64Add",
        ),
        (
            "let a = -0; let b = a - 0; let c = b + -0;",
            vec![ValType::F64; 3],
            "F64Neg",
        ),
        (
            "let a = 7; let b = a * 2; let c = b + 1;",
            vec![ValType::I32, ValType::F64, ValType::F64],
            "F64Mul",
        ),
        (
            "let a = 7; let b = -a; let c = b + 1;",
            vec![ValType::I32; 3],
            "I32Sub",
        ),
    ] {
        let source = format!("fn main() -> number {{ {declarations} return c; }}");
        let expected = compiler(OptimizationLevel::Basic)
            .compile_source(&source)
            .unwrap();
        for level in ENABLED {
            let bytes = compiler(level).compile_source(&source).unwrap();
            assert_eq!(bytes, expected, "{source}");
            let bodies = decode(&bytes);
            assert_eq!(bodies[0].locals, locals, "{level:?}: {source}");
            assert!(count(&bodies[0], required) > 0, "{source}: {required}");
        }
    }
}

#[test]
fn all_owner_local_writes_block_storage_even_when_skipped_or_unreachable() {
    for write in [
        "a = 1.5;",
        "if false { a = 1.5; }",
        "while false { a = 1.5; }",
        "false && { a = 1.5; true };",
        "let unused = { a = 1.5; 0 };",
        "return a; a = 1.5;",
        "if false { a += 1; }",
        "if false { a -= 1; }",
        "if false { a *= 1; }",
        "if false { a /= 1; }",
        "if false { a %= 1; }",
        "return a; a += 1;",
    ] {
        let source =
            format!("fn main() -> number {{ let a = 10; let b = a + 1; {write} return b + a; }}");
        for level in ENABLED {
            let bytes = compiler(level).compile_source(&source).unwrap();
            let bodies = decode(&bytes);
            assert_eq!(
                &bodies[0].locals[..2],
                [ValType::F64; 2],
                "{level:?}: {write}"
            );
            assert!(count(&bodies[0], "F64Add") > 0, "{write}");
        }
    }
}

#[test]
fn parameters_loop_carried_state_and_iteration_bindings_are_not_ordinary_locals() {
    let source = "fn main(p: number) -> number {
        let total = 0;
        for item in [1, 2] { let a = 10; let b = a + 2; total += b + item + p; }
        return total;
    }";
    for level in ENABLED {
        let bytes = compiler(level).compile_source(source).unwrap();
        let bodies = decode(&bytes);
        // Parameter is not in the local-declaration vector. Array iteration and
        // compound lowering can append scratch, after these source-order slots.
        assert_eq!(
            &bodies[0].locals[..4],
            [ValType::F64, ValType::F64, ValType::I32, ValType::I32]
        );
        assert!(bodies[0].ops.windows(4).any(|ops| ops
            == [
                "LocalGet { local_index: 3 }",
                "I32Const { value: 2 }",
                "I32Add",
                "LocalSet { local_index: 4 }",
            ]));
        assert!(count(&bodies[0], "F64Add") >= 3);
    }
    for source in [
        "fn main(p: number) -> number { let a = p; return a + 1; }",
        "fn main() -> number { let a = get(); return a + 1; } fn get() -> number { return 2; }",
    ] {
        for level in ENABLED {
            let bytes = compiler(level).compile_source(source).unwrap();
            assert_eq!(decode(&bytes)[0].locals, [ValType::F64]);
        }
    }
}

#[test]
fn comparisons_and_negation_compose_with_proven_local_reads() {
    for (source, required) in [
        (
            "fn main() -> bool { let a = 3; let b = a + 2; return a < b; }",
            "I32LtS",
        ),
        ("fn main() -> number { let a = 3; return -a; }", "I32Sub"),
    ] {
        for level in ENABLED {
            let bytes = compiler(level).compile_source(source).unwrap();
            let bodies = decode(&bytes);
            assert!(bodies[0].locals.iter().all(|ty| *ty == ValType::I32));
            assert_eq!(count(&bodies[0], required), 1);
            assert_eq!(
                count(&bodies[0], "F64ConvertI32S"),
                if required == "I32LtS" { 0 } else { 1 }
            );
        }
    }
}

fn scoped_comparison_source(operator: &str) -> String {
    format!(
        "fn main() -> bool {{
            let a = 7;
            let b = -a + 2;
            return {{
                let a = b - 1;
                match true {{
                    true => {{ let c = -a; c {operator} 6 }},
                    false => false,
                }}
            }};
        }}"
    )
}

#[test]
fn scoped_local_negation_chains_feed_all_comparisons_inside_match_arms() {
    for (operator, integer, floating) in [
        ("<", "I32LtS", "F64Lt"),
        ("<=", "I32LeS", "F64Le"),
        (">", "I32GtS", "F64Gt"),
        (">=", "I32GeS", "F64Ge"),
        ("==", "I32Eq", "F64Eq"),
        ("!=", "I32Ne", "F64Ne"),
    ] {
        let source = scoped_comparison_source(operator);
        let program = Compiler::new().analyze(&source).unwrap();
        let snapshot = program.clone();
        let enabled = compiler(OptimizationLevel::Basic)
            .compile(&program)
            .unwrap();
        for level in LEVELS {
            let selected = compiler(level);
            let bytes = selected.compile(&program).unwrap();
            assert_eq!(bytes, selected.compile_source(&source).unwrap());
            let bodies = decode(&bytes);
            let body = &bodies[0];
            let specialized = level != OptimizationLevel::None;
            assert_eq!(
                &body.locals[..4],
                [if specialized {
                    ValType::I32
                } else {
                    ValType::F64
                }; 4],
                "{level:?}: {operator}"
            );
            if specialized {
                // Match dispatch also uses i32.eq; identify the numeric consumer
                // by its local operand and literal rather than a global count.
                assert!(
                    body.ops.windows(3).any(|ops| ops
                        == [
                            "LocalGet { local_index: 3 }",
                            "I32Const { value: 6 }",
                            integer,
                        ]),
                    "{operator}: {body:?}"
                );
                assert_eq!(count(body, floating), 0);
            } else {
                assert_eq!(count(body, floating), 1);
            }
            assert_eq!(count(body, "F64ConvertI32S"), 0);
            assert_eq!(count(body, "I32Add"), usize::from(specialized));
            assert_eq!(count(body, "I32Sub"), if specialized { 3 } else { 0 });
            assert_eq!(count(body, "F64Neg"), if specialized { 0 } else { 2 });
            if specialized {
                assert_eq!(bytes, enabled);
            }
        }
        assert_eq!(program, snapshot);
    }
}

const CONTRACT_PIPELINE: &str = r#"
    fn main() -> number {
        let a = 7;
        let b = -a + 2;
        return b |> target();
    }
    fn target(x: number, y: number = -x + 1) -> number
        requires x < 0 requires y > 0 {
        let a = 3;
        let b = -a + 1;
        return x + y + b;
    }
"#;

#[test]
fn local_pipeline_arguments_widen_without_refining_defaults_contracts_or_parameters() {
    let program = Compiler::new().analyze(CONTRACT_PIPELINE).unwrap();
    let enabled = compiler(OptimizationLevel::Basic)
        .compile(&program)
        .unwrap();
    for level in LEVELS {
        let selected = compiler(level);
        let bytes = selected.compile(&program).unwrap();
        assert_eq!(bytes, selected.compile_source(CONTRACT_PIPELINE).unwrap());
        let bodies = decode(&bytes);
        assert_eq!(bodies.len(), 2);
        let specialized = level != OptimizationLevel::None;
        let ordinary = if specialized {
            ValType::I32
        } else {
            ValType::F64
        };
        assert_eq!(
            bodies[0].locals,
            [ordinary, ordinary, ValType::F64, ValType::F64]
        );
        assert_eq!(bodies[1].locals, [ordinary; 2]);
        // The proven input and independent default literal widen in the caller.
        // Reading the synthetic binding still keeps the default operations f64.
        assert_eq!(
            count(&bodies[0], "F64ConvertI32S"),
            if specialized { 2 } else { 0 }
        );
        assert_eq!(count(&bodies[0], "F64Neg"), if specialized { 1 } else { 2 });
        assert_eq!(count(&bodies[0], "F64Add"), if specialized { 1 } else { 2 });
        assert_eq!(count(&bodies[1], "F64Lt"), 1);
        assert_eq!(count(&bodies[1], "F64Gt"), 1);
        assert_eq!(count(&bodies[1], "F64Add"), if specialized { 2 } else { 3 });
        assert_eq!(
            count(&bodies[1], "F64ConvertI32S"),
            if specialized { 3 } else { 0 }
        );
        let mut signatures = Vec::new();
        for payload in Parser::new(0).parse_all(&bytes) {
            if let Payload::TypeSection(types) = payload.unwrap() {
                signatures.extend(types.into_iter_err_on_gc_types().map(Result::unwrap));
            }
        }
        assert_eq!(signatures.len(), 2);
        assert_eq!(signatures[0].params(), []);
        assert_eq!(signatures[0].results(), [ValType::F64]);
        assert_eq!(signatures[1].params(), [ValType::F64; 2]);
        assert_eq!(signatures[1].results(), [ValType::F64]);
        if specialized {
            assert_eq!(bytes, enabled);
        }
    }
}

#[test]
fn helpers_have_separate_write_sets_and_captured_cells_do_not_specialize() {
    let independent = "fn main() -> number {
        let a = 10;
        let helper = () => { let a = 1; a += 0.5; a };
        return a + helper();
    }";
    let captured = "fn main() -> number {
        let a = 10;
        let helper = () => { let b = 2; let c = b + 3; a + c };
        return helper();
    }";
    for level in ENABLED {
        let bytes = compiler(level).compile_source(independent).unwrap();
        let bodies = decode(&bytes);
        assert_eq!(bodies.len(), 2);
        assert_eq!(bodies[0].locals, [ValType::I32]);
        assert_eq!(bodies[1].locals, [ValType::F64]);
        assert_eq!(count(&bodies[0], "F64Add"), 1);
        assert_eq!(count(&bodies[1], "F64Add"), 1);

        let bytes = compiler(level).compile_source(captured).unwrap();
        let bodies = decode(&bytes);
        assert_eq!(bodies[0].locals[0], ValType::F64);
        let helper = bodies.last().unwrap();
        assert_eq!(&helper.locals[..2], [ValType::I32, ValType::I32]);
        assert_eq!(count(helper, "I32Add"), 1);
        assert_eq!(count(helper, "F64Add"), 1);
        assert_eq!(
            helper
                .ops
                .iter()
                .filter(|op| op.starts_with("F64Load {"))
                .count(),
            1
        );
    }
}

#[test]
fn unsafe_local_negation_and_value_joins_keep_f64_fallback_boundaries() {
    for (source, locals, conversions, floating_adds) in [
        (
            "fn main() -> number { let a = 1; let b = a - a; let c = -b; return c; }",
            vec![ValType::I32, ValType::I32, ValType::F64],
            1,
            0,
        ),
        (
            "fn main() -> number { let a = 0 - 2147483647 - 1; let b = -a; return b; }",
            vec![ValType::I32, ValType::F64],
            1,
            0,
        ),
        (
            "fn main() -> number { let a = 2147483647; let b = a + 1; let c = -b; return c; }",
            vec![ValType::I32, ValType::F64, ValType::F64],
            2,
            1,
        ),
        (
            "fn main() -> number { let a = 7; let b = -a + 2;
                let c = match true { true => b, false => -b }; return -c; }",
            vec![ValType::I32, ValType::I32, ValType::F64],
            2,
            0,
        ),
    ] {
        let enabled = compiler(OptimizationLevel::Basic)
            .compile_source(source)
            .unwrap();
        for level in ENABLED {
            let bytes = compiler(level).compile_source(source).unwrap();
            assert_eq!(bytes, enabled);
            let bodies = decode(&bytes);
            let body = &bodies[0];
            assert_eq!(&body.locals[..locals.len()], locals, "{source}");
            assert_eq!(count(body, "F64Neg"), 1, "{source}");
            assert_eq!(count(body, "F64ConvertI32S"), conversions, "{source}");
            assert_eq!(count(body, "F64Add"), floating_adds, "{source}");
        }
    }
}

const CAPTURE_COMPOSITION: &str = r#"
    fn captures() -> number {
        let captured = 7;
        let helper = () => {
            let a = 3;
            let b = -a + 1;
            if b > -captured { captured += 0.5; b + captured } else { 0 }
        };
        let first = helper();
        captured += 1;
        return first + helper();
    }
"#;

#[test]
fn helper_integer_chains_do_not_change_captured_cell_storage_or_comparison_proofs() {
    for level in LEVELS {
        let bytes = compiler(level).compile_source(CAPTURE_COMPOSITION).unwrap();
        let bodies = decode(&bytes);
        let creator = &bodies[0];
        let helper = bodies.last().unwrap();
        let specialized = level != OptimizationLevel::None;
        assert_eq!(creator.locals[0], ValType::F64);
        assert_eq!(
            helper.locals[..2],
            [if specialized {
                ValType::I32
            } else {
                ValType::F64
            }; 2]
        );
        assert_eq!(count(helper, "I32Add"), usize::from(specialized));
        assert_eq!(count(helper, "I32Sub"), usize::from(specialized));
        assert_eq!(count(helper, "F64Gt"), 1);
        assert_eq!(count(helper, "I32GtS"), 0);
        assert_eq!(count(helper, "F64Neg"), if specialized { 1 } else { 2 });
        assert!(helper.ops.iter().any(|op| op.starts_with("F64Load {")));
        assert!(helper.ops.iter().any(|op| op.starts_with("F64Store {")));
        assert!(creator.ops.iter().any(|op| op.starts_with("F64Store {")));
    }
}

#[test]
fn normalized_explicit_default_and_pipeline_bindings_keep_f64_storage() {
    for call in ["target(b: 3, a: 2)", "target(a: 2)", "2 |> target(b: 3)"] {
        let source = format!(
            "fn main() -> number {{ return {call}; }}
            fn target(a: number, b: number = a + 1) -> number {{ return a + b; }}"
        );
        let program = Compiler::new().analyze(&source).unwrap();
        let TypedStatementKind::Return(Some(expr)) = &program.functions[0].body[0].kind else {
            panic!("return")
        };
        let TypedExprKind::NormalizedCall(sequence) = &expr.kind else {
            panic!("normalized call: {call}")
        };
        assert_eq!(sequence.bindings.len(), 2);
        for level in ENABLED {
            let bytes = compiler(level).compile(&program).unwrap();
            assert_eq!(bytes, compiler(level).compile_source(&source).unwrap());
            let bodies = decode(&bytes);
            assert_eq!(bodies[0].locals, [ValType::F64; 2], "{call}");
            assert_eq!(count(&bodies[1], "F64Add"), 1);
        }
    }
}

#[test]
fn forged_sparse_reads_still_require_initialization_and_scope() {
    let mut before = sparse_chain();
    *initializer(&mut before, 0) = identifier(17);
    assert!(matches!(
        error_parity(&before),
        CodegenError::UninitializedVariable {
            id: VariableId(17),
            ..
        }
    ));

    let mut unknown = sparse_chain();
    *initializer(&mut unknown, 1) = identifier(999999);
    assert!(matches!(
        error_parity(&unknown),
        CodegenError::UnknownVariable {
            id: VariableId(999999),
            ..
        }
    ));

    let mut scoped = Compiler::new()
        .analyze("fn main() -> number { if true { let a = 10; } return 0; }")
        .unwrap();
    let TypedStatementKind::If { then, .. } = &mut scoped.functions[0].body[0].kind else {
        panic!("if")
    };
    let TypedStatementKind::Let { id, .. } = &mut then[0].kind else {
        panic!("let")
    };
    *id = VariableId(60000);
    let TypedStatementKind::Return(Some(expr)) = &mut scoped.functions[0].body[1].kind else {
        panic!("return")
    };
    *expr = identifier(60000);
    assert!(matches!(
        error_parity(&scoped),
        CodegenError::UninitializedVariable {
            id: VariableId(60000),
            ..
        }
    ));
}

#[test]
fn forged_duplicate_types_annotations_and_first_error_order_match_none() {
    let mut duplicate = sparse_chain();
    let TypedStatementKind::Let { id, .. } = &mut duplicate.functions[0].body[2].kind else {
        panic!("let")
    };
    *id = VariableId(9001);
    // Layout's duplicate error must precede an earlier malformed initializer.
    initializer(&mut duplicate, 0).kind = TypedExprKind::Number("invalid".into());
    assert!(matches!(
        error_parity(&duplicate),
        CodegenError::DuplicateVariableId {
            id: VariableId(9001),
            ..
        }
    ));

    for corruption in 0..6 {
        let mut program = sparse_chain();
        match corruption {
            0 => {
                let TypedStatementKind::Let { ty, .. } = &mut program.functions[0].body[0].kind
                else {
                    panic!("let")
                };
                *ty = Type::Bool;
            }
            1 => initializer(&mut program, 1).ty = ExprResult::Never,
            2 => {
                let TypedExprKind::Binary { left, .. } = &mut initializer(&mut program, 1).kind
                else {
                    panic!("binary")
                };
                left.ty = Type::Bool.into();
            }
            3 => initializer(&mut program, 0).kind = TypedExprKind::Number("1e3".into()),
            4 => initializer(&mut program, 0).kind = TypedExprKind::Number("9".repeat(400)),
            5 => {
                let TypedStatementKind::Let { ty, .. } = &mut program.functions[0].body[1].kind
                else {
                    panic!("let")
                };
                *ty = Type::Void;
            }
            _ => unreachable!(),
        }
        let expected_span = if corruption == 0 || corruption == 3 || corruption == 4 {
            program.functions[0].body[0].span
        } else {
            program.functions[0].body[1].span
        };
        // A later unknown read must not supersede the first corruption.
        *initializer(&mut program, 2) = identifier(999999);
        let error = error_parity(&program);
        assert_eq!(
            error.span(),
            Some(expected_span),
            "corruption {corruption}: {error:?}"
        );
    }
}

#[test]
fn skipped_and_unreachable_local_initializers_retain_checked_error_order() {
    for source in [
        "fn main() -> number { let a = 10; return a; let b = a + 2; }",
        "fn main() -> number { let a = 10; false && { let b = a + 2; b == 12 }; return a; }",
        "fn main() -> number { let a = 10; if false { let b = a + 2; } return a; }",
    ] {
        let mut program = Compiler::new().analyze(source).unwrap();
        let statement = &mut program.functions[0].body[1];
        let bad = match &mut statement.kind {
            TypedStatementKind::Return(_) => &mut program.functions[0].body[2],
            TypedStatementKind::Expression(expr) => {
                let TypedExprKind::Binary { right, .. } = &mut expr.kind else {
                    panic!("logical")
                };
                let TypedExprKind::Block(block) = &mut right.kind else {
                    panic!("block")
                };
                &mut block.statements[0]
            }
            TypedStatementKind::If { then, .. } => &mut then[0],
            _ => panic!("skipped fixture"),
        };
        let span = bad.span;
        let TypedStatementKind::Let { value, .. } = &mut bad.kind else {
            panic!("let")
        };
        let TypedExprKind::Binary { left, right, .. } = &mut value.kind else {
            panic!("add")
        };
        **left = identifier(999999);
        right.kind = TypedExprKind::Number("invalid".into());
        let error = error_parity(&program);
        assert!(matches!(
            error,
            CodegenError::UnknownVariable {
                id: VariableId(999999),
                ..
            }
        ));
        assert_eq!(error.span(), Some(span));
    }
}

#[test]
fn forged_negated_local_reads_in_skipped_match_arms_keep_validation_and_error_order() {
    for prefix in ["return false &&", "return true; return"] {
        for arm_index in 0..2 {
            let source = format!(
                "fn main() -> bool {{ let a = 7; let b = -a + 2;
                    {prefix} match true {{
                        true => {{ let c = -b; c < -a }},
                        false => {{ let c = -b; c >= -a }},
                    }};
                }}"
            );
            let mut program = Compiler::new().analyze(&source).unwrap();
            let TypedStatementKind::Return(Some(expr)) =
                &mut program.functions[0].body.last_mut().unwrap().kind
            else {
                panic!("return fixture")
            };
            let expr = if let TypedExprKind::Binary { right, .. } = &mut expr.kind {
                right.as_mut()
            } else {
                expr
            };
            let TypedExprKind::Match { arms, .. } = &mut expr.kind else {
                panic!("match")
            };
            let TypedExprKind::Block(block) = &mut arms[arm_index].body.kind else {
                panic!("arm block")
            };
            let span = block.statements[0].span;
            let TypedStatementKind::Let { value, .. } = &mut block.statements[0].kind else {
                panic!("let")
            };
            let TypedExprKind::Unary { expr, .. } = &mut value.kind else {
                panic!("negation")
            };
            **expr = identifier(999999);
            // A later invalid comparison operand must not mask the bad read.
            let TypedExprKind::Binary { right, .. } = &mut block.tail.as_mut().unwrap().kind else {
                panic!("comparison")
            };
            right.kind = TypedExprKind::Number("invalid".into());
            let error = error_parity(&program);
            assert!(matches!(
                error,
                CodegenError::UnknownVariable {
                    id: VariableId(999999),
                    ..
                }
            ));
            assert_eq!(error.span(), Some(span));
        }
    }
}

#[test]
fn repeated_mutation_cannot_reuse_local_proofs_across_compilations() {
    let mut program = sparse_chain();
    let selected = compiler(OptimizationLevel::Basic);
    let before = selected.compile(&program).unwrap();
    initializer(&mut program, 0).kind = TypedExprKind::Number("1.5".into());
    let after = selected.compile(&program).unwrap();
    assert_ne!(before, after);
    assert_eq!(decode(&after)[0].locals, [ValType::F64; 3]);
    initializer(&mut program, 0).kind = TypedExprKind::Number("20".into());
    assert_eq!(selected.compile(&program).unwrap(), before);
}

#[test]
#[ignore = "requires Node.js 24+ on PATH"]
fn bounded_composition_executes_scopes_loops_calls_contracts_matches_and_exclusions() {
    let mut source = format!("{CONTRACT_PIPELINE}\n{CAPTURE_COMPOSITION}");
    for (index, operator) in ["<", "<=", ">", ">=", "==", "!="].into_iter().enumerate() {
        source.push_str(
            &scoped_comparison_source(operator).replace("fn main", &format!("fn compare_{index}")),
        );
    }
    source.push_str(
        r#"
        fn loops() -> number {
            let sum = 0;
            for i in 0..4 {
                let a = 7;
                let b = -a + 2;
                if b >= -5 {
                    match i { 1 => { continue; }, _ => { sum += -b + i; } }
                }
            }
            let n = 0;
            while n < 2 {
                let a = 3;
                let b = -a + 1;
                if b == -2 { sum += -b; }
                n += 1;
            }
            let exit = loop {
                let a = 4;
                let b = -a + 1;
                if b != -a { break -b; }
            };
            return sum + exit;
        }
        fn ordered() -> number {
            let a = 7;
            let b = -a + 2;
            return target(y: mark(-b), x: mark(b));
        }
        fn contract_trap() -> number {
            let a = 7;
            let b = -a + 2;
            return target(x: mark(-b), y: mark(b)) + mark(99);
        }
        fn fallback(which: number) -> number {
            let a = 1;
            let zero = a - a;
            let minimum = 0 - 2147483647 - 1;
            let large = -minimum;
            return match which {
                0 => -zero,
                1 => large,
                2 => (2147483647 + 1) - 1,
                _ => zero / zero,
            };
        }
        fn unknown(x: number) -> bool {
            let a = 7;
            let b = -a + 2;
            return b < x;
        }
        fn array_alias() -> number {
            let a = 7;
            let b = -a + 2;
            let values = [b];
            let alias = values;
            alias[0] = -b;
            return match values[0] { 5 => -b, _ => 99 };
        }
    "#,
    );
    let mut enabled = None;
    for level in LEVELS {
        let mut selected = compiler(level);
        selected
            .register_host_function("mark", "env", "mark", vec![Type::Number], Type::Number)
            .unwrap();
        let program = selected.analyze(&source).unwrap();
        let bytes = selected.compile(&program).unwrap();
        assert_eq!(bytes, selected.compile_source(&source).unwrap());
        decode(&bytes);
        if level != OptimizationLevel::None {
            if let Some(expected) = &enabled {
                assert_eq!(&bytes, expected);
            } else {
                enabled = Some(bytes.clone());
            }
        }
        let script = r#"
            const assert = require('node:assert/strict');
            const events = [];
            const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)), {
                env: { mark(x) { events.push(x); return x; } }
            }).exports;
            for (let repeat = 0; repeat < 2; repeat++) {
                // The scoped chain produces 6 on both sides of each comparison.
                for (const [name, expected] of [
                    ['compare_0', 0], ['compare_1', 1], ['compare_2', 0],
                    ['compare_3', 1], ['compare_4', 1], ['compare_5', 0]
                ]) assert.equal(e[name](), expected, name);
                assert.equal(e.main(), -1);
                assert.equal(e.target(-5, 6), -1);
                // Raw exports do not apply source defaults: missing y becomes NaN.
                assert.throws(() => e.target(-5), WebAssembly.RuntimeError);
                assert.equal(e.loops(), 27); // 5+7+8, then 2+2, then a value break of 3.
                assert.equal(e.captures(), 12.5);
                assert.equal(e.array_alias(), 5);
                assert.equal(e.ordered(), -2);
                assert.deepEqual(events.splice(0), [5, -5]);
                assert.throws(() => e.contract_trap(), WebAssembly.RuntimeError);
                assert.deepEqual(events.splice(0), [5, -5]);
                assert.ok(Object.is(e.fallback(0), -0));
                assert.equal(e.fallback(1), 2147483648);
                assert.equal(e.fallback(2), 2147483647);
                assert.ok(Number.isNaN(e.fallback(3)));
                for (const [value, expected] of [
                    [NaN, 0], [Infinity, 1], [-Infinity, 0], [-0, 1],
                    [0, 1], [-5, 0], [-4.5, 1], [-6, 0]
                ]) assert.equal(e.unknown(value), expected);
            }
        "#;
        let output = test_process::output(
            Command::new("node").args(["-e", script]),
            Some(&bytes),
            Duration::from_secs(30),
        );
        assert!(
            output.status.success(),
            "{level:?}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "requires Node.js 24+ on PATH"]
fn bounded_runtime_checks_effects_zeros_overflow_scopes_loops_calls_and_captures() {
    let source = r#"
        fn identity(x: number) -> number { return x; }
        fn target(a: number, b: number = a + 1) -> number { return a - b; }
        fn chain() -> number { let a = 20; let b = a + 3; let c = b - a; return c + b; }
        fn effects() -> number { let a = 10; let b = a + 2; return mark(b) - mark(a); }
        fn zero() -> number { let a = 10; let b = a - a; return b; }
        fn negative_zero() -> number { let a = -0; let b = a - 0; return b; }
        fn negate_zero() -> number { let a = 0; return -a; }
        fn overflow() -> number { let a = 2147483647; let b = a + 1; return b + 1; }
        fn underflow() -> number { let a = 0 - 2147483647 - 1; let b = a - 1; return b - 1; }
        fn scopes() -> number {
            let a = 10;
            let b = { let a = 3; a + 2 };
            if false { a = 1.5; }
            return a + b;
        }
        fn loops() -> number {
            let sum = 0;
            for i in 0..4 { let a = 10; let b = a + 2; if i == 1 { continue; } sum += b + i; }
            return sum;
        }
        fn calls() -> number { let a = 10; let b = a + 2; return identity(b) + target(a: a); }
        fn captures() -> number {
            let a = 10;
            let helper = () => { let b = 2; let c = b + 3; a += 0.5; a + c };
            let first = helper();
            a += 1;
            return first + helper();
        }
        fn trap_order() -> number {
            let a = [0]; let i = 1; a[mark(i)] = mark(2); return 0;
        }
    "#;
    for level in LEVELS {
        let mut selected = compiler(level);
        selected
            .register_host_function("mark", "env", "mark", vec![Type::Number], Type::Number)
            .unwrap();
        let program = selected.analyze(source).unwrap();
        let bytes = selected.compile(&program).unwrap();
        assert_eq!(bytes, selected.compile_source(source).unwrap());
        Validator::new().validate_all(&bytes).unwrap();
        let script = r#"
            const assert = require('node:assert/strict');
            const bytes = require('node:fs').readFileSync(0);
            const events = [];
            const e = new WebAssembly.Instance(new WebAssembly.Module(bytes), {
                env: { mark(x) { events.push(x); return x; } }
            }).exports;
            for (let repeat = 0; repeat < 2; repeat++) {
                assert.equal(e.chain(), 26);
                assert.equal(e.effects(), 2);
                assert.deepEqual(events.splice(0), [12, 10]);
                assert.ok(Object.is(e.zero(), 0));
                assert.ok(Object.is(e.negative_zero(), -0));
                assert.ok(Object.is(e.negate_zero(), -0));
                assert.equal(e.overflow(), 2147483649);
                assert.equal(e.underflow(), -2147483650);
                assert.equal(e.scopes(), 15);
                assert.equal(e.loops(), 41);
                assert.equal(e.calls(), 11);
                assert.equal(e.captures(), 32.5);
                assert.throws(() => e.trap_order(), WebAssembly.RuntimeError);
                assert.deepEqual(events.splice(0), [1]);
                for (const x of [-0, NaN, Infinity, -Infinity, Number.MIN_VALUE, 3.5]) {
                    const actual = e.identity(x);
                    if (Number.isNaN(x)) assert.ok(Number.isNaN(actual));
                    else assert.ok(Object.is(actual, x));
                }
            }
        "#;
        let output = test_process::output(
            Command::new("node").args(["-e", script]),
            Some(&bytes),
            Duration::from_secs(30),
        );
        assert!(
            output.status.success(),
            "{level:?}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
