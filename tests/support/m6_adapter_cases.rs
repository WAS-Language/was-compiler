use was_compiler::{CompileError, Compiler, ExportOptions, OptimizationLevel, Type, codegen};
use wasmparser::{Operator, Parser, Payload, Validator};

pub const LEVELS: [OptimizationLevel; 4] = [
    OptimizationLevel::None,
    OptimizationLevel::Basic,
    OptimizationLevel::Standard,
    OptimizationLevel::Aggressive,
];
pub const SOURCE: &str = include_str!("m6_range_allocations.was");
pub const EXPORTS: [&str; 5] = [
    "direct",
    "local",
    "indirect_binding",
    "indirect_call",
    "abrupt",
];
pub const CONTROL: &str = "fn main(a: number, b: number) -> number { let r = a..b; let sum = a; for i in r { sum += i; } return sum; }";
pub const SCALAR: &str = "fn main(n: number) -> number { return n / 2.5; }";
const DIRECT_ONLY: &str = "fn main(a: number, b: number) -> number { let sum = a; for i in a..b { sum += i; } return sum; }";
const COMPOSED: &str = r#"
fn main() -> number {
    let base = 6 + 3;
    let sum = base + 1;
    for i in (1 + 2)..(2 * 3) {
        checkpoint(i);
        sum += i;
    }
    return sum + (1.5 * 2);
}
"#;
const INVALID: [&str; 2] = [
    "// λ🙂\r\nfn main() { for i in 0.. by 1 {} }",
    "// λ🙂\r\nfn main() { return; for i in 0..2 by false {} }",
];
pub const PRIVATE_HOST_HINT: &str = "; range values are private and cannot cross host imports; use number, bool, string, number[], bool[], string[] parameters/results or void results";

#[derive(Clone, Copy)]
pub enum Selection {
    Selected,
    None,
    All,
}

pub const CASES: [(&str, &str, Selection); 6] = [
    ("selected", SOURCE, Selection::Selected),
    ("internal", SOURCE, Selection::None),
    ("indirect", CONTROL, Selection::All),
    ("scalar", SCALAR, Selection::All),
    ("composed", COMPOSED, Selection::All),
    ("direct", DIRECT_ONLY, Selection::All),
];

pub fn compiler() -> Compiler {
    let mut compiler = Compiler::new();
    // Keywords are allowed in external names. Even the unused import is required.
    compiler
        .register_host_function("checkpoint", "range", "for", vec![Type::Number], Type::Void)
        .unwrap();
    compiler
        .register_host_function("unused", "by", "in", vec![], Type::Void)
        .unwrap();
    compiler
}

pub fn diagnostics() -> [(&'static str, CompileError); 3] {
    let mut compiler = compiler();
    let errors = [INVALID[0], INVALID[1], SOURCE].map(|source| {
        compiler.set_optimization_level(OptimizationLevel::None);
        let baseline = compiler.compile_source(source).unwrap_err();
        for level in LEVELS {
            compiler.set_optimization_level(level);
            assert_eq!(compiler.compile_source(source).unwrap_err(), baseline);
        }
        (source, baseline)
    });
    assert!(matches!(errors[0].1, CompileError::Parse(_)) && errors[0].1.span().is_some());
    assert!(matches!(errors[1].1, CompileError::Semantic(_)) && errors[1].1.span().is_some());
    assert!(matches!(
        errors[2].1,
        CompileError::Codegen(codegen::CodegenError::RangeExport { .. })
    ));
    errors
}

pub fn core_matrix() -> [[Vec<u8>; 4]; CASES.len()] {
    let matrix = CASES.map(|(_, source, selection)| {
        let mut core = compiler();
        let exports = match selection {
            Selection::Selected => ExportOptions::Only(EXPORTS.map(String::from).to_vec()),
            Selection::None => ExportOptions::None,
            Selection::All => ExportOptions::All,
        };
        core.set_exports(exports.clone());
        let default = core.compile_source(source).unwrap();
        let typed = core.analyze(source).unwrap();
        let baseline = codegen::compile_with_exports(&typed, &exports).unwrap();

        let bytes = LEVELS.map(|level| {
            core.set_optimization_level(level);
            let bytes = core.compile_source(source).unwrap();
            Validator::new().validate_all(&bytes).unwrap();
            bytes
        });
        assert_eq!(bytes[0], baseline);
        assert_eq!(bytes[1], default);
        assert_eq!(bytes[1], bytes[2]);
        assert_eq!(bytes[1], bytes[3]);
        if matches!(source, CONTROL | SCALAR) {
            assert_eq!(bytes[0], bytes[1], "no eligible loops or integer rewrites");
        } else {
            assert_ne!(
                bytes[0], bytes[1],
                "eligible fixture must produce different O0 and O1 bytes"
            );
        }
        bytes
    });

    for level in 0..4 {
        assert_eq!(
            bodies(&matrix[0][level]),
            bodies(&matrix[1][level]),
            "exports do not change bodies"
        );
    }

    let baseline = bodies(&matrix[0][0]);
    let optimized = bodies(&matrix[0][1]);
    // The fixture has seven source functions before seven memory helpers and two
    // range helpers, then two local callables. Compare helpers in index order.
    assert_eq!(baseline.len(), 18);
    assert_eq!(optimized.len(), baseline.len());
    assert_eq!(&baseline[7..16], &optimized[7..16]);

    // `run` is the first local callable. Check that its loop specifically stops
    // calling both range helpers (indices 16 and 17 after the two imports).
    let baseline_calls = body_calls(&matrix[0][0], 16);
    let optimized_calls = body_calls(&matrix[0][1], 16);
    for helper in [16, 17] {
        assert!(baseline_calls.contains(&helper));
        assert!(!optimized_calls.contains(&helper));
    }

    // The direct-only fixture has no other consumer of the range helpers. Their
    // bodies and indices survive even though the only user function stops calling them.
    let direct = matrix.last().unwrap();
    assert_eq!(body_calls(&direct[0], 0), [10, 11]); // 2 imports, 1 user, 7 memory helpers
    assert!(body_calls(&direct[1], 0).is_empty());
    assert_eq!(&bodies(&direct[0])[1..], &bodies(&direct[1])[1..]);
    matrix
}

fn body_calls(bytes: &[u8], body_index: usize) -> Vec<u32> {
    let mut operators = Parser::new(0)
        .parse_all(bytes)
        .filter_map(|payload| match payload.unwrap() {
            Payload::CodeSectionEntry(body) => Some(body.get_operators_reader().unwrap()),
            _ => None,
        })
        .nth(body_index)
        .unwrap();
    let mut calls = Vec::new();
    while !operators.eof() {
        if let Operator::Call { function_index } = operators.read().unwrap() {
            calls.push(function_index);
        }
    }
    calls
}

fn bodies(bytes: &[u8]) -> Vec<&[u8]> {
    Parser::new(0)
        .parse_all(bytes)
        .filter_map(|payload| match payload.unwrap() {
            Payload::CodeSectionEntry(body) => Some(body.as_bytes()),
            _ => None,
        })
        .collect()
}
