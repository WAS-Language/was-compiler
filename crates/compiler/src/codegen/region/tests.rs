use super::*;
use crate::{Compiler, OptimizationLevel, Type, typed_ast::TypedStatementKind};
use wasmparser::{Operator, Parser, Payload, Validator};

const LEVELS: [OptimizationLevel; 4] = [
    OptimizationLevel::None,
    OptimizationLevel::Basic,
    OptimizationLevel::Standard,
    OptimizationLevel::Aggressive,
];

#[test]
fn unrepresentable_inventory_discards_all_slices_and_keeps_a_deterministic_reason() {
    let program = Compiler::new()
        .analyze("fn main() { let a = [1]; let b = [2]; }")
        .unwrap();
    let literals: Vec<_> = crate::lifetime::temporary_literals(&program)
        .map(|(_, literal)| literal)
        .collect();
    let mut selection = Selection::Ineligible;
    // Synthetic footprints exercise the real selection transition without a multi-GiB AST.
    selection.reserve(literals[0], Some(4_294_901_752));
    assert!(matches!(selection, Selection::Region(_)));
    selection.reserve(literals[1], Some(16));
    assert!(matches!(selection, Selection::CapacityExceeded));
    selection.reserve(literals[0], Some(8));
    assert!(matches!(selection, Selection::CapacityExceeded));
    let mut selection = Selection::Ineligible;
    selection.reserve(literals[0], None);
    assert!(matches!(selection, Selection::CapacityExceeded));
}

#[test]
fn capacity_overflow_lowers_the_whole_function_with_baseline_storage() {
    fn overflow(program: &TypedProgram) -> Vec<Selection<'_>> {
        let mut selection = Selection::Ineligible;
        let literals: Vec<_> = crate::lifetime::temporary_literals(program)
            .map(|(_, literal)| literal)
            .collect();
        assert_eq!(literals.len(), 2);
        selection.reserve(literals[0], Some(4_294_901_752));
        selection.reserve(literals[1], Some(16));
        assert!(matches!(selection, Selection::CapacityExceeded));
        vec![selection]
    }

    let program = Compiler::new()
        .analyze("fn main(n: number) -> number { let a = [n]; let b = [a[0]]; return b[0]; }")
        .unwrap();
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        let policy = level.numeric_policy();
        let validated = super::super::validation::ValidatedProgram::with_region_selection(
            &program,
            &ExportOptions::All,
            policy,
            overflow,
        )
        .unwrap();
        let bytes = super::super::assemble(&validated);
        Validator::new().validate_all(&bytes).unwrap();
        assert_eq!(
            bytes,
            super::super::compile_with_policy(&program, &ExportOptions::All, policy).unwrap()
        );
        assert_eq!(bytes, super::super::assemble(&validated));
        let mut globals = 0;
        let mut calls = 0;
        for payload in Parser::new(0).parse_all(&bytes) {
            match payload.unwrap() {
                Payload::GlobalSection(section) => globals = section.count(),
                Payload::CodeSectionEntry(body) if calls == 0 => {
                    calls = body
                        .get_operators_reader()
                        .unwrap()
                        .into_iter()
                        .map(Result::unwrap)
                        .filter(|op| matches!(op, Operator::Call { function_index: 1 }))
                        .count();
                }
                _ => {}
            }
        }
        assert_eq!(globals, 1, "fallback has only the ordinary heap cursor");
        assert_eq!(calls, 2, "both literals use ordinary allocation");

        let mut forged = program.clone();
        let TypedStatementKind::Return(Some(value)) = &mut forged.functions[0].body[2].kind else {
            panic!("expected return")
        };
        value.kind = TypedExprKind::Number("NaN".into());
        let expected = CodegenError::InvalidNumberLiteral {
            function: "main".into(),
            literal: "NaN".into(),
            span: forged.functions[0].body[2].span,
        };
        let ordinary =
            super::super::compile_with_policy(&forged, &ExportOptions::All, policy).unwrap_err();
        let experimental = super::super::validation::ValidatedProgram::with_region_selection(
            &forged,
            &ExportOptions::All,
            policy,
            overflow,
        )
        .err()
        .unwrap();
        assert_eq!(ordinary, expected);
        assert_eq!(experimental, expected);
    }
}

#[test]
fn checked_capacity_includes_aligned_empty_and_unreachable_literals() {
    for (length, width, expected) in [
        (0, 8, 8),
        (0, 1, 8),
        (1, 8, 16),
        (2, 8, 24),
        (1, 1, 16),
        (7, 1, 16),
        (8, 1, 16),
        (9, 1, 24),
        (16, 1, 24),
        (17, 1, 32),
    ] {
        assert_eq!(footprint(length, width), Some(expected));
    }
    assert_eq!(footprint(usize::MAX, 8), None);
    assert_eq!(footprint(536_862_719, 8), Some(4_294_901_760));
    assert_eq!(footprint(536_862_720, 8), None);
    assert_eq!(footprint(4_294_901_752, 1), Some(4_294_901_760));
    assert_eq!(footprint(4_294_901_753, 1), None);
    assert_eq!(add_capacity(4_294_901_752, 8), Some(4_294_901_760));
    assert_eq!(add_capacity(4_294_901_752, 16), None);
    assert_eq!(add_capacity(u32::MAX, 8), None);

    let program = Compiler::new().analyze(
        "fn main() -> number { let a = [1, 2]; return a[0]; let b: bool[] = []; let c = [true, false, true, false, true, false, true, false, true]; }").unwrap();
    let selected = select(&program);
    let Selection::Region(plan) = &selected[0] else {
        panic!("expected region storage")
    };
    assert_eq!(plan.capacity, 56);
    assert_eq!(
        plan.literals
            .iter()
            .map(|(_, offset)| *offset)
            .collect::<Vec<_>>(),
        [0, 24, 32]
    );
    assert_eq!(plan.cache, 1);
    let bytes = compile(&program, &ExportOptions::All, NumericPolicy::Baseline).unwrap();
    Validator::new().validate_all(&bytes).unwrap();
    let mut first = true;
    for payload in Parser::new(0).parse_all(&bytes) {
        if let Payload::CodeSectionEntry(body) = payload.unwrap()
            && first
        {
            first = false;
            let ops: Vec<_> = body
                .get_operators_reader()
                .unwrap()
                .into_iter()
                .map(Result::unwrap)
                .collect();
            assert!(
                ops.iter()
                    .any(|op| matches!(op, Operator::I32Const { value: 56 }))
            );
            assert!(
                ops.iter()
                    .any(|op| matches!(op, Operator::I32Const { value: 24 }))
            );
            assert!(
                ops.iter()
                    .any(|op| matches!(op, Operator::I32Const { value: 32 }))
            );
            assert!(
                !ops.iter()
                    .any(|op| matches!(op, Operator::GlobalSet { global_index: 0 }))
            );
            let take = ops
                .iter()
                .position(|op| matches!(op, Operator::GlobalGet { global_index: 1 }))
                .unwrap();
            let clear = ops
                .iter()
                .position(|op| matches!(op, Operator::GlobalSet { global_index: 1 }))
                .unwrap();
            let alloc = ops
                .iter()
                .position(|op| matches!(op, Operator::Call { function_index: 1 }))
                .unwrap();
            let descriptor = ops
                .iter()
                .position(|op| matches!(op, Operator::I32Store { memarg } if memarg.offset == 4))
                .unwrap();
            let element = ops
                .iter()
                .position(|op| matches!(op, Operator::F64Store { .. }))
                .unwrap();
            let ret = ops
                .iter()
                .position(|op| matches!(op, Operator::Return))
                .unwrap();
            assert!(
                take < clear
                    && clear < alloc
                    && alloc < descriptor
                    && descriptor < element
                    && element < ret
            );
            assert!(
                ops[..ret]
                    .iter()
                    .filter(|op| matches!(op, Operator::GlobalSet { global_index: 1 }))
                    .count()
                    == 2,
                "cache is cleared on acquisition and restored before return"
            );
        }
    }
}

#[test]
fn excluded_forms_and_no_candidates_emit_no_region_machinery() {
    for source in [
        "fn main() -> number { return 1; }",
        "fn main() -> number[] { let a = [1]; return a; }",
        "fn main() -> number { let a = [1]; let b = a; return b[0]; }",
        "fn main() -> number { let a = [1]; a = [2]; return a[0]; }",
        "fn main() -> number { let a = [1]; if true { return 1; } return a[0]; }",
        "fn main() -> number { let a = [1]; return a[0]; loop {} }",
        "fn main() -> number { let a = [1]; let f = () => a[0]; return f(); }",
        "fn other() -> number { return 1; } fn main() -> number { let a = [1]; return other(); }",
        "fn main() -> bool { let a = [\"x\"]; return a[0] == \"x\"; }",
    ] {
        let program = Compiler::new().analyze(source).unwrap();
        assert!(
            select(&program)
                .iter()
                .all(|selection| matches!(selection, Selection::Ineligible)),
            "{source}"
        );
        for policy in [NumericPolicy::Baseline, NumericPolicy::ProvenIntegers] {
            assert_eq!(
                compile(&program, &ExportOptions::All, policy).unwrap(),
                super::super::compile_with_policy(&program, &ExportOptions::All, policy).unwrap(),
                "{source}"
            );
        }
    }
}

#[test]
fn malformed_unreachable_candidate_and_invalid_function_id_preserve_error_order() {
    use crate::typed_ast::{FunctionId, TypedExprKind};

    let program = Compiler::new().analyze(
        "fn first() -> number { let a = [1]; return a[0]; let b = [2]; } fn second() { let c = [3]; }",
    ).unwrap();
    let mut forged = program.clone();
    let TypedStatementKind::Let { value, .. } = &mut forged.functions[0].body[2].kind else {
        panic!("expected unreachable declaration")
    };
    let TypedExprKind::Array(elements, _) = &mut value.kind else {
        panic!("expected array")
    };
    elements[0].kind = TypedExprKind::Number("NaN".into());
    let Selection::Region(plan) = &select(&forged)[0] else {
        panic!("expected positive classification")
    };
    assert_eq!(plan.capacity, 32);
    for level in LEVELS {
        let policy = level.numeric_policy();
        let ordinary = super::super::compile_with_policy(&forged, &ExportOptions::None, policy);
        assert_eq!(
            ordinary,
            Err(CodegenError::InvalidNumberLiteral {
                function: "first".into(),
                literal: "NaN".into(),
                span: forged.functions[0].body[2].span,
            })
        );
        assert_eq!(compile(&forged, &ExportOptions::None, policy), ordinary);
    }
    forged.functions[1].id = FunctionId(99);
    for level in LEVELS {
        let policy = level.numeric_policy();
        let ordinary = super::super::compile_with_policy(&forged, &ExportOptions::None, policy);
        assert_eq!(
            ordinary,
            Err(CodegenError::InvalidFunctionId {
                function: "second".into(),
                expected: 1,
                actual: 99,
            })
        );
        assert_eq!(compile(&forged, &ExportOptions::None, policy), ordinary);
    }
}

#[test]
fn forged_binding_ids_and_skipped_children_keep_ordinary_diagnostics() {
    use crate::typed_ast::{TypedExprKind, VariableId};

    let mut duplicate = Compiler::new()
        .analyze("fn bad() { let a = [1]; let b = [2]; }")
        .unwrap();
    let TypedStatementKind::Let { id: first, .. } = duplicate.functions[0].body[0].kind else {
        panic!("expected let")
    };
    let TypedStatementKind::Let { id, .. } = &mut duplicate.functions[0].body[1].kind else {
        panic!("expected let")
    };
    *id = first;
    let expected = CodegenError::DuplicateVariableId {
        function: "bad".into(),
        id: first,
        span: duplicate.functions[0].body[1].span,
    };
    let mut unknown = Compiler::new()
        .analyze("fn bad() -> number { let a = [1]; return a[0]; }")
        .unwrap();
    let TypedStatementKind::Return(Some(value)) = &mut unknown.functions[0].body[1].kind else {
        panic!("expected return")
    };
    let TypedExprKind::Index { expr, .. } = &mut value.kind else {
        panic!("expected index")
    };
    let TypedExprKind::Identifier { id, .. } = &mut expr.kind else {
        panic!("expected identifier")
    };
    *id = VariableId(99);
    let unknown_error = CodegenError::UnknownVariable {
        function: "bad".into(),
        id: VariableId(99),
        span: unknown.functions[0].body[1].span,
    };
    let mut skipped = Compiler::new()
        .analyze("fn bad() -> bool { let a = [1]; return false && a[0] == 0; }")
        .unwrap();
    let TypedStatementKind::Return(Some(value)) = &mut skipped.functions[0].body[1].kind else {
        panic!("expected return")
    };
    let TypedExprKind::Binary { right, .. } = &mut value.kind else {
        panic!("expected logical expression")
    };
    let TypedExprKind::Binary { left, .. } = &mut right.kind else {
        panic!("expected comparison")
    };
    let TypedExprKind::Index { index, .. } = &mut left.kind else {
        panic!("expected index")
    };
    index.kind = TypedExprKind::Number("NaN".into());
    let skipped_error = CodegenError::InvalidNumberLiteral {
        function: "bad".into(),
        literal: "NaN".into(),
        span: skipped.functions[0].body[1].span,
    };
    for (program, expected) in [
        (duplicate, expected),
        (unknown, unknown_error),
        (skipped, skipped_error),
    ] {
        assert!(matches!(select(&program)[0], Selection::Ineligible));
        for level in [
            OptimizationLevel::None,
            OptimizationLevel::Basic,
            OptimizationLevel::Standard,
            OptimizationLevel::Aggressive,
        ] {
            let policy = level.numeric_policy();
            for exports in [ExportOptions::All, ExportOptions::None] {
                let ordinary =
                    super::super::compile_with_policy(&program, &exports, policy).unwrap_err();
                let experimental = compile(&program, &exports, policy).unwrap_err();
                assert_eq!(ordinary, expected);
                assert_eq!(experimental, expected);
                assert_eq!(experimental.to_string(), ordinary.to_string());
                assert_eq!(experimental.span(), ordinary.span());
            }
        }
    }
}

#[test]
fn mixed_selected_and_excluded_hidden_bodies_keep_source_order_errors() {
    for (first_suffix, second_suffix) in [("", "loop {}"), ("loop {}", "")] {
        let source = format!(
            "fn first() -> number {{ let a = [1]; return a[0]; {first_suffix} }} fn second() -> number {{ let b = [2]; return b[0]; {second_suffix} }}"
        );
        let mut program = Compiler::new().analyze(&source).unwrap();
        for (function, literal) in program.functions.iter_mut().zip(["NaN", "1e3"]) {
            let TypedStatementKind::Let { value, .. } = &mut function.body[0].kind else {
                panic!("expected let")
            };
            let TypedExprKind::Array(elements, _) = &mut value.kind else {
                panic!("expected array")
            };
            elements[0].kind = TypedExprKind::Number(literal.into());
        }
        assert_eq!(
            matches!(select(&program)[0], Selection::Region(_)),
            first_suffix.is_empty()
        );
        assert_eq!(
            matches!(select(&program)[1], Selection::Region(_)),
            second_suffix.is_empty()
        );
        let expected = CodegenError::InvalidNumberLiteral {
            function: "first".into(),
            literal: "NaN".into(),
            span: program.functions[0].body[0].span,
        };
        for level in LEVELS {
            for exports in [ExportOptions::All, ExportOptions::None] {
                let ordinary =
                    super::super::compile_with_policy(&program, &exports, level.numeric_policy())
                        .unwrap_err();
                let experimental = compile(&program, &exports, level.numeric_policy()).unwrap_err();
                assert_eq!(ordinary, expected);
                assert_eq!(experimental, expected);
                assert_eq!(experimental.to_string(), ordinary.to_string());
                assert_eq!(experimental.span(), ordinary.span());
            }
        }
    }
}

#[test]
fn sparse_binding_ids_still_select_and_lower_checked_storage() {
    use crate::typed_ast::VariableId;

    let mut program = Compiler::new()
        .analyze("fn main() -> number { let a = [7]; return a[0]; }")
        .unwrap();
    let TypedStatementKind::Let { id, .. } = &mut program.functions[0].body[0].kind else {
        panic!("expected let")
    };
    *id = VariableId(99);
    let TypedStatementKind::Return(Some(value)) = &mut program.functions[0].body[1].kind else {
        panic!("expected return")
    };
    let TypedExprKind::Index { expr, .. } = &mut value.kind else {
        panic!("expected index")
    };
    let TypedExprKind::Identifier { id, .. } = &mut expr.kind else {
        panic!("expected identifier")
    };
    *id = VariableId(99);
    assert!(matches!(select(&program)[0], Selection::Region(_)));
    for level in LEVELS {
        let bytes = compile(&program, &ExportOptions::All, level.numeric_policy()).unwrap();
        Validator::new().validate_all(&bytes).unwrap();
        assert_eq!(
            bytes,
            compile(&program, &ExportOptions::All, level.numeric_policy()).unwrap()
        );
    }
}

#[test]
fn ordinary_bytes_presets_exports_and_diagnostics_remain_independent() {
    let mut compiler = Compiler::new();
    let program = compiler
        .analyze(
            "fn main(n: number) -> number { let a = [n]; let k = 1 + 2; a[0] += k; return a[0]; }",
        )
        .unwrap();
    let original = program.clone();
    let mut enabled = None;
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        compiler.set_optimization_level(level);
        let before = compiler.compile(&program).unwrap();
        let policy = if level == OptimizationLevel::None {
            NumericPolicy::Baseline
        } else {
            NumericPolicy::ProvenIntegers
        };
        let bytes = compile(&program, &ExportOptions::All, policy).unwrap();
        assert_eq!(
            bytes,
            compile(&program, &ExportOptions::All, policy).unwrap()
        );
        assert_eq!(before, compiler.compile(&program).unwrap());
        if level != OptimizationLevel::None {
            if let Some(expected) = &enabled {
                assert_eq!(&bytes, expected);
            } else {
                enabled = Some(bytes);
            }
        }
        let hidden = compile(&program, &ExportOptions::None, policy).unwrap();
        Validator::new().validate_all(&hidden).unwrap();
        let mut names = Vec::new();
        for payload in Parser::new(0).parse_all(&hidden) {
            if let Payload::ExportSection(section) = payload.unwrap() {
                names.extend(section.into_iter().map(|export| export.unwrap().name));
            }
        }
        assert_eq!(names, ["memory", "__alloc"]);
    }
    assert_eq!(program, original);
    // This still earns M1's positive proof, but not the backend's required-return check.
    let mut forged = program.clone();
    forged.functions[0].body.pop();
    for policy in [NumericPolicy::Baseline, NumericPolicy::ProvenIntegers] {
        assert!(matches!(select(&forged)[0], Selection::Region(_)));
        assert_eq!(
            compile(&forged, &ExportOptions::All, policy),
            super::super::compile_with_policy(&forged, &ExportOptions::All, policy)
        );
        assert!(compile(&forged, &ExportOptions::All, policy).is_err());
    }
    let mut forged = program.clone();
    if let TypedStatementKind::Let { value, .. } = &mut forged.functions[0].body[0].kind {
        value.ty = Type::Bool.into();
    }
    for policy in [NumericPolicy::Baseline, NumericPolicy::ProvenIntegers] {
        assert_eq!(
            compile(&forged, &ExportOptions::All, policy),
            super::super::compile_with_policy(&forged, &ExportOptions::All, policy)
        );
        assert!(compile(&forged, &ExportOptions::All, policy).is_err());
    }
}
