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

#[test]
fn pop_lowering_validates_authority_and_consumed_value_before_length_publication() {
    for (ty, consumer) in [("number", None), ("bool", Some(8)), ("string", Some(2))] {
        let mut program = Compiler::new()
            .analyze(&format!("fn main(a: {ty}[]) -> {ty} {{ return a[0]; }}"))
            .unwrap();
        let value = result(&mut program);
        let TypedExprKind::Index { expr: receiver, .. } = &value.kind else {
            panic!("index")
        };
        value.kind = TypedExprKind::Pop {
            receiver: receiver.clone(),
        };
        let bytes = codegen::compile(&program).unwrap();
        let bodies = operators(&bytes);
        let helper = bodies.last().unwrap();
        let helper_calls = calls(helper);
        assert!(matches!(&helper[..9], [
            Operator::LocalGet { local_index: 0 }, Operator::Call { .. }, Operator::LocalSet { local_index: 1 },
            Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg }, Operator::I32Eqz,
            Operator::If { .. }, Operator::Unreachable, Operator::End
        ] if memarg.offset == 12));
        assert!(matches!(&helper[9..14], [
            Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg }, Operator::I32Const { value: 1 }, Operator::I32Sub, Operator::LocalSet { local_index: 2 }
        ] if memarg.offset == 12));
        let load = &helper[22];
        assert!(
            match load {
                Operator::F64Load { memarg } if ty == "number" =>
                    memarg.offset == 0 && memarg.align == 3,
                Operator::I32Load8U { memarg } if ty == "bool" =>
                    memarg.offset == 0 && memarg.align == 0,
                Operator::I32Load { memarg } if ty == "string" =>
                    memarg.offset == 0 && memarg.align == 2,
                _ => false,
            },
            "{ty}: {load:?}"
        );
        assert_eq!(helper_calls.len(), if consumer.is_some() { 2 } else { 1 });
        if let Some(consumer) = consumer {
            assert_eq!(helper_calls[1], consumer);
        }
        let first_store = helper
            .iter()
            .position(|op| matches!(op, Operator::I32Store { .. }))
            .unwrap();
        let last_call = helper
            .iter()
            .rposition(|op| matches!(op, Operator::Call { .. }))
            .unwrap();
        assert!(last_call < first_store);
        let offsets = helper
            .iter()
            .filter_map(|op| match op {
                Operator::I32Store { memarg } => Some(memarg.offset),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(offsets, [4, 12]);
        assert!(!helper.iter().any(|op| matches!(
            op,
            Operator::MemoryGrow { .. } | Operator::I64Store { .. } | Operator::I32Store8 { .. }
        )));
    }
}

fn take_from_index(value: &mut TypedExpr, method: &str) {
    let TypedExprKind::Index {
        expr: receiver,
        index,
    } = &value.kind
    else {
        panic!("index seam")
    };
    value.kind = match method {
        "pop" => TypedExprKind::Pop {
            receiver: receiver.clone(),
        },
        "remove" => TypedExprKind::Remove {
            receiver: receiver.clone(),
            index: index.clone(),
        },
        _ => unreachable!(),
    };
}

#[test]
fn forged_pop_remove_check_shapes_types_results_and_original_captures() {
    use was_compiler::typed_ast::{TypedClosureBody, VariableId};
    for ty in ["number", "bool", "string"] {
        for method in ["pop", "remove"] {
            for captured in [false, true] {
                for case in 0..17 {
                    let source = if captured {
                        format!(
                            "fn main(a: {ty}[]) -> {ty} {{ let take = () => a[0]; return take(); }}"
                        )
                    } else {
                        format!("fn main(a: {ty}[]) -> {ty} {{ return a[0]; }}")
                    };
                    let mut program = Compiler::new().analyze(&source).unwrap();
                    let value = if captured {
                        let TypedStatementKind::Closure(closure) =
                            &mut program.functions[0].body[0].kind
                        else {
                            panic!()
                        };
                        if case == 16 {
                            closure.captures[0].variable = VariableId(999);
                        }
                        let TypedClosureBody::Expression(value) = &mut closure.body else {
                            panic!()
                        };
                        value.as_mut()
                    } else {
                        result(&mut program)
                    };
                    take_from_index(value, method);
                    let receiver = match &mut value.kind {
                        TypedExprKind::Pop { receiver }
                        | TypedExprKind::Remove { receiver, .. } => receiver,
                        _ => unreachable!(),
                    };
                    match case {
                        0 => receiver.ty = Type::Number.into(),
                        1 => receiver.ty = ExprResult::Void,
                        2 => receiver.ty = ExprResult::Value(Type::Void),
                        3 => receiver.ty = ExprResult::Never,
                        4 => receiver.kind = TypedExprKind::Array(vec![], ArrayElementType::Number),
                        5 => **receiver = constructor((**receiver).clone()),
                        6 => receiver.kind = TypedExprKind::Number("1".into()),
                        7 => value.ty = ExprResult::Void,
                        8 => value.ty = ExprResult::Never,
                        9 => value.ty = ExprResult::Value(Type::Void),
                        10 => value.ty = Type::Array(ArrayElementType::Number).into(),
                        11 => {
                            receiver.ty = Type::Array(if ty == "bool" {
                                ArrayElementType::String
                            } else {
                                ArrayElementType::Bool
                            })
                            .into();
                            value.ty = if ty == "bool" {
                                Type::String
                            } else {
                                Type::Bool
                            }
                            .into();
                        }
                        12..=15 if method == "remove" => {
                            let TypedExprKind::Remove { index, .. } = &mut value.kind else {
                                unreachable!()
                            };
                            index.ty = match case {
                                12 => Type::Bool.into(),
                                13 => ExprResult::Never,
                                14 => ExprResult::Void,
                                _ => ExprResult::Value(Type::Void),
                            };
                            if case == 13 {
                                value.ty = ExprResult::Never;
                            }
                        }
                        16 if captured => {}
                        _ => continue,
                    }
                    let error = rejected_push(&program);
                    assert!(
                        error.span().is_some(),
                        "{method}, {ty}, {captured}, {case}: {error:?}"
                    );
                    if (4..=6).contains(&case) {
                        assert!(matches!(
                            error,
                            CodegenError::InvalidAssignmentTarget { .. }
                        ));
                    }
                }
            }
        }
    }
}

#[test]
fn remove_lowering_captures_once_revalidates_and_shifts_raw_bytes_before_shared_lengths() {
    use was_compiler::typed_ast::TypedClosureBody;
    for (ty, ordinary, address_helper, consumer, width) in [
        ("number", 7, 5, None, 8),
        ("bool", 12, 11, Some(9), 1),
        ("string", 11, 10, Some(3), 4),
    ] {
        for captured in [false, true] {
            let source = if captured {
                format!(
                    "fn index() -> number {{ return 0; }} fn main(a: {ty}[]) -> {ty} {{ let take = () => a[index()]; return take(); }}"
                )
            } else {
                format!(
                    "fn index() -> number {{ return 0; }} fn main(a: {ty}[]) -> {ty} {{ return a[index()]; }}"
                )
            };
            let mut program = Compiler::new().analyze(&source).unwrap();
            let value = if captured {
                let TypedStatementKind::Closure(closure) = &mut program.functions[1].body[0].kind
                else {
                    panic!()
                };
                let TypedClosureBody::Expression(value) = &mut closure.body else {
                    panic!()
                };
                value.as_mut()
            } else {
                let TypedStatementKind::Return(Some(value)) =
                    &mut program.functions[1].body[0].kind
                else {
                    panic!()
                };
                value
            };
            take_from_index(value, "remove");
            let mut enabled = None;
            for level in levels() {
                let mut compiler = Compiler::new();
                compiler.set_optimization_level(level);
                let bytes = compiler.compile(&program).unwrap();
                let bodies = operators(&bytes);
                let callable = 2 + ordinary;
                let first = callable + usize::from(captured);
                let validate = (first + 8) as u32;
                let remove = (first + 9) as u32;
                assert_eq!(bodies.len(), first + 10);
                let body = &bodies[if captured { callable } else { 1 }];
                let body_calls = calls(body);
                assert_eq!(&body_calls[body_calls.len() - 3..], [validate, 0, remove]);
                let check = body.iter().position(|op| matches!(op, Operator::Call { function_index } if *function_index == validate)).unwrap();
                let saved = match &body[check - 2..check] {
                    [
                        Operator::LocalSet { local_index: a },
                        Operator::LocalGet { local_index: b },
                    ] if a == b => *a,
                    _ => panic!("saved receiver"),
                };
                assert!(
                    matches!(&body[check+1..check+5], [Operator::Drop, Operator::LocalGet { local_index }, Operator::Call { function_index: 0 }, Operator::Call { function_index }] if *local_index == saved && *function_index == remove)
                );
                let helper = &bodies[remove as usize];
                let mut expected = vec![validate, address_helper];
                expected.extend(consumer);
                assert_eq!(calls(helper), expected);
                assert!(matches!(&helper[..8], [
                    Operator::LocalGet { local_index: 0 }, Operator::Call { function_index }, Operator::LocalSet { local_index: 2 },
                    Operator::LocalGet { local_index: 0 }, Operator::LocalGet { local_index: 1 }, Operator::Call { function_index: checked_address }, Operator::LocalSet { local_index: 4 }, Operator::LocalGet { local_index: 2 }
                ] if *function_index == validate && *checked_address == address_helper));
                let consumed_load = helper
                    .iter()
                    .position(|op| match op {
                        Operator::F64Load { memarg } if ty == "number" => memarg.offset == 0,
                        Operator::I32Load8U { memarg } if ty == "bool" => memarg.offset == 0,
                        Operator::I32Load { memarg } if ty == "string" => memarg.offset == 0,
                        _ => false,
                    })
                    .unwrap();
                let first_write = helper
                    .iter()
                    .position(|op| matches!(op, Operator::I32Store8 { .. }))
                    .unwrap();
                let last_call = helper
                    .iter()
                    .rposition(|op| matches!(op, Operator::Call { .. }))
                    .unwrap();
                assert!(consumed_load < first_write && last_call < first_write);
                assert!(matches!(&helper[first_write-3..first_write+1], [
                    Operator::LocalGet { local_index: 5 }, Operator::LocalGet { local_index: 5 }, Operator::I32Load8U { memarg: load }, Operator::I32Store8 { memarg: store }
                ] if load.offset == width && load.align == 0 && store.offset == 0 && store.align == 0));
                assert!(helper.windows(4).any(|ops| matches!(
                    ops,
                    [
                        Operator::LocalGet { local_index: 5 },
                        Operator::LocalGet { local_index: 6 },
                        Operator::I32GeU,
                        Operator::BrIf { relative_depth: 1 }
                    ]
                )));
                assert!(matches!(&helper[helper.len()-8..], [
                    Operator::LocalGet { local_index: 0 }, Operator::LocalGet { local_index: 3 }, Operator::I32Store { memarg: public },
                    Operator::LocalGet { local_index: 2 }, Operator::LocalGet { local_index: 3 }, Operator::I32Store { memarg: private }, Operator::LocalGet { local_index: 7 }, Operator::End
                ] if public.offset == 4 && private.offset == 12));
                assert!(!helper[first_write..].iter().any(|op| matches!(
                    op,
                    Operator::Call { .. } | Operator::Unreachable | Operator::MemoryGrow { .. }
                )));
                assert!(!helper.iter().any(|op| matches!(
                    op,
                    Operator::F64Store { .. }
                        | Operator::I64Store { .. }
                        | Operator::F64Add
                        | Operator::F64Sub
                )));
                if level != OptimizationLevel::None {
                    if let Some(expected) = &enabled {
                        assert_eq!(&bytes, expected);
                    } else {
                        enabled = Some(bytes);
                    }
                }
            }
        }
    }
}

#[test]
fn pop_remove_remain_forbidden_in_defaults_contracts_skipped_and_unreachable_nodes() {
    for (ty, literal) in [("number", "1"), ("bool", "true"), ("string", "\"x\"")] {
        for method in ["pop", "remove"] {
            let mut source = Compiler::new()
                .analyze(&format!("fn main(a: {ty}[]) -> {ty} {{ return a[0]; }}"))
                .unwrap();
            take_from_index(result(&mut source), method);
            let operation = result(&mut source).clone();
            let mut program = Compiler::new().analyze(&format!("fn target(a: {ty}[], item: {ty} = {literal}) {{}} fn main(a: {ty}[]) {{ target(a); }}")).unwrap();
            let TypedStatementKind::Expression(value) = &mut program.functions[1].body[0].kind
            else {
                panic!()
            };
            let TypedExprKind::NormalizedCall(sequence) = &mut value.kind else {
                panic!()
            };
            sequence.bindings[1].value = operation.clone();
            assert!(matches!(
                rejected_push(&program),
                CodegenError::InvalidNormalizedCall { .. }
            ));

            let mut program = Compiler::new()
                .analyze(&format!("fn main(a: {ty}[]) {{}}"))
                .unwrap();
            let comparison = TypedExpr {
                ty: Type::Bool.into(),
                kind: TypedExprKind::Binary {
                    left: Box::new(operation.clone()),
                    op: BinaryOp::Equal,
                    right: Box::new(operation.clone()),
                },
            };
            program.functions[0].contracts.push(TypedContract {
                span: Span::new(10, 20),
                value: TypedExpr {
                    ty: Type::Bool.into(),
                    kind: TypedExprKind::Binary {
                        left: Box::new(TypedExpr {
                            ty: Type::Bool.into(),
                            kind: TypedExprKind::Bool(false),
                        }),
                        op: BinaryOp::And,
                        right: Box::new(comparison),
                    },
                },
            });
            assert!(
                matches!(rejected_push(&program), CodegenError::InvalidContractExpression { span, .. } if span == Span::new(10, 20))
            );

            for unreachable in [false, true] {
                let source = if unreachable {
                    format!("fn main(a: {ty}[]) {{ return; a[0]; }}")
                } else {
                    format!("fn main(a: {ty}[]) -> bool {{ return false && (a[0] == {literal}); }}")
                };
                let mut program = Compiler::new().analyze(&source).unwrap();
                let (value, span) = if unreachable {
                    let statement = &mut program.functions[0].body[1];
                    let TypedStatementKind::Expression(value) = &mut statement.kind else {
                        panic!()
                    };
                    (value, statement.span)
                } else {
                    let span = program.functions[0].body[0].span;
                    let TypedExprKind::Binary { right, .. } = &mut result(&mut program).kind else {
                        panic!()
                    };
                    let TypedExprKind::Binary { left, .. } = &mut right.kind else {
                        panic!()
                    };
                    (left.as_mut(), span)
                };
                take_from_index(value, method);
                let receiver = match &mut value.kind {
                    TypedExprKind::Pop { receiver } | TypedExprKind::Remove { receiver, .. } => {
                        receiver
                    }
                    _ => unreachable!(),
                };
                receiver.kind = TypedExprKind::Array(vec![], ArrayElementType::Number);
                assert!(
                    matches!(rejected_push(&program), CodegenError::InvalidAssignmentTarget { span: found, .. } if found == span)
                );
            }
        }
    }
}

#[test]
fn remove_preserves_genuine_never_and_plans_every_index_child() {
    for ty in ["number", "bool", "string"] {
        for index in ["loop {}", "{ return 7; 0 }"] {
            let mut program = Compiler::new()
                .analyze(&format!(
                    "fn main(a: {ty}[]) -> number {{ a[{index}]; return 1; }}"
                ))
                .unwrap();
            take_from_index(expression(&mut program), "remove");
            assert_eq!(expression(&mut program).ty, ExprResult::Never);
            for level in levels() {
                let mut compiler = Compiler::new();
                compiler.set_optimization_level(level);
                operators(&compiler.compile(&program).unwrap());
            }
        }
    }
    let mut program = Compiler::new().analyze("fn main(a: number[]) -> number { return a[{ let n = 5 % 2; let text = \"remove-index-child\"; n }]; }").unwrap();
    take_from_index(result(&mut program), "remove");
    let bytes = codegen::compile(&program).unwrap();
    let bodies = operators(&bytes);
    assert!(
        bodies
            .iter()
            .any(|body| body.iter().any(|op| matches!(op, Operator::F64Abs)))
    );
    assert!(
        Parser::new(0)
            .parse_all(&bytes)
            .any(|payload| match payload.unwrap() {
                Payload::DataSection(section) => section.into_iter().any(|segment| segment
                    .unwrap()
                    .data
                    .windows(18)
                    .any(|window| window == b"remove-index-child")),
                _ => false,
            })
    );
    // Parameter, nested n/text declarations, then receiver scratch: each policy
    // must traverse the index block even though the operation owns its receiver.
    assert_eq!(
        first_body_locals(&bytes),
        vec![
            wasmparser::ValType::F64,
            wasmparser::ValType::I32,
            wasmparser::ValType::I32
        ]
    );
}

#[test]
fn pop_remove_append_helpers_without_changing_push_reserve_or_other_existing_bodies() {
    use was_compiler::typed_ast::TypedStatement;
    let bodies = |bytes: &[u8]| {
        Parser::new(0)
            .parse_all(bytes)
            .filter_map(|payload| match payload.unwrap() {
                Payload::CodeSectionEntry(body) => Some(body.as_bytes().to_vec()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    for (ty, literal) in [("number", "3"), ("bool", "true"), ("string", "\"x\"")] {
        let mut program = Compiler::new()
            .analyze(&format!(
                "fn main(a: {ty}[]) {{ a.reserve(2); a.push({literal}); }}"
            ))
            .unwrap();
        let before = codegen::compile(&program).unwrap();
        let before_bodies = bodies(&before);
        for method in ["pop", "remove"] {
            let mut source = Compiler::new()
                .analyze(&format!("fn main(a: {ty}[]) -> {ty} {{ return a[0]; }}"))
                .unwrap();
            take_from_index(result(&mut source), method);
            program.functions[0].body.push(TypedStatement {
                span: Span::new(0, 0),
                kind: TypedStatementKind::Expression(result(&mut source).clone()),
            });
        }
        let after = codegen::compile(&program).unwrap();
        operators(&after);
        let after_bodies = bodies(&after);
        assert_eq!(after_bodies.len(), before_bodies.len() + 2);
        assert_eq!(&after_bodies[1..before_bodies.len()], &before_bodies[1..]);
        assert_eq!(after, codegen::compile(&program).unwrap());
    }
}

#[test]
fn pop_remove_only_helpers_use_scalar_signatures_and_are_discovered_when_skipped() {
    use wasmparser::ValType::{F64, I32};
    for (ty, literal, scalar) in [
        ("number", "1", F64),
        ("bool", "true", I32),
        ("string", "\"x\"", I32),
    ] {
        for method in ["pop", "remove"] {
            let mut program = Compiler::new()
                .analyze(&format!(
                    "fn main(a: {ty}[]) -> bool {{ return false && (a[0] == {literal}); }}"
                ))
                .unwrap();
            let TypedExprKind::Binary { right, .. } = &mut result(&mut program).kind else {
                panic!()
            };
            let TypedExprKind::Binary { left, .. } = &mut right.kind else {
                panic!()
            };
            take_from_index(left, method);
            let mut enabled = None;
            for level in levels() {
                let mut compiler = Compiler::new();
                compiler.set_optimization_level(level);
                let bytes = compiler.compile(&program).unwrap();
                let bodies = operators(&bytes);
                let mut signatures = Vec::new();
                let mut functions = Vec::new();
                let mut exports = Vec::new();
                for payload in Parser::new(0).parse_all(&bytes) {
                    match payload.unwrap() {
                        Payload::TypeSection(reader) => signatures
                            .extend(reader.into_iter_err_on_gc_types().map(Result::unwrap)),
                        Payload::FunctionSection(reader) => {
                            functions.extend(reader.into_iter().map(Result::unwrap))
                        }
                        Payload::ExportSection(reader) => exports.extend(
                            reader
                                .into_iter()
                                .map(|entry| entry.unwrap().name.to_owned()),
                        ),
                        _ => {}
                    }
                }
                let signature = &signatures[*functions.last().unwrap() as usize];
                assert_eq!(
                    signature.params(),
                    if method == "pop" {
                        vec![I32]
                    } else {
                        vec![I32, F64]
                    }
                );
                assert_eq!(signature.results(), [scalar]);
                // No constructor/reserve/push is needed: ordinary memory helpers,
                // seven allocator helpers, one lookup/validate pair and one take.
                let ordinary = match ty {
                    "number" => 7,
                    "bool" => 12,
                    _ => 11,
                };
                assert_eq!(bodies.len(), 1 + ordinary + 7 + 2 + 1);
                assert_eq!(exports, ["main", "memory", "__alloc"]);
                if level != OptimizationLevel::None {
                    if let Some(expected) = &enabled {
                        assert_eq!(&bytes, expected);
                    } else {
                        enabled = Some(bytes);
                    }
                }
            }
        }
    }
}

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

fn reserve(receiver: TypedExpr, additional: TypedExpr) -> TypedExpr {
    TypedExpr {
        ty: ExprResult::Void,
        kind: TypedExprKind::Reserve {
            receiver: Box::new(receiver),
            additional: Box::new(additional),
        },
    }
}

fn expression(program: &mut was_compiler::typed_ast::TypedProgram) -> &mut TypedExpr {
    let TypedStatementKind::Expression(value) = &mut program.functions[0].body[0].kind else {
        panic!("expression")
    };
    value
}

fn levels() -> [OptimizationLevel; 4] {
    [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ]
}

fn rejected_push(program: &was_compiler::typed_ast::TypedProgram) -> CodegenError {
    let mut baseline = None;
    for level in levels() {
        let mut compiler = Compiler::new();
        compiler.set_exports(ExportOptions::None);
        compiler.set_optimization_level(level);
        let error = compiler.compile(program).expect_err("forged Push");
        if let Some(expected) = &baseline {
            assert_eq!(&error, expected, "{level:?}");
        } else {
            baseline = Some(error);
        }
    }
    baseline.unwrap()
}

#[test]
fn forged_push_checks_receiver_item_and_result_even_when_unexported() {
    for case in 0..23 {
        let mut program = Compiler::new()
            .analyze("fn main(a: bool[]) { a.push(true); }")
            .unwrap();
        let span = program.functions[0].body[0].span;
        let value = expression(&mut program);
        let TypedExprKind::Push { receiver, item } = &mut value.kind else {
            panic!("push")
        };
        match case {
            0 => receiver.ty = Type::Number.into(),
            1 => receiver.ty = Type::Array(ArrayElementType::String).into(),
            2 => receiver.ty = ExprResult::Never,
            3 => receiver.ty = ExprResult::Void,
            4 => receiver.ty = ExprResult::Value(Type::Void),
            5 => receiver.kind = TypedExprKind::Array(vec![], ArrayElementType::Bool),
            6 => **receiver = constructor((**receiver).clone()),
            7 | 8 => {
                let source = if case == 7 {
                    "fn main(a: bool[]) -> bool[] { return { a }; }"
                } else {
                    "fn main(a: bool[]) -> bool[] { return if true { a } else { a }; }"
                };
                let mut other = Compiler::new().analyze(source).unwrap();
                **receiver = result(&mut other).clone();
            }
            9 => receiver.kind = TypedExprKind::Number("1".into()),
            10 => item.ty = Type::Number.into(),
            11 => item.ty = ExprResult::Never,
            12 => item.ty = ExprResult::Void,
            13 => item.ty = ExprResult::Value(Type::Void),
            14 => item.kind = TypedExprKind::Number("1".into()),
            15 => value.ty = Type::Bool.into(),
            16 => value.ty = ExprResult::Never,
            17 => value.ty = ExprResult::Value(Type::Void),
            // Claiming divergence on both nodes must not hide a completing child.
            18 => {
                receiver.ty = ExprResult::Never;
                value.ty = ExprResult::Never;
            }
            19 => {
                item.ty = ExprResult::Never;
                value.ty = ExprResult::Never;
            }
            20 => {
                let mut other = Compiler::new()
                    .analyze("fn main(a: bool[]) -> void { main(a); }")
                    .unwrap();
                **item = expression(&mut other).clone();
            }
            21 => item.ty = Type::Array(ArrayElementType::Bool).into(),
            22 => item.ty = Type::String.into(),
            _ => unreachable!(),
        }
        if matches!(case, 1 | 2 | 18) {
            let TypedExprKind::Push { item, .. } = &mut value.kind else {
                unreachable!()
            };
            **item = if case == 1 {
                TypedExpr {
                    kind: TypedExprKind::String("x".into()),
                    ty: Type::String.into(),
                }
            } else {
                TypedExpr {
                    kind: TypedExprKind::Number("1".into()),
                    ty: Type::Number.into(),
                }
            };
        }
        let error = rejected_push(&program);
        assert_eq!(error.span(), Some(span), "case {case}");
        match case {
            5..=9 => assert!(
                matches!(error, CodegenError::InvalidAssignmentTarget { .. }),
                "case {case}: {error:?}"
            ),
            0 | 1 | 3 | 10 | 12 | 14 | 15 | 20..=22 => assert!(
                matches!(error, CodegenError::TypeMismatch { .. }),
                "case {case}: {error:?}"
            ),
            _ => assert!(
                matches!(error, CodegenError::ExprResultMismatch { .. }),
                "case {case}: {error:?}"
            ),
        }
    }
}

#[test]
fn forged_push_captures_check_annotations_and_original_binding_metadata() {
    use was_compiler::typed_ast::{
        BindingOwner, CaptureMutability, FunctionId, TypedClosureBody, VariableId,
    };

    for case in 0..9 {
        let mut program = Compiler::new()
            .analyze("fn main(a: bool[]) { let grow = () => a.push(true); grow(); }")
            .unwrap();
        let TypedStatementKind::Closure(closure) = &mut program.functions[0].body[0].kind else {
            panic!("closure")
        };
        let TypedClosureBody::Expression(value) = &mut closure.body else {
            panic!("expression")
        };
        let TypedExprKind::Push { receiver, .. } = &mut value.kind else {
            panic!("push")
        };
        assert!(matches!(receiver.kind, TypedExprKind::Captured { .. }));
        match case {
            0 => receiver.ty = Type::Number.into(),
            1 => receiver.ty = Type::Array(ArrayElementType::String).into(),
            2 => receiver.ty = ExprResult::Never,
            3 => receiver.ty = ExprResult::Void,
            4 => receiver.ty = ExprResult::Value(Type::Void),
            5 => {
                // Internally consistent forged metadata still disagrees with the original bool[] parameter.
                closure.captures[0].ty = Type::Array(ArrayElementType::String);
                receiver.ty = closure.captures[0].ty.into();
                let TypedExprKind::Push { item, .. } = &mut value.kind else {
                    unreachable!()
                };
                **item = TypedExpr {
                    kind: TypedExprKind::String("x".into()),
                    ty: Type::String.into(),
                };
            }
            6 => closure.captures[0].variable = VariableId(999),
            7 => closure.captures[0].owner = BindingOwner::Function(FunctionId(999)),
            8 => closure.captures[0].mutability = CaptureMutability::ReadOnly,
            _ => unreachable!(),
        }
        if matches!(case, 1 | 2) {
            let TypedExprKind::Push { item, .. } = &mut value.kind else {
                unreachable!()
            };
            **item = if case == 1 {
                TypedExpr {
                    kind: TypedExprKind::String("x".into()),
                    ty: Type::String.into(),
                }
            } else {
                TypedExpr {
                    kind: TypedExprKind::Number("1".into()),
                    ty: Type::Number.into(),
                }
            };
        }
        let error = rejected_push(&program);
        match case {
            0 | 3 => assert!(
                matches!(error, CodegenError::TypeMismatch { .. }),
                "{error:?}"
            ),
            2 | 4 => assert!(
                matches!(error, CodegenError::ExprResultMismatch { .. }),
                "{error:?}"
            ),
            _ => assert!(
                matches!(error, CodegenError::InvalidClosure { .. }),
                "{error:?}"
            ),
        }
    }
}

#[test]
fn forged_push_is_checked_in_skipped_and_unreachable_children() {
    for source in [
        "fn main(a: bool[]) { if false { a.push(true); } }",
        "fn main(a: bool[]) { return; a.push(true); }",
        "fn main(a: bool[]) -> bool { return false && { a.push(true); true }; }",
    ] {
        let mut program = Compiler::new().analyze(source).unwrap();
        let index = usize::from(matches!(
            program.functions[0].body[0].kind,
            TypedStatementKind::Return(None)
        ));
        let statement = match &mut program.functions[0].body[index].kind {
            TypedStatementKind::If { then, .. } => &mut then[0],
            TypedStatementKind::Return(Some(value)) => {
                let TypedExprKind::Binary { right, .. } = &mut value.kind else {
                    panic!()
                };
                let TypedExprKind::Block(block) = &mut right.kind else {
                    panic!()
                };
                &mut block.statements[0]
            }
            _ => &mut program.functions[0].body[index],
        };
        let span = statement.span;
        let TypedStatementKind::Expression(value) = &mut statement.kind else {
            panic!()
        };
        let TypedExprKind::Push { item, .. } = &mut value.kind else {
            panic!()
        };
        **item = TypedExpr {
            kind: TypedExprKind::Number("1".into()),
            ty: Type::Number.into(),
        };
        assert!(
            matches!(rejected_push(&program), CodegenError::TypeMismatch { expected: Type::Bool, found: Type::Number, span: found, .. } if found == span),
            "{source}"
        );
    }
}

#[test]
fn forged_push_is_forbidden_in_defaults_and_skipped_contracts() {
    let mut program = Compiler::new()
        .analyze("fn target(a: bool[], n: number = 1) {} fn main(a: bool[]) { target(a); }")
        .unwrap();
    let TypedStatementKind::Expression(value) = &mut program.functions[1].body[0].kind else {
        panic!()
    };
    let TypedExprKind::NormalizedCall(sequence) = &mut value.kind else {
        panic!()
    };
    let mut source = Compiler::new()
        .analyze("fn main(a: bool[]) -> number { return { a.push(true); 1 }; }")
        .unwrap();
    // Keep the default's numeric result valid so only purity forbids it.
    sequence.bindings[1].value = result(&mut source).clone();
    assert!(matches!(
        rejected_push(&program),
        CodegenError::InvalidNormalizedCall { .. }
    ));

    let mut source = Compiler::new()
        .analyze("fn main(a: bool[]) -> bool { return false && { a.push(true); true }; }")
        .unwrap();
    let forbidden = result(&mut source).clone();
    let mut program = Compiler::new().analyze("fn main(a: bool[]) {}").unwrap();
    program.functions[0].contracts.push(TypedContract {
        span: Span::new(10, 20),
        value: forbidden,
    });
    assert!(
        matches!(rejected_push(&program), CodegenError::InvalidContractExpression { span, .. } if span == Span::new(10, 20))
    );
}

#[test]
fn push_accepts_genuine_never_and_original_captured_receivers_with_preset_parity() {
    for source in [
        "fn main(a: number[]) { a.push(loop {}); }",
        "fn main(a: bool[]) -> number { a.push({ return 7; true }); }",
        "fn main(a: string[]) { a.push(loop {}); }",
        "fn main(a: number[]) { let grow = () => a.push(1); grow(); }",
        "fn main(a: bool[]) { let outer = () => { let inner = () => a.push(true); inner(); }; outer(); }",
        "fn main(a: string[]) { let grow = () => a.push(\"x\"); grow(); }",
    ] {
        let program = Compiler::new().analyze(source).unwrap();
        let mut enabled = None;
        for level in levels() {
            let mut compiler = Compiler::new();
            compiler.set_exports(ExportOptions::None);
            compiler.set_optimization_level(level);
            let bytes = compiler
                .compile(&program)
                .unwrap_or_else(|error| panic!("{source}: {error}"));
            Validator::new().validate_all(&bytes).unwrap();
            if level != OptimizationLevel::None {
                if let Some(expected) = &enabled {
                    assert_eq!(&bytes, expected, "{source}, {level:?}");
                } else {
                    enabled = Some(bytes);
                }
            }
        }
    }
}

#[test]
fn push_saves_validates_and_evaluates_item_once_before_the_typed_helper() {
    for (ty, literal, ordinary_helpers, consumer, width) in [
        ("number", "3", 7, None, 8),
        ("bool", "true", 12, Some(9), 1),
        ("string", "\"x\"", 11, Some(3), 4),
    ] {
        for captured in [false, true] {
            let body = if captured {
                "let grow = () => a.push(item()); grow();"
            } else {
                "a.push(item());"
            };
            let source = format!(
                "fn item() -> {ty} {{ return {literal}; }} fn main(a: {ty}[]) {{ {body} }}"
            );
            let mut enabled = None;
            for level in levels() {
                let mut compiler = Compiler::new();
                compiler.set_optimization_level(level);
                let bytes = compiler.compile_source(&source).unwrap();
                let bodies = operators(&bytes);
                // Independent layout: two users, ordinary helpers, optional callable,
                // seven allocator helpers, lookup, authority validation, reserve, push.
                let callable = 2 + ordinary_helpers;
                let first = callable + usize::from(captured);
                let validate = (first + 8) as u32;
                let reserve = (first + 9) as u32;
                let push = (first + 10) as u32;
                assert_eq!(bodies.len(), first + 11, "{ty}, captured={captured}");
                let body = &bodies[if captured { callable } else { 1 }];
                let body_calls = calls(body);
                assert_eq!(&body_calls[body_calls.len() - 3..], [validate, 0, push]);
                for callee in [validate, 0, push] {
                    assert_eq!(
                        body_calls.iter().filter(|&&index| index == callee).count(),
                        1
                    );
                }
                let validation = body.iter().position(|op| matches!(op, Operator::Call { function_index } if *function_index == validate)).unwrap();
                let saved = match &body[validation - 2..validation] {
                    [
                        Operator::LocalSet { local_index: saved },
                        Operator::LocalGet { local_index: read },
                    ] if saved == read => *saved,
                    _ => panic!("receiver must be saved and reloaded before authority validation"),
                };
                assert!(
                    matches!(&body[validation + 1..validation + 5], [Operator::Drop, Operator::LocalGet { local_index }, Operator::Call { function_index: 0 }, Operator::Call { function_index }] if *local_index == saved && *function_index == push)
                );
                if captured {
                    assert!(
                        matches!(&body[validation - 3], Operator::I32Load { memarg } if memarg.offset == 0)
                    );
                }

                let helper = &bodies[push as usize];
                let mut expected = vec![validate];
                expected.extend(consumer);
                expected.push(reserve);
                assert_eq!(
                    calls(helper),
                    expected,
                    "{ty}: authority, consumed item, shared reserve"
                );
                assert!(
                    matches!(&helper[..3], [Operator::LocalGet { local_index: 0 }, Operator::Call { function_index }, Operator::LocalSet { local_index: 2 }] if *function_index == validate)
                );
                if let Some(consumer) = consumer {
                    assert!(
                        matches!(&helper[3..6], [Operator::LocalGet { local_index: 1 }, Operator::Call { function_index }, Operator::Drop] if *function_index == consumer)
                    );
                }
                let growth = helper.iter().position(|op| matches!(op, Operator::Call { function_index } if *function_index == reserve)).unwrap();
                assert!(
                    matches!(&helper[growth - 2..growth], [Operator::LocalGet { local_index: 0 }, Operator::F64Const { value }] if value.bits() == 1.0_f64.to_bits())
                );
                // Current private length/payload are read only after shared reserve.
                assert!(matches!(&helper[growth + 1..growth + 10], [
                    Operator::LocalGet { local_index: 2 }, Operator::I32Load { memarg: length }, Operator::LocalSet { local_index: 3 },
                    Operator::LocalGet { local_index: 2 }, Operator::I32Load { memarg: payload }, Operator::LocalGet { local_index: 3 },
                    Operator::I32Const { value }, Operator::I32Mul, Operator::I32Add
                ] if length.offset == 12 && payload.offset == 24 && *value == width));
                assert!(matches!(
                    helper[growth + 10],
                    Operator::LocalGet { local_index: 1 }
                ));
                match &helper[growth + 11] {
                    Operator::F64Store { memarg } if ty == "number" => {
                        assert_eq!((memarg.offset, memarg.align), (0, 3))
                    }
                    Operator::I32Store8 { memarg } if ty == "bool" => {
                        assert_eq!((memarg.offset, memarg.align), (0, 0))
                    }
                    Operator::I32Store { memarg } if ty == "string" => {
                        assert_eq!((memarg.offset, memarg.align), (0, 2))
                    }
                    op => panic!("wrong {ty} slot store: {op:?}"),
                }
                assert!(matches!(&helper[growth + 12..], [
                    Operator::LocalGet { local_index: 0 }, Operator::LocalGet { local_index: 3 }, Operator::I32Const { value: 1 }, Operator::I32Add, Operator::I32Store { memarg: public },
                    Operator::LocalGet { local_index: 2 }, Operator::LocalGet { local_index: 3 }, Operator::I32Const { value: 1 }, Operator::I32Add, Operator::I32Store { memarg: private }, Operator::End
                ] if public.offset == 4 && private.offset == 12));
                if level != OptimizationLevel::None {
                    if let Some(expected) = &enabled {
                        assert_eq!(&bytes, expected, "{ty}, {captured}, {level:?}");
                    } else {
                        enabled = Some(bytes);
                    }
                }
            }
        }
    }
}

#[test]
fn forged_reserve_checks_operand_shapes_types_and_result_even_when_unexported() {
    for case in 0..18 {
        let mut program = Compiler::new()
            .analyze("fn main(a: bool[]) { a.reserve(1); }")
            .unwrap();
        let span = program.functions[0].body[0].span;
        let value = expression(&mut program);
        let TypedExprKind::Reserve {
            receiver,
            additional,
        } = &mut value.kind
        else {
            panic!("reserve")
        };
        match case {
            0 => receiver.ty = Type::Number.into(),
            1 => receiver.ty = Type::Array(ArrayElementType::String).into(),
            2 => receiver.ty = ExprResult::Never,
            3 => receiver.ty = ExprResult::Void,
            4 => receiver.ty = ExprResult::Value(Type::Void),
            5 => receiver.kind = TypedExprKind::Array(vec![], ArrayElementType::Bool),
            6 => **receiver = constructor((**receiver).clone()),
            7 => {
                let mut other = Compiler::new()
                    .analyze("fn main(a: bool[]) -> bool[] { return { a }; }")
                    .unwrap();
                **receiver = result(&mut other).clone();
            }
            8 => additional.ty = Type::Bool.into(),
            9 => additional.ty = ExprResult::Never,
            10 => additional.ty = ExprResult::Void,
            11 => additional.ty = ExprResult::Value(Type::Void),
            12 => additional.kind = TypedExprKind::Bool(true),
            13 => value.ty = Type::Number.into(),
            14 => value.ty = ExprResult::Never,
            15 => {
                let mut other = Compiler::new()
                    .analyze("fn main(a: bool[]) -> bool[] { return if true { a } else { a }; }")
                    .unwrap();
                **receiver = result(&mut other).clone();
            }
            16 => receiver.kind = TypedExprKind::Number("1".into()),
            17 => value.ty = ExprResult::Value(Type::Void),
            _ => unreachable!(),
        }
        let expected = match case {
            0 | 1 | 3 | 8 | 10 | 12 | 13 => {
                let (expected, found) = match case {
                    0 => (Type::Array(ArrayElementType::Number), Type::Number),
                    1 => (
                        Type::Array(ArrayElementType::Bool),
                        Type::Array(ArrayElementType::String),
                    ),
                    3 => (Type::Array(ArrayElementType::Number), Type::Void),
                    8 => (Type::Number, Type::Bool),
                    10 => (Type::Number, Type::Void),
                    12 => (Type::Bool, Type::Number),
                    13 => (Type::Void, Type::Number),
                    _ => unreachable!(),
                };
                CodegenError::TypeMismatch {
                    function: "main".into(),
                    expected,
                    found,
                    span,
                }
            }
            5 | 6 | 7 | 15 | 16 => CodegenError::InvalidAssignmentTarget {
                function: "main".into(),
                span,
            },
            2 | 4 | 9 | 11 | 14 | 17 => {
                let (expected, found) = match case {
                    // Never operands first require a Never result on the Reserve node.
                    2 | 9 => (ExprResult::Never, ExprResult::Void),
                    4 => (
                        Type::Array(ArrayElementType::Number).into(),
                        ExprResult::Value(Type::Void),
                    ),
                    11 => (Type::Number.into(), ExprResult::Value(Type::Void)),
                    14 => (ExprResult::Void, ExprResult::Never),
                    17 => (ExprResult::Void, ExprResult::Value(Type::Void)),
                    _ => unreachable!(),
                };
                CodegenError::ExprResultMismatch {
                    function: "main".into(),
                    expected,
                    found,
                    span,
                }
            }
            _ => unreachable!(),
        };
        for level in [
            OptimizationLevel::None,
            OptimizationLevel::Basic,
            OptimizationLevel::Standard,
            OptimizationLevel::Aggressive,
        ] {
            let mut compiler = Compiler::new();
            compiler.set_optimization_level(level);
            compiler.set_exports(ExportOptions::None);
            let error = compiler
                .compile(&program)
                .expect_err(&format!("case {case}, {level:?}"));
            assert_eq!(error, expected, "case {case}, {level:?}");
        }
    }
}

#[test]
fn forged_reserve_capture_annotations_are_checked_against_original_bindings() {
    use was_compiler::typed_ast::TypedClosureBody;
    for annotation in [
        Type::Number.into(),
        Type::Array(ArrayElementType::String).into(),
        ExprResult::Never,
        ExprResult::Void,
        ExprResult::Value(Type::Void),
    ] {
        let mut program = Compiler::new()
            .analyze("fn main(a: bool[]) { let grow = () => a.reserve(1); grow(); }")
            .unwrap();
        let TypedStatementKind::Closure(closure) = &mut program.functions[0].body[0].kind else {
            panic!()
        };
        let span = closure.body_span;
        let TypedClosureBody::Expression(value) = &mut closure.body else {
            panic!()
        };
        let TypedExprKind::Reserve { receiver, .. } = &mut value.kind else {
            panic!()
        };
        assert!(matches!(receiver.kind, TypedExprKind::Captured { .. }));
        receiver.ty = annotation;
        let expected = match annotation {
            ExprResult::Value(Type::Number) | ExprResult::Void => CodegenError::TypeMismatch {
                function: "<closure>".into(),
                expected: Type::Array(ArrayElementType::Number),
                found: if annotation == ExprResult::Void {
                    Type::Void
                } else {
                    Type::Number
                },
                span,
            },
            ExprResult::Value(Type::Array(ArrayElementType::String)) => {
                CodegenError::InvalidClosure {
                    reason: "invalid capture metadata",
                    span,
                }
            }
            ExprResult::Never => CodegenError::ExprResultMismatch {
                function: "<closure>".into(),
                expected: ExprResult::Never,
                found: ExprResult::Void,
                span,
            },
            ExprResult::Value(Type::Void) => CodegenError::ExprResultMismatch {
                function: "<closure>".into(),
                expected: Type::Array(ArrayElementType::Number).into(),
                found: annotation,
                span,
            },
            _ => unreachable!(),
        };
        let mut compiler = Compiler::new();
        compiler.set_exports(ExportOptions::None);
        let error = compiler
            .compile(&program)
            .expect_err("forged captured receiver annotation");
        assert_eq!(
            error, expected,
            "captured receiver annotation {annotation:?}"
        );
    }
}

#[test]
fn forged_reserve_is_checked_in_skipped_and_unreachable_children() {
    for source in [
        "fn main(a: bool[]) { if false { a.reserve(1); } }",
        "fn main(a: bool[]) { return; a.reserve(1); }",
        "fn main(a: bool[]) -> bool { return false && { a.reserve(1); true }; }",
    ] {
        let mut program = Compiler::new().analyze(source).unwrap();
        // Locate through the source-shaped containers without relying on backend traversal.
        let statement_index = usize::from(matches!(
            program.functions[0].body[0].kind,
            TypedStatementKind::Return(None)
        ));
        let value = match &mut program.functions[0].body[statement_index].kind {
            TypedStatementKind::If { then, .. } => {
                let TypedStatementKind::Expression(value) = &mut then[0].kind else {
                    panic!()
                };
                value
            }
            TypedStatementKind::Return(Some(value)) => {
                let TypedExprKind::Binary { right, .. } = &mut value.kind else {
                    panic!()
                };
                let TypedExprKind::Block(block) = &mut right.kind else {
                    panic!()
                };
                let TypedStatementKind::Expression(value) = &mut block.statements[0].kind else {
                    panic!()
                };
                value
            }
            TypedStatementKind::Expression(value) => value,
            _ => panic!(),
        };
        let TypedExprKind::Reserve { additional, .. } = &mut value.kind else {
            panic!()
        };
        **additional = TypedExpr {
            kind: TypedExprKind::Bool(true),
            ty: Type::Bool.into(),
        };
        let mut compiler = Compiler::new();
        compiler.set_exports(ExportOptions::None);
        assert!(compiler.compile(&program).is_err(), "{source}");
    }
}

#[test]
fn forged_reserve_is_rejected_in_pure_defaults_and_skipped_contracts() {
    let mut program = Compiler::new()
        .analyze("fn target(a: bool[], n: number = 1) {} fn main(a: bool[]) { target(a); }")
        .unwrap();
    let TypedStatementKind::Expression(value) = &mut program.functions[1].body[0].kind else {
        panic!()
    };
    let TypedExprKind::NormalizedCall(sequence) = &mut value.kind else {
        panic!()
    };
    sequence.bindings[1].value = reserve(
        sequence.bindings[0].value.clone(),
        sequence.bindings[1].value.clone(),
    );
    assert!(matches!(
        codegen::compile(&program),
        Err(CodegenError::InvalidNormalizedCall { .. })
    ));

    let mut source = Compiler::new()
        .analyze("fn main(a: bool[]) -> bool { return false && { a.reserve(1); true }; }")
        .unwrap();
    let forbidden = result(&mut source).clone();
    let mut program = Compiler::new().analyze("fn main(a: bool[]) {}").unwrap();
    program.functions[0].contracts.push(TypedContract {
        span: Span::new(10, 20),
        value: forbidden,
    });
    assert!(
        matches!(codegen::compile(&program), Err(CodegenError::InvalidContractExpression { span, .. }) if span == Span::new(10, 20))
    );
}

#[test]
fn reserve_accepts_genuinely_never_additional_and_original_captured_receivers() {
    for source in [
        "fn main(a: bool[]) { a.reserve(loop {}); }",
        "fn main(a: string[]) -> number { a.reserve({ return 7; 1 }); }",
        "fn main(a: number[]) { let grow = () => a.reserve(1); grow(); }",
        "fn main(a: bool[]) { let outer = () => { let inner = () => a.reserve(1); inner(); }; outer(); }",
    ] {
        let program = Compiler::new().analyze(source).unwrap();
        let mut compiler = Compiler::new();
        compiler.set_exports(ExportOptions::None);
        let bytes = compiler
            .compile(&program)
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        Validator::new().validate_all(&bytes).unwrap();
    }
}

#[test]
fn reserve_captures_and_validates_receiver_before_argument_then_calls_reserve_once() {
    for captured in [false, true] {
        let body = if captured {
            "let grow = () => a.reserve(amount()); grow();"
        } else {
            "a.reserve(amount());"
        };
        let source =
            format!("fn amount() -> number {{ return 3; }} fn main(a: number[]) {{ {body} }}");
        let bytes = Compiler::new().compile_source(&source).unwrap();
        let bodies = operators(&bytes);
        assert_eq!(bodies.len(), if captured { 20 } else { 19 });
        // Two users and seven ordinary helpers precede the callable. M4 helpers
        // follow it: core 10..=16, lookup 17, validate 18, reserve 19.
        let body = if captured { &bodies[9] } else { &bodies[1] };
        let validate_index = if captured { 18 } else { 17 };
        let reserve_index = if captured { 19 } else { 18 };
        assert_eq!(
            calls(body),
            if captured {
                vec![18, 0, 19]
            } else {
                vec![4, 17, 0, 18]
            }
        );
        let validate = body
            .iter()
            .position(|op| matches!(op, Operator::Call { function_index } if *function_index == validate_index))
            .unwrap();
        let argument = body
            .iter()
            .position(|op| matches!(op, Operator::Call { function_index: 0 }))
            .unwrap();
        let reserve = body
            .iter()
            .position(|op| matches!(op, Operator::Call { function_index } if *function_index == reserve_index))
            .unwrap();
        assert!(validate < argument && argument < reserve);
        let saved = body[..validate]
            .iter()
            .rev()
            .find_map(|op| match op {
                Operator::LocalSet { local_index } | Operator::LocalTee { local_index } => {
                    Some(*local_index)
                }
                _ => None,
            })
            .expect("receiver captured before validation");
        assert!(
            body[validate + 1..reserve].iter().any(
                |op| matches!(op, Operator::LocalGet { local_index } if *local_index == saved)
            )
        );
        if captured {
            assert!(
                body[..validate]
                    .iter()
                    .any(|op| matches!(op, Operator::I32Load { memarg } if memarg.offset == 0))
            );
        }
    }
}

fn first_body_locals(bytes: &[u8]) -> Vec<wasmparser::ValType> {
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::CodeSectionEntry(body) = payload.unwrap() {
            return body
                .get_locals_reader()
                .unwrap()
                .into_iter()
                .flat_map(|group| {
                    let (count, ty) = group.unwrap();
                    std::iter::repeat_n(ty, count as usize)
                })
                .collect();
        }
    }
    panic!("user body")
}

#[test]
fn pending_write_scratch_precedes_index_and_rhs_reserve_scratch() {
    use wasmparser::ValType::{F64, I32};
    let bytes = Compiler::new()
        .compile_source(
            "fn main(a: number[]) {
            a[{ a.reserve(1); 0 }] = { a.reserve(2); 3 };
            a[{ a.reserve(4); 0 }] += { a.reserve(5); 6 };
        }",
        )
        .unwrap();
    let bodies = operators(&bytes);
    assert_eq!(
        first_body_locals(&bytes),
        [
            I32, I32, F64, I32, F64, // assignment: address, handle, index, movable, value
            I32, I32, // index reserve, RHS reserve
            I32, F64, I32, F64, I32,
            F64, // compound: address, old, handle, index, movable, result
            I32, I32, // index reserve, RHS reserve
        ]
    );
    let captures: Vec<_> = bodies[0]
        .iter()
        .filter_map(|op| match op {
            Operator::LocalSet { local_index } | Operator::LocalTee { local_index }
                if [6, 7, 14, 15].contains(local_index) =>
            {
                Some(*local_index)
            }
            _ => None,
        })
        .collect();
    assert_eq!(captures, [6, 7, 14, 15]);
    let body = &bodies[0];
    let reserves: Vec<_> = body
        .iter()
        .enumerate()
        .filter_map(|(i, op)| matches!(op, Operator::Call { function_index: 17 }).then_some(i))
        .collect();
    assert_eq!(reserves.len(), 4);

    // One user, seven ordinary helpers, seven core helpers, lookup 15,
    // authority validation 16, reserve 17. Numeric checked addressing is helper 4.
    for (saved_address, handle, index, movable, value, rhs, next) in [
        (1, 2, 3, 4, 5, reserves[1], reserves[2]),
        (8, 10, 11, 12, 13, reserves[3], body.len()),
    ] {
        let capture = body
            .windows(6)
            .position(|ops| {
                matches!(ops, [
            Operator::LocalGet { local_index: captured_index },
            Operator::Call { function_index: 4 },
            Operator::LocalSet { local_index: address },
            Operator::LocalGet { local_index: captured_handle },
            Operator::Call { function_index: 15 },
            Operator::LocalSet { local_index: flag },
        ] if *captured_index == index && *address == saved_address
            && *captured_handle == handle && *flag == movable)
            })
            .expect("pre-RHS checked address and authority lookup are captured");
        assert!(capture < rhs);

        let store = body
            .windows(14)
            .position(|ops| {
                matches!(ops, [
            Operator::LocalSet { local_index: result },
            Operator::LocalGet { local_index: flag },
            Operator::If { blockty: wasmparser::BlockType::Type(wasmparser::ValType::I32) },
            Operator::LocalGet { local_index: checked_handle },
            Operator::Call { function_index: 16 },
            Operator::Drop,
            Operator::LocalGet { local_index: addressed_handle },
            Operator::LocalGet { local_index: captured_index },
            Operator::Call { function_index: 4 },
            Operator::Else,
            Operator::LocalGet { local_index: original_address },
            Operator::End,
            Operator::LocalGet { local_index: stored_value },
            Operator::F64Store { memarg },
        ] if *result == value && *flag == movable && *checked_handle == handle
            && *addressed_handle == handle && *captured_index == index
            && *original_address == saved_address && *stored_value == value
            && memarg.offset == 0)
            })
            .expect(
                "post-RHS movable validation/address resolution or fixed saved address, then store",
            );
        assert!(rhs < store && store + 13 < next);
    }
    let old = body
        .windows(4)
        .position(|ops| {
            matches!(ops, [
        Operator::LocalGet { local_index: 8 },
        Operator::F64Load { memarg },
        Operator::LocalSet { local_index: 9 },
        Operator::LocalGet { local_index: 9 },
    ] if memarg.offset == 0)
        })
        .expect("compound retains the pre-RHS old value");
    assert!(reserves[2] < old && old < reserves[3]);
    assert!(body[reserves[3] + 1..].windows(2).any(|ops| matches!(
        ops,
        [Operator::F64Add, Operator::LocalSet { local_index: 13 },]
    )));
}

#[test]
fn m4_write_value_scratch_matches_each_element_type_but_no_m4_keeps_legacy_scratch() {
    use wasmparser::ValType::{F64, I32};
    for (ty, value, stored_type) in [
        ("number", "1", F64),
        ("bool", "true", I32),
        ("string", "\"value\"", I32),
    ] {
        let source = format!(
            "fn main(a: {ty}[]) {{ a[0] = {value}; }} fn unused(a: {ty}[]) {{ resizable(a); }}"
        );
        let bytes = Compiler::new().compile_source(&source).unwrap();
        operators(&bytes);
        assert_eq!(first_body_locals(&bytes), [I32, I32, F64, I32, stored_type]);
    }
    let bytes = Compiler::new()
        .compile_source("fn main(a: number[]) { a[0] = 1; a[0] += 2; }")
        .unwrap();
    operators(&bytes);
    assert_eq!(first_body_locals(&bytes), [I32, F64]);
}

#[test]
fn no_m4_source_compilation_preserves_the_independent_emitter_golden() {
    let mut compiler = Compiler::new();
    compiler
        .register_host_function("marker", "env", "marker", vec![Type::Number], Type::Number)
        .unwrap();
    compiler
        .register_host_function("signal", "env", "signal", vec![Type::Bool], Type::Bool)
        .unwrap();
    let bytes = compiler
        .compile_source(include_str!("../../../tests/fixtures/emitter/control.was"))
        .unwrap();
    assert_eq!(
        bytes,
        include_bytes!("../../../tests/fixtures/emitter/control.wasm")
    );
}

#[test]
fn authority_lookup_checks_exact_registered_handle_and_current_metadata_without_mutation() {
    for (ty, kind, width, first, lookup_index, outer_validator) in [
        ("number", 0, 8, 8, 15, 3),
        ("bool", 1, 1, 13, 20, 9),
        ("string", 2, 4, 12, 19, 8),
    ] {
        let bytes = Compiler::new()
            .compile_source(&format!("fn main(a: {ty}[]) {{ a.reserve(0); }}"))
            .unwrap();
        let bodies = operators(&bytes);
        let lookup = &bodies[lookup_index];
        // Independent ABI expectations: follow the private registry, comparing its
        // exact handle field, rather than deriving authority from handle arithmetic.
        assert!(matches!(&lookup[..22], [
            Operator::GlobalGet { global_index: 32 },
            Operator::LocalSet { local_index: 1 },
            Operator::Block { .. }, Operator::Loop { .. },
            Operator::LocalGet { local_index: 1 }, Operator::I32Eqz,
            Operator::BrIf { relative_depth: 1 },
            Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg: handle },
            Operator::LocalGet { local_index: 0 }, Operator::I32Eq,
            Operator::If { .. },
            Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg: element },
            Operator::I32Const { value }, Operator::I32Ne,
            Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg: reserved },
            Operator::I32Or, Operator::If { .. }, Operator::Unreachable, Operator::End,
        ] if handle.offset == 4 && element.offset == 8 && *value == kind && reserved.offset == 28));
        assert!(matches!(&lookup[22..25], [
            Operator::LocalGet { local_index: 0 },
            Operator::Call { function_index }, Operator::Drop,
        ] if *function_index == outer_validator));
        let metadata = lookup.windows(33).position(|ops| matches!(ops, [
            Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg: length },
            Operator::LocalGet { local_index: 0 }, Operator::I32Load { memarg: public_length },
            Operator::I32Ne,
            Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg: payload },
            Operator::LocalGet { local_index: 0 }, Operator::I32Load { memarg: public_payload },
            Operator::I32Ne, Operator::I32Or,
            Operator::LocalGet { local_index: 1 }, Operator::I64Load { memarg: minimum },
            Operator::I64Const { value: 8 }, Operator::I64LtU, Operator::I32Or,
            Operator::LocalGet { local_index: 1 }, Operator::I64Load { memarg: maximum },
            Operator::I64Const { value: 2_147_483_648 }, Operator::I64GtU, Operator::I32Or,
            Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg: live_length },
            Operator::I64ExtendI32U, Operator::I64Const { value }, Operator::I64Mul,
            Operator::LocalGet { local_index: 1 }, Operator::I64Load { memarg: capacity },
            Operator::I64GtU, Operator::I32Or,
            Operator::If { .. }, Operator::Unreachable, Operator::End,
        ] if length.offset == 12 && public_length.offset == 4
            && payload.offset == 24 && public_payload.offset == 0
            && minimum.offset == 16 && maximum.offset == 16
            && live_length.offset == 12 && *value == width && capacity.offset == 16))
            .expect("public/private length and payload agreement, capacity bounds and live-byte fit must trap together");
        assert_eq!(metadata, 25);
        assert!(matches!(&lookup[metadata + 33..], [
            Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg: payload },
            Operator::LocalGet { local_index: 1 }, Operator::I64Load { memarg: identity },
            Operator::LocalGet { local_index: 1 }, Operator::I64Load { memarg: capacity },
            Operator::Call { function_index },
            Operator::LocalGet { local_index: 1 }, Operator::Return, Operator::End,
            Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg: next },
            Operator::LocalSet { local_index: 1 }, Operator::Br { relative_depth: 0 },
            Operator::End, Operator::End, Operator::I32Const { value: 0 }, Operator::End,
        ] if payload.offset == 24 && identity.offset == 32 && capacity.offset == 16
            && *function_index == first + 5 && next.offset == 0));
        assert_eq!(calls(lookup), [outer_validator, first + 5]);
        assert!(!lookup.iter().any(|op| matches!(
            op,
            Operator::I32Store { .. }
                | Operator::I64Store { .. }
                | Operator::I32Store8 { .. }
                | Operator::GlobalSet { .. }
                | Operator::MemoryGrow { .. }
        )));
    }
}

#[test]
fn current_acquisition_preflight_requires_exact_live_registered_payload_identity_and_extent() {
    let bytes = Compiler::new()
        .compile_source("fn main(a: number[]) { a.reserve(0); }")
        .unwrap();
    let bodies = operators(&bytes);
    // One user and seven memory helpers precede the core. Check the emitted
    // allocator contract independently, rather than trusting a call to Validate.
    let find = &bodies[10];
    assert!(matches!(find.as_slice(), [
        Operator::GlobalGet { global_index: 2 }, Operator::LocalSet { local_index: 1 },
        Operator::Block { .. }, Operator::Loop { .. },
        Operator::LocalGet { local_index: 1 }, Operator::I32Eqz,
        Operator::BrIf { relative_depth: 1 },
        Operator::LocalGet { local_index: 1 }, Operator::I32Const { value: 32 }, Operator::I32Add,
        Operator::LocalGet { local_index: 0 }, Operator::I32Eq, Operator::If { .. },
        Operator::LocalGet { local_index: 1 }, Operator::Return, Operator::End,
        Operator::LocalGet { local_index: 1 }, Operator::I32Load { memarg: next },
        Operator::LocalSet { local_index: 1 }, Operator::Br { relative_depth: 0 },
        Operator::End, Operator::End, Operator::I32Const { value: 0 }, Operator::End,
    ] if next.offset == 0));
    let validate = &bodies[13];
    assert!(matches!(validate.as_slice(), [
        Operator::LocalGet { local_index: 0 }, Operator::Call { function_index: 10 },
        Operator::LocalTee { local_index: 3 }, Operator::I32Eqz,
        Operator::If { .. }, Operator::Unreachable, Operator::End,
        Operator::LocalGet { local_index: 3 }, Operator::I64Load { memarg: identity },
        Operator::LocalGet { local_index: 1 }, Operator::I64Ne,
        Operator::LocalGet { local_index: 3 }, Operator::I32Load { memarg: active },
        Operator::I32Const { value: 1 }, Operator::I32Ne, Operator::I32Or,
        Operator::LocalGet { local_index: 3 }, Operator::I64Load { memarg: requested },
        Operator::LocalGet { local_index: 2 }, Operator::I64Ne, Operator::I32Or,
        Operator::I64Const { value: 8 },
        Operator::LocalGet { local_index: 3 }, Operator::I32Load { memarg: class },
        Operator::I64ExtendI32U, Operator::I64Shl,
        Operator::LocalGet { local_index: 2 }, Operator::I64Ne, Operator::I32Or,
        Operator::If { .. }, Operator::Unreachable, Operator::End, Operator::End,
    ] if identity.offset == 16 && active.offset == 12 && requested.offset == 24 && class.offset == 8));
    // Exhaustive sequences exclude writes, release, allocation, callbacks and an
    // early success before all four live-acquisition comparisons have completed.
}

#[test]
fn reserve_preflights_current_acquisition_before_noop_or_replacement_and_validates_new_before_copy()
{
    for (ty, first, lookup_index, validate_index, reserve_index) in [
        ("number", 8, 15, 16, 17),
        ("bool", 13, 20, 21, 22),
        ("string", 12, 19, 20, 21),
    ] {
        let bytes = Compiler::new()
            .compile_source(&format!("fn main(a: {ty}[]) {{ a.reserve(100); }}"))
            .unwrap();
        let bodies = operators(&bytes);
        // A registry miss is a trap, not fixed-storage permission or a no-op.
        assert!(matches!(bodies[validate_index].as_slice(), [
            Operator::LocalGet { local_index: 0 }, Operator::Call { function_index },
            Operator::LocalTee { local_index: 1 }, Operator::I32Eqz,
            Operator::If { .. }, Operator::Unreachable, Operator::End,
            Operator::LocalGet { local_index: 1 }, Operator::End,
        ] if *function_index == lookup_index));
        let reserve = &bodies[reserve_index];
        assert!(matches!(&reserve[..3], [
            Operator::LocalGet { local_index: 0 }, Operator::Call { function_index },
            Operator::LocalSet { local_index: 2 },
        ] if *function_index == validate_index as u32));
        let no_growth = reserve
            .windows(7)
            .position(|ops| {
                matches!(ops, [
            Operator::LocalGet { local_index: 7 },
            Operator::LocalGet { local_index: 2 }, Operator::I64Load { memarg },
            Operator::I64LeU, Operator::If { .. }, Operator::Return, Operator::End,
        ] if memarg.offset == 16)
            })
            .expect("capacity-sufficient path still follows current-state preflight");
        let acquire = reserve
            .windows(9)
            .position(|ops| {
                matches!(ops, [
            Operator::LocalGet { local_index: 7 }, Operator::Call { function_index: acquire },
            Operator::LocalSet { local_index: 9 }, Operator::LocalSet { local_index: 4 },
            Operator::LocalGet { local_index: 4 }, Operator::LocalGet { local_index: 9 },
            Operator::LocalGet { local_index: 7 }, Operator::Call { function_index: validate },
            Operator::Block { .. },
        ] if *acquire == first + 1 && *validate == first + 5)
            })
            .expect("full replacement capacity and returned identity are validated before copying");
        assert!(3 < no_growth && no_growth + 7 < acquire);
        assert!(!reserve[..acquire].iter().any(|op| matches!(
            op,
            Operator::I32Store { .. }
                | Operator::I64Store { .. }
                | Operator::I32Store8 { .. }
                | Operator::GlobalSet { .. }
                | Operator::MemoryGrow { .. }
        )));
        let first_write = reserve
            .iter()
            .position(|op| {
                matches!(
                    op,
                    Operator::I32Store { .. }
                        | Operator::I64Store { .. }
                        | Operator::I32Store8 { .. }
                )
            })
            .unwrap();
        assert!(acquire + 8 < first_write);
        assert_eq!(
            calls(reserve),
            [
                validate_index as u32,
                first,
                first + 1,
                first + 5,
                first + 6
            ]
        );
    }
}

#[test]
fn reserve_relocation_raw_copies_before_publication_and_releases_only_after() {
    for (ty, width, first, lookup, validate, reserve_index, body_count, outer_validator) in [
        ("number", 8, 8, 15, 16, 17, 18, 3),
        ("bool", 1, 13, 20, 21, 22, 23, 9),
        ("string", 4, 12, 19, 20, 21, 22, 8),
    ] {
        let bytes = Compiler::new()
            .compile_source(&format!("fn main(a: {ty}[]) {{ a.reserve(100); }}"))
            .unwrap();
        let bodies = operators(&bytes);
        assert_eq!(bodies.len(), body_count);
        assert_eq!(
            calls(&bodies[0]),
            [outer_validator, validate, reserve_index as u32]
        );
        assert_eq!(calls(&bodies[lookup]), [outer_validator, first + 5]);
        assert_eq!(calls(&bodies[validate as usize]), [lookup as u32]);
        let body = &bodies[reserve_index];
        assert_eq!(
            calls(body),
            [validate, first, first + 1, first + 5, first + 6]
        );
        // Copy the logical byte length, not capacity or a consumed-value scan.
        assert!(body.windows(6).any(|ops| matches!(ops, [
            Operator::LocalGet { local_index: 2 },
            Operator::I32Load { memarg },
            Operator::I64ExtendI32U,
            Operator::I64Const { value },
            Operator::I64Mul,
            Operator::LocalSet { local_index: 6 },
        ] if memarg.offset == 12 && *value == width)));
        assert!(body.windows(3).any(|ops| matches!(ops, [
            Operator::LocalGet { local_index: 2 },
            Operator::I32Load { memarg },
            Operator::LocalSet { local_index: 3 },
        ] if memarg.offset == 24)));
        let copy = body
            .windows(22)
            .position(|ops| {
                matches!(ops, [
            Operator::Block { .. },
            Operator::Loop { .. },
            Operator::LocalGet { local_index: 5 },
            Operator::I64ExtendI32U,
            Operator::LocalGet { local_index: 6 },
            Operator::I64GeU,
            Operator::BrIf { relative_depth: 1 },
            Operator::LocalGet { local_index: 4 },
            Operator::LocalGet { local_index: 5 },
            Operator::I32Add,
            Operator::LocalGet { local_index: 3 },
            Operator::LocalGet { local_index: 5 },
            Operator::I32Add,
            Operator::I32Load8U { memarg: load },
            Operator::I32Store8 { memarg: store },
            Operator::LocalGet { local_index: 5 },
            Operator::I32Const { value: 1 },
            Operator::I32Add,
            Operator::LocalSet { local_index: 5 },
            Operator::Br { relative_depth: 0 },
            Operator::End,
            Operator::End,
        ] if load.offset == 0 && load.align == 0 && store.offset == 0 && store.align == 0)
            })
            .expect("bounded raw byte copy from old payload to new payload");
        let copy_end = copy + 21;
        assert_eq!(
            body.iter()
                .filter(|op| matches!(op, Operator::I32Load8U { .. }))
                .count(),
            1
        );
        assert_eq!(
            body.iter()
                .filter(|op| matches!(op, Operator::I32Store8 { .. }))
                .count(),
            1
        );
        let stores: Vec<_> = body
            .iter()
            .enumerate()
            .filter_map(|(i, op)| match op {
                Operator::I32Store { memarg } => Some((i, memarg.offset)),
                Operator::I64Store { memarg } => Some((i, memarg.offset)),
                _ => None,
            })
            .collect();
        assert_eq!(
            stores.iter().map(|(_, offset)| *offset).collect::<Vec<_>>(),
            [0, 24, 16, 32]
        );
        let release = body.iter().position(|op| matches!(op, Operator::Call { function_index } if *function_index == first + 6)).unwrap();
        assert!(stores.iter().all(|(i, _)| copy_end < *i && *i < release));
        assert!(
            body[copy + 1..release]
                .iter()
                .all(|op| !matches!(op, Operator::Call { .. }))
        );
        assert!(!body.iter().any(|op| matches!(op, Operator::F64Load { .. })));
        // The complete call list above excludes bool/string consumed validators;
        // the byte load feeds its store directly, without interpreting the value.
    }
}

#[test]
fn source_runs_once_and_ordinary_literals_keep_fixed_storage_in_an_m4_module() {
    let bytes = Compiler::new().compile_source("fn source() -> number[] { return [1]; } fn main() -> number[] { return resizable(source()); }").unwrap();
    let bodies = operators(&bytes);
    // Two users, seven memory helpers, seven core helpers, one constructor,
    // then the numeric authority lookup/validate pair.
    assert_eq!(bodies.len(), 19);
    assert_eq!(calls(&bodies[1]), [0, 16]);
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
    // Three users, sixteen memory helpers, seven core helpers, three constructors,
    // then number/bool/string lookup/validate pairs. Type dedup never shifts functions.
    assert_eq!(functions.len(), 35);
    let expected = [
        (vec![I64], vec![I32]),
        (vec![I64], vec![I32, I64]),
        (vec![I32], vec![I32]),
        (vec![I32], vec![I32]),
        (vec![I32, I32], vec![]),
        (vec![I32, I64, I64], vec![]),
        (vec![I32, I64], vec![]),
        (vec![I32], vec![I32]),
        (vec![I32], vec![I32]),
        (vec![I32], vec![I32]),
    ];
    assert_eq!(expected.len(), 10);
    for (&type_index, (params, results)) in functions[19..29].iter().zip(expected) {
        let signature = &signatures[type_index as usize];
        assert_eq!(signature.params(), params);
        assert_eq!(signature.results(), results);
    }
    for slot in [21, 22, 26, 27, 28, 29, 30, 31, 32, 33, 34] {
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
    for (ty, width, first, copy_index, body_count, validator, consumer, kind) in [
        ("number", 8, 8, 15, 18, 3, None, 0),
        ("bool", 1, 13, 20, 23, 9, Some(8), 1),
        ("string", 4, 12, 19, 22, 8, Some(2), 2),
    ] {
        let bytes = Compiler::new()
            .compile_source(&format!(
                "fn main(a: {ty}[]) -> {ty}[] {{ return resizable(a); }}"
            ))
            .unwrap();
        let bodies = operators(&bytes);
        assert_eq!(bodies.len(), body_count);
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
