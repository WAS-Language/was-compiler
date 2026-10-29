use was_compiler::host::HostRegistrationError;
use was_compiler::semantic::SemanticError;
use was_compiler::typed_ast::{BuiltinFunction, FunctionRef, TypedExprKind, TypedStatementKind};
use was_compiler::{ArrayElementType, CompileError, Compiler, Type};

#[test]
fn pop_and_remove_return_each_exact_receiver_element_type() {
    for (array, scalar) in [
        ("number[]", Type::Number),
        ("bool[]", Type::Bool),
        ("string[]", Type::String),
    ] {
        for method in ["pop()", "remove(-0)"] {
            let source = format!("fn main(a: {array}) {{ return a.{method}; }}");
            let program = Compiler::new().analyze(&source).unwrap();
            assert_eq!(program.functions[0].return_type, scalar);
        }
    }
}

#[test]
fn pop_remove_check_structure_receiver_and_index_in_order() {
    for (method, expected, args, found) in [
        ("pop", 0, "missing", 1),
        ("pop", 0, "1, 2", 2),
        ("remove", 1, "", 0),
        ("remove", 1, "missing, other", 2),
    ] {
        let source = format!("fn main() {{ unknown.{method}({args}); }}");
        assert!(
            matches!(semantic_error(&source), SemanticError::ArgumentCountMismatch { name, expected: e, found: f, .. } if name == method && e == expected && f == found)
        );
    }
    for method in ["pop", "remove"] {
        let source = format!("fn main() {{ unknown.{method}(index: missing); }}");
        assert!(
            matches!(semantic_error(&source), SemanticError::NamedArgumentsNotSupported { name, .. } if name == method)
        );
        for ty in ["number", "bool", "string", "range"] {
            let args = if method == "pop" { "" } else { "missing" };
            let source = format!("fn main(a: {ty}) {{ a.{method}({args}); }}");
            assert!(
                matches!(
                    semantic_error(&source),
                    SemanticError::TypeMismatch {
                        expected: Type::Array(_),
                        ..
                    }
                ),
                "{source}"
            );
        }
    }
    for index in ["true", "\"x\"", "[]", "0..1", "empty()"] {
        let source = format!("fn empty() {{}} fn main(a: bool[]) {{ a.remove({index}); }}");
        assert!(
            matches!(
                semantic_error(&source),
                SemanticError::TypeMismatch {
                    expected: Type::Number,
                    ..
                }
            ),
            "{source}"
        );
    }
}

#[test]
fn pop_remove_keep_identifier_only_contextual_method_syntax_and_purity() {
    for method in ["pop()", "remove(0)"] {
        for expression in [
            format!("(a).{method}"),
            format!("make().{method}"),
            format!("a[0].{method}"),
            format!("[1].{method}"),
            format!("a.{method}.pop()"),
            format!("a.{method}[0]"),
            format!("a.{method}()"),
            format!("1 |> a.{method}"),
        ] {
            let source = format!("fn main(a: number[]) {{ {expression}; }}");
            assert!(Compiler::new().analyze(&source).is_err(), "{source}");
        }
        for source in [
            format!("fn main(a: number[], n: bool = false && a.{method} == 1) {{}}"),
            format!("fn main(a: number[]) requires true || a.{method} == 1 {{}}"),
        ] {
            assert!(matches!(
                semantic_error(&source),
                SemanticError::InvalidDefaultExpression { .. }
                    | SemanticError::InvalidContractExpression { .. }
            ));
        }
    }
    assert!(
        matches!(semantic_error("fn pop() -> number { return 0; } fn main(pop: number[]) { pop.pop(); pop.remove(pop()); }"), SemanticError::NotCallable { name, .. } if name == "pop")
    );
    Compiler::new().analyze("fn pop() -> number { return 0; } fn remove() -> number { return 0; } fn main(a: number[]) { a /* λ */ . pop (); a.remove(remove()); }").unwrap();
    let mut compiler = Compiler::new();
    for name in ["pop", "remove"] {
        compiler
            .register_host_function(name, "env", name, vec![], Type::Number)
            .unwrap();
    }
    compiler
        .analyze("fn main(a: number[]) { a.remove(pop()); return a.pop() + remove(); }")
        .unwrap();
}

#[test]
fn pop_remove_raw_nodes_preserve_utf8_spans_and_positional_arguments() {
    use was_compiler::{
        ast::{Expr, StatementKind},
        lexer::Lexer,
        parser::Parser,
        tokens::Span,
    };
    let source = "fn main(a: string[]) { /* 雪 */ a.pop(); a.remove(-0); }";
    let tokens = Lexer::new(source).tokenize();
    let program = Parser::new(&tokens).parse_program().unwrap();
    for (statement, method) in program.functions[0]
        .body
        .iter()
        .zip(["pop()", "remove(-0)"])
    {
        let StatementKind::Expression(expression) = &statement.kind else {
            panic!()
        };
        let (receiver, args, span) = match expression {
            Expr::Pop {
                receiver,
                args,
                span,
            }
            | Expr::Remove {
                receiver,
                args,
                span,
            } => (receiver, args, span),
            _ => panic!("expected dedicated array method"),
        };
        assert_eq!(receiver.as_ref(), &Expr::Identifier("a".into()));
        assert_eq!(args.len(), usize::from(method.starts_with("remove")));
        assert!(args.iter().all(|arg| arg.name.is_none()));
        let start = source.find(&format!("a.{method}")).unwrap();
        assert_eq!(*span, Span::new(start, start + 2 + method.len()));
    }
}

#[test]
fn pop_remove_placeholder_results_and_indices_use_exact_region_local_evidence() {
    for (array, scalar, item) in [
        ("number[]", Type::Number, "1"),
        ("bool[]", Type::Bool, "true"),
        ("string[]", Type::String, "\"x\""),
    ] {
        let source = format!("fn main(a: {array}) {{ let take = a.remove(_); take(0); }}");
        let program = Compiler::new().analyze(&source).unwrap();
        let TypedStatementKind::Closure(closure) = &program.functions[0].body[0].kind else {
            panic!()
        };
        assert_eq!(closure.params[0].ty, Type::Number);
        assert_eq!(closure.return_type, scalar);
        for method in ["pop()", "remove(0)"] {
            let source = format!(
                "fn main() {{ let take = {{ let a = _; a.{method} == {item} }}; take([{item}]); }}"
            );
            let program = Compiler::new().analyze(&source).unwrap();
            let TypedStatementKind::Closure(closure) = &program.functions[0].body[0].kind else {
                panic!()
            };
            assert_eq!(
                closure.params[0].ty,
                Type::Array(match scalar {
                    Type::Number => ArrayElementType::Number,
                    Type::Bool => ArrayElementType::Bool,
                    Type::String => ArrayElementType::String,
                    _ => unreachable!(),
                })
            );
            assert_eq!(closure.return_type, Type::Bool);
        }
    }
    for source in [
        "fn main() { let take = { let a = _; a.pop() }; take([true]); }",
        "fn main() { let take = { let a = _; a.remove(_) }; take([true], 0); }",
        "fn main() { let take = { let a = _; return; a.pop() == true }; take([true]); }",
        "fn main() { let take = { let a = _; let inner = () => a.pop() == true; inner() }; take([true]); }",
    ] {
        assert!(
            matches!(
                semantic_error(source),
                SemanticError::CannotInferPlaceholderParameter { .. }
            ),
            "{source}"
        );
    }
    Compiler::new().analyze("fn main() { let take = { let a = _; let v = a.pop(); v == true }; take([true]); let later = { let a = _; let index = _; let v = a.remove(index); a == [\"x\"]; v }; later([\"y\"], 0); }").unwrap();
}

#[test]
fn pop_remove_preserve_never_skipped_checks_captures_and_result_dependencies() {
    use was_compiler::typed_ast::{ExprResult, TypedClosureBody};
    for array in ["number[]", "bool[]", "string[]"] {
        let source = format!("fn main(a: {array}) -> number {{ a.remove({{ return 7; true }}); }}");
        let program = Compiler::new().analyze(&source).unwrap();
        let TypedStatementKind::Expression(value) = &program.functions[0].body[0].kind else {
            panic!()
        };
        assert_eq!(value.ty, ExprResult::Never);
    }
    for body in [
        "return; a.remove(true);",
        "false && a.remove(true) == 0;",
        "a.remove({ return; missing });",
        "if false { missing.pop(); }",
    ] {
        assert!(
            Compiler::new()
                .analyze(&format!("fn main(a: number[]) {{ {body} }}"))
                .is_err()
        );
    }
    let program = Compiler::new().analyze("fn main(a: string[]) { let outer = () => { let inner = () => a.remove(index()); inner() }; return outer(); } fn index() { return 0; }").unwrap();
    assert_eq!(program.functions[0].return_type, Type::String);
    assert_eq!(program.functions[1].return_type, Type::Number);
    let TypedStatementKind::Closure(outer) = &program.functions[0].body[0].kind else {
        panic!()
    };
    let TypedClosureBody::Block(block) = &outer.body else {
        panic!()
    };
    let TypedStatementKind::Closure(inner) = &block.statements[0].kind else {
        panic!()
    };
    assert_eq!(outer.captures, inner.captures);
    let TypedClosureBody::Expression(value) = &inner.body else {
        panic!()
    };
    let TypedExprKind::Remove { receiver, .. } = &value.kind else {
        panic!()
    };
    assert!(matches!(receiver.kind, TypedExprKind::Captured { .. }));
}

#[test]
fn push_raw_syntax_retains_receiver_argument_and_utf8_method_span() {
    use was_compiler::{
        ast::{Expr, StatementKind},
        lexer::Lexer,
        parser::Parser,
        tokens::Span,
    };
    let source = "fn main(a: string[]) { /* λ */ a.push(\"雪\"); }";
    let tokens = Lexer::new(source).tokenize();
    let program = Parser::new(&tokens).parse_program().unwrap();
    let StatementKind::Expression(Expr::Push {
        receiver,
        args,
        span,
    }) = &program.functions[0].body[0].kind
    else {
        panic!("expected dedicated raw push")
    };
    assert_eq!(receiver.as_ref(), &Expr::Identifier("a".into()));
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, None);
    assert_eq!(args[0].value, Expr::String("雪".into()));
    let start = source.find("a.push").unwrap();
    assert_eq!(*span, Span::new(start, start + "a.push(\"雪\")".len()));
}

#[test]
fn push_accepts_each_exact_array_element_with_a_void_result() {
    for (ty, item) in [
        ("number[]", "3"),
        ("bool[]", "true"),
        ("string[]", "\"hello\""),
    ] {
        let source = format!("fn main(values: {ty}) {{ values.push({item}); }}");
        let program = Compiler::new().analyze(&source).unwrap();
        let TypedStatementKind::Expression(value) = &program.functions[0].body[0].kind else {
            panic!()
        };
        assert_eq!(value.ty, Type::Void);
        let TypedExprKind::Push { receiver, item } = &value.kind else {
            panic!("expected push operation")
        };
        let Type::Array(element) = receiver.ty.value_type().unwrap() else {
            panic!("expected array receiver")
        };
        assert_eq!(item.ty, element.value_type());
        assert!(matches!(receiver.kind, TypedExprKind::Identifier { .. }));
    }
}

#[test]
fn push_placeholder_types_follow_the_exact_region_local_array_element_relationship() {
    for (array, scalar, item) in [
        ("number[]", Type::Number, "1"),
        ("bool[]", Type::Bool, "true"),
        ("string[]", Type::String, "\"x\""),
    ] {
        let source = format!("fn main(a: {array}) {{ let append = a.push(_); append({item}); }}");
        let program = Compiler::new().analyze(&source).unwrap();
        let TypedStatementKind::Closure(closure) = &program.functions[0].body[0].kind else {
            panic!()
        };
        assert_eq!(closure.params[0].ty, scalar);
        assert_eq!(closure.return_type, Type::Void);
        let source = format!(
            "fn main() {{ let append = {{ let a = _; a.push({item}); }}; append([{item}]); }}"
        );
        let program = Compiler::new().analyze(&source).unwrap();
        let TypedStatementKind::Closure(closure) = &program.functions[0].body[0].kind else {
            panic!()
        };
        let element = match scalar {
            Type::Number => ArrayElementType::Number,
            Type::Bool => ArrayElementType::Bool,
            Type::String => ArrayElementType::String,
            _ => unreachable!(),
        };
        assert_eq!(closure.params[0].ty, Type::Array(element));
    }
}

#[test]
fn push_requires_identifier_receivers_and_does_not_add_chaining_or_method_pipeline_stages() {
    for expression in [
        "(a).push(1)",
        "make().push(1)",
        "a[0].push(1)",
        "[1].push(1)",
        "a.push(1).push(2)",
        "a.reserve(1).push(2)",
        "a.push(1)[0]",
        "a.push(1)()",
        "a.push",
        "a.push(,)",
        "1 |> a.push(1)",
        "a |> push(1)",
    ] {
        let source = format!("fn main(a: number[]) {{ {expression}; }}");
        assert!(Compiler::new().analyze(&source).is_err(), "{source}");
    }
    Compiler::new().analyze("fn main(a: number[]) { a /* receiver */ . /* method */ push /* item */ (1); 0..1; 0..=1; 1.5; }").unwrap();
}

#[test]
fn push_is_contextual_without_callable_resolution_or_new_pipeline_discard_permissions() {
    assert!(
        matches!(semantic_error("fn push(n: number) -> number { return n; } fn main(push: number[]) { push.push(push(1)); }"), SemanticError::NotCallable { name, .. } if name == "push")
    );
    Compiler::new().analyze("fn push(n: number) -> number { return n; } fn main(a: number[]) { a.push(push(1)); let push = (a: bool[]) => a.push(true); [true] |> push; }").unwrap();
    let mut compiler = Compiler::new();
    compiler
        .register_host_function("push", "env", "push", vec![], Type::String)
        .unwrap();
    compiler
        .analyze("fn main(a: string[]) { a.push(push()); }")
        .unwrap();
    assert!(matches!(
        semantic_error("fn stage(n: number) {} fn main(a: number[]) { a.push(1 |> stage); }"),
        SemanticError::InvalidPipeline { .. }
    ));
}

#[test]
fn push_checks_call_structure_before_receiver_and_item() {
    for (args, count) in [("", 0), ("missing, other", 2)] {
        let source = format!("fn main() {{ unknown.push({args}); }}");
        assert!(
            matches!(semantic_error(&source), SemanticError::ArgumentCountMismatch { name, expected: 1, found, .. } if name == "push" && found == count)
        );
    }
    for args in ["item: missing", "1, item: missing", "item: 1, 2"] {
        let source = format!("fn main() {{ unknown.push({args}); }}");
        assert!(
            matches!(semantic_error(&source), SemanticError::NamedArgumentsNotSupported { name, .. } if name == "push")
        );
    }
}

#[test]
fn push_checks_receiver_then_exact_scalar_without_proving_runtime_authority() {
    for ty in ["number", "bool", "string", "range"] {
        let source = format!("fn main(a: {ty}) {{ a.push(missing); }}");
        assert!(
            matches!(
                semantic_error(&source),
                SemanticError::TypeMismatch {
                    expected: Type::Array(_),
                    ..
                }
            ),
            "{source}"
        );
    }
    for (array, expected, wrong) in [
        ("number[]", Type::Number, "true"),
        ("bool[]", Type::Bool, "1"),
        ("string[]", Type::String, "false"),
    ] {
        for item in [wrong, "[]", "0..1", "empty()"] {
            let source = format!("fn empty() {{}} fn main(a: {array}) {{ a.push({item}); }}");
            assert!(
                matches!(semantic_error(&source), SemanticError::TypeMismatch { expected: found, .. } if found == expected),
                "{source}"
            );
        }
    }
    Compiler::new().analyze("fn main() { let a = [1]; a.push(-0); a.push(0 / 0); a.push(1 / 0); let b = [true]; b.push(false); let s = [\"x\"]; s.push(\"λ\"); }").unwrap();
    assert!(matches!(
        semantic_error("fn main(a: number[]) { let result = a.push(1); }"),
        SemanticError::InvalidVoidType { .. }
    ));
}

#[test]
fn push_preserves_never_and_checks_every_unreachable_or_skipped_child() {
    use was_compiler::typed_ast::ExprResult;
    for (array, tail) in [("number[]", "true"), ("bool[]", "1"), ("string[]", "false")] {
        let source = format!("fn main(a: {array}) -> number {{ a.push({{ return 7; {tail} }}); }}");
        let program = Compiler::new().analyze(&source).unwrap();
        let TypedStatementKind::Expression(value) = &program.functions[0].body[0].kind else {
            panic!()
        };
        assert_eq!(value.ty, ExprResult::Never);
    }
    for body in [
        "a.push({ return; let invalid: number = true; 1 });",
        "return; a.push(true);",
        "false && { a.push(true); true };",
        "if false { a.push(true); }",
        "a.push({ return; missing });",
    ] {
        let source = format!("fn main(a: number[]) {{ {body} }}");
        assert!(Compiler::new().analyze(&source).is_err(), "{source}");
    }
}

#[test]
fn push_is_forbidden_in_pure_defaults_and_contracts_including_skipped_operands() {
    for source in [
        "fn main(a: number[], n: bool = false && a.push(1)) {}",
        "fn main(a: bool[]) requires true || a.push(true) {}",
    ] {
        assert!(matches!(
            semantic_error(source),
            SemanticError::InvalidDefaultExpression { .. }
                | SemanticError::InvalidContractExpression { .. }
        ));
    }
}

#[test]
fn push_item_participates_in_named_result_dependencies_and_nested_capture_discovery() {
    use was_compiler::typed_ast::TypedClosureBody;
    let program = Compiler::new().analyze("fn main(a: string[]) { a.push(item()); let outer = () => { let inner = () => a.push({ let make = () => \"x\"; make() }); inner(); }; outer(); } fn item() { return \"y\"; }").unwrap();
    assert_eq!(program.functions[1].return_type, Type::String);
    let TypedStatementKind::Closure(outer) = &program.functions[0].body[1].kind else {
        panic!()
    };
    let TypedClosureBody::Block(block) = &outer.body else {
        panic!()
    };
    let TypedStatementKind::Closure(inner) = &block.statements[0].kind else {
        panic!()
    };
    assert_eq!(inner.captures, outer.captures);
    assert_eq!(outer.captures.len(), 1);
    let TypedClosureBody::Expression(value) = &inner.body else {
        panic!()
    };
    let TypedExprKind::Push { receiver, .. } = &value.kind else {
        panic!()
    };
    assert!(
        matches!(receiver.kind, TypedExprKind::Captured { owner, id, .. } if owner == outer.captures[0].owner && id == outer.captures[0].variable)
    );
}

#[test]
fn push_placeholder_evidence_does_not_come_from_callers_or_unreachable_items() {
    for source in [
        "fn main() { let append = { let a = _; a.push(_); }; append([true], true); }",
        "fn main() { let append = { let a = _; a.push(_); }; true |> append(true); }",
        "fn main(a: bool[]) { let append = { return; a.push(_); }; append(true); }",
        "fn main() { let append = { let a = _; return; a.push(true); }; append([true]); }",
        "fn main() { let append = { let a = _; a.push({ return; true }); }; append([true]); }",
    ] {
        assert!(
            matches!(
                semantic_error(source),
                SemanticError::CannotInferPlaceholderParameter { .. }
            ),
            "{source}"
        );
    }
    for source in [
        "fn main() { let append = { let a = _; a.push(true); a.push(1); }; }",
        "fn main() { let append = { let a = _; a + 1; a.push(true); }; }",
    ] {
        assert!(
            matches!(
                semantic_error(source),
                SemanticError::ConflictingPlaceholderConstraints { .. }
            ),
            "{source}"
        );
    }
}

#[test]
fn push_placeholder_relationship_propagates_delayed_evidence_and_preserves_region_boundaries() {
    for source in [
        "fn main() { let append = { let a = _; a.push(_); a == [true]; }; append([false], true); }",
        "fn main() { let append = { let a = _; a.push(_); a == [\"x\"]; }; append([\"y\"], \"z\"); }",
        "fn main() { let append = { let a = _; false && { a.push(true); true }; }; append([true]); }",
        "fn main() { let append = { let a = _; a.push(true); return; }; append([true]); }",
        "fn main(a: bool[]) { true |> (a.push(_)); }",
        "fn main() { let append = { let a = _; a.push(true); let inner = () => a.push(false); inner(); }; append([true]); }",
    ] {
        Compiler::new()
            .analyze(source)
            .unwrap_or_else(|error| panic!("{source}: {error:?}"));
    }
    assert!(matches!(
        semantic_error(
            "fn main() { let append = { let a = _; let inner = () => a.push(true); inner(); }; append([true]); }"
        ),
        SemanticError::CannotInferPlaceholderParameter { .. }
    ));
    assert!(matches!(
        semantic_error("fn main(a: bool[]) { let append = { a.push(_); return; a.push(1); }; }"),
        SemanticError::TypeMismatch {
            expected: Type::Bool,
            found: Type::Number,
            ..
        }
    ));
}

#[test]
fn push_diagnostics_preserve_utf8_spans_and_optimization_parity() {
    use was_compiler::{OptimizationLevel, tokens::Span};
    let source = "fn main(a: bool[]) { /* λ */ a.push(1); missing; }";
    let start = source.find("a.push").unwrap();
    let expected = CompileError::Semantic(SemanticError::TypeMismatch {
        expected: Type::Bool,
        found: Type::Number,
        span: Span::new(start, start + "a.push(1);".len()),
    });
    let mut compiler = Compiler::new();
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        compiler.set_optimization_level(level);
        assert_eq!(compiler.analyze(source).unwrap_err(), expected);
        compiler
            .analyze("fn main(a: bool[]) { a.push(true); }")
            .unwrap();
    }
    let source = "fn main(a: number[]) { /* λ */ a.push(item: 1); }";
    let start = source.find("item:").unwrap();
    assert_eq!(
        semantic_error(source).span(),
        Some(Span::new(start, start + "item: 1".len()))
    );
}

#[test]
fn reserve_accepts_each_array_kind_and_fixed_storage_with_a_void_result() {
    for ty in ["number[]", "bool[]", "string[]"] {
        let source = format!("fn main(values: {ty}) {{ values.reserve(3); }}");
        let program = Compiler::new().analyze(&source).unwrap();
        let TypedStatementKind::Expression(value) = &program.functions[0].body[0].kind else {
            panic!()
        };
        assert_eq!(value.ty, Type::Void);
        let TypedExprKind::Reserve {
            receiver,
            additional,
        } = &value.kind
        else {
            panic!("expected reserve operation")
        };
        assert!(matches!(receiver.kind, TypedExprKind::Identifier { .. }));
        assert!(matches!(&additional.kind, TypedExprKind::Number(n) if n == "3"));
    }
}

#[test]
fn reserve_keeps_identifier_receivers_without_general_postfix_syntax() {
    for expression in [
        "(values).reserve(1)",
        "make().reserve(1)",
        "values[0].reserve(1)",
        "[1].reserve(1)",
        "values.reserve(1).reserve(1)",
        "values.reserve(1)[0]",
        "values.reserve(1)()",
        "values.reserve",
        "values.reserve(,)",
        ".5",
        "1.",
    ] {
        let source = format!("fn main(values: number[]) {{ {expression}; }}");
        assert!(
            matches!(
                Compiler::new().analyze(&source),
                Err(CompileError::Parse(_))
            ),
            "{source}"
        );
    }
    Compiler::new().analyze("fn main(values: number[]) { values /* receiver */ . /* method */ reserve /* args */ (1); 0..1; 0..=1; 1.5; }").unwrap();
}

#[test]
fn reserve_is_contextual_and_does_not_use_callable_name_resolution() {
    let source = "fn reserve(n: number) -> number { return n; } fn main(reserve: number[]) { reserve.reserve(1); let invoke = (reserve: bool[]) => reserve.reserve(2); invoke([true]); }";
    Compiler::new().analyze(source).unwrap();
    let mut compiler = Compiler::new();
    compiler
        .register_host_function(
            "reserve",
            "env",
            "reserve",
            vec![Type::Number],
            Type::Number,
        )
        .unwrap();
    compiler
        .analyze("fn main(a: string[]) { a.reserve(reserve(1)); }")
        .unwrap();
}

#[test]
fn reserve_rejects_arity_and_named_arguments_semantically_before_children() {
    for (args, count) in [("", 0), ("missing, other", 2)] {
        let source = format!("fn main() {{ unknown.reserve({args}); }}");
        assert!(
            matches!(semantic_error(&source), SemanticError::ArgumentCountMismatch { name, expected: 1, found, .. } if name == "reserve" && found == count)
        );
    }
    for args in [
        "additional: missing",
        "1, additional: missing",
        "additional: 1, 2",
    ] {
        let source = format!("fn main() {{ unknown.reserve({args}); }}");
        assert!(
            matches!(semantic_error(&source), SemanticError::NamedArgumentsNotSupported { name, .. } if name == "reserve")
        );
    }
}

#[test]
fn reserve_checks_receiver_and_numeric_argument_without_proving_runtime_authority() {
    for ty in ["number", "bool", "string", "range"] {
        let source = format!("fn main(value: {ty}) {{ value.reserve(missing); }}");
        assert!(
            matches!(
                semantic_error(&source),
                SemanticError::TypeMismatch {
                    expected: Type::Array(_),
                    ..
                }
            ),
            "{source}"
        );
    }
    for argument in ["true", "\"one\"", "[]", "0..1", "empty()"] {
        let source = format!("fn empty() {{}} fn main(a: number[]) {{ a.reserve({argument}); }}");
        assert!(
            matches!(
                semantic_error(&source),
                SemanticError::TypeMismatch {
                    expected: Type::Number,
                    ..
                }
            ),
            "{source}"
        );
    }
    Compiler::new().analyze("fn main() { let fixed = [1]; fixed.reserve(-0); fixed.reserve(-1); fixed.reserve(0.5); fixed.reserve(0 / 0); fixed.reserve(1 / 0); }").unwrap();
    assert!(matches!(
        semantic_error("fn main(a: number[]) { let result = a.reserve(0); }"),
        SemanticError::InvalidVoidType { .. }
    ));
}

#[test]
fn reserve_preserves_never_flow_and_checks_unreachable_and_skipped_children() {
    use was_compiler::typed_ast::ExprResult;
    let program = Compiler::new()
        .analyze("fn main(a: bool[]) -> number { a.reserve({ return 7; 1 }); }")
        .unwrap();
    let TypedStatementKind::Expression(value) = &program.functions[0].body[0].kind else {
        panic!()
    };
    assert_eq!(value.ty, ExprResult::Never);
    let program = Compiler::new()
        .analyze("fn main(a: number[]) { a.reserve({ return; true }); }")
        .unwrap();
    let TypedStatementKind::Expression(value) = &program.functions[0].body[0].kind else {
        panic!()
    };
    assert_eq!(value.ty, ExprResult::Never);
    for body in [
        "a.reserve({ return; let invalid: number = true; 1 });",
        "return; a.reserve(true);",
        "false && { a.reserve(true); true };",
    ] {
        let source = format!("fn main(a: number[]) {{ {body} }}");
        assert!(
            matches!(semantic_error(&source), SemanticError::TypeMismatch { .. }),
            "{source}"
        );
    }
}

#[test]
fn reserve_rejects_defaults_and_contracts_even_in_skipped_operands() {
    for source in [
        "fn main(a: number[], n: bool = false && a.reserve(1)) {}",
        "fn main(a: number[]) requires true || a.reserve(1) {}",
    ] {
        assert!(matches!(
            semantic_error(source),
            SemanticError::InvalidDefaultExpression { .. }
                | SemanticError::InvalidContractExpression { .. }
        ));
    }
}

#[test]
fn reserve_receiver_uses_original_captures_in_nested_helpers() {
    use was_compiler::typed_ast::TypedClosureBody;
    let program = Compiler::new().analyze("fn main(a: string[]) { let outer = () => { let inner = () => a.reserve(2); inner(); }; outer(); }").unwrap();
    let TypedStatementKind::Closure(outer) = &program.functions[0].body[0].kind else {
        panic!()
    };
    assert_eq!(outer.captures.len(), 1);
    let TypedClosureBody::Block(block) = &outer.body else {
        panic!()
    };
    let TypedStatementKind::Closure(inner) = &block.statements[0].kind else {
        panic!()
    };
    assert_eq!(inner.captures, outer.captures);
    let TypedClosureBody::Expression(value) = &inner.body else {
        panic!()
    };
    let TypedExprKind::Reserve { receiver, .. } = &value.kind else {
        panic!()
    };
    assert!(
        matches!(receiver.kind, TypedExprKind::Captured { owner, id, .. } if owner == outer.captures[0].owner && id == outer.captures[0].variable)
    );
}

#[test]
fn reserve_arguments_participate_in_named_result_dependencies_and_callable_discovery() {
    let program = Compiler::new().analyze("fn main(a: number[]) { a.reserve(count()); a.reserve({ let amount = () => 2; amount() }); } fn count() { return 3; }").unwrap();
    assert_eq!(program.functions[1].return_type, Type::Number);
}

#[test]
fn reserve_placeholder_argument_infers_number_with_region_local_array_evidence() {
    for ty in ["number[]", "bool[]", "string[]"] {
        let source = format!("fn main(a: {ty}) {{ let grow = a.reserve(_); grow(2); }}");
        let program = Compiler::new().analyze(&source).unwrap();
        let TypedStatementKind::Closure(closure) = &program.functions[0].body[0].kind else {
            panic!()
        };
        assert_eq!(closure.params[0].ty, Type::Number);
        assert_eq!(closure.return_type, Type::Void);
    }
    Compiler::new()
        .analyze(
            "fn main() { let grow = { let a = _; a == [true]; a.reserve(_); }; grow([false], 2); }",
        )
        .unwrap();
    for source in [
        "fn main() { let grow = { let a = _; a.reserve(1); }; grow([1]); }",
        "fn main(a: number[]) { let grow = { return; a.reserve(_); }; }",
    ] {
        assert!(
            matches!(
                semantic_error(source),
                SemanticError::CannotInferPlaceholderParameter { .. }
            ),
            "{source}"
        );
    }
    assert!(matches!(
        semantic_error("fn main() { let grow = { let a = _; a + 1; a.reserve(_); }; }"),
        SemanticError::ConflictingPlaceholderConstraints { .. }
    ));
}

#[test]
fn reserve_diagnostics_preserve_utf8_spans_and_optimization_parity() {
    use was_compiler::{OptimizationLevel, tokens::Span};
    let source = "fn main(a: number[]) { /* λ */ a.reserve(true); missing; }";
    let start = source.find("a.reserve").unwrap();
    let expected = CompileError::Semantic(SemanticError::TypeMismatch {
        expected: Type::Number,
        found: Type::Bool,
        span: Span::new(start, start + "a.reserve(true);".len()),
    });
    let mut compiler = Compiler::new();
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        compiler.set_optimization_level(level);
        assert_eq!(compiler.analyze(source).unwrap_err(), expected);
        compiler
            .analyze("fn main(a: bool[]) { a.reserve(1); }")
            .unwrap();
    }
    let source = "fn main(a: number[]) { /* λ */ a.reserve(additional: 1); }";
    let start = source.find("additional").unwrap();
    assert_eq!(
        semantic_error(source).span(),
        Some(Span::new(start, start + "additional: 1".len()))
    );
}

#[test]
fn destination_array_context_reaches_the_source_through_nested_constructors() {
    for (ty, element) in [
        ("bool[]", ArrayElementType::Bool),
        ("string[]", ArrayElementType::String),
    ] {
        let source = format!("fn main() {{ let values: {ty} = resizable(resizable([])); }}");
        let program = Compiler::new().analyze(&source).unwrap();
        let TypedStatementKind::Let { value, .. } = &program.functions[0].body[0].kind else {
            panic!()
        };
        let mut value = value;
        for _ in 0..2 {
            assert_eq!(value.ty, Type::Array(element));
            let TypedExprKind::Call { func, args, .. } = &value.kind else {
                panic!()
            };
            assert_eq!(*func, FunctionRef::Builtin(BuiltinFunction::Resizable));
            assert_eq!(args.len(), 1);
            value = &args[0];
        }
        assert!(matches!(value.kind, TypedExprKind::Array(_, actual) if actual == element));
    }
}

#[test]
fn placeholder_constructor_relates_source_and_result_exactly_within_its_region() {
    for (body, expected) in [
        ("consume(resizable(_))", Type::Array(ArrayElementType::Bool)),
        (
            "{ let items = _; let copy = resizable(items); consume(copy); items }",
            Type::Array(ArrayElementType::Bool),
        ),
        ("consume(resizable([_]))", Type::Bool),
        (
            "{ let items = _; items == [true]; resizable(items) }",
            Type::Array(ArrayElementType::Bool),
        ),
    ] {
        let source = format!(
            "fn consume(items: bool[]) -> bool[] {{ return items; }} fn main() {{ let copy = {body}; }}"
        );
        let program = Compiler::new()
            .analyze(&source)
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        let TypedStatementKind::Closure(closure) = &program.functions[1].body[0].kind else {
            panic!()
        };
        assert_eq!(closure.params[0].ty, expected, "{body}");
        assert_eq!(closure.return_type, Type::Array(ArrayElementType::Bool));
    }
}

#[test]
fn constructor_name_is_reserved_for_hosts_but_not_external_module_or_field_names() {
    let mut compiler = Compiler::new();
    assert_eq!(
        compiler.register_host_function("resizable", "env", "copy", vec![], Type::Void),
        Err(HostRegistrationError::ReservedName {
            name: "resizable".into()
        }),
    );
    let id = compiler
        .register_host_function("copy", "resizable", "resizable", vec![], Type::Void)
        .unwrap();
    assert_eq!(id.0, 0);
    compiler.analyze("fn main() { copy(); }").unwrap();
}

fn semantic_error(source: &str) -> SemanticError {
    match Compiler::new().analyze(source) {
        Err(CompileError::Semantic(error)) => error,
        other => panic!("expected semantic error for {source}: {other:?}"),
    }
}

#[test]
fn placeholder_scalar_result_conflicts_before_unrelated_ambiguity() {
    let source = "fn take(n: number) {} fn main() { let copy = { _; take(resizable(_)); 1 }; }";
    assert!(matches!(
        semantic_error(source),
        SemanticError::ConflictingPlaceholderConstraints { .. }
    ));
}

#[test]
fn constructor_uses_ordinary_identifier_resolution_and_lexical_shadowing() {
    for source in [
        "fn resizable() {}",
        "fn resizable(a: bool[]) -> bool[] { return a; }",
    ] {
        assert!(
            matches!(semantic_error(source), SemanticError::DuplicateFunction { name, .. } if name == "resizable")
        );
    }
    for source in [
        "fn main(resizable: number) { resizable([]); }",
        "fn main() { let resizable = 1; resizable([]); }",
    ] {
        assert!(
            matches!(semantic_error(source), SemanticError::NotCallable { name, .. } if name == "resizable")
        );
    }
    let program = Compiler::new()
        .analyze("fn main() { let resizable = (x: number) => x + 1; return 2 |> resizable; }")
        .unwrap();
    assert_eq!(program.functions[0].return_type, Type::Number);
    assert!(
        matches!(semantic_error("fn main() { resizable; }"), SemanticError::UndefinedVariable { name, .. } if name == "resizable")
    );
}

#[test]
fn constructor_checks_call_structure_before_argument_expressions() {
    for (source, found) in [
        ("fn main() { resizable(); }", 0),
        ("fn main() { resizable(missing, absent); }", 2),
    ] {
        assert!(
            matches!(semantic_error(source), SemanticError::ArgumentCountMismatch { name, expected: 1, found: actual, .. } if name == "resizable" && actual == found)
        );
    }
    assert!(
        matches!(semantic_error("fn main() { resizable(source: missing); }"), SemanticError::NamedArgumentsNotSupported { name, .. } if name == "resizable")
    );
    assert!(matches!(
        semantic_error("fn main() { [] |> resizable; }"),
        SemanticError::InvalidPipeline { .. }
    ));
    for value in ["1", "true", r#""text""#, "0..2"] {
        let source = format!("fn main() {{ resizable({value}); }}");
        assert!(
            matches!(
                semantic_error(&source),
                SemanticError::TypeMismatch {
                    expected: Type::Array(ArrayElementType::Number),
                    ..
                }
            ),
            "{source}"
        );
    }
    assert!(matches!(
        semantic_error("fn nothing() {} fn main() { resizable(nothing()); }"),
        SemanticError::TypeMismatch { .. }
    ));
}

#[test]
fn defaults_and_contracts_reject_constructor_calls_including_skipped_operands() {
    for expression in [
        "resizable(a)",
        "len(resizable(a)) == 0",
        "false && len(resizable(a)) == 0",
        "true || len(resizable(a)) == 0",
    ] {
        let source = format!("fn main(a: bool[], b: bool = {expression}) {{}}");
        assert!(
            matches!(
                semantic_error(&source),
                SemanticError::InvalidDefaultExpression {
                    reason: "only positional builtin len calls are allowed",
                    ..
                }
            ),
            "{source}"
        );
        let source = format!("fn main(a: bool[]) requires {expression} {{}}");
        assert!(
            matches!(
                semantic_error(&source),
                SemanticError::InvalidContractExpression {
                    reason: "only positional builtin len calls are allowed",
                    ..
                }
            ),
            "{source}"
        );
    }
}

#[test]
fn placeholder_constructor_stays_ambiguous_without_region_local_element_evidence() {
    for body in [
        "resizable(_)",
        "resizable([_])",
        "{ let items = _; resizable(items) == [] }",
        "{ let items = _; len(resizable(items)) }",
        "{ let items = _; return 1; consume(resizable(items)); }",
        "{ let items = _; let nested = () => consume(resizable(items)); 1 }",
    ] {
        let source = format!(
            "fn consume(a: bool[]) -> bool[] {{ return a; }} fn main() {{ let copy = {body}; copy([true]); }}"
        );
        assert!(
            matches!(
                semantic_error(&source),
                SemanticError::CannotInferPlaceholderParameter { .. }
            ),
            "{source}"
        );
    }
    assert!(matches!(
        semantic_error("fn main() { [true] |> (resizable(_)); }"),
        SemanticError::CannotInferPlaceholderParameter { .. }
    ));
}

#[test]
fn destination_context_does_not_refine_siblings_or_concrete_function_results() {
    for source in [
        "fn empty() { return []; } fn main() { let flags: bool[] = resizable(empty()); }",
        "fn main() { let empty = () => []; let flags: bool[] = resizable(empty()); }",
        "fn main() { let flags = if true { resizable([]) } else { [true] }; }",
        "fn main() { let flags = resizable([]) == [true]; }",
        "fn main() { let flags: bool[] = resizable([1]); }",
    ] {
        assert!(
            matches!(
                semantic_error(source),
                SemanticError::TypeMismatch { .. } | SemanticError::InvalidBinaryOperands { .. }
            ),
            "{source}"
        );
    }
    for source in [
        "fn main() -> bool[] { return resizable(if true { [] } else { [] }); }",
        "fn take(a: bool[]) {} fn main() { take(resizable([])); }",
        "fn take(a: bool[]) {} fn main() { take(a: resizable([])); }",
        "fn main() { let a: string[] = []; a = resizable({ [] }); }",
        "fn main() -> bool[] { return resizable(loop { break []; }); }",
        "fn main() -> bool[] { return resizable(match true { true => [], false => [] }); }",
        "fn main() { let copy = (a: bool[]) => resizable(a); let a: bool[] = copy([]); }",
    ] {
        Compiler::new()
            .analyze(source)
            .unwrap_or_else(|error| panic!("{source}: {error}"));
    }
}

#[test]
fn divergent_sources_preserve_never_and_contextual_literal_storage() {
    use was_compiler::typed_ast::ExprResult;
    for (ty, element) in [
        ("number[]", ArrayElementType::Number),
        ("bool[]", ArrayElementType::Bool),
        ("string[]", ArrayElementType::String),
    ] {
        for source_value in ["loop {}", "[loop {}]"] {
            let source = format!("fn main() {{ let values: {ty} = resizable({source_value}); }}");
            let program = Compiler::new().analyze(&source).unwrap();
            let TypedStatementKind::Let { value, .. } = &program.functions[0].body[0].kind else {
                panic!()
            };
            assert_eq!(value.ty, ExprResult::Never);
            let TypedExprKind::Call { args, func, .. } = &value.kind else {
                panic!()
            };
            assert_eq!(*func, FunctionRef::Builtin(BuiltinFunction::Resizable));
            assert_eq!(args[0].ty, ExprResult::Never);
            if source_value.starts_with('[') {
                assert!(
                    matches!(args[0].kind, TypedExprKind::Array(_, actual) if actual == element)
                );
            }
        }
    }
    assert!(matches!(
        semantic_error("fn main() { let values = resizable(loop {}); }"),
        SemanticError::CannotInferNeverType { .. }
    ));
    assert!(
        matches!(semantic_error("fn main() { resizable({ return; missing }); }"), SemanticError::UndefinedVariable { name, .. } if name == "missing")
    );
    assert!(matches!(
        semantic_error("fn main() { return; resizable(1); }"),
        SemanticError::TypeMismatch { .. }
    ));
    assert!(matches!(
        semantic_error("fn main() { false && len(resizable(1)) == 0; }"),
        SemanticError::TypeMismatch { .. }
    ));
    Compiler::new()
        .analyze("fn main() -> number { resizable({ return 1; [] }); }")
        .unwrap();
}

#[test]
fn constructor_diagnostics_keep_utf8_statement_spans_and_preset_parity() {
    use was_compiler::{OptimizationLevel, tokens::Span};
    let source = "fn main() { /* λ */ let copy = resizable(1); missing; }";
    let start = source.find("let copy").unwrap();
    let expected = CompileError::Semantic(SemanticError::TypeMismatch {
        expected: Type::Array(ArrayElementType::Number),
        found: Type::Number,
        span: Span::new(start, start + "let copy = resizable(1);".len()),
    });
    let mut compiler = Compiler::new();
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        compiler.set_optimization_level(level);
        let error = compiler.analyze(source).unwrap_err();
        assert_eq!(error, expected);
        assert_eq!(error.span(), expected.span());
        let program = compiler
            .analyze("fn main() -> bool[] { return resizable([]); }")
            .unwrap();
        assert_eq!(
            program.functions[0].return_type,
            Type::Array(ArrayElementType::Bool)
        );
    }
}

#[test]
fn placeholder_constructor_reachable_evidence_composes_with_control_and_nested_regions() {
    for body in [
        "{ let items = _; if true { consume(resizable(items)); } resizable(items) }",
        "{ let items = _; false && len(consume(resizable(items))) == 0; items }",
        "{ let items = _; consume(resizable(items)); let inner = () => resizable(items); inner() }",
        "{ let items = _; let copy = resizable(items); copy[0] = true; items }",
        "{ let items = _; let copy = resizable(items); for flag in copy { !flag; } items }",
    ] {
        let source = format!(
            "fn consume(a: bool[]) -> bool[] {{ return a; }} fn main() {{ let copy = {body}; }}"
        );
        let program = Compiler::new()
            .analyze(&source)
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        let TypedStatementKind::Closure(closure) = &program.functions[1].body[0].kind else {
            panic!()
        };
        assert_eq!(closure.params[0].ty, Type::Array(ArrayElementType::Bool));
    }
    for body in [
        "{ let items = _; consume(resizable(items)); items == [1] }",
        "{ let items = _; let copy = resizable(items); copy[0] = true; items[0] + 1 }",
    ] {
        let source = format!(
            "fn consume(a: bool[]) -> bool[] {{ return a; }} fn main() {{ let copy = {body}; }}"
        );
        assert!(
            matches!(
                semantic_error(&source),
                SemanticError::ConflictingPlaceholderConstraints { .. }
            ),
            "{source}"
        );
    }
}

#[test]
fn placeholder_constructor_preserves_all_exact_array_kinds_including_immediate_stages() {
    for (ty, literal, element) in [
        ("number[]", "[1]", ArrayElementType::Number),
        ("bool[]", "[true]", ArrayElementType::Bool),
        ("string[]", r#"["one"]"#, ArrayElementType::String),
    ] {
        let source = format!(
            "fn take(a: {ty}) -> {ty} {{ return a; }} fn main() {{ let copy = take(resizable(_)); return copy({literal}); }}"
        );
        let program = Compiler::new().analyze(&source).unwrap();
        let TypedStatementKind::Closure(closure) = &program.functions[1].body[0].kind else {
            panic!()
        };
        assert_eq!(closure.params[0].ty, Type::Array(element));
        assert_eq!(closure.return_type, Type::Array(element));
        let source = format!(
            "fn take(a: {ty}) -> {ty} {{ return a; }} fn main() {{ return {literal} |> (take(resizable(_))); }}"
        );
        let program = Compiler::new().analyze(&source).unwrap();
        assert_eq!(program.functions[1].return_type, Type::Array(element));
    }
    for body in [
        "{ let value = _; value + 1; resizable(value) }",
        "{ let value = _; let copied = resizable(value); value + 1; copied }",
    ] {
        let source = format!("fn main() {{ let copy = {body}; }}");
        assert!(
            matches!(
                semantic_error(&source),
                SemanticError::ConflictingPlaceholderConstraints { .. }
            ),
            "{source}"
        );
    }
}

#[test]
fn divergent_constructor_iteration_retains_bool_and_string_element_metadata() {
    use was_compiler::typed_ast::ExprResult;
    let compiler = Compiler::new();
    for (scalar, literal, expected) in [
        ("bool", "true", Type::Bool),
        ("string", r#""text""#, Type::String),
    ] {
        for iterable in [
            format!("[loop {{}}, {literal}]"),
            format!("resizable([loop {{}}, {literal}])"),
            format!("resizable(resizable([loop {{}}, {literal}]))"),
        ] {
            let source = format!("fn main() {{ for x in {iterable} {{ let y: {scalar} = x; }} }}");
            let program = compiler
                .analyze(&source)
                .unwrap_or_else(|error| panic!("{source}: {error}"));
            let TypedStatementKind::For {
                element_type,
                iterable,
                ..
            } = &program.functions[0].body[0].kind
            else {
                panic!()
            };
            assert_eq!(*element_type, expected);
            assert_eq!(iterable.ty, ExprResult::Never);
            let bytes = compiler
                .compile(&program)
                .unwrap_or_else(|error| panic!("{source}: {error}"));
            wasmparser::Validator::new().validate_all(&bytes).unwrap();
        }
    }
}

#[test]
fn divergent_constructor_iteration_still_checks_forged_metadata_and_call_arity() {
    let compiler = Compiler::new();
    for (scalar, literal, wrong_element) in [
        ("bool", "true", Type::String),
        ("string", r#""text""#, Type::Bool),
    ] {
        for depth in 1..=2 {
            let mut iterable = format!("[loop {{}}, {literal}]");
            for _ in 0..depth {
                iterable = format!("resizable({iterable})");
            }
            let source = format!("fn main() {{ for x in {iterable} {{ let y: {scalar} = x; }} }}");
            let original = compiler.analyze(&source).unwrap();
            let bytes = compiler.compile(&original).unwrap();
            wasmparser::Validator::new().validate_all(&bytes).unwrap();

            let mut forged = original.clone();
            let TypedStatementKind::For { element_type, .. } =
                &mut forged.functions[0].body[0].kind
            else {
                panic!()
            };
            *element_type = wrong_element;
            assert!(
                compiler.compile(&forged).is_err(),
                "forged iteration metadata: {source}"
            );

            for argument_count in [0, 2] {
                let mut forged = original.clone();
                let TypedStatementKind::For { iterable, .. } =
                    &mut forged.functions[0].body[0].kind
                else {
                    panic!()
                };
                let TypedExprKind::Call { args, .. } = &mut iterable.kind else {
                    panic!()
                };
                if argument_count == 0 {
                    args.clear();
                } else {
                    args.push(args[0].clone());
                }
                assert!(
                    compiler.compile(&forged).is_err(),
                    "forged arity {argument_count}: {source}"
                );
            }

            let mut forged = original.clone();
            let TypedStatementKind::For { iterable, .. } = &mut forged.functions[0].body[0].kind
            else {
                panic!()
            };
            let mut value = iterable.as_mut();
            for _ in 0..depth {
                let TypedExprKind::Call { args, .. } = &mut value.kind else {
                    panic!()
                };
                value = &mut args[0];
            }
            let TypedExprKind::Array(_, element) = &mut value.kind else {
                panic!()
            };
            *element = ArrayElementType::Number;
            assert!(
                compiler.compile(&forged).is_err(),
                "forged literal metadata: {source}"
            );

            let mut forged = original;
            let TypedStatementKind::For { iterable, .. } = &mut forged.functions[0].body[0].kind
            else {
                panic!()
            };
            let TypedExprKind::Call { func, .. } = &mut iterable.kind else {
                panic!()
            };
            *func = FunctionRef::Builtin(BuiltinFunction::Len);
            assert!(
                compiler.compile(&forged).is_err(),
                "forged resolved builtin: {source}"
            );
        }
    }
}

#[test]
fn constructor_preserves_each_source_array_type() {
    for (literal, element) in [
        ("[1, 2]", ArrayElementType::Number),
        ("[true, false]", ArrayElementType::Bool),
        (r#"["one", "two"]"#, ArrayElementType::String),
        ("[]", ArrayElementType::Number),
    ] {
        let source = format!("fn main() {{ return resizable({literal}); }}");
        let program = Compiler::new()
            .analyze(&source)
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert_eq!(program.functions[0].return_type, Type::Array(element));
    }
}
