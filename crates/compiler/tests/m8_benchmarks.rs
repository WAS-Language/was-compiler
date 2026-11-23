#[path = "../benches/support/m8_corpus.rs"]
mod corpus;
#[path = "../../../tests/support/process.rs"]
mod process;

use std::{fs, path::Path, process::Command, time::Duration};
use wasmparser::{Operator, Parser, Payload};

#[test]
fn ordinary_presets_preserve_m6_and_m7_composition_controls() {
    for (name, source) in corpus::cases() {
        let (_, [none, basic, _, _]) = corpus::prepare(&source);
        let body = |bytes: &[u8], matches: fn(Operator<'_>) -> bool| {
            Parser::new(0)
                .parse_all(bytes)
                .find_map(|payload| match payload.unwrap() {
                    Payload::CodeSectionEntry(body) => Some(body),
                    _ => None,
                })
                .unwrap()
                .get_operators_reader()
                .unwrap()
                .into_iter()
                .map(Result::unwrap)
                .filter(|op| matches(op.clone()))
                .count()
        };

        match name {
            "integer_local" => {
                assert_eq!(body(&none, |op| matches!(op, Operator::I32Add)), 0);
                assert_eq!(body(&basic, |op| matches!(op, Operator::I32Add)), 1);
            }
            "range_direct" => {
                assert_eq!(body(&none, |op| matches!(op, Operator::Call { .. })), 2);
                assert_eq!(body(&basic, |op| matches!(op, Operator::Call { .. })), 0);
            }
            _ => {}
        }
    }
}

#[test]
#[ignore = "requires Node.js"]
fn node_oracle_checks_ordinary_scalar_results_and_growth_at_every_preset() {
    let directory = std::env::temp_dir().join(format!("was-m8-oracle-{}", std::process::id()));
    corpus::export(&directory, corpus::LEVELS, |_, source| {
        corpus::prepare(source).1
    })
    .unwrap();
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/support/m8_scalar_timing.cjs");
    let result = process::output(
        Command::new("node")
            .arg(script)
            .arg(&directory)
            .arg("verify"),
        None,
        Duration::from_secs(30),
    );
    let growth = fs::read_to_string(directory.join("growth.csv"));
    fs::remove_dir_all(directory).unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(growth.unwrap().lines().count(), 1 + 13 * 4 * 5);
}
