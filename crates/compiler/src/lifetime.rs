//! Explicit, test-only lifetime analysis. Candidate status never selects storage.

use crate::{
    ast::{ArrayElementType, BinaryOp, Type, UnaryOp},
    typed_ast::{
        BuiltinFunction, FunctionRef, TypedExpr, TypedExprKind, TypedFunction, TypedProgram,
        TypedStatementKind, VariableId,
    },
};

mod diagnostics;

#[cfg(test)]
mod conservative_tests;
#[cfg(test)]
mod forged_tests;
#[cfg(test)]
mod noninterference_tests;
#[cfg(test)]
mod runtime_tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnprovenReason {
    OutsideSubset,
    HandleUse,
    HandleReassignment,
    CallBoundary,
    ControlFlow,
    CallableDeclaration,
    UnsupportedType,
    InconsistentTree,
    AliasCreation,
    HostObservation,
    CaptureStorage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EscapePath {
    ReturnedHandle,
    PotentialHostPublication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reason {
    Escape(EscapePath),
    Unproven(UnprovenReason),
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    TemporaryCandidate,
    Escape(EscapePath),
    Unproven(UnprovenReason),
}

#[derive(Debug)]
struct Origin<'tree> {
    function: &'tree TypedFunction,
    binding: VariableId,
    literal: &'tree TypedExpr,
    element: ArrayElementType,
}

#[derive(Debug)]
struct Fact<'tree> {
    origin: Origin<'tree>,
    outcome: Outcome,
    reasons: Vec<Reason>,
}

// This slice reports only direct function-body let literals. Nested allocation
// discovery is deferred; their enclosing syntax already blocks positive proof.
fn analyze(program: &TypedProgram) -> Vec<Fact<'_>> {
    let mut facts = Vec::new();
    for function in &program.functions {
        let proof = prove_function(function);
        for statement in &function.body {
            if let TypedStatementKind::Let { id, value, .. } = &statement.kind
                && let TypedExprKind::Array(_, element) = &value.kind
            {
                let mut reasons = diagnostics::collect(function, value);
                if let Err(reason) = proof
                    && !reasons.contains(&Reason::Unproven(reason))
                {
                    reasons.push(Reason::Unproven(reason));
                }
                let outcome = if reasons.contains(&Reason::Escape(EscapePath::ReturnedHandle)) {
                    Outcome::Escape(EscapePath::ReturnedHandle)
                } else if reasons.contains(&Reason::Escape(EscapePath::PotentialHostPublication)) {
                    Outcome::Escape(EscapePath::PotentialHostPublication)
                } else {
                    match proof {
                        Ok(()) => Outcome::TemporaryCandidate,
                        Err(reason) => Outcome::Unproven(reason),
                    }
                };
                facts.push(Fact {
                    origin: Origin {
                        function,
                        binding: *id,
                        literal: value,
                        element: *element,
                    },
                    outcome,
                    reasons,
                });
            }
        }
    }
    facts
}

// Borrowed positive facts only; storage consumers cannot alter or widen the proof.
pub(crate) fn temporary_literals(
    program: &TypedProgram,
) -> impl Iterator<Item = (&TypedFunction, &TypedExpr)> {
    analyze(program).into_iter().filter_map(|fact| {
        (fact.outcome == Outcome::TemporaryCandidate)
            .then_some((fact.origin.function, fact.origin.literal))
    })
}

type Bindings = std::collections::HashMap<VariableId, Type>;
type Proof<T = ()> = Result<T, UnprovenReason>;

fn scalar_type(ty: Type) -> Proof<Type> {
    match ty {
        Type::Number | Type::Bool => Ok(ty),
        _ => Err(UnprovenReason::UnsupportedType),
    }
}

fn insert_binding(bindings: &mut Bindings, id: VariableId, ty: Type) -> Proof {
    if bindings.insert(id, ty).is_some() {
        return Err(UnprovenReason::InconsistentTree);
    }
    Ok(())
}

fn prove_function(function: &TypedFunction) -> Proof {
    let mut bindings = Bindings::new();
    for param in &function.params {
        insert_binding(&mut bindings, param.id, scalar_type(param.ty)?)?;
    }
    for contract in &function.contracts {
        require_scalar(&contract.value, Type::Bool, &bindings)?;
    }
    for statement in &function.body {
        match &statement.kind {
            TypedStatementKind::Let { id, ty, value, .. } => {
                match (&value.kind, ty) {
                    (TypedExprKind::Array(elements, element), Type::Array(expected))
                        if element == expected
                            && matches!(
                                element,
                                ArrayElementType::Number | ArrayElementType::Bool
                            )
                            && value.ty == *ty =>
                    {
                        for element_value in elements {
                            require_scalar(element_value, element.value_type(), &bindings)?;
                        }
                    }
                    _ => require_scalar(value, scalar_type(*ty)?, &bindings)?,
                }
                insert_binding(&mut bindings, *id, *ty)?;
            }
            TypedStatementKind::Assignment { target, value } => {
                let ty = place(target, &bindings)?;
                require_scalar(value, ty, &bindings)?;
            }
            TypedStatementKind::CompoundAssignment { target, op, value } => {
                if !matches!(target.kind, TypedExprKind::Index { .. }) || !arithmetic(*op) {
                    return Err(UnprovenReason::OutsideSubset);
                }
                if place(target, &bindings)? != Type::Number {
                    return Err(UnprovenReason::InconsistentTree);
                }
                require_scalar(value, Type::Number, &bindings)?;
            }
            TypedStatementKind::Expression(expr) => {
                scalar(expr, &bindings)?;
            }
            TypedStatementKind::Return(value) => match value {
                Some(value) => {
                    require_scalar(value, scalar_type(function.return_type)?, &bindings)?
                }
                None if function.return_type == Type::Void => {}
                None => return Err(UnprovenReason::InconsistentTree),
            },
            TypedStatementKind::Closure(_) => return Err(UnprovenReason::CallableDeclaration),
            TypedStatementKind::If { .. }
            | TypedStatementKind::Match { .. }
            | TypedStatementKind::While { .. }
            | TypedStatementKind::For { .. }
            | TypedStatementKind::Loop { .. }
            | TypedStatementKind::Break { .. }
            | TypedStatementKind::BreakValue { .. }
            | TypedStatementKind::Continue { .. } => {
                return Err(UnprovenReason::ControlFlow);
            }
        }
    }
    Ok(())
}

fn arithmetic(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder
    )
}

fn require_scalar(expr: &TypedExpr, expected: Type, bindings: &Bindings) -> Proof {
    if scalar(expr, bindings)? != expected {
        return Err(UnprovenReason::InconsistentTree);
    }
    Ok(())
}

fn array_binding(expr: &TypedExpr, bindings: &Bindings) -> Proof<Type> {
    if let TypedExprKind::Identifier { id, .. } = expr.kind
        && let Some(ty @ Type::Array(ArrayElementType::Number | ArrayElementType::Bool)) =
            bindings.get(&id)
        && expr.ty == *ty
    {
        return Ok(*ty);
    }
    Err(UnprovenReason::HandleUse)
}

fn place(expr: &TypedExpr, bindings: &Bindings) -> Proof<Type> {
    match &expr.kind {
        TypedExprKind::Identifier { id, .. } => {
            let ty = *bindings.get(id).ok_or(UnprovenReason::InconsistentTree)?;
            if matches!(ty, Type::Array(_)) {
                return Err(UnprovenReason::HandleReassignment);
            }
            Err(UnprovenReason::OutsideSubset)
        }
        TypedExprKind::Index { .. } => scalar(expr, bindings),
        _ => Err(UnprovenReason::OutsideSubset),
    }
}

fn scalar(expr: &TypedExpr, bindings: &Bindings) -> Proof<Type> {
    let ty = match &expr.kind {
        TypedExprKind::Number(_) => Type::Number,
        TypedExprKind::Bool(_) => Type::Bool,
        TypedExprKind::Identifier { id, .. } => {
            scalar_type(*bindings.get(id).ok_or(UnprovenReason::InconsistentTree)?)?
        }
        TypedExprKind::Unary { op, expr } => {
            let ty = match op {
                UnaryOp::Negate => Type::Number,
                UnaryOp::Not => Type::Bool,
            };
            require_scalar(expr, ty, bindings)?;
            ty
        }
        TypedExprKind::Binary { left, op, right } => {
            if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::In) {
                return Err(UnprovenReason::ControlFlow);
            }
            let left = scalar(left, bindings)?;
            require_scalar(right, left, bindings)?;
            if arithmetic(*op) {
                if left != Type::Number {
                    return Err(UnprovenReason::InconsistentTree);
                }
                Type::Number
            } else {
                if !matches!(op, BinaryOp::Equal | BinaryOp::NotEqual) && left != Type::Number {
                    return Err(UnprovenReason::InconsistentTree);
                }
                Type::Bool
            }
        }
        TypedExprKind::Index { expr, index } => {
            let Type::Array(element) = array_binding(expr, bindings)? else {
                unreachable!()
            };
            require_scalar(index, Type::Number, bindings)?;
            element.value_type()
        }
        TypedExprKind::Call {
            func: FunctionRef::Builtin(BuiltinFunction::Len),
            args,
            ..
        } if args.len() == 1 => {
            array_binding(&args[0], bindings)?;
            Type::Number
        }
        TypedExprKind::Call { .. }
        | TypedExprKind::NormalizedCall(_)
        | TypedExprKind::LexicalCall { .. } => {
            return Err(UnprovenReason::CallBoundary);
        }
        TypedExprKind::Block(_)
        | TypedExprKind::If { .. }
        | TypedExprKind::Match { .. }
        | TypedExprKind::Loop { .. } => {
            return Err(UnprovenReason::ControlFlow);
        }
        TypedExprKind::Reserve { .. }
        | TypedExprKind::Push { .. }
        | TypedExprKind::Pop { .. }
        | TypedExprKind::Remove { .. }
        | TypedExprKind::String(_)
        | TypedExprKind::Captured { .. }
        | TypedExprKind::Range { .. }
        | TypedExprKind::Array(..) => {
            return Err(UnprovenReason::OutsideSubset);
        }
    };
    if expr.ty != ty {
        return Err(UnprovenReason::InconsistentTree);
    }
    Ok(ty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Compiler;

    #[test]
    fn lifetime_alias_return_is_an_identified_path_not_just_an_unknown_alias() {
        assert_eq!(
            outcomes("fn main() -> number[] { let a = [1]; let b = a; let c = b; return c; }"),
            vec![Outcome::Escape(EscapePath::ReturnedHandle)]
        );
    }

    #[test]
    fn lifetime_reports_later_host_and_return_paths_after_unsupported_syntax() {
        let mut compiler = Compiler::new();
        compiler
            .register_host_function(
                "publish",
                "env",
                "publish",
                vec![Type::Array(ArrayElementType::Number)],
                Type::Void,
            )
            .unwrap();
        let program = compiler
            .analyze("fn main() -> number[] { let a = [1]; if false {} publish(a); return a; }")
            .unwrap();
        let facts = analyze(&program);
        assert_eq!(
            facts[0].outcome,
            Outcome::Escape(EscapePath::ReturnedHandle)
        );
        assert!(
            facts[0]
                .reasons
                .contains(&Reason::Unproven(UnprovenReason::ControlFlow))
        );
        assert!(
            facts[0]
                .reasons
                .contains(&Reason::Escape(EscapePath::PotentialHostPublication))
        );
        assert!(
            facts[0]
                .reasons
                .contains(&Reason::Escape(EscapePath::ReturnedHandle))
        );
        assert_eq!(
            facts[0].reasons,
            vec![
                Reason::Unproven(UnprovenReason::ControlFlow),
                Reason::Unproven(UnprovenReason::HostObservation),
                Reason::Escape(EscapePath::PotentialHostPublication),
                Reason::Escape(EscapePath::ReturnedHandle),
            ]
        );
        assert_eq!(facts[0].reasons, analyze(&program)[0].reasons);
    }

    #[test]
    fn lifetime_returned_block_tail_preserves_an_identifiable_path() {
        assert_eq!(
            outcomes("fn main() -> number[] { let a = [1]; return { a }; }"),
            vec![Outcome::Escape(EscapePath::ReturnedHandle)]
        );
    }

    #[test]
    fn lifetime_host_barriers_are_invocation_wide_and_publication_is_only_potential() {
        let mut compiler = Compiler::new();
        compiler
            .register_host_function("observe", "env", "observe", vec![], Type::Bool)
            .unwrap();
        compiler
            .register_host_function(
                "publish",
                "env",
                "publish",
                vec![Type::Array(ArrayElementType::Number)],
                Type::Bool,
            )
            .unwrap();
        for source in [
            "fn main() { observe(); let a = [1]; }",
            "fn main() { let a = [1]; let n = a[0]; observe(); }",
            "fn main() { let a = [1]; return; observe(); }",
        ] {
            let program = compiler.analyze(source).unwrap();
            let facts = analyze(&program);
            assert!(matches!(facts[0].outcome, Outcome::Unproven(_)), "{source}");
            assert!(
                facts[0]
                    .reasons
                    .contains(&Reason::Unproven(UnprovenReason::HostObservation))
            );
            assert!(
                !facts[0]
                    .reasons
                    .iter()
                    .any(|r| matches!(r, Reason::Escape(_)))
            );
        }
        let program = compiler
            .analyze("fn main() { let a = [1]; let alias = a; publish(alias); }")
            .unwrap();
        assert_eq!(
            analyze(&program)[0].outcome,
            Outcome::Escape(EscapePath::PotentialHostPublication)
        );
    }

    #[test]
    fn lifetime_nested_and_skipped_children_do_not_hide_host_paths() {
        let mut compiler = Compiler::new();
        compiler
            .register_host_function(
                "publish",
                "env",
                "publish",
                vec![Type::Array(ArrayElementType::Number)],
                Type::Bool,
            )
            .unwrap();
        for snippet in [
            "if false { publish(a); }",
            "while false { publish(a); }",
            "for n in 0..0 { publish(a); }",
            "loop { break; publish(a); }",
            "match true { true => false, false => publish(a) };",
            "false && publish(a);",
            "true || publish(a);",
            "let n = { publish(a); 1 };",
            "let n = if true { 1 } else { publish(a); 2 };",
            "let b = [publish(a)];",
            "a[{ publish(a); 0 }] = 1;",
            "a[0] += { publish(a); 1 };",
            "let f = () => { publish(a); 1 };",
            "1 |> ((n: number) => { publish(a); n });",
            "helper(n: { publish(a); 1 });",
            "publish({ let copy = a; copy });",
        ] {
            let source =
                format!("fn helper(n: number = 1) {{}} fn main() {{ let a = [1]; {snippet} }}");
            let program = compiler
                .analyze(&source)
                .unwrap_or_else(|e| panic!("{source}: {e}"));
            let facts = analyze(&program);
            assert_eq!(
                facts[0].outcome,
                Outcome::Escape(EscapePath::PotentialHostPublication),
                "{snippet}: {:?}",
                facts[0].reasons
            );
            assert!(
                facts[0]
                    .reasons
                    .contains(&Reason::Unproven(UnprovenReason::HostObservation))
            );
        }
    }

    #[test]
    fn lifetime_aliases_and_reassignments_are_not_automatic_escape_evidence() {
        for source in [
            "fn main() { let a = [1]; let alias = a; }",
            "fn main() -> number[] { let a = [1]; let alias = a; alias = [2]; return alias; }",
            "fn main() -> number[] { let a = [1]; a = [2]; return a; }",
            "fn main() -> number[] { let a = [1]; let alias = a; if false { alias = [2]; } return alias; }",
        ] {
            let program = Compiler::new().analyze(source).unwrap();
            let facts = analyze(&program);
            assert!(matches!(facts[0].outcome, Outcome::Unproven(_)), "{source}");
            assert!(
                !facts[0]
                    .reasons
                    .iter()
                    .any(|r| matches!(r, Reason::Escape(_))),
                "{source}"
            );
            assert!(facts[0].reasons.iter().any(|r| matches!(
                r,
                Reason::Unproven(
                    UnprovenReason::AliasCreation | UnprovenReason::HandleReassignment
                )
            )));
        }
    }

    #[test]
    fn lifetime_contracts_and_normalized_defaults_are_structurally_inspected() {
        use crate::typed_ast::TypedContract;
        let mut compiler = Compiler::new();
        compiler
            .register_host_function("observe", "env", "observe", vec![], Type::Number)
            .unwrap();
        let original = compiler
            .analyze("fn helper(n: number = 1) {} fn main() { let a = [1]; helper(); observe(); }")
            .unwrap();
        for in_contract in [false, true] {
            let mut program = original.clone();
            let function = &mut program.functions[1];
            let statement = function.body.pop().unwrap();
            let TypedStatementKind::Expression(host) = statement.kind else {
                panic!()
            };
            if in_contract {
                function.contracts.push(TypedContract {
                    value: host,
                    span: statement.span,
                });
            } else {
                let TypedStatementKind::Expression(expr) = &mut function.body[1].kind else {
                    panic!()
                };
                let TypedExprKind::NormalizedCall(call) = &mut expr.kind else {
                    panic!()
                };
                call.bindings[0].value = host;
            }
            let facts = analyze(&program);
            assert!(matches!(facts[0].outcome, Outcome::Unproven(_)));
            assert!(
                facts[0]
                    .reasons
                    .contains(&Reason::Unproven(UnprovenReason::HostObservation))
            );
        }
    }

    #[test]
    fn lifetime_direct_numeric_literal_is_a_candidate() {
        let program = Compiler::new()
            .analyze("fn main() { let a = [1]; }")
            .unwrap();
        let facts = analyze(&program);
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].outcome, Outcome::TemporaryCandidate);
        assert!(std::ptr::eq(
            facts[0].origin.function,
            &program.functions[0]
        ));
        assert_eq!(facts[0].origin.binding, VariableId(0));
        assert_eq!(facts[0].origin.element, ArrayElementType::Number);
        let TypedStatementKind::Let { value, .. } = &program.functions[0].body[0].kind else {
            panic!()
        };
        assert!(std::ptr::eq(facts[0].origin.literal, value));
    }

    fn outcomes(source: &str) -> Vec<Outcome> {
        let program = Compiler::new()
            .analyze(source)
            .unwrap_or_else(|e| panic!("{source}: {e}"));
        analyze(&program)
            .into_iter()
            .map(|fact| fact.outcome)
            .collect()
    }

    #[test]
    fn lifetime_boolean_empty_and_nested_scalar_operations() {
        for source in [
            "fn main(n: number, flag: bool) -> bool { let a = [n + 1, -n]; let b = [!flag, a[0] < n]; let k = len(a) - 2; a[k] = a[a[0] - a[0]] * 2; b[len(b) - 1] = !(b[0] == flag); return b[0] != b[1]; }",
            "fn main() { let a: number[] = []; let b: bool[] = []; return; }",
            "fn main() -> number { let a = [1, 2]; let b = [len(a), a[0] / 0 % 2]; return -b[0]; }",
        ] {
            assert_eq!(
                outcomes(source),
                vec![Outcome::TemporaryCandidate, Outcome::TemporaryCandidate],
                "{source}"
            );
        }
    }

    #[test]
    fn lifetime_each_scalar_operator_and_numeric_element_compound() {
        for op in ["+", "-", "*", "/", "%", "<", ">", "<=", ">=", "==", "!="] {
            let source = format!("fn main(n: number) {{ let a = [1]; let value = a[0] {op} -n; }}");
            assert_eq!(outcomes(&source), vec![Outcome::TemporaryCandidate], "{op}");
        }
        for op in ["+=", "-=", "*=", "/=", "%="] {
            let source =
                format!("fn main(n: number) {{ let a = [1]; a[len(a) - 1] {op} a[0] + n; }}");
            assert_eq!(outcomes(&source), vec![Outcome::TemporaryCandidate], "{op}");
        }
    }

    #[test]
    fn lifetime_excluded_forms_block_the_entire_invocation_even_after_return() {
        for suffix in [
            "let alias = a;",
            "a = [2];",
            "a;",
            "a == a;",
            "let s = [\"text\"];",
            "let r = 0..1;",
            "let s = \"text\";",
            "if true {}",
            "while false {}",
            "loop { break; }",
            "for n in a {}",
            "match true { _ => 1 };",
            "let x = { 1 };",
            "let x = if true { 1 } else { 2 };",
            "let x = loop { break 1; };",
            "let x = true && false;",
            "let x = true || false;",
            "let f = () => 1;",
            "let f = () => a[0];",
            "helper();",
            "let x = [helper()];",
            "a[helper()] = 1;",
            "a[0] = helper();",
            "let x = [if true { 1 } else { 2 }];",
            "a[{ 0 }] = 1;",
            "a[0] += { 1 };",
            "let x = !(true && false);",
        ] {
            let source = format!(
                "fn helper() -> number {{ return 0; }} fn main() {{ let a = [1]; return; {suffix} }}"
            );
            let results = outcomes(&source);
            assert!(!results.is_empty());
            assert!(
                results
                    .iter()
                    .all(|outcome| matches!(outcome, Outcome::Unproven(_))),
                "{suffix}: {results:?}"
            );
        }
    }

    #[test]
    fn lifetime_reasons_distinguish_returns_reassignment_calls_and_control_flow() {
        for (source, expected) in [
            (
                "fn main() -> number[] { let a = [1]; return a; }",
                Outcome::Escape(EscapePath::ReturnedHandle),
            ),
            (
                "fn main() { let a = [1]; a = [2]; }",
                Outcome::Unproven(UnprovenReason::HandleReassignment),
            ),
            (
                "fn main() { let a = [1]; if false {} }",
                Outcome::Unproven(UnprovenReason::ControlFlow),
            ),
            (
                "fn helper() {} fn main() { let a = [1]; helper(); }",
                Outcome::Unproven(UnprovenReason::CallBoundary),
            ),
            (
                "fn main() { let a = [1]; let f = () => 1; }",
                Outcome::Unproven(UnprovenReason::CallableDeclaration),
            ),
        ] {
            assert_eq!(outcomes(source), vec![expected]);
        }
    }

    #[test]
    fn lifetime_host_observation_is_not_proof_of_actual_retention() {
        let mut compiler = Compiler::new();
        compiler
            .register_host_function("observe", "env", "observe", vec![], Type::Void)
            .unwrap();
        let program = compiler
            .analyze("fn main() { let a = [true]; observe(); }")
            .unwrap();
        assert_eq!(
            analyze(&program)[0].outcome,
            Outcome::Unproven(UnprovenReason::CallBoundary)
        );
    }

    #[test]
    fn lifetime_scalar_writes_are_not_implicitly_added_to_the_closed_set() {
        assert_eq!(
            outcomes("fn main() { let a = [1]; let n = 0; n = 1; }"),
            vec![Outcome::Unproven(UnprovenReason::OutsideSubset)]
        );
    }

    #[test]
    fn lifetime_contracts_and_handle_parameters_cannot_bypass_the_subset() {
        for source in [
            "fn main(p: number[]) { let a = [1]; }",
            "fn main(p: bool) requires p || true { let a = [1]; }",
            "fn main(p: number[]) requires len(p) > 0 { let a = [1]; }",
        ] {
            assert!(
                matches!(outcomes(source)[0], Outcome::Unproven(_)),
                "{source}"
            );
        }
        assert_eq!(
            outcomes("fn main(p: number) requires p >= 0 { let a = [p]; }"),
            vec![Outcome::TemporaryCandidate]
        );
    }

    #[test]
    fn lifetime_fact_order_and_binding_identity_are_owner_local() {
        let program = Compiler::new().analyze("fn first() { let a = [1]; let b = [false]; } fn second() { let a = [2]; if false {} }").unwrap();
        for _ in 0..2 {
            let facts = analyze(&program);
            assert_eq!(
                facts
                    .iter()
                    .map(|f| f.origin.function.name.as_str())
                    .collect::<Vec<_>>(),
                ["first", "first", "second"]
            );
            assert_eq!(
                facts.iter().map(|f| f.origin.binding).collect::<Vec<_>>(),
                [VariableId(0), VariableId(1), VariableId(0)]
            );
            assert_eq!(facts[0].outcome, Outcome::TemporaryCandidate);
            assert_eq!(facts[1].outcome, Outcome::TemporaryCandidate);
            assert_eq!(
                facts[2].outcome,
                Outcome::Unproven(UnprovenReason::ControlFlow)
            );
        }
    }

    #[test]
    fn lifetime_forged_annotations_remain_unproven_and_backend_checked() {
        let compiler = Compiler::new();
        let mut program = compiler.analyze("fn main() { let a = [true]; }").unwrap();
        let TypedStatementKind::Let { value, .. } = &mut program.functions[0].body[0].kind else {
            panic!()
        };
        let TypedExprKind::Array(elements, _) = &mut value.kind else {
            panic!()
        };
        elements[0].ty = Type::Number.into();
        assert_eq!(
            analyze(&program)[0].outcome,
            Outcome::Unproven(UnprovenReason::InconsistentTree)
        );
        assert!(compiler.compile(&program).is_err());
    }

    #[test]
    fn lifetime_analysis_preserves_bytes_and_tree_at_each_preset() {
        use crate::{CompilerOptions, OptimizationLevel};
        let program = Compiler::new()
            .analyze(
                "fn main(n: number) -> number { let a = [n, 2]; a[0] %= 3; return a[0] + len(a); }",
            )
            .unwrap();
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
            let before = compiler.compile(&program).unwrap();
            assert_eq!(analyze(&program)[0].outcome, Outcome::TemporaryCandidate);
            assert_eq!(before, compiler.compile(&program).unwrap());
            wasmparser::Validator::new().validate_all(&before).unwrap();
            assert_eq!(program, original);
        }
    }
}
