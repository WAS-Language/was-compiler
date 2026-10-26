use super::*;
use crate::Compiler;
use crate::typed_ast::{BindingOwner, TypedClosureBody};

fn program(source: &str) -> TypedProgram {
    Compiler::new()
        .analyze(source)
        .unwrap_or_else(|error| panic!("source failed analysis:\n{source}\n{error}"))
}

fn assert_unproven(fact: &Fact<'_>, reasons: &[UnprovenReason], source: &str) {
    assert!(
        matches!(fact.outcome, Outcome::Unproven(_)),
        "{source}\nexpected unproven, got {fact:?}"
    );
    for reason in reasons {
        assert!(
            fact.reasons.contains(&Reason::Unproven(*reason)),
            "{source}\nmissing {reason:?}: {fact:?}"
        );
    }
    assert!(
        fact.reasons
            .iter()
            .all(|reason| !matches!(reason, Reason::Escape(_))),
        "{source}\nunproven boundary is not an identified escape: {fact:?}"
    );
}

#[test]
fn conservative_sparse_captured_binding_keeps_original_owner_identity() {
    let source = "fn main() { let a = [1]; let unused = () => { return a; }; }";
    let mut tree = program(source);
    let sparse = VariableId(10_001);
    let TypedStatementKind::Let { id, .. } = &mut tree.functions[0].body[0].kind else {
        panic!("root binding")
    };
    *id = sparse;
    let owner = BindingOwner::Function(tree.functions[0].id);
    let TypedStatementKind::Closure(helper) = &mut tree.functions[0].body[1].kind else {
        panic!("helper")
    };
    assert_eq!(helper.captures.len(), 1);
    assert_eq!(helper.captures[0].owner, owner);
    helper.captures[0].variable = sparse;
    let TypedClosureBody::Block(body) = &mut helper.body else {
        panic!("helper block")
    };
    let TypedStatementKind::Return(Some(value)) = &mut body.statements[0].kind else {
        panic!("helper return")
    };
    let TypedExprKind::Captured {
        owner: captured_owner,
        id,
        ..
    } = &mut value.kind
    else {
        panic!("capture read")
    };
    assert_eq!(*captured_owner, owner);
    *id = sparse;

    let facts = analyze(&tree);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].origin.binding, sparse);
    assert_unproven(
        &facts[0],
        &[
            UnprovenReason::CallableDeclaration,
            UnprovenReason::CaptureStorage,
        ],
        source,
    );
    assert!(
        !facts[0]
            .reasons
            .contains(&Reason::Unproven(UnprovenReason::InconsistentTree))
    );
}

#[test]
fn conservative_duplicate_helper_ids_are_not_hidden_by_owner_boundary() {
    let source =
        "fn main() { let a = [1]; let helper = (n: number) => { let own = [n]; return own; }; }";
    let mut tree = program(source);
    let TypedStatementKind::Closure(helper) = &mut tree.functions[0].body[1].kind else {
        panic!("helper")
    };
    let duplicate = helper.params[0].id;
    let TypedClosureBody::Block(body) = &mut helper.body else {
        panic!("helper block")
    };
    let TypedStatementKind::Let { id, .. } = &mut body.statements[0].kind else {
        panic!("helper binding")
    };
    *id = duplicate;
    let TypedStatementKind::Return(Some(value)) = &mut body.statements[1].kind else {
        panic!("helper return")
    };
    let TypedExprKind::Identifier { id, .. } = &mut value.kind else {
        panic!("returned binding")
    };
    *id = duplicate;
    let facts = analyze(&tree);
    assert_eq!(facts.len(), 1);
    assert_unproven(
        &facts[0],
        &[
            UnprovenReason::CallableDeclaration,
            UnprovenReason::InconsistentTree,
        ],
        source,
    );
}

#[test]
fn conservative_duplicate_root_ids_after_callable_barrier_are_inconsistent() {
    let source = "fn main() { let a = [1]; let unused = () => 0; let b = [2]; }";
    let mut tree = program(source);
    let TypedStatementKind::Let { id: first, .. } = tree.functions[0].body[0].kind else {
        panic!("first binding")
    };
    let TypedStatementKind::Let { id, .. } = &mut tree.functions[0].body[2].kind else {
        panic!("second binding")
    };
    *id = first;
    let facts = analyze(&tree);
    assert_eq!(facts.len(), 2);
    assert_eq!(facts[0].origin.binding, facts[1].origin.binding);
    assert!(!std::ptr::eq(
        facts[0].origin.literal,
        facts[1].origin.literal
    ));
    for fact in &facts {
        assert_unproven(
            fact,
            &[
                UnprovenReason::CallableDeclaration,
                UnprovenReason::InconsistentTree,
            ],
            source,
        );
    }
}

#[test]
fn conservative_sparse_binding_ids_preserve_candidate_origin() {
    let source = "fn main() { let a = [true]; a[0]; }";
    let mut tree = program(source);
    let sparse = VariableId(10_001);
    let TypedStatementKind::Let { id, .. } = &mut tree.functions[0].body[0].kind else {
        panic!("array binding")
    };
    *id = sparse;
    let TypedStatementKind::Expression(read) = &mut tree.functions[0].body[1].kind else {
        panic!("element read")
    };
    let TypedExprKind::Index { expr, .. } = &mut read.kind else {
        panic!("index")
    };
    let TypedExprKind::Identifier { id, .. } = &mut expr.kind else {
        panic!("array identifier")
    };
    *id = sparse;

    let facts = analyze(&tree);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].origin.binding, sparse);
    assert_eq!(facts[0].origin.element, ArrayElementType::Bool);
    let TypedStatementKind::Let { value, .. } = &tree.functions[0].body[0].kind else {
        panic!("array literal")
    };
    assert!(std::ptr::eq(facts[0].origin.literal, value));
    assert_eq!(facts[0].outcome, Outcome::TemporaryCandidate);
    assert!(facts[0].reasons.is_empty());
}

#[test]
fn conservative_recursive_calls_do_not_supply_interprocedural_lifetime_proofs() {
    for source in [
        "fn main() -> number { let a = [1]; return main(); }",
        "fn first() -> number { let a = [1]; return second(); }
         fn second() -> number { let b = [2]; return first(); }",
        "fn main() -> number { let a = [1]; let recurse = () => main(); return recurse(); }",
    ] {
        let tree = program(source);
        let facts = analyze(&tree);
        assert_eq!(facts.len(), tree.functions.len());
        for fact in &facts {
            assert_unproven(fact, &[UnprovenReason::CallBoundary], source);
        }
    }
}

#[test]
fn conservative_placeholder_declarations_remain_barriers_even_when_unused() {
    for suffix in ["", "increment(2);", "2 |> increment;"] {
        let source = format!("fn main() {{ let a = [1]; let increment = _ + a[0]; {suffix} }}");
        let tree = program(&source);
        let facts = analyze(&tree);
        assert_eq!(facts.len(), 1);
        assert_unproven(
            &facts[0],
            &[
                UnprovenReason::CallableDeclaration,
                UnprovenReason::CaptureStorage,
            ],
            &source,
        );
        if !suffix.is_empty() {
            assert!(
                facts[0]
                    .reasons
                    .contains(&Reason::Unproven(UnprovenReason::CallBoundary)),
                "{source}"
            );
        }
    }
}

#[test]
fn conservative_immediate_and_placeholder_stages_do_not_expand_positive_subset() {
    for stage in [
        "((n: number) => n + 1)",
        "(_ + 1)",
        "((n: number) => n + a[0])",
        "(_ + a[0])",
    ] {
        let source = format!("fn main() {{ let a = [1]; a[0] |> {stage}; }}");
        let tree = program(&source);
        let facts = analyze(&tree);
        assert_eq!(facts.len(), 1);
        let mut reasons = vec![
            UnprovenReason::CallBoundary,
            UnprovenReason::CallableDeclaration,
        ];
        if stage.contains("a[0]") {
            reasons.push(UnprovenReason::CaptureStorage);
        }
        assert_unproven(&facts[0], &reasons, &source);
    }
}

#[test]
fn conservative_lexical_calls_and_known_pipeline_stages_are_boundaries() {
    for invocation in ["read(a[0])", "a[0] |> read"] {
        let source = format!(
            "fn main() {{ let a = [1]; let read = (n: number) => n + a[0]; {invocation}; }}"
        );
        let tree = program(&source);
        let facts = analyze(&tree);
        assert_eq!(facts.len(), 1);
        assert_unproven(
            &facts[0],
            &[
                UnprovenReason::CallableDeclaration,
                UnprovenReason::CaptureStorage,
                UnprovenReason::CallBoundary,
            ],
            &source,
        );
    }
}

#[test]
fn conservative_named_and_default_calls_retain_call_boundary_reasons() {
    for invocation in [
        "combine(y: 3, x: a[0])",
        "combine(a[0])",
        "combine(x: a[0])",
        "a[0] |> combine(y: 3)",
        "a[0] |> combine",
    ] {
        let source = format!(
            "fn combine(x: number, y: number = x + 1) -> number {{ return x + y; }}
             fn main() {{ let a = [1]; {invocation}; }}"
        );
        let tree = program(&source);
        let TypedStatementKind::Expression(expression) = &tree.functions[1].body[1].kind else {
            panic!("call expression: {source}")
        };
        assert!(
            matches!(expression.kind, TypedExprKind::NormalizedCall(_)),
            "{source}"
        );
        let facts = analyze(&tree);
        assert_eq!(facts.len(), 1);
        assert_unproven(&facts[0], &[UnprovenReason::CallBoundary], &source);
    }
}

#[test]
fn conservative_handle_arguments_to_internal_calls_are_not_positive_proofs() {
    for invocation in ["inspect(a)", "inspect(value: a)"] {
        let source = format!(
            "fn inspect(value: number[]) -> number {{ return len(value); }}
             fn main() {{ let a = [1]; {invocation}; }}"
        );
        let tree = program(&source);
        let facts = analyze(&tree);
        assert_eq!(facts.len(), 1);
        assert_unproven(&facts[0], &[UnprovenReason::CallBoundary], &source);
    }
}

#[test]
fn conservative_internal_call_blocks_caller_not_independent_callee() {
    let source = "fn helper() -> number { let own = [2]; return own[0]; }
        fn main() { let a = [1]; helper(); }";
    let tree = program(source);
    let facts = analyze(&tree);
    assert_eq!(facts.len(), 2);
    assert_eq!(facts[0].origin.function.name, "helper");
    assert_eq!(facts[0].outcome, Outcome::TemporaryCandidate);
    assert!(facts[0].reasons.is_empty());
    assert_eq!(facts[1].origin.function.name, "main");
    assert_unproven(&facts[1], &[UnprovenReason::CallBoundary], source);
}

#[test]
fn conservative_unused_lexical_dependency_keeps_capture_storage_barrier() {
    let source = "fn main() {
        let a = [1];
        let first = () => a[0];
        let second = () => first();
        let unused = () => second();
    }";
    let tree = program(source);
    let TypedStatementKind::Closure(unused) = &tree.functions[0].body[3].kind else {
        panic!("unused helper")
    };
    assert!(!unused.dependencies.is_empty());
    let facts = analyze(&tree);
    assert_eq!(facts.len(), 1);
    assert_unproven(
        &facts[0],
        &[
            UnprovenReason::CallableDeclaration,
            UnprovenReason::CaptureStorage,
            UnprovenReason::CallBoundary,
        ],
        source,
    );
}

#[test]
fn conservative_nested_helpers_forward_original_capture_without_root_escape() {
    let source = "fn main() {
        let a = [1];
        let outer = () => {
            let middle = () => {
                let inner = () => { return a; };
                return inner();
            };
            return middle();
        };
    }";
    let tree = program(source);
    let TypedStatementKind::Closure(outer) = &tree.functions[0].body[1].kind else {
        panic!("outer helper")
    };
    assert_eq!(outer.captures.len(), 1);
    assert_eq!(
        outer.captures[0].owner,
        BindingOwner::Function(tree.functions[0].id)
    );
    assert_eq!(outer.captures[0].variable, VariableId(0));
    let facts = analyze(&tree);
    assert_eq!(facts.len(), 1);
    assert_unproven(
        &facts[0],
        &[
            UnprovenReason::CallableDeclaration,
            UnprovenReason::CaptureStorage,
            UnprovenReason::CallBoundary,
        ],
        source,
    );
}

#[test]
fn conservative_unused_helper_return_of_capture_is_not_root_return() {
    let source = "fn main() { let a = [1]; let unused = () => { return a; }; }";
    let tree = program(source);
    let facts = analyze(&tree);
    assert_eq!(facts.len(), 1);
    assert_unproven(
        &facts[0],
        &[
            UnprovenReason::CallableDeclaration,
            UnprovenReason::CaptureStorage,
        ],
        source,
    );
}

#[test]
fn conservative_helper_local_ids_do_not_alias_root_bindings() {
    let source = "fn main() { let a = [1]; let helper = () => { let own = [2]; return own; }; }";
    let tree = program(source);
    let TypedStatementKind::Let { id: root_id, .. } = &tree.functions[0].body[0].kind else {
        panic!("root array binding")
    };
    let TypedStatementKind::Closure(helper) = &tree.functions[0].body[1].kind else {
        panic!("helper declaration")
    };
    let TypedClosureBody::Block(body) = &helper.body else {
        panic!("helper block")
    };
    let TypedStatementKind::Let { id: helper_id, .. } = &body.statements[0].kind else {
        panic!("helper array binding")
    };
    assert_eq!(
        root_id, helper_id,
        "fixture must exercise owner-local ID reuse"
    );

    let facts = analyze(&tree);
    assert_eq!(
        facts.len(),
        1,
        "helper literals are outside origin discovery"
    );
    assert_eq!(facts[0].origin.binding, *root_id);
    assert!(std::ptr::eq(facts[0].origin.function, &tree.functions[0]));
    assert_unproven(&facts[0], &[UnprovenReason::CallableDeclaration], source);
    assert!(
        !facts[0]
            .reasons
            .contains(&Reason::Unproven(UnprovenReason::InconsistentTree))
    );
}
