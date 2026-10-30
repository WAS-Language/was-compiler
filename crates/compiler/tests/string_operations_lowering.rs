use was_compiler::{
    CodegenError, Compiler, ExportOptions, OptimizationLevel, Type,
    ast::BinaryOp,
    codegen,
    typed_ast::{
        BuiltinFunction, ExprResult, FunctionId, FunctionRef, TypedClosureBody, TypedContract,
        TypedExpr, TypedExprKind, TypedStatementKind,
    },
};
use wasmparser::{Operator, Parser, Payload, Validator};

fn operations(bytes: &[u8]) -> Vec<Vec<Operator<'_>>> {
    Validator::new().validate_all(bytes).unwrap();
    Parser::new(0)
        .parse_all(bytes)
        .filter_map(|payload| match payload.unwrap() {
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

fn call(builtin: BuiltinFunction, args: Vec<TypedExpr>, ty: Type) -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::Call {
            func: FunctionRef::Builtin(builtin),
            name: "not_a_name_lookup".into(),
            args,
        },
        ty: ty.into(),
    }
}

fn string(value: &str) -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::String(value.into()),
        ty: Type::String.into(),
    }
}

fn program(value: TypedExpr) -> was_compiler::typed_ast::TypedProgram {
    let mut program = Compiler::new().analyze("fn main() -> void {}").unwrap();
    program.functions[0].return_type = value.ty.value_type().unwrap_or(Type::Number);
    program.functions[0]
        .body
        .push(was_compiler::typed_ast::TypedStatement {
            kind: TypedStatementKind::Return(Some(value)),
            span: was_compiler::tokens::Span::new(1, 2),
        });
    program
}

fn calls(ops: &[Operator<'_>]) -> Vec<u32> {
    ops.iter()
        .filter_map(|op| match op {
            Operator::Call { function_index } => Some(*function_index),
            _ => None,
        })
        .collect()
}

#[test]
fn concat_validates_both_before_reuse_and_allocates_one_widened_result() {
    let bytes = codegen::compile(&program(call(
        BuiltinFunction::StringConcat,
        vec![string(""), string("é")],
        Type::String,
    )))
    .unwrap();
    let bodies = operations(&bytes);
    assert_eq!(
        bodies.len(),
        9,
        "concat does not acquire byte-length or M4 helpers"
    );
    let concat = bodies.last().unwrap();
    assert_eq!(
        calls(concat),
        [2, 2, 1],
        "left/right validation, one bump allocation"
    );
    assert!(matches!(
        &concat[..6],
        [
            Operator::LocalGet { local_index: 0 },
            Operator::Call { function_index: 2 },
            Operator::Drop,
            Operator::LocalGet { local_index: 1 },
            Operator::Call { function_index: 2 },
            Operator::Drop,
        ]
    ));
    assert!(matches!(&concat[6..14], [
        Operator::LocalGet { local_index: 0 }, Operator::I32Load { memarg }, Operator::LocalTee { local_index: 2 },
        Operator::I32Eqz, Operator::If { .. }, Operator::LocalGet { local_index: 1 }, Operator::Return, Operator::End,
    ] if memarg.offset == 4));
    assert!(matches!(&concat[14..22], [
        Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg }, Operator::LocalTee { local_index: 3 },
        Operator::I32Eqz, Operator::If { .. }, Operator::LocalGet { local_index: 0 }, Operator::Return, Operator::End,
    ] if memarg.offset == 4));
    assert!(matches!(
        &concat[22..37],
        [
            Operator::LocalGet { local_index: 2 },
            Operator::I64ExtendI32U,
            Operator::LocalGet { local_index: 3 },
            Operator::I64ExtendI32U,
            Operator::I64Add,
            Operator::LocalTee { local_index: 9 },
            Operator::I64Const { value: 8 },
            Operator::I64Add,
            Operator::I64Const {
                value: 4_294_901_760
            },
            Operator::I64GtU,
            Operator::If { .. },
            Operator::Unreachable,
            Operator::End,
            Operator::LocalGet { local_index: 9 },
            Operator::I64Const { value: 8 },
        ]
    ));
    let stores = concat
        .iter()
        .filter_map(|op| match op {
            Operator::I32Store { memarg } => Some((memarg.offset, memarg.align)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(stores, [(0, 2), (4, 2)], "new descriptor fields only");
    assert_eq!(concat.iter().filter(|op| matches!(op, Operator::I32Store8 { memarg } if memarg.offset == 0 && memarg.align == 0)).count(), 2);
    assert_eq!(concat.iter().filter(|op| matches!(op, Operator::I32Load8U { memarg } if memarg.offset == 0 && memarg.align == 0)).count(), 2);
}

#[test]
fn byte_length_uses_string_validation_unsigned_length_and_no_allocation() {
    let program = program(call(
        BuiltinFunction::StringByteLen,
        vec![string("é\0")],
        Type::Number,
    ));
    let bytes = codegen::compile(&program).unwrap();
    let bodies = operations(&bytes);
    assert_eq!(
        bodies.len(),
        9,
        "one user, seven legacy helpers, byte length"
    );
    assert!(matches!(bodies.last().unwrap().as_slice(), [
        Operator::LocalGet { local_index: 0 },
        Operator::Call { function_index: 2 },
        Operator::I32Load { memarg },
        Operator::F64ConvertI32U,
        Operator::End,
    ] if memarg.offset == 4 && memarg.align == 2));
    assert!(matches!(
        bodies[0].as_slice(),
        [
            Operator::I32Const { .. },
            Operator::Call { function_index: 8 },
            Operator::Return,
            Operator::End
        ]
    ));
}

#[test]
fn slice_helper_validates_before_numeric_checks_reuse_and_widened_copy_allocation() {
    let bytes = Compiler::new().compile_source(
        "fn main(s: string, a: number, b: number) -> string { return string_slice_bytes(s, a, b); }"
    ).unwrap();
    let bodies = operations(&bytes);
    assert_eq!(bodies.len(), 9, "only one new helper");
    let slice = bodies.last().unwrap();
    assert!(matches!(
        &slice[..3],
        [
            Operator::LocalGet { local_index: 0 },
            Operator::Call { function_index: 2 },
            Operator::Drop,
        ]
    ));
    assert_eq!(
        calls(slice),
        [2, 1],
        "validate source then one ordinary allocation"
    );
    let conversion = slice
        .iter()
        .position(|op| matches!(op, Operator::I32TruncF64U))
        .unwrap();
    assert_eq!(
        slice[..conversion]
            .iter()
            .filter(|op| matches!(op, Operator::F64Trunc))
            .count(),
        2
    );
    assert_eq!(
        slice
            .iter()
            .filter(|op| matches!(op, Operator::I32Load8U { .. }))
            .count(),
        3,
        "two guarded endpoint probes plus one copy loop, no UTF8 scan"
    );
    assert_eq!(
        slice
            .iter()
            .filter(|op| matches!(op, Operator::I32Store8 { .. }))
            .count(),
        1
    );
    let allocation = slice
        .iter()
        .position(|op| matches!(op, Operator::Call { function_index: 1 }))
        .unwrap();
    assert!(
        slice[..allocation]
            .iter()
            .any(|op| matches!(op, Operator::Return))
    );
    assert!(
        slice[..allocation]
            .iter()
            .any(|op| matches!(op, Operator::I64GtU))
    );
    assert!(slice[..allocation].iter().any(|op| matches!(
        op,
        Operator::I64Const {
            value: 4_294_901_760
        }
    )));
}

fn replace_args() -> Vec<TypedExpr> {
    vec![string("aaa"), string("aa"), string("b")]
}

#[test]
fn replace_validates_three_strings_before_checks_and_content_reuse_then_allocates_once() {
    let bytes = codegen::compile(&program(call(
        BuiltinFunction::StringReplaceAll,
        replace_args(),
        Type::String,
    )))
    .unwrap();
    let bodies = operations(&bytes);
    assert_eq!(
        bodies.len(),
        9,
        "one user, seven legacy helpers, replace only"
    );
    assert_eq!(calls(&bodies[0]), [8]);

    let mut signatures = Vec::new();
    let mut function_types = Vec::new();
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            Payload::TypeSection(reader) => {
                signatures.extend(reader.into_iter_err_on_gc_types().map(Result::unwrap))
            }
            Payload::FunctionSection(reader) => {
                function_types.extend(reader.into_iter().map(Result::unwrap))
            }
            Payload::ImportSection(reader) => {
                assert_eq!(reader.count(), 0, "no host search/copy imports")
            }
            _ => {}
        }
    }
    let signature = &signatures[function_types[8] as usize];
    assert_eq!(signature.params(), [wasmparser::ValType::I32; 3]);
    assert_eq!(signature.results(), [wasmparser::ValType::I32]);

    let replace = &bodies[8];
    assert_eq!(
        calls(replace),
        [2, 2, 2, 6, 1],
        "three validations, content equality, one ordinary allocation; no intermediate strings"
    );
    assert!(
        matches!(
            &replace[..9],
            [
                Operator::LocalGet { local_index: 0 },
                Operator::Call { function_index: 2 },
                Operator::Drop,
                Operator::LocalGet { local_index: 1 },
                Operator::Call { function_index: 2 },
                Operator::Drop,
                Operator::LocalGet { local_index: 2 },
                Operator::Call { function_index: 2 },
                Operator::Drop,
            ]
        ),
        "even unused operands are validated before constraints or reuse"
    );
    let equality = replace
        .iter()
        .position(|op| matches!(op, Operator::Call { function_index: 6 }))
        .unwrap();
    assert!(
        matches!(
            &replace[equality - 2..=equality],
            [
                Operator::LocalGet { local_index: 1 },
                Operator::LocalGet { local_index: 2 },
                Operator::Call { function_index: 6 }
            ]
        ),
        "reuse compares needle/replacement contents, not handles"
    );
    assert!(replace[9..equality].windows(2).any(|ops| matches!(ops,
        [Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg }] if memarg.offset == 4
    )), "read needle byte length after all three validations");
    assert!(
        replace[9..equality]
            .iter()
            .any(|op| matches!(op, Operator::I32Eqz))
    );
    assert!(
        replace[9..equality]
            .iter()
            .any(|op| matches!(op, Operator::Unreachable)),
        "empty needle traps even on content-identical reuse"
    );
    let first_return = replace
        .iter()
        .position(|op| matches!(op, Operator::Return))
        .unwrap();
    assert!(
        equality < first_return,
        "no source reuse before content equality"
    );
    assert!(matches!(
        replace[first_return - 1],
        Operator::LocalGet { local_index: 0 }
    ));

    let allocation = replace
        .iter()
        .position(|op| matches!(op, Operator::Call { function_index: 1 }))
        .unwrap();
    assert!(first_return < allocation);
    assert!(
        replace[..allocation]
            .windows(2)
            .filter(|ops| matches!(
                ops,
                [Operator::LocalGet { local_index: 0 }, Operator::Return]
            ))
            .count()
            >= 2,
        "both content-identical and no-match paths reuse the validated source before allocation"
    );
    let sizing = &replace[equality + 1..allocation];
    assert!(
        sizing
            .iter()
            .any(|op| matches!(op, Operator::I64ExtendI32U)),
        "unsigned widened lengths"
    );
    assert!(
        sizing.iter().any(|op| matches!(op, Operator::I64Add)),
        "widened result/descriptor sizing"
    );
    assert!(
        sizing.iter().any(|op| matches!(op, Operator::I64GtU)),
        "checked memory32 allocation bound"
    );
    assert!(
        sizing.iter().any(|op| matches!(
            op,
            Operator::I64Const {
                value: 4_294_901_760
            }
        )),
        "existing allocator ceiling, not an arbitrary string cap"
    );
    assert!(
        sizing
            .iter()
            .any(|op| matches!(op, Operator::I64Const { value: 8 })),
        "descriptor included before allocation"
    );
    assert!(sizing.iter().any(|op| matches!(op, Operator::Unreachable)));
    assert!(
        sizing.iter().any(|op| matches!(op, Operator::I32WrapI64)),
        "narrow only after checked sizing"
    );
    assert!(
        replace[..allocation]
            .iter()
            .all(|op| !matches!(op, Operator::I32Store8 { .. })),
        "sizing/search does not mutate input payloads"
    );
    assert!(
        sizing.iter().any(|op| matches!(op, Operator::Loop { .. })),
        "search/count pass before allocation"
    );
    assert!(sizing.iter().filter(|op| matches!(op, Operator::I32Load8U { memarg } if memarg.offset == 0 && memarg.align == 0)).count() >= 2, "compare source and needle bytes, not descriptor identity");
    assert!(
        sizing
            .iter()
            .any(|op| matches!(op, Operator::I32Ne | Operator::I32Eq)),
        "byte content comparison"
    );
    let construction = &replace[allocation + 1..];
    assert!(
        construction
            .iter()
            .any(|op| matches!(op, Operator::Loop { .. })),
        "construct directly in final payload"
    );
    assert!(construction.iter().any(
        |op| matches!(op, Operator::I32Load8U { memarg } if memarg.offset == 0 && memarg.align == 0)
    ));
    assert!(construction.iter().any(
        |op| matches!(op, Operator::I32Store8 { memarg } if memarg.offset == 0 && memarg.align == 0)
    ));
    let stores = replace
        .iter()
        .filter_map(|op| match op {
            Operator::I32Store { memarg } => Some((memarg.offset, memarg.align)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        stores,
        [(0, 2), (4, 2)],
        "only final descriptor fields are published"
    );
}

#[test]
fn replace_checked_input_rejects_forged_arity_types_categories_and_all_children() {
    let builtin = BuiltinFunction::StringReplaceAll;
    for count in [0, 1, 2, 4] {
        let mut value = call(builtin, vec![string("x"); count], Type::String);
        value.ty = ExprResult::Never;
        assert!(
            matches!(
                codegen::compile_with_exports(&program(value), &ExportOptions::None),
                Err(CodegenError::ArgumentCountMismatch { expected: 3, found, .. }) if found == count
            ),
            "arity {count} is checked even with a forged Never result and no exports"
        );
    }
    for category in [
        Type::Number.into(),
        Type::Bool.into(),
        Type::Range.into(),
        Type::Array(was_compiler::ArrayElementType::String).into(),
        ExprResult::Void,
        ExprResult::Value(Type::Void),
        ExprResult::Never,
    ] {
        let mut value = call(builtin, replace_args(), Type::String);
        value.ty = category;
        let mut program = program(value);
        // Keep the enclosing signature valid so the forged call reaches emission.
        program.functions[0].return_type = Type::String;
        assert!(
            matches!(
                codegen::compile_with_exports(&program, &ExportOptions::None),
                Err(CodegenError::TypeMismatch { .. } | CodegenError::ExprResultMismatch { .. })
            ),
            "forged result {category:?} is rejected by checked lowering, not missing helper discovery"
        );
    }
    for position in 0..3 {
        for bad in [
            number("1"),
            TypedExpr {
                kind: TypedExprKind::Bool(true),
                ty: Type::Bool.into(),
            },
            TypedExpr {
                kind: TypedExprKind::Number("1".into()),
                ty: Type::String.into(),
            },
        ] {
            let mut args = replace_args();
            args[position] = bad;
            assert!(
                codegen::compile_with_exports(
                    &program(call(builtin, args, Type::String)),
                    &ExportOptions::None
                )
                .is_err(),
                "operand {position}: exact type and child kind"
            );
        }
        for category in [
            ExprResult::Void,
            ExprResult::Value(Type::Void),
            ExprResult::Never,
        ] {
            let mut args = replace_args();
            args[position].ty = category;
            let mut value = call(builtin, args, Type::String);
            if category == ExprResult::Never {
                value.ty = ExprResult::Never;
            }
            assert!(
                codegen::compile(&program(value)).is_err(),
                "operand {position}: forged category {category:?}"
            );
        }
        for earlier in 0..position {
            for bad in [
                number("1"),
                TypedExpr {
                    kind: TypedExprKind::Bool(true),
                    ty: Type::String.into(),
                },
            ] {
                let mut args = replace_args();
                args[earlier] = divergent();
                args[position] = bad;
                let mut value = call(builtin, args, Type::String);
                value.ty = ExprResult::Never;
                assert!(
                    codegen::compile_with_exports(&program(value), &ExportOptions::None).is_err(),
                    "check child {position} after Never child {earlier}"
                );
            }
        }
    }
}

#[test]
fn replace_structural_discovery_handles_each_never_operand_and_all_never_annotations() {
    for divergent_positions in [vec![0], vec![1], vec![2], vec![0, 1, 2]] {
        let mut args = replace_args();
        for position in divergent_positions {
            args[position] = divergent();
            let TypedExprKind::Loop { id, .. } = &mut args[position].kind else {
                panic!()
            };
            *id = was_compiler::typed_ast::LoopId(position);
        }
        let mut value = call(BuiltinFunction::StringReplaceAll, args, Type::String);
        value.ty = ExprResult::Never;
        let bytes = codegen::compile_with_exports(&program(value), &ExportOptions::None).unwrap();
        let bodies = operations(&bytes);
        assert_eq!(
            bodies.len(),
            9,
            "Never calls still require replace, but no other M5 helper"
        );
        assert_eq!(calls(&bodies[0]), [8]);
        assert_eq!(calls(&bodies[8]), [2, 2, 2, 6, 1]);
    }
}

#[test]
fn replace_helper_is_appended_after_slice_and_other_required_m5_helpers_only() {
    for with_concat_and_length in [false, true] {
        let value = call(
            BuiltinFunction::StringReplaceAll,
            vec![
                call(
                    BuiltinFunction::StringSliceBytes,
                    slice_args(),
                    Type::String,
                ),
                string("é"),
                string("x"),
            ],
            Type::String,
        );
        let value = if with_concat_and_length {
            call(
                BuiltinFunction::StringByteLen,
                vec![call(
                    BuiltinFunction::StringConcat,
                    vec![value, string("!")],
                    Type::String,
                )],
                Type::Number,
            )
        } else {
            value
        };
        let bytes = codegen::compile(&program(value)).unwrap();
        let bodies = operations(&bytes);
        if with_concat_and_length {
            assert_eq!(bodies.len(), 12);
            assert_eq!(
                calls(&bodies[0]),
                [10, 11, 9, 8],
                "legacy group, byte length, concat, slice, replace"
            );
        } else {
            assert_eq!(
                bodies.len(),
                10,
                "slice and replace do not acquire concat or byte length"
            );
            assert_eq!(calls(&bodies[0]), [8, 9]);
        }
        assert_eq!(calls(bodies.last().unwrap()), [2, 2, 2, 6, 1]);
    }
}

#[test]
fn replace_nested_arguments_are_once_in_order_with_a_live_prefix_at_all_presets() {
    let source = r#"
        fn source() -> string { return "aaa"; }
        fn needle() -> string { return "aa"; }
        fn replacement() -> string { return "b"; }
        fn main() -> number {
            return 7 + string_byte_len(string_replace_all(source(), needle(), replacement()));
        }
    "#;
    let mut enabled = None;
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        let mut compiler = Compiler::new();
        compiler.set_optimization_level(level);
        let bytes = compiler.compile_source(source).unwrap();
        let bodies = operations(&bytes);
        assert_eq!(
            bodies.len(),
            13,
            "four users, seven legacy helpers, length, replace"
        );
        assert_eq!(
            calls(&bodies[3]),
            [0, 1, 2, 12, 11],
            "each argument once, source/needle/replacement, then builtin checks"
        );
        assert!(
            matches!(bodies[3][0], Operator::F64Const { value } if value.bits() == 7.0_f64.to_bits())
                || matches!(bodies[3][0], Operator::I32Const { value: 7 })
        );
        assert!(bodies[3].iter().any(|op| matches!(op, Operator::F64Add)));
        assert_eq!(bytes, compiler.compile_source(source).unwrap());
        if level != OptimizationLevel::None {
            if let Some(expected) = &enabled {
                assert_eq!(&bytes, expected);
            } else {
                enabled = Some(bytes);
            }
        }
    }
}

#[test]
fn replace_discovery_visits_nested_capturing_callables_and_unreachable_calls() {
    let source = r#"
        fn main(s: string) -> string {
            let outer = () => {
                let inner = () => string_replace_all(s, "a", "b");
                inner()
            };
            return outer();
        }
    "#;
    let bytes = Compiler::new().compile_source(source).unwrap();
    let bodies = operations(&bytes);
    assert_eq!(
        bodies.len(),
        11,
        "user, seven legacy helpers, replace, two callables"
    );
    assert_eq!(calls(&bodies[8]), [2, 2, 2, 6, 1]);
    assert_eq!(
        calls(bodies.last().unwrap())
            .iter()
            .filter(|index| **index == 8)
            .count(),
        1
    );
    assert_eq!(
        calls(&bodies[0])
            .iter()
            .filter(|index| **index == 8)
            .count(),
        0,
        "replace is discovered only inside the nested callable, not the creator"
    );

    let mut unreachable = program(call(
        BuiltinFunction::StringReplaceAll,
        replace_args(),
        Type::String,
    ));
    unreachable.functions[0].body.insert(
        0,
        was_compiler::typed_ast::TypedStatement {
            kind: TypedStatementKind::Return(Some(string("early"))),
            span: was_compiler::tokens::Span::new(1, 2),
        },
    );
    let bytes = codegen::compile_with_exports(&unreachable, &ExportOptions::None).unwrap();
    let bodies = operations(&bytes);
    assert_eq!(bodies.len(), 9);
    assert_eq!(
        calls(&bodies[0]),
        [8],
        "unreachable unexported call is still discovered/emitted"
    );

    let mut program = Compiler::new()
        .analyze("fn main() -> string { let f = () => { return \"x\"; }; return f(); }")
        .unwrap();
    let TypedStatementKind::Closure(closure) = &mut program.functions[0].body[0].kind else {
        panic!()
    };
    let TypedClosureBody::Block(block) = &mut closure.body else {
        panic!()
    };
    let mut value = call(
        BuiltinFunction::StringReplaceAll,
        vec![divergent(), divergent(), divergent()],
        Type::String,
    );
    let TypedExprKind::Call { args, .. } = &mut value.kind else {
        panic!()
    };
    for (position, arg) in args.iter_mut().enumerate() {
        let TypedExprKind::Loop { id, .. } = &mut arg.kind else {
            panic!()
        };
        *id = was_compiler::typed_ast::LoopId(position);
    }
    value.ty = ExprResult::Never;
    // Preserve a valid closure result while checking discovery in unreachable code.
    block
        .statements
        .push(was_compiler::typed_ast::TypedStatement {
            kind: TypedStatementKind::Expression(value),
            span: closure.body_span,
        });
    let bytes = codegen::compile(&program).unwrap();
    let bodies = operations(&bytes);
    assert_eq!(
        bodies.len(),
        10,
        "all-Never nested call still discovers memory and replace structurally"
    );
    assert_eq!(calls(&bodies[8]), [2, 2, 2, 6, 1]);
    assert_eq!(calls(&bodies[9]), [8]);
}

fn slice_args() -> Vec<TypedExpr> {
    vec![string("é"), number("0"), number("2")]
}

fn number(value: &str) -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::Number(value.into()),
        ty: Type::Number.into(),
    }
}

#[test]
fn slice_checked_input_rejects_forged_arity_types_categories_and_later_children() {
    let builtin = BuiltinFunction::StringSliceBytes;
    for count in [0, 1, 2, 4] {
        let mut args = slice_args();
        args.resize(count, number("0"));
        let mut value = call(builtin, args, Type::String);
        value.ty = ExprResult::Never;
        assert!(
            matches!(codegen::compile_with_exports(&program(value), &ExportOptions::None),
            Err(CodegenError::ArgumentCountMismatch { expected: 3, found, .. }) if found == count)
        );
    }
    for category in [
        Type::Number.into(),
        Type::Bool.into(),
        ExprResult::Void,
        ExprResult::Value(Type::Void),
        ExprResult::Never,
    ] {
        let mut value = call(builtin, slice_args(), Type::String);
        value.ty = category;
        assert!(
            codegen::compile(&program(value)).is_err(),
            "result {category:?}"
        );
    }
    for position in 0..3 {
        for bad in [
            string("wrong"),
            number("1"),
            TypedExpr {
                kind: TypedExprKind::Bool(true),
                ty: Type::Bool.into(),
            },
        ] {
            if bad.ty
                == if position == 0 {
                    Type::String
                } else {
                    Type::Number
                }
            {
                continue;
            }
            let mut args = slice_args();
            args[position] = bad;
            assert!(
                codegen::compile_with_exports(
                    &program(call(builtin, args, Type::String)),
                    &ExportOptions::None
                )
                .is_err()
            );
        }
        for category in [
            ExprResult::Void,
            ExprResult::Value(Type::Void),
            ExprResult::Never,
        ] {
            let mut args = slice_args();
            args[position].ty = category;
            let mut value = call(builtin, args, Type::String);
            if category == ExprResult::Never {
                value.ty = ExprResult::Never;
            }
            assert!(codegen::compile(&program(value)).is_err());
        }
        let mut args = slice_args();
        args[position].kind = TypedExprKind::Bool(true);
        assert!(
            codegen::compile(&program(call(builtin, args, Type::String))).is_err(),
            "check child kind independently"
        );
    }
    for position in [1, 2] {
        let mut args = slice_args();
        args[0] = divergent();
        args[position] = string("not numeric");
        let mut value = call(builtin, args, Type::String);
        value.ty = ExprResult::Never;
        assert!(
            codegen::compile(&program(value)).is_err(),
            "check after Never"
        );
    }
}

#[test]
fn slice_structural_discovery_and_argument_order_compose_with_never_helpers_and_presets() {
    for position in 0..3 {
        let mut args = slice_args();
        args[position] = divergent();
        let mut value = call(BuiltinFunction::StringSliceBytes, args, Type::String);
        value.ty = ExprResult::Never;
        assert_eq!(
            operations(&codegen::compile(&program(value)).unwrap()).len(),
            9
        );
    }
    let source = r#"
        fn source() -> string { return "é"; }
        fn start() -> number { return 0; }
        fn end() -> number { return 2; }
        fn main() -> number {
            let f = () => string_byte_len(string_slice_bytes(source(), start(), end()));
            return 7 + f();
            string_slice_bytes("x", 0, 1);
        }
    "#;
    let mut enabled = None;
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        let mut compiler = Compiler::new();
        compiler.set_optimization_level(level);
        let bytes = compiler.compile_source(source).unwrap();
        let bodies = operations(&bytes);
        assert_eq!(calls(bodies.last().unwrap()), [0, 1, 2, 12, 11]);
        assert_eq!(bytes, compiler.compile_source(source).unwrap());
        if level != OptimizationLevel::None {
            if let Some(expected) = &enabled {
                assert_eq!(&bytes, expected);
            } else {
                enabled = Some(bytes);
            }
        }
    }
}

fn divergent() -> TypedExpr {
    let program = Compiler::new()
        .analyze("fn main() -> string { return loop {}; }")
        .unwrap();
    let TypedStatementKind::Return(Some(value)) = &program.functions[0].body[0].kind else {
        panic!()
    };
    value.clone()
}

#[test]
fn checked_calls_reject_arity_types_categories_and_forged_children_even_when_unexported() {
    for (builtin, arity, ty) in [
        (BuiltinFunction::StringByteLen, 1, Type::Number),
        (BuiltinFunction::StringConcat, 2, Type::String),
    ] {
        for count in [0, arity + 1] {
            let mut value = call(builtin, vec![string("x"); count], ty);
            value.ty = ExprResult::Never;
            assert!(matches!(
                codegen::compile_with_exports(&program(value), &ExportOptions::None),
                Err(CodegenError::ArgumentCountMismatch { expected, found, .. })
                    if expected == arity && found == count
            ));
        }
        for category in [
            Type::Bool.into(),
            ExprResult::Void,
            ExprResult::Value(Type::Void),
            ExprResult::Never,
        ] {
            let mut value = call(builtin, vec![string("x"); arity], ty);
            value.ty = category;
            assert!(
                codegen::compile_with_exports(&program(value), &ExportOptions::None).is_err(),
                "{builtin:?}, {category:?}"
            );
        }
        for position in 0..arity {
            for category in [
                Type::Number.into(),
                Type::Bool.into(),
                ExprResult::Void,
                ExprResult::Value(Type::Void),
                ExprResult::Never,
            ] {
                let mut args = vec![string("x"); arity];
                args[position].ty = category;
                let mut value = call(builtin, args, ty);
                if category == ExprResult::Never {
                    value.ty = ExprResult::Never;
                }
                assert!(
                    codegen::compile_with_exports(&program(value), &ExportOptions::None).is_err(),
                    "{builtin:?}, {position}, {category:?}"
                );
            }
            let mut args = vec![string("x"); arity];
            args[position].kind = TypedExprKind::Number("1".into());
            assert!(
                codegen::compile(&program(call(builtin, args, ty))).is_err(),
                "child kind independently checked"
            );
        }
    }
}

#[test]
fn never_arguments_discover_helpers_without_concrete_string_types_and_check_later_children() {
    for (builtin, args, ty) in [
        (
            BuiltinFunction::StringByteLen,
            vec![divergent()],
            Type::Number,
        ),
        (
            BuiltinFunction::StringConcat,
            vec![divergent(), divergent()],
            Type::String,
        ),
    ] {
        // Give each loop a distinct owner-local identity.
        let mut args = args;
        if args.len() == 2 {
            let TypedExprKind::Loop { id, .. } = &mut args[1].kind else {
                panic!()
            };
            *id = was_compiler::typed_ast::LoopId(1);
        }
        let mut value = call(builtin, args, ty);
        value.ty = ExprResult::Never;
        let bytes = codegen::compile(&program(value)).unwrap();
        assert_eq!(operations(&bytes).len(), 9);
    }
    for bad in [
        TypedExpr {
            kind: TypedExprKind::Number("1".into()),
            ty: Type::Number.into(),
        },
        TypedExpr {
            kind: TypedExprKind::Number("1".into()),
            ty: Type::String.into(),
        },
    ] {
        let mut value = call(
            BuiltinFunction::StringConcat,
            vec![divergent(), bad],
            Type::String,
        );
        value.ty = ExprResult::Never;
        assert!(
            codegen::compile(&program(value)).is_err(),
            "Never must not skip a later child"
        );
    }
}

#[test]
fn nested_arguments_are_once_in_order_with_a_live_numeric_prefix_at_all_presets() {
    let mut program = Compiler::new()
        .analyze(
            r#"
        fn left() -> string { return "a"; }
        fn right() -> string { return "b"; }
        fn main() -> number { return 7; }
    "#,
        )
        .unwrap();
    let arg = |id| TypedExpr {
        kind: TypedExprKind::Call {
            func: FunctionRef::User(FunctionId(id)),
            name: "ignored".into(),
            args: vec![],
        },
        ty: Type::String.into(),
    };
    let value = call(
        BuiltinFunction::StringByteLen,
        vec![call(
            BuiltinFunction::StringConcat,
            vec![arg(0), arg(1)],
            Type::String,
        )],
        Type::Number,
    );
    let TypedStatementKind::Return(Some(result)) = &mut program.functions[2].body[0].kind else {
        panic!()
    };
    *result = TypedExpr {
        kind: TypedExprKind::Binary {
            left: Box::new(TypedExpr {
                kind: TypedExprKind::Number("7".into()),
                ty: Type::Number.into(),
            }),
            op: BinaryOp::Add,
            right: Box::new(value),
        },
        ty: Type::Number.into(),
    };
    let mut enabled = None;
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        let mut compiler = Compiler::new();
        compiler.set_optimization_level(level);
        let bytes = compiler.compile(&program).unwrap();
        assert_eq!(compiler.compile(&program).unwrap(), bytes);
        let bodies = operations(&bytes);
        assert_eq!(calls(&bodies[2]), [0, 1, 11, 10]);
        assert!(matches!(
            bodies[2][0],
            Operator::F64Const { .. } | Operator::I32Const { .. }
        ));
        if level != OptimizationLevel::None {
            if let Some(expected) = &enabled {
                assert_eq!(&bytes, expected);
            } else {
                enabled = Some(bytes);
            }
        }
    }
}

#[test]
fn discovery_visits_capture_helpers_and_unreachable_expressions() {
    let mut program = Compiler::new()
        .analyze(
            r#"
        fn main(s: string) -> string {
            let text = () => s;
            return text();
        }
    "#,
        )
        .unwrap();
    let TypedStatementKind::Closure(closure) = &mut program.functions[0].body[0].kind else {
        panic!()
    };
    let TypedClosureBody::Expression(value) = &mut closure.body else {
        panic!()
    };
    **value = call(
        BuiltinFunction::StringConcat,
        vec![(**value).clone(), string("!")],
        Type::String,
    );
    let bytes = codegen::compile(&program).unwrap();
    let bodies = operations(&bytes);
    assert!(
        calls(bodies.last().unwrap()).contains(&8),
        "closure calls planned concat"
    );

    let mut program = self::program(call(
        BuiltinFunction::StringByteLen,
        vec![string("")],
        Type::Number,
    ));
    program.functions[0].body.insert(
        0,
        was_compiler::typed_ast::TypedStatement {
            kind: TypedStatementKind::Return(Some(TypedExpr {
                kind: TypedExprKind::Number("0".into()),
                ty: Type::Number.into(),
            })),
            span: was_compiler::tokens::Span::new(1, 2),
        },
    );
    assert_eq!(operations(&codegen::compile(&program).unwrap()).len(), 9);
}

#[test]
fn m5_calls_remain_forbidden_in_forged_contracts_and_defaults() {
    for value in [
        call(
            BuiltinFunction::StringReplaceAll,
            replace_args(),
            Type::String,
        ),
        call(
            BuiltinFunction::StringSliceBytes,
            slice_args(),
            Type::String,
        ),
        call(
            BuiltinFunction::StringByteLen,
            vec![string("x")],
            Type::Number,
        ),
        call(
            BuiltinFunction::StringConcat,
            vec![string("x"), string("y")],
            Type::String,
        ),
    ] {
        let mut program = Compiler::new().analyze("fn main() -> void {}").unwrap();
        program.functions[0].contracts.push(TypedContract {
            value: value.clone(),
            span: was_compiler::tokens::Span::new(1, 2),
        });
        assert!(matches!(
            codegen::compile(&program),
            Err(CodegenError::InvalidContractExpression { .. })
        ));

        let mut program = Compiler::new()
            .analyze("fn target(s: string = \"x\", n: number = 1) {} fn main() { target(); }")
            .unwrap();
        let TypedStatementKind::Expression(expr) = &mut program.functions[1].body[0].kind else {
            panic!()
        };
        let TypedExprKind::NormalizedCall(sequence) = &mut expr.kind else {
            panic!()
        };
        let index = if value.ty == Type::String { 0 } else { 1 };
        sequence.bindings[index].value = value;
        assert!(matches!(
            codegen::compile(&program),
            Err(CodegenError::InvalidNormalizedCall { .. })
        ));
    }
}
