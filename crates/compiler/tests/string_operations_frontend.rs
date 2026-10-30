use was_compiler::host::HostRegistrationError;
use was_compiler::semantic::SemanticError;
use was_compiler::{CompileError, Compiler, Type};

fn error(source: &str) -> SemanticError {
    match Compiler::new().analyze(source).unwrap_err() {
        CompileError::Semantic(error) => error,
        other => panic!("expected semantic error, got {other:?}"),
    }
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
    for name in ["string_byte_len", "string_concat", "string_slice_bytes"] {
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
