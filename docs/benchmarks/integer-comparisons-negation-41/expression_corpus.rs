#[path = "../../../tests/support/integer_corpus.rs"]
mod historical;
pub use historical::{compiler, measurement_level, Case};

fn balanced_sum(terms: &[String]) -> String {
    if terms.len() == 1 {
        return terms[0].clone();
    }
    let middle = terms.len() / 2;
    format!(
        "({} + {})",
        balanced_sum(&terms[..middle]),
        balanced_sum(&terms[middle..])
    )
}

pub fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for size in [1, 32] {
        let comparisons = [
            ("<", "-149", 1),
            ("<=", "-150", 2),
            (">", "-151", 4),
            (">=", "-150", 8),
            ("==", "-150", 16),
            ("!=", "-149", 32),
        ];
        let terms: Vec<_> = comparisons
            .iter()
            .map(|(op, rhs, weight)| {
                format!("(if -(120 + 30) {op} {rhs} {{ {weight} }} else {{ 0 }})")
            })
            .collect();
        for (family, expression) in [
            ("expr_compare", balanced_sum(&terms)),
            ("expr_negate", "(-(120 + 30) + 7)".to_owned()),
        ] {
            cases.push(Case {
                name: format!("{family}_{size}"),
                source: format!(
                    "fn main(x: number) -> number {{ return {}; }}",
                    balanced_sum(&vec![expression; size])
                ),
            });
        }
    }
    for (name, expression) in [
        ("expr_zero", "-(0 - 0)"),
        ("expr_min", "-((0 - 2147483647) - 1)"),
    ] {
        cases.push(Case {
            name: name.to_owned(),
            source: format!("fn main(x: number) -> number {{ return {expression}; }}"),
        });
    }
    cases
}
