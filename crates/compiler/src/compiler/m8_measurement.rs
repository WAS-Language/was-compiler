#[path = "../../benches/support/m8_corpus.rs"]
mod corpus;
#[path = "../../benches/support/m8_native.rs"]
mod native;

use std::{env, path::Path};
use wasmparser::{Operator, Parser, Payload, Validator};

use super::{OptimizationLevel, TypedProgram, codegen::FoldingPolicy};
use native::Boundary;

#[derive(Debug, Clone, Copy)]
enum Variant {
    FoldOff,
    FoldOn,
}

const VARIANTS: [Variant; 2] = [Variant::FoldOff, Variant::FoldOn];

impl Variant {
    fn compile(self, boundary: Boundary<'_>) -> Vec<u8> {
        let compiler = corpus::compiler(OptimizationLevel::Basic);

        let folding = match self {
            Self::FoldOff => FoldingPolicy::Disabled,
            Self::FoldOn => FoldingPolicy::ClosedScalars,
        };
        let compile = |typed: &TypedProgram| compiler.compile_with_folding(typed, folding).unwrap();
        match boundary {
            Boundary::Source(source) => compile(&compiler.analyze(source).unwrap()),
            Boundary::Typed(typed) => compile(typed),
        }
    }
}

fn wide_arithmetic(bytes: &[u8]) -> (usize, usize) {
    let mut bodies = Parser::new(0)
        .parse_all(bytes)
        .filter_map(|part| match part.unwrap() {
            Payload::CodeSectionEntry(body) => Some(body),
            _ => None,
        });
    let main = bodies.next().unwrap();
    assert!(bodies.next().is_none());
    let mut counts = (0, 0);
    for op in main.get_operators_reader().unwrap() {
        match op.unwrap() {
            Operator::F64Add => counts.0 += 1,
            Operator::F64Mul => counts.1 += 1,
            _ => {}
        }
    }
    counts
}

fn prepare(name: &str, source: &str) -> [Vec<u8>; 2] {
    let (typed, [_, basic, _, _]) = corpus::prepare(source);
    let pair = VARIANTS.map(|variant| {
        let bytes = variant.compile(Boundary::Source(source));
        assert_eq!(
            variant.compile(Boundary::Typed(&typed)),
            bytes,
            "{name}/{variant:?}"
        );
        Validator::new().validate_all(&bytes).unwrap();
        bytes
    });
    let [off, on] = &pair;
    assert_eq!(
        *on, basic,
        "{name}: private folding-on must match public Basic"
    );
    match name {
        "wide_closed" => {
            assert_eq!(wide_arithmetic(off), (31, 16));
            assert_eq!(wide_arithmetic(on), (0, 0));
        }
        "fraction_closed" | "bool_closed" | "negative_zero" | "subnormal_tie" => {
            assert_ne!(off, on, "{name}: expected folding to activate");
        }
        _ => assert_eq!(off, on, "{name}: no-fold control changed"),
    }
    pair
}

#[test]
fn baseline_corpus_isolates_m8_with_m6_and_m7_enabled() {
    for (name, source) in corpus::cases() {
        prepare(name, &source);
    }
}

#[test]
#[ignore = "explicit offline measurement via record_m8_baseline.py --isolated"]
fn record() {
    let Some(mode) = env::var_os("WAS_M8_MODE") else {
        return;
    };
    let directory = env::var_os("WAS_M8_DIRECTORY").expect("measurement directory required");
    let directory = Path::new(&directory);
    match mode.to_str().unwrap() {
        "prepare" => corpus::export(directory, VARIANTS, prepare).unwrap(),
        "forward" | "reverse" => {
            native::record(directory, mode == "reverse", VARIANTS, Variant::compile).unwrap();
        }
        _ => panic!("unknown M8 measurement mode: {mode:?}"),
    }
}
