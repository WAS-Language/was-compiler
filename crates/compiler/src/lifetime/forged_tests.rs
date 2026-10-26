use super::*;
use crate::codegen::CodegenError;
use crate::tokens::Span;
use crate::{Compiler, CompilerOptions, OptimizationLevel};

fn span() -> Span {
    Span::new(12, 34)
}

fn number(literal: &str) -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::Number(literal.into()),
        ty: Type::Number.into(),
    }
}

fn boolean() -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::Bool(true),
        ty: Type::Bool.into(),
    }
}

fn array(elements: Vec<TypedExpr>) -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::Array(elements, ArrayElementType::Number),
        ty: Type::Array(ArrayElementType::Number).into(),
    }
}

fn index(base: TypedExpr, index: TypedExpr) -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::Index {
            expr: Box::new(base),
            index: Box::new(index),
        },
        ty: Type::Number.into(),
    }
}

fn len(args: Vec<TypedExpr>) -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::Call {
            func: FunctionRef::Builtin(BuiltinFunction::Len),
            name: "len".into(),
            args,
        },
        ty: Type::Number.into(),
    }
}

fn mismatch(expected: Type, found: Type) -> (CodegenError, String) {
    (
        CodegenError::TypeMismatch {
            function: "bad".into(),
            expected,
            found,
            span: span(),
        },
        format!(
            "type mismatch in function `bad`: expected {expected:?}, found {found:?} at Span {{ start: 12, end: 34 }}"
        ),
    )
}

fn invalid_literal(literal: &str) -> (CodegenError, String) {
    (
        CodegenError::InvalidNumberLiteral {
            function: "bad".into(),
            literal: literal.into(),
            span: span(),
        },
        format!(
            "invalid number literal `{literal}` in function `bad` at Span {{ start: 12, end: 34 }}"
        ),
    )
}

fn assert_lifetime_noninterference(program: &TypedProgram, expected: &CodegenError, message: &str) {
    let original = program.clone();
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        let compiler = Compiler::with_options(CompilerOptions {
            optimization_level: level,
        });
        let before = compiler.compile(program).expect_err("reject forged tree");
        assert_eq!(&before, expected, "before lifetime analysis at {level:?}");
        assert_eq!(before.to_string(), message, "{level:?}");
        assert_eq!(before.span(), Some(span()), "{level:?}");

        let facts = analyze(program);
        assert!(!facts.is_empty(), "fixture must exercise lifetime origins");
        assert_eq!(format!("{facts:?}"), format!("{:?}", analyze(program)));
        assert_eq!(program, &original, "analysis changed tree at {level:?}");

        let after = compiler
            .compile(program)
            .expect_err("still reject forged tree");
        assert_eq!(&after, expected, "after lifetime analysis at {level:?}");
        assert_eq!(after.to_string(), message, "{level:?}");
        assert_eq!(after.span(), Some(span()), "{level:?}");
        assert_eq!(program, &original, "compilation changed tree at {level:?}");
    }
}

#[test]
fn lifetime_forged_array_index_and_len_metadata_preserve_independent_errors() {
    // Expected variants and ordering follow codegen/memory_tests/validation.rs,
    // not a compilation of the same forged fixture used as an oracle.
    let mut wrong_array = array(vec![]);
    wrong_array.ty = Type::Number.into();
    let mut wrong_element_tag = array(vec![]);
    wrong_element_tag.kind = TypedExprKind::Array(vec![], ArrayElementType::Bool);
    let mut wrong_index = index(array(vec![]), number("0"));
    wrong_index.ty = Type::Bool.into();
    let mut wrong_len = len(vec![array(vec![])]);
    wrong_len.ty = Type::Bool.into();
    let mut wrong_element = number("1");
    wrong_element.ty = Type::Bool.into();
    let cases = [
        (
            wrong_array,
            mismatch(Type::Array(ArrayElementType::Number), Type::Number),
        ),
        (
            wrong_element_tag,
            mismatch(
                Type::Array(ArrayElementType::Bool),
                Type::Array(ArrayElementType::Number),
            ),
        ),
        (wrong_index, mismatch(Type::Number, Type::Bool)),
        (wrong_len, mismatch(Type::Number, Type::Bool)),
        (
            array(vec![wrong_element]),
            mismatch(Type::Number, Type::Bool),
        ),
        (
            index(boolean(), number("NaN")),
            mismatch(Type::Array(ArrayElementType::Number), Type::Bool),
        ),
        (
            index(array(vec![]), boolean()),
            mismatch(Type::Number, Type::Bool),
        ),
        (
            len(vec![boolean()]),
            mismatch(Type::Array(ArrayElementType::Number), Type::Bool),
        ),
        (
            array(vec![number("NaN"), boolean()]),
            invalid_literal("NaN"),
        ),
        (
            array(vec![boolean(), number("NaN")]),
            mismatch(Type::Number, Type::Bool),
        ),
        (
            index(array(vec![number("-1")]), number("NaN")),
            invalid_literal("-1"),
        ),
    ];
    for (value, (expected, message)) in cases {
        let mut program = Compiler::new()
            .analyze("fn bad() { let a = [1]; a[0]; }")
            .unwrap();
        let statement = &mut program.functions[0].body[1];
        statement.kind = TypedStatementKind::Expression(value);
        statement.span = span();
        assert_lifetime_noninterference(&program, &expected, &message);
    }
}

#[test]
fn lifetime_forged_len_arity_precedes_hidden_malformed_arguments() {
    for args in [vec![], vec![array(vec![]), number("NaN")]] {
        let count = args.len();
        let mut program = Compiler::new()
            .analyze("fn bad() { let a = [1]; len(a); }")
            .unwrap();
        let statement = &mut program.functions[0].body[1];
        statement.kind = TypedStatementKind::Expression(len(args));
        statement.span = span();
        let expected = CodegenError::ArgumentCountMismatch {
            function: "bad".into(),
            reference: FunctionRef::Builtin(BuiltinFunction::Len),
            expected: 1,
            found: count,
            span: span(),
        };
        let message = format!(
            "call to Builtin(Len) in function `bad` expects 1 arguments, found {count} at Span {{ start: 12, end: 34 }}"
        );
        assert_lifetime_noninterference(&program, &expected, &message);
    }
}

#[test]
fn lifetime_forged_short_circuit_operands_remain_backend_checked() {
    for expression in ["false && a[0] == 0", "true || a[0] == 0"] {
        let source = format!("fn bad() {{ let a = [1]; {expression}; }}");
        let mut program = Compiler::new().analyze(&source).unwrap();
        let statement = &mut program.functions[0].body[1];
        statement.span = span();
        let TypedStatementKind::Expression(expression) = &mut statement.kind else {
            panic!("logical expression")
        };
        let TypedExprKind::Binary { right, .. } = &mut expression.kind else {
            panic!("short circuit")
        };
        let TypedExprKind::Binary { left, .. } = &mut right.kind else {
            panic!("comparison")
        };
        let TypedExprKind::Index { index, .. } = &mut left.kind else {
            panic!("array read")
        };
        **index = number("NaN");
        let (expected, message) = invalid_literal("NaN");
        assert_lifetime_noninterference(&program, &expected, &message);
    }
}

#[test]
fn lifetime_forged_unreachable_and_skipped_nested_operands_remain_backend_checked() {
    for prefix in ["", "return;"] {
        let source =
            format!("fn bad() {{ let a = [1]; {prefix} if false {{ while false {{ a[0]; }} }} }}");
        let mut program = Compiler::new().analyze(&source).unwrap();
        let position = if prefix.is_empty() { 1 } else { 2 };
        let TypedStatementKind::If { then, .. } = &mut program.functions[0].body[position].kind
        else {
            panic!("skipped if")
        };
        let TypedStatementKind::While { body, .. } = &mut then[0].kind else {
            panic!("skipped while")
        };
        body[0].span = span();
        let TypedStatementKind::Expression(expression) = &mut body[0].kind else {
            panic!("nested read")
        };
        let TypedExprKind::Index { index, .. } = &mut expression.kind else {
            panic!("array index")
        };
        **index = number("NaN");
        let (expected, message) = invalid_literal("NaN");
        assert_lifetime_noninterference(&program, &expected, &message);
    }
}
