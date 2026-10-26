use super::*;
use crate::{Compiler, OptimizationLevel};
use wasmparser::{Operator, Parser, Payload, Validator};

#[test]
fn lifetime_candidates_preserve_typed_widths_helpers_and_numeric_preset_lowering() {
    for (source, element, expected_functions) in [
        (
            "fn main(n: number) -> number { let a = [n, 2]; let k = 1 + 2; a[0] += k; return a[0] + len(a); }",
            ArrayElementType::Number,
            8,
        ),
        (
            "fn main(flag: bool) -> bool { let a = [flag, false]; a[1] = !a[0]; len(a); return a[1]; }",
            ArrayElementType::Bool,
            13,
        ),
    ] {
        let mut compiler = Compiler::new();
        let tree = compiler.analyze(source).unwrap();
        let original = tree.clone();
        for level in [
            OptimizationLevel::None,
            OptimizationLevel::Basic,
            OptimizationLevel::Standard,
            OptimizationLevel::Aggressive,
        ] {
            compiler.set_optimization_level(level);
            let before = compiler.compile(&tree).unwrap();
            let facts = analyze(&tree);
            let repeated = analyze(&tree);
            assert_eq!(facts.len(), 1);
            assert_eq!(repeated.len(), facts.len());
            for (fact, again) in facts.iter().zip(&repeated) {
                assert_eq!(fact.outcome, Outcome::TemporaryCandidate);
                assert!(fact.reasons.is_empty());
                assert_eq!(fact.origin.element, element);
                assert_eq!(fact.outcome, again.outcome);
                assert_eq!(fact.reasons, again.reasons);
                assert_eq!(fact.origin.binding, again.origin.binding);
                assert!(std::ptr::eq(fact.origin.function, again.origin.function));
                assert!(std::ptr::eq(fact.origin.literal, again.origin.literal));
            }
            assert_eq!(tree, original);
            let after = compiler.compile(&tree).unwrap();
            assert_eq!(before, after, "{level:?}: {source}");
            Validator::new().validate_all(&after).unwrap();

            let mut functions = 0;
            let mut memories = 0;
            let mut user = None;
            for payload in Parser::new(0).parse_all(&after) {
                match payload.unwrap() {
                    Payload::FunctionSection(section) => functions = section.count(),
                    Payload::MemorySection(section) => memories = section.count(),
                    Payload::ImportSection(section) => assert_eq!(section.count(), 0),
                    Payload::CodeSectionEntry(body) if user.is_none() => {
                        user = Some(
                            body.get_operators_reader()
                                .unwrap()
                                .into_iter()
                                .map(Result::unwrap)
                                .collect::<Vec<_>>(),
                        );
                    }
                    _ => {}
                }
            }
            assert_eq!(functions, expected_functions);
            assert_eq!(memories, 1);
            let user = user.unwrap();
            match element {
                ArrayElementType::Number => {
                    for offset in [8, 16] {
                        assert!(user.iter().any(|op| matches!(op,
                            Operator::F64Store { memarg } if memarg.offset == offset && memarg.align == 3)));
                    }
                    assert!(user.iter().any(|op| matches!(op,
                        Operator::F64Load { memarg } if memarg.offset == 0 && memarg.align == 3)));
                    assert_eq!(
                        user.iter()
                            .filter(|op| matches!(op, Operator::F64Add))
                            .count(),
                        if level == OptimizationLevel::None {
                            3
                        } else {
                            2
                        },
                        "only the proven scalar 1 + 2 specializes; array values remain f64"
                    );
                }
                ArrayElementType::Bool => {
                    for offset in [8, 9] {
                        assert!(user.iter().any(|op| matches!(op,
                            Operator::I32Store8 { memarg } if memarg.offset == offset && memarg.align == 0)));
                    }
                    assert!(
                        user.iter().any(|op| matches!(op,
                        Operator::I32Load8U { memarg } if memarg.offset == 0 && memarg.align == 0))
                    );
                }
                ArrayElementType::String => unreachable!(),
            }
        }
    }
}

#[test]
fn lifetime_excluded_programs_preserve_bytes_facts_and_tree_at_each_preset() {
    let mut compiler = Compiler::new();
    for source in [
        "fn main() -> number[] { let a = [1]; return a; }",
        "fn main() -> bool { let a = [true]; let alias = a; alias[0] = false; return a[0]; }",
        "fn main() -> number { let a = [1]; let read = () => a[0]; return read(); }",
        "fn main() -> number { let a = [1]; if false { a[0] = 2; } return a[0]; }",
        "fn helper(x: number = 2) -> number { return x; } fn main() -> number { let a = [1]; return a[0] |> helper; }",
    ] {
        let tree = compiler.analyze(source).unwrap();
        let original = tree.clone();
        for level in [
            OptimizationLevel::None,
            OptimizationLevel::Basic,
            OptimizationLevel::Standard,
            OptimizationLevel::Aggressive,
        ] {
            compiler.set_optimization_level(level);
            let before = compiler.compile(&tree).unwrap();
            let facts = analyze(&tree);
            assert_eq!(facts.len(), 1);
            assert_ne!(facts[0].outcome, Outcome::TemporaryCandidate);
            let repeated = analyze(&tree);
            assert_eq!(facts[0].outcome, repeated[0].outcome);
            assert_eq!(facts[0].reasons, repeated[0].reasons);
            assert!(std::ptr::eq(
                facts[0].origin.literal,
                repeated[0].origin.literal
            ));
            assert_eq!(tree, original);
            let after = compiler.compile(&tree).unwrap();
            assert_eq!(before, after, "{level:?}: {source}");
            Validator::new().validate_all(&after).unwrap();
        }
    }
}
