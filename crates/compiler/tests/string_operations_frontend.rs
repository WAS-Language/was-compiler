use was_compiler::host::HostRegistrationError;
use was_compiler::semantic::SemanticError;
use was_compiler::typed_ast::{
    BuiltinFunction, ExprResult, FunctionRef, TypedExprKind, TypedStatementKind,
};
use was_compiler::{CompileError, Compiler, Type};

fn error(source: &str) -> SemanticError {
    match Compiler::new().analyze(source).unwrap_err() {
        CompileError::Semantic(error) => error,
        other => panic!("expected semantic error, got {other:?}"),
    }
}

#[test]
fn join_contextually_types_empty_parts_as_strings() {
    let program = Compiler::new()
        .analyze("fn main() { return string_join([], \"雪\"); }")
        .unwrap();
    assert_eq!(program.functions[0].return_type, Type::String);
    let TypedStatementKind::Return(Some(value)) = &program.functions[0].body[0].kind else {
        panic!("expected explicit return");
    };
    let TypedExprKind::Call { args, .. } = &value.kind else {
        panic!("expected ordinary typed call");
    };
    assert_eq!(
        args[0].ty,
        ExprResult::Value(Type::Array(was_compiler::ArrayElementType::String))
    );
}

#[test]
fn join_signature_supplies_exact_contexts_and_placeholder_constraints() {
    for source in [
        "fn main() { return string_join([\"雪\", \"!\"], \"\"); }",
        "fn main() { return string_join(resizable([]), \"\"); }",
        "fn main() { return string_join(if true { [] } else { [\"x\"] }, \"\"); }",
        "fn main() { return string_join(match true { true => [], false => [\"x\"] }, \"\"); }",
        "fn main() { return string_join(loop { break []; }, \"\"); }",
        "fn main() { let f = string_join(_, \",\"); return f([]); }",
        "fn main() { let f = string_join([], _); return f(\",\"); }",
        "fn main() { let f = string_join(_, _); return f([\"x\"], \",\"); }",
        "fn main() { return [\"x\"] |> (string_join(_, \",\")); }",
        "fn main(parts: string[]) { let f = (s: string) => string_join(parts, s); return f(\",\"); }",
        "fn main() { return join([]); } fn join(parts: string[]) { return string_join(parts, \",\"); }",
    ] {
        let program = Compiler::new().analyze(source).unwrap();
        assert_eq!(program.functions[0].return_type, Type::String, "{source}");
    }
    for source in [
        "fn main() { let parts = []; return string_join(parts, \",\"); }",
        "fn parts() { return []; } fn main() { return string_join(parts(), \",\"); }",
        "fn main() { let f = string_join(_, \",\"); return f([1]); }",
        "fn main() { let f = string_join([], _); return f(1); }",
        "fn main() { return [1] |> (string_join(_, \",\")); }",
    ] {
        assert!(
            matches!(error(source), SemanticError::TypeMismatch { .. }),
            "{source}"
        );
    }
    assert!(matches!(
        error("fn main() { let f = string_join(_, false); }"),
        SemanticError::ConflictingPlaceholderConstraints { .. }
    ));
}

#[test]
fn join_retains_builtin_identity_and_ordered_arguments() {
    let program = Compiler::new()
        .analyze("fn main(parts: string[]) { return string_join(parts, \"separator\"); }")
        .unwrap();
    let TypedStatementKind::Return(Some(value)) = &program.functions[0].body[0].kind else {
        panic!("expected explicit return");
    };
    assert_eq!(value.ty, ExprResult::Value(Type::String));
    let TypedExprKind::Call { func, name, args } = &value.kind else {
        panic!("expected ordinary typed call");
    };
    assert_eq!(*func, FunctionRef::Builtin(BuiltinFunction::StringJoin));
    assert_eq!(name, "string_join");
    assert_eq!(args.len(), 2);
    assert_eq!(
        args[0].ty,
        ExprResult::Value(Type::Array(was_compiler::ArrayElementType::String))
    );
    assert!(matches!(args[0].kind, TypedExprKind::Identifier { .. }));
    assert!(matches!(&args[1].kind, TypedExprKind::String(text) if text == "separator"));
}

#[test]
fn join_requires_two_exact_positional_arguments_before_checking_children() {
    for (args, found) in [("", 0), ("missing", 1), ("missing, other, third", 3)] {
        assert!(matches!(
            error(&format!("fn main() {{ string_join({args}); }}")),
            SemanticError::ArgumentCountMismatch { expected: 2, found: count, .. } if count == found
        ));
    }
    for args in [
        "parts: missing, separator: other",
        "missing, separator: other",
    ] {
        assert!(matches!(
            error(&format!("fn main() {{ string_join({args}); }}")),
            SemanticError::NamedArgumentsNotSupported { .. }
        ));
    }
    for value in ["1", "true", "\"x\"", "[1]", "[true]", "0..1", "empty()"] {
        assert!(matches!(
            error(&format!(
                "fn empty() {{}} fn main() {{ string_join({value}, \",\"); }}"
            )),
            SemanticError::TypeMismatch { .. }
        ));
    }
    for value in ["1", "true", "[]", "0..1", "empty()"] {
        assert!(matches!(
            error(&format!(
                "fn empty() {{}} fn main() {{ string_join([], {value}); }}"
            )),
            SemanticError::TypeMismatch {
                expected: Type::String,
                ..
            }
        ));
    }
    assert!(matches!(
        error("fn main() { string_join(first, second); }"),
        SemanticError::UndefinedVariable { name, .. } if name == "first"
    ));
}

#[test]
fn join_propagates_never_but_checks_all_children_and_skipped_calls() {
    for (args, position) in [("loop {}, \",\"", 0), ("[], loop {}", 1)] {
        let program = Compiler::new()
            .analyze(&format!(
                "fn main() -> string {{ return string_join({args}); }}"
            ))
            .unwrap();
        let TypedStatementKind::Return(Some(value)) = &program.functions[0].body[0].kind else {
            panic!("expected explicit return");
        };
        assert_eq!(value.ty, ExprResult::Never);
        let TypedExprKind::Call { args, .. } = &value.kind else {
            panic!("expected call even when divergent");
        };
        assert_eq!(args.len(), 2);
        assert_eq!(args[position].ty, ExprResult::Never);
    }
    for source in [
        "fn main() -> string { return string_join(loop {}, false); }",
        "fn main() -> string { return string_join([1], loop {}); }",
        "fn main() { return; string_join([], false); }",
        "fn main() { return false && string_join([], false) == \"\"; }",
    ] {
        assert!(
            matches!(error(source), SemanticError::TypeMismatch { .. }),
            "{source}"
        );
    }
    assert!(matches!(
        error("fn main() -> string { return string_join(loop {}, missing); }"),
        SemanticError::UndefinedVariable { name, .. } if name == "missing"
    ));
}

#[test]
fn join_rejects_direct_builtin_stages_defaults_and_contracts() {
    for source in [
        "fn main(parts: string[]) { return parts |> string_join; }",
        "fn main(parts: string[]) { return parts |> string_join(\",\"); }",
        "fn main() { let f = string_join(_, \",\") |> string_join(\",\"); }",
    ] {
        assert!(
            matches!(error(source), SemanticError::InvalidPipeline { .. }),
            "{source}"
        );
    }
    assert!(matches!(
        error("fn main(parts: string[], s: string = string_join(parts, \",\")) {}"),
        SemanticError::InvalidDefaultExpression { .. }
    ));
    for guard in ["true ||", "false &&"] {
        assert!(matches!(
            error(&format!(
                "fn main(parts: string[]) requires {guard} string_join(parts, \",\") == \"\" {{}}"
            )),
            SemanticError::InvalidContractExpression { .. }
        ));
    }
}

#[test]
fn replace_all_signature_supplies_exact_region_local_contexts() {
    for source in [
        "fn main() { return string_replace_all(\"雪雪\", \"雪\", \"!\"); }",
        "fn main() { return string_replace_all(\"x\", \"\", \"\"); }",
        "fn main(s: string) { let f = string_replace_all(_, \"x\", \"!\"); return f(s); }",
        "fn main() { let f = string_replace_all(\"x\", _, _); return f(\"x\", \"!\"); }",
        "fn main() { let f = string_replace_all(_, _, _); return f(\"x\", \"x\", \"!\"); }",
        "fn main() { return \"x\" |> (string_replace_all(_, \"x\", \"!\")); }",
        "fn main(s: string) { let f = (x: string) => string_replace_all(s, x, \"!\"); return f(\"x\"); }",
        "fn main() { return replace(\"x\"); } fn replace(s: string) { return string_replace_all(s, \"x\", \"!\"); }",
    ] {
        let program = Compiler::new().analyze(source).unwrap();
        assert_eq!(program.functions[0].return_type, Type::String, "{source}");
    }
}

#[test]
fn replace_all_retains_builtin_identity_and_ordered_arguments() {
    let program = Compiler::new()
        .analyze(
            "fn main() { return string_replace_all(\"source\", \"needle\", \"replacement\"); }",
        )
        .unwrap();
    let TypedStatementKind::Return(Some(value)) = &program.functions[0].body[0].kind else {
        panic!("expected explicit return");
    };
    assert_eq!(value.ty, ExprResult::Value(Type::String));
    let TypedExprKind::Call { func, name, args } = &value.kind else {
        panic!("expected ordinary typed call");
    };
    assert_eq!(
        *func,
        FunctionRef::Builtin(BuiltinFunction::StringReplaceAll)
    );
    assert_eq!(name, "string_replace_all");
    assert_eq!(args.len(), 3);
    for (arg, expected) in args.iter().zip(["source", "needle", "replacement"]) {
        assert_eq!(arg.ty, ExprResult::Value(Type::String));
        assert!(matches!(&arg.kind, TypedExprKind::String(text) if text == expected));
    }
}

#[test]
fn replace_all_requires_three_exact_positional_strings_before_checking_children() {
    for (args, found) in [
        ("", 0),
        ("missing", 1),
        ("missing, other", 2),
        ("missing, other, third, fourth", 4),
    ] {
        assert!(matches!(
            error(&format!("fn main() {{ string_replace_all({args}); }}")),
            SemanticError::ArgumentCountMismatch { expected: 3, found: count, .. } if count == found
        ));
    }
    for args in [
        "source: missing, needle: other, replacement: third",
        "missing, needle: other, replacement: third",
    ] {
        assert!(matches!(
            error(&format!("fn main() {{ string_replace_all({args}); }}")),
            SemanticError::NamedArgumentsNotSupported { .. }
        ));
    }
    for value in ["1", "true", "[]", "0..1", "empty()"] {
        for position in 0..3 {
            let mut args = ["\"x\""; 3];
            args[position] = value;
            assert!(matches!(
                error(&format!(
                    "fn empty() {{}} fn main() {{ string_replace_all({}); }}",
                    args.join(", ")
                )),
                SemanticError::TypeMismatch {
                    expected: Type::String,
                    ..
                }
            ));
        }
    }
    assert!(matches!(
        error("fn main() { string_replace_all(first, second, third); }"),
        SemanticError::UndefinedVariable { name, .. } if name == "first"
    ));
}

#[test]
fn replace_all_rejects_direct_stages_and_pure_defaults_and_contracts() {
    for source in [
        "fn main() { return \"x\" |> string_replace_all; }",
        "fn main() { return \"x\" |> string_replace_all(\"x\", \"!\"); }",
        "fn main() { let f = string_replace_all(_, \"x\", \"!\") |> string_replace_all(\"x\", \"!\"); }",
    ] {
        assert!(
            matches!(error(source), SemanticError::InvalidPipeline { .. }),
            "{source}"
        );
    }
    assert!(matches!(
        error("fn main(s: string = string_replace_all(\"x\", \"x\", \"!\")) {}"),
        SemanticError::InvalidDefaultExpression { .. }
    ));
    for guard in ["true ||", "false &&"] {
        assert!(matches!(
            error(&format!(
                "fn main() requires {guard} string_replace_all(\"x\", \"x\", \"!\") == \"!\" {{}}"
            )),
            SemanticError::InvalidContractExpression { .. }
        ));
    }
}

#[test]
fn replace_all_propagates_never_but_checks_all_children_and_skipped_calls() {
    for position in 0..3 {
        let mut args = ["\"x\""; 3];
        args[position] = "loop {}";
        let program = Compiler::new()
            .analyze(&format!(
                "fn main() -> string {{ return string_replace_all({}); }}",
                args.join(", ")
            ))
            .unwrap();
        let TypedStatementKind::Return(Some(value)) = &program.functions[0].body[0].kind else {
            panic!("expected explicit return");
        };
        assert_eq!(value.ty, ExprResult::Never);
        let TypedExprKind::Call { args, .. } = &value.kind else {
            panic!("expected call even when divergent");
        };
        assert_eq!(args.len(), 3);
        assert_eq!(args[position].ty, ExprResult::Never);
    }
    for source in [
        "fn main() -> string { return string_replace_all(loop {}, false, \"!\"); }",
        "fn main() -> string { return string_replace_all(\"x\", loop {}, false); }",
        "fn main() { return; string_replace_all(\"x\", \"x\", false); }",
        "fn main() { return false && string_replace_all(false, \"x\", \"!\") == \"!\"; }",
    ] {
        assert!(
            matches!(
                error(source),
                SemanticError::TypeMismatch {
                    expected: Type::String,
                    ..
                }
            ),
            "{source}"
        );
    }
    assert!(matches!(
        error("fn main() { let f = string_replace_all(_, \"x\", false); }"),
        SemanticError::ConflictingPlaceholderConstraints { .. }
    ));
    assert!(matches!(
        error("fn main() -> string { return string_replace_all(loop {}, \"x\", missing); }"),
        SemanticError::UndefinedVariable { name, .. } if name == "missing"
    ));
}

#[test]
fn slice_signature_supplies_exact_region_local_contexts() {
    for source in [
        "fn main() { return string_slice_bytes(\"雪\", 0, 3); }",
        "fn main(s: string) { let f = string_slice_bytes(_, 0, 1); return f(s); }",
        "fn main(s: string) { let f = string_slice_bytes(s, _, _); return f(0, 1); }",
        "fn main() { return \"x\" |> (string_slice_bytes(_, 0, 1)); }",
        "fn main(s: string) { let f = (n: number) => string_slice_bytes(s, 0, n); return f(0); }",
    ] {
        let program = Compiler::new().analyze(source).unwrap();
        assert_eq!(program.functions[0].return_type, Type::String, "{source}");
    }
}

#[test]
fn exact_signatures_compose_with_inference_captures_and_placeholders() {
    for (source, expected) in [
        (
            "fn main() { return string_byte_len(\"雪\"); }",
            Type::Number,
        ),
        (
            "fn main() { return string_concat(\"a\", \"b\"); }",
            Type::String,
        ),
        (
            "fn main(s: string) { let f = string_concat(_, s); return f(\"x\"); }",
            Type::String,
        ),
        (
            "fn main() { return \"x\" |> (string_byte_len(_)); }",
            Type::Number,
        ),
        (
            "fn main(s: string) { let f = (x: string) => string_concat(s, x); return f(\"!\"); }",
            Type::String,
        ),
        (
            "fn main() { return length(\"雪\"); } fn length(s: string) { return string_byte_len(string_concat(s, \"!\")); }",
            Type::Number,
        ),
    ] {
        let program = Compiler::new().analyze(source).unwrap();
        assert_eq!(program.functions[0].return_type, expected, "{source}");
    }
}

#[test]
fn calls_check_structure_before_expressions_and_require_exact_types() {
    for (name, args, expected, found) in [
        ("string_byte_len", "missing, other", 1, 2),
        ("string_concat", "missing", 2, 1),
        ("string_concat", "", 2, 0),
        ("string_slice_bytes", "missing, other", 3, 2),
        ("string_slice_bytes", "missing, other, third, fourth", 3, 4),
    ] {
        assert!(
            matches!(error(&format!("fn main() {{ {name}({args}); }}")), SemanticError::ArgumentCountMismatch { expected: e, found: f, .. } if e == expected && f == found)
        );
    }
    for call in [
        "string_byte_len(source: missing)",
        "string_concat(left: missing, right: other)",
        "string_slice_bytes(source: missing, start: other, end: third)",
    ] {
        assert!(matches!(
            error(&format!("fn main() {{ {call}; }}")),
            SemanticError::NamedArgumentsNotSupported { .. }
        ));
    }
    for value in ["1", "true", "[]", "0..1", "empty()"] {
        for call in [
            format!("string_byte_len({value})"),
            format!("string_concat(\"x\", {value})"),
        ] {
            assert!(matches!(
                error(&format!("fn empty() {{}} fn main() {{ {call}; }}")),
                SemanticError::TypeMismatch {
                    expected: Type::String,
                    ..
                }
            ));
        }
    }
}

#[test]
fn builtin_names_are_reserved_globally_but_lexically_shadowable() {
    for name in [
        "string_byte_len",
        "string_concat",
        "string_slice_bytes",
        "string_replace_all",
        "string_join",
    ] {
        assert!(
            Compiler::new()
                .analyze(&format!("fn {name}() {{}}"))
                .is_err()
        );
        assert!(matches!(
            Compiler::new().register_host_function(name, "env", "field", vec![], Type::Void),
            Err(HostRegistrationError::ReservedName { .. })
        ));
        Compiler::new()
            .register_host_function("host", name, name, vec![], Type::Void)
            .unwrap();
        assert!(matches!(
            error(&format!("fn main({name}: string) {{ {name}(); }}")),
            SemanticError::NotCallable { .. }
        ));
        Compiler::new()
            .analyze(&format!(
                "fn main() {{ let {name} = (x: number) => x + 1; return 1 |> {name}; }}"
            ))
            .unwrap();
    }
}

#[test]
fn string_operations_do_not_expand_syntax_pipelines_or_purity() {
    for source in [
        "fn main() { return len(\"x\"); }",
        "fn main() { return \"x\" |> string_slice_bytes(0, 1); }",
        "fn main(s: string = string_slice_bytes(\"x\", 0, 1)) {}",
        "fn main() requires true || string_slice_bytes(\"x\", 0, 1) == \"x\" {}",
        "fn main() { return; string_slice_bytes(\"x\", false, 1); }",
        "fn main() { return string_slice_bytes(loop {}, 0, false); }",
        "fn main() { return false && string_slice_bytes(\"x\", 0, true) == \"x\"; }",
        "fn main() { return string_slice_bytes(1, 0, 1); }",
        "fn main() { return string_slice_bytes(\"x\", \"0\", 1); }",
        "fn main() { return string_slice_bytes(\"x\", 0, \"1\"); }",
        "fn main() { return \"a\" + \"b\"; }",
        "fn main() { return \"x\" |> string_byte_len; }",
        "fn main() { return \"x\" |> string_concat(\"y\"); }",
        "fn main(s: string = string_concat(\"a\", \"b\")) {}",
        "fn main(n: number = string_byte_len(\"a\")) {}",
        "fn main() requires true || string_byte_len(\"a\") == 1 {}",
        "fn main() requires false && string_concat(\"a\", \"b\") == \"ab\" {}",
        "fn main() { return; string_concat(\"x\", 1); }",
        "fn main() { return string_concat({ loop {} }, 1); }",
        "fn main() { return false && string_byte_len(true) == 0; }",
    ] {
        assert!(Compiler::new().analyze(source).is_err(), "{source}");
    }
    Compiler::new()
        .analyze("fn main() -> string { return string_concat({ loop {} }, \"x\"); }")
        .unwrap();
}
