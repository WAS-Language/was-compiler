use was_compiler::host::HostRegistrationError;
use was_compiler::semantic::SemanticError;
use was_compiler::typed_ast::{BuiltinFunction, FunctionRef, TypedExprKind, TypedStatementKind};
use was_compiler::{ArrayElementType, CompileError, Compiler, Type};

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
