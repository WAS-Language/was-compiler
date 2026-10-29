use was_compiler::{
    ArrayElementType, CodegenError, Compiler, ExportOptions, OptimizationLevel, Type,
    ast::BinaryOp,
    codegen,
    tokens::Span,
    typed_ast::{
        BuiltinFunction, ExprResult, FunctionRef, TypedContract, TypedExpr, TypedExprKind,
        TypedStatementKind,
    },
};
use wasmparser::{Operator, Parser, Payload, Validator};

fn operators(bytes: &[u8]) -> Vec<Vec<Operator<'_>>> {
    Validator::new().validate_all(bytes).unwrap();
    Parser::new(0)
        .parse_all(bytes)
        .filter_map(|p| match p.unwrap() {
            Payload::CodeSectionEntry(body) => Some(
                body.get_operators_reader()
                    .unwrap()
                    .into_iter()
                    .map(Result::unwrap)
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}

fn calls(ops: &[Operator<'_>]) -> Vec<u32> {
    ops.iter()
        .filter_map(|op| match op {
            Operator::Call { function_index } => Some(*function_index),
            _ => None,
        })
        .collect()
}

fn result(program: &mut was_compiler::typed_ast::TypedProgram) -> &mut TypedExpr {
    let TypedStatementKind::Return(Some(value)) = &mut program.functions[0].body[0].kind else {
        panic!("return")
    };
    value
}

fn constructor(source: TypedExpr) -> TypedExpr {
    TypedExpr {
        ty: source.ty,
        kind: TypedExprKind::Call {
            func: FunctionRef::Builtin(BuiltinFunction::Resizable),
            name: "not_a_lookup".into(),
            args: vec![source],
        },
    }
}

#[test]
fn source_runs_once_and_ordinary_literals_keep_fixed_storage_in_an_m4_module() {
    let bytes = Compiler::new().compile_source("fn source() -> number[] { return [1]; } fn main() -> number[] { return resizable(source()); }").unwrap();
    let bodies = operators(&bytes);
    // Two users, seven existing helpers, six allocator operations, one constructor.
    assert_eq!(bodies.len(), 16);
    assert_eq!(calls(&bodies[1]), [0, 15]);
    assert_eq!(calls(&bodies[0]), [2]);
    assert!(bodies[0].windows(2).any(|ops| matches!(
        ops,
        [
            Operator::I32Const { value: 16 },
            Operator::Call { function_index: 2 }
        ]
    )));
    assert!(
        bodies[0]
            .iter()
            .any(|op| matches!(op, Operator::F64Store { memarg } if memarg.offset == 8))
    );
    assert!(
        !bodies[0]
            .iter()
            .any(|op| matches!(op, Operator::GlobalSet { .. }))
    );
    // The internal acquire signature is planned and encoded before user lowering;
    // it is the sole multivalue transport and is never a source ABI signature.
    let signatures: Vec<_> = Parser::new(0)
        .parse_all(&bytes)
        .filter_map(|p| match p.unwrap() {
            Payload::TypeSection(reader) => Some(
                reader
                    .into_iter_err_on_gc_types()
                    .map(Result::unwrap)
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect();
    let multiple: Vec<_> = signatures
        .iter()
        .filter(|s| s.results().len() > 1)
        .collect();
    assert_eq!(multiple.len(), 1);
    assert_eq!(multiple[0].params(), [wasmparser::ValType::I64]);
    assert_eq!(
        multiple[0].results(),
        [wasmparser::ValType::I32, wasmparser::ValType::I64]
    );
}

#[test]
fn m4_signatures_are_interned_and_each_planned_function_uses_the_right_type() {
    use wasmparser::ValType::{I32, I64};
    let bytes = Compiler::new()
        .compile_source(
            "fn numbers(a: number[]) -> number[] { return resizable(a); }
         fn flags(a: bool[]) -> bool[] { return resizable(a); }
         fn texts(a: string[]) -> string[] { return resizable(a); }",
        )
        .unwrap();
    Validator::new().validate_all(&bytes).unwrap();
    let mut signatures = Vec::new();
    let mut functions = Vec::new();
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            Payload::TypeSection(reader) => {
                signatures.extend(reader.into_iter_err_on_gc_types().map(Result::unwrap))
            }
            Payload::FunctionSection(reader) => {
                functions.extend(reader.into_iter().map(Result::unwrap))
            }
            _ => {}
        }
    }
    for (index, signature) in signatures.iter().enumerate() {
        assert!(
            !signatures[..index].contains(signature),
            "duplicate signature {signature:?}"
        );
    }
    // Three users, all sixteen ordinary memory helpers, then six M4 core helpers
    // and three statically ordered constructors. Type dedup never shifts functions.
    assert_eq!(functions.len(), 28);
    let expected = [
        (vec![I64], vec![I32]),
        (vec![I64], vec![I32, I64]),
        (vec![I32], vec![I32]),
        (vec![I32], vec![I32]),
        (vec![I32, I32], vec![]),
        (vec![I32, I64, I64], vec![]),
        (vec![I32], vec![I32]),
        (vec![I32], vec![I32]),
        (vec![I32], vec![I32]),
    ];
    for (&type_index, (params, results)) in functions[19..].iter().zip(expected) {
        let signature = &signatures[type_index as usize];
        assert_eq!(signature.params(), params);
        assert_eq!(signature.results(), results);
    }
    for slot in [21, 22, 25, 26, 27] {
        assert_eq!(functions[slot], functions[0]);
    }
}

#[test]
fn numeric_constructor_is_planned_valid_and_private() {
    let bytes = Compiler::new()
        .compile_source("fn main(a: number[]) -> number[] { return resizable(a); }")
        .unwrap();
    Validator::new().validate_all(&bytes).unwrap();
    let mut globals = 0;
    let mut exports = Vec::new();
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            Payload::GlobalSection(section) => globals = section.count(),
            Payload::ExportSection(section) => {
                exports.extend(section.into_iter().map(|e| e.unwrap().name.to_owned()));
            }
            _ => {}
        }
    }
    assert_eq!(globals, 33); // bump + identity + allocator registry + 29 heads + array registry
    assert_eq!(exports, ["main", "memory", "__alloc"]);
}

#[test]
fn forged_constructor_arity_operands_and_annotations_are_checked_even_unexported() {
    for case in 0..7 {
        let mut program = Compiler::new()
            .analyze("fn main(a: bool[]) -> bool[] { return resizable(a); }")
            .unwrap();
        let value = result(&mut program);
        match case {
            0 | 1 => {
                let TypedExprKind::Call { args, .. } = &mut value.kind else {
                    panic!("call")
                };
                if case == 0 {
                    args.clear();
                } else {
                    args.push(args[0].clone());
                }
            }
            2 => value.ty = Type::Array(ArrayElementType::String).into(),
            3 => value.ty = ExprResult::Never,
            4..=6 => {
                let TypedExprKind::Call { args, .. } = &mut value.kind else {
                    panic!("call")
                };
                args[0] = TypedExpr {
                    kind: TypedExprKind::Bool(true),
                    ty: match case {
                        4 => Type::Bool.into(),
                        5 => ExprResult::Void,
                        _ => ExprResult::Value(Type::Void),
                    },
                };
            }
            _ => unreachable!(),
        }
        for level in [
            OptimizationLevel::None,
            OptimizationLevel::Basic,
            OptimizationLevel::Standard,
            OptimizationLevel::Aggressive,
        ] {
            let mut compiler = Compiler::new();
            compiler.set_optimization_level(level);
            compiler.set_exports(ExportOptions::None);
            let error = compiler.compile(&program).unwrap_err();
            assert!(
                match case {
                    0 | 1 => matches!(
                        error,
                        CodegenError::ArgumentCountMismatch { expected: 1, .. }
                    ),
                    _ => matches!(
                        error,
                        CodegenError::TypeMismatch { .. } | CodegenError::ExprResultMismatch { .. }
                    ),
                },
                "case {case}: {error:?}"
            );
            assert!(error.span().is_some());
        }
    }
}

#[test]
fn forged_constructor_is_rejected_independently_in_skipped_contract_and_default() {
    let mut program = Compiler::new().analyze("fn target(a: bool[], b: bool[] = a) -> bool[] { return b; } fn main(a: bool[]) -> bool[] { return target(a); }").unwrap();
    let TypedStatementKind::Return(Some(value)) = &mut program.functions[1].body[0].kind else {
        panic!("return")
    };
    let TypedExprKind::NormalizedCall(sequence) = &mut value.kind else {
        panic!("normalized")
    };
    sequence.bindings[1].value = constructor(sequence.bindings[1].value.clone());
    assert!(matches!(
        codegen::compile(&program),
        Err(CodegenError::InvalidNormalizedCall { .. })
    ));

    let mut program = Compiler::new()
        .analyze("fn main(a: bool[]) -> bool { return true; }")
        .unwrap();
    let source = TypedExpr {
        ty: Type::Array(ArrayElementType::Bool).into(),
        kind: TypedExprKind::Identifier {
            id: program.functions[0].params[0].id,
            name: "a".into(),
        },
    };
    let copied = constructor(source.clone());
    let forbidden = TypedExpr {
        ty: Type::Bool.into(),
        kind: TypedExprKind::Binary {
            left: Box::new(copied),
            op: BinaryOp::Equal,
            right: Box::new(source),
        },
    };
    program.functions[0].contracts.push(TypedContract {
        span: Span::new(10, 20),
        value: TypedExpr {
            ty: Type::Bool.into(),
            kind: TypedExprKind::Binary {
                left: Box::new(TypedExpr {
                    kind: TypedExprKind::Bool(false),
                    ty: Type::Bool.into(),
                }),
                op: BinaryOp::And,
                right: Box::new(forbidden),
            },
        },
    });
    assert!(
        matches!(codegen::compile(&program), Err(CodegenError::InvalidContractExpression { span, .. }) if span == Span::new(10, 20))
    );
}

#[test]
fn never_skipped_nested_and_immediate_callable_constructors_are_planned() {
    for source in [
        "fn main() -> bool[] { return resizable([loop {}]); }",
        "fn main() { resizable(loop {}); }",
        "fn main(a: bool[]) { if false { resizable(a); } }",
        "fn main(a: bool[]) -> bool[] { let copy = () => resizable(a); return copy(); }",
        "fn main(a: string[]) -> string[] { return a |> ((x: string[]) => resizable(x)); }",
        "fn main(a: bool[]) -> bool[] { return resizable(resizable(a)); }",
        "fn main(a: bool[]) -> bool[] { return a; resizable(a); }",
    ] {
        let mut compiler = Compiler::new();
        let baseline = compiler.compile_source(source).unwrap();
        Validator::new().validate_all(&baseline).unwrap();
        let mut enabled = None;
        for level in [
            OptimizationLevel::Basic,
            OptimizationLevel::Standard,
            OptimizationLevel::Aggressive,
        ] {
            compiler.set_optimization_level(level);
            let bytes = compiler.compile_source(source).unwrap();
            Validator::new().validate_all(&bytes).unwrap();
            assert_eq!(*enabled.get_or_insert_with(|| bytes.clone()), bytes);
            assert_eq!(compiler.compile_source(source).unwrap(), bytes);
        }
    }
}

#[test]
fn every_typed_copy_checks_consumed_values_and_publishes_after_preparation() {
    for (ty, width, memory_helpers, validator, consumer, kind) in [
        ("number", 8, 7, 3, None, 0),
        ("bool", 1, 12, 9, Some(8), 1),
        ("string", 4, 11, 8, Some(2), 2),
    ] {
        let bytes = Compiler::new()
            .compile_source(&format!(
                "fn main(a: {ty}[]) -> {ty}[] {{ return resizable(a); }}"
            ))
            .unwrap();
        let bodies = operators(&bytes);
        let first = 1 + memory_helpers;
        let copy_index = first + 6;
        assert_eq!(bodies.len(), copy_index + 1);
        assert_eq!(calls(&bodies[0]), [validator, copy_index as u32]); // parameter validation, once-only constructor
        let copy = &bodies[copy_index];
        let mut expected = vec![
            validator,
            first as u32,
            first as u32 + 1,
            first as u32 + 5,
            1,
        ];
        expected.extend(consumer);
        assert_eq!(calls(copy), expected);
        assert!(copy.windows(3).any(|w| matches!(w, [Operator::I64Const { value }, Operator::LocalGet { local_index: 1 }, Operator::I64ExtendI32U] if *value == width)));
        assert!(copy.windows(4).any(|w| matches!(
            w,
            [
                Operator::I64Const { value: 1 },
                Operator::LocalGet { local_index: 6 },
                Operator::I64Shl,
                Operator::LocalTee { local_index: 6 }
            ]
        )));
        // Independent physical contract: retained reservation, record at descriptor+8,
        // metadata stores and registry publication occur only after the copy loop.
        assert!(copy.windows(2).any(|w| matches!(
            w,
            [
                Operator::I32Const { value: 48 },
                Operator::Call { function_index: 1 }
            ]
        )));
        assert!(copy.windows(3).any(|w| matches!(w, [Operator::LocalGet { local_index: 4 }, Operator::I32Const { value }, Operator::I32Store { memarg }] if *value == kind && memarg.offset == 8)));
        for offset in [0, 4, 8, 12, 24, 28] {
            assert!(
                copy.iter().any(
                    |op| matches!(op, Operator::I32Store { memarg } if memarg.offset == offset)
                )
            );
        }
        for offset in [16, 32] {
            assert!(copy.iter().any(|op| matches!(op, Operator::I64Store { memarg } if memarg.offset == offset && memarg.align == 3)));
        }
        assert!(matches!(
            &copy[copy.len() - 4..],
            [
                Operator::LocalGet { local_index: 4 },
                Operator::GlobalSet { global_index: 32 },
                Operator::LocalGet { local_index: 3 },
                Operator::End
            ]
        ));
        match ty {
            "number" => assert!(
                copy.iter()
                    .any(|op| matches!(op, Operator::F64Load { memarg } if memarg.offset == 0))
            ),
            "bool" => assert!(
                copy.iter()
                    .any(|op| matches!(op, Operator::I32Load8U { memarg } if memarg.offset == 0))
            ),
            _ => assert!(
                copy.iter()
                    .any(|op| matches!(op, Operator::I32Load { memarg } if memarg.offset == 0))
            ),
        }
        // Full capacity and acquisition identity are validated by allocator-owned
        // registry lookup, never by M4 subtracting a record header from a pointer.
        assert_eq!(calls(&bodies[first + 5]), [first as u32 + 2]);
        assert!(
            bodies[first + 5]
                .iter()
                .any(|op| matches!(op, Operator::I64Load { memarg } if memarg.offset == 24))
        );
    }
}
