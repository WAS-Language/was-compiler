use super::*;
use crate::{Compiler, OptimizationLevel, Type, typed_ast::TypedStatementKind};
use wasmparser::{Operator, Parser, Payload, Validator};

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
