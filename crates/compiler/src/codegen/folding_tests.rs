use std::{process::Command, time::Duration};

use crate::test_process;
use crate::tokens::Span;
use crate::typed_ast::{TypedExpr, TypedExprKind, TypedStatementKind};
use crate::{Compiler, ExportOptions, OptimizationLevel, Type, TypedProgram};
use wasmparser::{FunctionBody, Operator, Parser, Payload, Validator};

use super::{CodegenError, FoldingPolicy, NumericPolicy, RangePolicy, compile_with_policies};

fn compile(program: &TypedProgram, folding: FoldingPolicy) -> Result<Vec<u8>, CodegenError> {
    compile_with_policies(
        program,
        &ExportOptions::All,
        NumericPolicy::ProvenIntegers,
        RangePolicy::ScalarDirectLoops,
        folding,
    )
}

fn pair(source: &str) -> [Vec<u8>; 2] {
    pair_with_compiler(Compiler::new(), source)
}

fn pair_with_compiler(mut compiler: Compiler, source: &str) -> [Vec<u8>; 2] {
    let program = compiler.analyze(source).unwrap();
    let pair = [FoldingPolicy::Disabled, FoldingPolicy::ClosedScalars].map(|policy| {
        let bytes = compile(&program, policy).unwrap();
        Validator::new().validate_all(&bytes).unwrap();
        bytes
    });
    assert_eq!(
        compiler.compile(&program).unwrap(),
        super::compile(&program).unwrap()
    );
    for level in [
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        compiler.set_optimization_level(level);
        assert_eq!(compiler.compile(&program).unwrap(), pair[1]);
    }
    pair
}

fn bodies(bytes: &[u8]) -> Vec<FunctionBody<'_>> {
    Parser::new(0)
        .parse_all(bytes)
        .filter_map(|payload| match payload.unwrap() {
            Payload::CodeSectionEntry(body) => Some(body),
            _ => None,
        })
        .collect()
}

fn operators<'a>(body: &FunctionBody<'a>) -> Vec<Operator<'a>> {
    body.get_operators_reader()
        .unwrap()
        .into_iter()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn folded_sites_preserve_original_local_representation_and_range_plans() {
    let [off, on] = pair(
        "fn main() -> number {
        let exact = -(7 + 2);
        let real = 2 * 3;
        for i in (1 + 2)..(5 + 1) { (i + exact) % real; }
        return exact + real;
    }",
    );
    let sections = |bytes: &[u8]| {
        Parser::new(0)
            .parse_all(bytes)
            .filter_map(|payload| payload.unwrap().as_section())
            .filter(|(id, _)| *id != wasm_encoder::SectionId::Code as u8)
            .map(|(id, range)| {
                let start = usize::try_from(range.start).unwrap();
                let end = usize::try_from(range.end).unwrap();
                (id, bytes[start..end].to_vec())
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(sections(&on), sections(&off));
    let [off, on] = [&off, &on].map(|bytes| bodies(bytes));
    assert!(off.len() > 3);
    for (before, after) in off[1..].iter().zip(&on[1..]) {
        assert_eq!(before.as_bytes(), after.as_bytes());
    }
    let locals = |body: &FunctionBody<'_>| {
        body.get_locals_reader()
            .unwrap()
            .into_iter()
            .map(Result::unwrap)
            .collect::<Vec<_>>()
    };
    assert_eq!(locals(&on[0]), locals(&off[0]));
    assert_eq!(
        locals(&on[0])[..2],
        [(1, wasmparser::ValType::I32), (1, wasmparser::ValType::F64)]
    );
    let before = operators(&off[0]);
    let after = operators(&on[0]);
    assert!(
        matches!(after[..4], [Operator::I32Const { value: -9 }, Operator::LocalSet { .. }, Operator::F64Const { value }, Operator::LocalSet { .. }] if value.bits() == 6.0_f64.to_bits())
    );
    assert!(
        before
            .iter()
            .any(|op| matches!(op, Operator::I32Sub | Operator::F64Mul))
    );
    assert!(
        !after
            .iter()
            .any(|op| matches!(op, Operator::I32Sub | Operator::F64Mul | Operator::I32Add))
    );
    let calls = |ops: &[Operator<'_>]| {
        ops.iter()
            .filter_map(|op| match op {
                Operator::Call { function_index } => Some(*function_index),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    // Range construction/validation stays scalar; only remainder is called.
    assert_eq!(calls(&after), [8]);
    assert_eq!(calls(&after), calls(&before));
}

#[test]
fn no_fold_keeps_bytes_with_m6_and_m7_enabled() {
    let [off, on] = pair(
        "fn main(n: number) -> number {
        let exact = 2;
        let next = exact + 1;
        for i in 1..n { i; }
        return n * next + 1.5 / 2 + (4 % 3);
    }
    fn excluded() -> bool {
        let flag = true;
        return !flag || (1 / 2 == 0.5) || (4 % 3 == 1) ||
            (\"a\" == \"a\") || ([true] == [true]) || (1 in 0..2) ||
            !(if true { true } else { false });
    }",
    );
    assert_eq!(on, off);
}

fn edge_source() -> String {
    let tiny = format!("0.{}5", "0".repeat(323));
    let huge = format!("1{}", "0".repeat(308));
    format!(
        "
        fn zero() -> number {{ return -(1.5 - 1.5); }}
        fn tie() -> number {{ return -{tiny} * 0.5; }}
        fn subnormal() -> number {{ return {tiny} * 1.5; }}
        fn rounded() -> number {{ return (9007199254740992 + 1) - 9007199254740992; }}
        fn overflow() -> number {{ return ({huge} * 10) * (1.5 - 1.5); }}
    "
    )
}

#[test]
fn finite_fold_constants_preserve_bits_and_overflow_keeps_its_parent() {
    let [off, on] = pair(&edge_source());
    let [off, on] = [&off, &on].map(|bytes| bodies(bytes));
    for (body, bits) in on[..4]
        .iter()
        .zip([0x8000_0000_0000_0000, 0x8000_0000_0000_0000, 2, 0])
    {
        assert!(
            matches!(operators(body).as_slice(), [Operator::F64Const { value }, Operator::Return, Operator::End] if value.bits() == bits)
        );
    }
    let overflow = operators(&on[4]);
    assert_eq!(
        overflow
            .iter()
            .filter(|op| matches!(op, Operator::F64Mul))
            .count(),
        2
    );
    assert!(!overflow.iter().any(|op| matches!(op, Operator::F64Sub)));
    assert!(
        operators(&off[4])
            .iter()
            .any(|op| matches!(op, Operator::F64Sub))
    );
}

#[test]
#[ignore = "requires Node.js"]
fn m8_on_and_off_match_independent_binary64_oracles() {
    run_node_pair(
        pair(&edge_source()),
        r#"
            const e = new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;
            assert.ok(Object.is(e.zero(), -0));
            assert.ok(Object.is(e.tie(), -0));
            assert.equal(e.subnormal(), 1e-323);
            assert.ok(Object.is(e.rounded(), +0));
            assert.ok(Number.isNaN(e.overflow()));
    "#,
    );
}

fn scalar_source() -> String {
    let huge = format!("1{}", "0".repeat(308));
    format!(
        "fn less() -> bool {{ return -2 < 1 + 2; }}
         fn less_equal() -> bool {{ return 2 <= 1; }}
         fn greater() -> bool {{ return 2.5 * 2 > 4.5; }}
         fn greater_equal() -> bool {{ return -0 >= 0; }}
         fn same() -> bool {{ return -0 == 0; }}
         fn different() -> bool {{ return -0 != 0; }}
         fn bool_same() -> bool {{ return (2 < 4) == !false; }}
         fn bool_different() -> bool {{ return !true != true; }}
         fn not() -> bool {{ return !(false == false); }}
         fn infinity() -> bool {{ return ({huge} * 10) > (2.5 - 1.5); }}
         fn nan() -> bool {{ return !((({huge} * 10) * 0) == 0); }}"
    )
}

#[test]
fn scalar_folds_emit_canonical_booleans_and_keep_nonfinite_comparisons() {
    let [off, on] = pair(&scalar_source());
    let [off, on] = [&off, &on].map(|bytes| bodies(bytes));
    for ((before, after), expected) in off.iter().zip(&on).zip([1, 0, 1, 1, 1, 0, 1, 1, 0]) {
        assert!(operators(before).len() > 3);
        assert!(matches!(
            operators(after).as_slice(),
            [Operator::I32Const { value }, Operator::Return, Operator::End] if *value == expected
        ));
    }
    let infinity = operators(&on[9]);
    assert!(infinity.iter().any(|op| matches!(op, Operator::F64Mul)));
    assert!(infinity.iter().any(|op| matches!(op, Operator::F64Gt)));
    assert!(!infinity.iter().any(|op| matches!(op, Operator::F64Sub)));
    assert_eq!(on[10].as_bytes(), off[10].as_bytes());
}

#[test]
fn m7_comparison_representation_is_independent_of_m8_foldability() {
    for (op, floating, integer) in [
        ("<", Operator::F64Lt, Operator::I32LtS),
        ("<=", Operator::F64Le, Operator::I32LeS),
        (">", Operator::F64Gt, Operator::I32GtS),
        (">=", Operator::F64Ge, Operator::I32GeS),
        ("==", Operator::F64Eq, Operator::I32Eq),
        ("!=", Operator::F64Ne, Operator::I32Ne),
    ] {
        for unknown in ["-0", "1.5", "2147483648", "-2147483649", "2 * 3"] {
            let [off, _] = pair(&format!(
                "fn left() -> bool {{ return ({unknown}) {op} (2 + 3); }}
                 fn right() -> bool {{ return (2 + 3) {op} ({unknown}); }}
                 fn proven() -> bool {{ return (0 - 2147483647 - 1) {op} (2147483646 + 1); }}"
            ));
            let off = bodies(&off);
            for body in &off[..2] {
                let ops = operators(body);
                assert!(ops.contains(&floating), "{unknown} {op}");
                assert!(ops.contains(&Operator::F64ConvertI32S), "{unknown} {op}");
                assert!(!ops.contains(&integer), "{unknown} {op}");
            }
            let ops = operators(&off[2]);
            assert!(ops.contains(&integer), "{op}");
            assert!(!ops.contains(&Operator::F64ConvertI32S), "{op}");
        }
    }
}

#[test]
#[ignore = "requires Node.js"]
fn m8_scalar_comparisons_match_independent_results() {
    run_node_pair(
        pair(&scalar_source()),
        r#"
        const e = new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;
        assert.deepEqual(
            [e.less(), e.less_equal(), e.greater(), e.greater_equal(), e.same(), e.different(),
             e.bool_same(), e.bool_different(), e.not(), e.infinity(), e.nan()],
            [1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1],
        );
        "#,
    );
}

const CHECKED_SITES: &str = r#"
fn stage(x: number, y: number = 1.5 * 2) -> number requires x > (2.5 - 1.5) {
    let offset = () => -(1.5 + 2.25);
    mark(y);
    return offset();
}
fn main(x: number) -> number {
    let result = stage(mark(x));
    mark(4);
    return result;
}
"#;

fn checked_sites_pair() -> [Vec<u8>; 2] {
    let mut compiler = Compiler::new();
    compiler
        .register_host_function("mark", "env", "mark", vec![Type::Number], Type::Number)
        .unwrap();
    pair_with_compiler(compiler, CHECKED_SITES)
}

#[test]
fn checked_defaults_contracts_and_local_callables_fold_without_removing_guards() {
    let [off, on] = checked_sites_pair();
    let [off, on] = [&off, &on].map(|bytes| bodies(bytes));
    assert_eq!(on.len(), 3);
    let [contract, caller, helper] = [0, 1, 2].map(|index| operators(&on[index]));
    let constant = |op: &Operator<'_>, expected: f64| matches!(op, Operator::F64Const { value } if value.bits() == expected.to_bits());
    assert!(contract.windows(5).any(|ops| constant(&ops[0], 1.0)
        && matches!(
            ops[1..],
            [
                Operator::F64Gt,
                Operator::I32Eqz,
                Operator::If { .. },
                Operator::Unreachable
            ]
        )));
    assert!(caller.iter().any(|op| constant(op, 3.0)));
    assert!(constant(&helper[0], -3.75));
    assert_eq!(helper.len(), 2);
    for (index, predicate) in [
        (
            0,
            (|op| matches!(op, Operator::F64Sub)) as fn(&Operator<'_>) -> bool,
        ),
        (1, |op| matches!(op, Operator::F64Mul)),
        (2, |op| matches!(op, Operator::F64Neg)),
    ] {
        assert!(operators(&off[index]).iter().any(predicate));
        assert!(!operators(&on[index]).iter().any(predicate));
    }
}

#[test]
#[ignore = "requires Node.js"]
fn m8_on_and_off_preserve_host_effect_order_around_contract_traps() {
    run_node_pair(
        checked_sites_pair(),
        r#"
        const effects = [];
        const e = new WebAssembly.Instance(new WebAssembly.Module(bytes), {
            env: { mark(value) { effects.push(value); return value; } }
        }).exports;
        assert.equal(e.main(2), -3.75);
        assert.deepEqual(effects.splice(0), [2, 3, 4]);
        assert.throws(() => e.main(1), WebAssembly.RuntimeError);
        assert.deepEqual(effects, [1]);
    "#,
    );
}

fn checked_boolean_sites_pair() -> [Vec<u8>; 2] {
    let mut compiler = Compiler::new();
    compiler
        .register_host_function("mark", "env", "mark", vec![Type::Number], Type::Number)
        .unwrap();
    pair_with_compiler(
        compiler,
        r#"
        fn stage(x: number, flag: bool = !(2 >= 3)) -> bool
            requires !(0 != -0) requires x > 1 {
            let closed = () => !(3.5 <= 2);
            false && { mark(99); 1 == 2 };
            return flag == closed();
        }
        fn main(x: number) -> bool { return stage(mark(x)); }
        fn reject() -> number requires !true { return mark(98); }
        "#,
    )
}

#[test]
fn scalar_folds_stay_in_contracts_defaults_callables_and_skipped_branches() {
    let [off, on] = checked_boolean_sites_pair();
    let [off, on] = [&off, &on].map(|bytes| bodies(bytes));
    let [stage, main, reject, helper] = [0, 1, 2, 3].map(|index| operators(&on[index]));
    assert!(matches!(
        stage[..4],
        [
            Operator::I32Const { value: 1 },
            Operator::I32Eqz,
            Operator::If { .. },
            Operator::Unreachable
        ]
    ));
    assert!(
        stage
            .windows(2)
            .any(|ops| matches!(ops, [Operator::Call { function_index: 0 }, Operator::Drop]))
    );
    assert_eq!(
        stage
            .iter()
            .filter(|op| matches!(op, Operator::If { .. }))
            .count(),
        3
    );
    assert!(matches!(
        reject[..4],
        [
            Operator::I32Const { value: 0 },
            Operator::I32Eqz,
            Operator::If { .. },
            Operator::Unreachable
        ]
    ));
    assert!(matches!(
        helper.as_slice(),
        [Operator::I32Const { value: 1 }, Operator::End]
    ));
    for (index, operator) in [
        (
            0,
            (|op| matches!(op, Operator::I32Eq | Operator::F64Ne)) as fn(&Operator<'_>) -> bool,
        ),
        (1, |op| matches!(op, Operator::I32GeS)),
        (3, |op| matches!(op, Operator::F64Le)),
    ] {
        assert!(operators(&off[index]).iter().any(operator));
    }
    assert!(!stage.iter().any(|op| matches!(op, Operator::F64Ne)));
    // The skipped RHS's equality disappears; the final equality over a binding and a call remains.
    assert_eq!(
        stage
            .iter()
            .filter(|op| matches!(op, Operator::I32Eq))
            .count(),
        1
    );
    assert!(!main.iter().any(|op| matches!(op, Operator::I32GeS)));
    assert!(main.windows(3).any(|ops| matches!(
        ops,
        [
            Operator::Call { function_index: 0 },
            Operator::LocalSet { .. },
            Operator::I32Const { value: 1 }
        ]
    )));
}

#[test]
#[ignore = "requires Node.js"]
fn m8_boolean_sites_preserve_guards_skipping_and_host_effects() {
    run_node_pair(
        checked_boolean_sites_pair(),
        r#"
        const effects = [];
        const e = new WebAssembly.Instance(new WebAssembly.Module(bytes), {
            env: { mark(value) { effects.push(value); return value; } }
        }).exports;
        assert.equal(e.main(2), 1);
        assert.throws(() => e.main(1), WebAssembly.RuntimeError);
        assert.throws(() => e.reject(), WebAssembly.RuntimeError);
        assert.deepEqual(effects, [2, 1]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn trapping_default_still_runs_after_all_written_arguments_and_before_the_body() {
    let mut compiler = Compiler::new();
    let array = Type::Array(crate::ArrayElementType::Bool);
    compiler
        .register_host_function(
            "mark",
            "env",
            "mark",
            vec![array, Type::Number],
            Type::Number,
        )
        .unwrap();
    let pair = pair_with_compiler(
        compiler,
        r#"
        fn stage(a: bool[], x: number, y: number, flag: bool = (a == a) == !(2 >= 3)) {
            mark(a, x + y);
        }
        fn main() {
            let a = [true];
            stage(a, mark(a, 1), mark(a, 2));
        }
        "#,
    );
    let [off, on] = [&pair[0], &pair[1]].map(|bytes| bodies(bytes));
    assert!(
        operators(&off[1])
            .iter()
            .any(|op| matches!(op, Operator::I32GeS))
    );
    assert!(
        !operators(&on[1])
            .iter()
            .any(|op| matches!(op, Operator::I32GeS))
    );
    run_node_pair(
        pair,
        r#"
        const effects = [];
        const e = new WebAssembly.Instance(new WebAssembly.Module(bytes), {
            env: { mark(handle, value) {
                effects.push(value);
                if (value === 2) {
                    const memory = new DataView(e.memory.buffer);
                    // A noncanonical element traps when consumed by equality in the default.
                    memory.setUint8(memory.getUint32(handle, true), 2);
                }
                return value;
            } }
        }).exports;
        assert.throws(() => e.main(), WebAssembly.RuntimeError);
        assert.deepEqual(effects, [1, 2]);
        "#,
    );
}

fn run_node_pair([off, on]: [Vec<u8>; 2], assertions: &str) {
    let mut input = u32::try_from(off.len()).unwrap().to_le_bytes().to_vec();
    input.extend(off);
    input.extend(on);
    let script = format!(
        r#"
        const assert = require('node:assert/strict');
        const input = require('node:fs').readFileSync(0);
        const split = 4 + input.readUInt32LE(0);
        for (const bytes of [input.subarray(4, split), input.subarray(split)]) {{
            {assertions}
        }}
    "#
    );
    let output = test_process::output(
        Command::new("node").args(["-e", &script]),
        Some(&input),
        Duration::from_secs(15),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn skipped_and_unreachable_scalar_operators_still_check_their_operands() {
    for (source, function_index, statement_index) in [
        ("fn main() { false && !(1 == 2); }", 0, 0),
        ("fn main() { return; !(1 == 2); }", 0, 1),
        ("fn main() {} fn hidden() { !(1 == 2); }", 1, 0),
    ] {
        let mut compiler = Compiler::new();
        compiler.set_exports(ExportOptions::Only(vec!["main".into()]));
        let mut program = compiler.analyze(source).unwrap();
        let function = &mut program.functions[function_index];
        let statement = &mut function.body[statement_index];
        let TypedStatementKind::Expression(expr) = &mut statement.kind else {
            panic!("expected expression statement")
        };
        let negation = if let TypedExprKind::Binary { right, .. } = &mut expr.kind {
            right
        } else {
            expr
        };
        let TypedExprKind::Unary { expr: equality, .. } = &mut negation.kind else {
            panic!("expected boolean negation")
        };
        let TypedExprKind::Binary { left, right, .. } = &mut equality.kind else {
            panic!("expected numeric equality")
        };
        let expected = corrupt_literals(left, right, &function.name, statement.span);
        for level in [
            OptimizationLevel::None,
            OptimizationLevel::Basic,
            OptimizationLevel::Standard,
            OptimizationLevel::Aggressive,
        ] {
            compiler.set_optimization_level(level);
            assert_eq!(compiler.compile(&program).unwrap_err(), expected);
        }
    }
}

fn corrupt_literals(
    left: &mut TypedExpr,
    right: &mut TypedExpr,
    function: &str,
    span: Span,
) -> CodegenError {
    left.kind = TypedExprKind::Number("first".into());
    right.kind = TypedExprKind::Number("second".into());
    CodegenError::InvalidNumberLiteral {
        function: function.into(),
        literal: "first".into(),
        span,
    }
}

#[test]
fn skipped_and_unreachable_arithmetic_preserves_first_diagnostic() {
    for (source, index) in [
        ("fn main() { false && (((1.5 + 2.5) * 3) > 0); }", 0),
        ("fn main() { return; (1.5 + 2.5) * 3; }", 1),
    ] {
        let mut compiler = Compiler::new();
        let mut program = compiler.analyze(source).unwrap();
        let statement = &mut program.functions[0].body[index];
        let span = statement.span;
        let TypedStatementKind::Expression(expr) = &mut statement.kind else {
            panic!("expected expression statement")
        };
        let arithmetic = if index == 0 {
            let TypedExprKind::Binary { right, .. } = &mut expr.kind else {
                panic!("expected short-circuit operator")
            };
            let TypedExprKind::Binary { left, .. } = &mut right.kind else {
                panic!("expected comparison")
            };
            left
        } else {
            expr
        };
        let TypedExprKind::Binary { left, right, .. } = &mut arithmetic.kind else {
            panic!("expected multiplication")
        };
        let TypedExprKind::Binary { left, .. } = &mut left.kind else {
            panic!("expected addition")
        };
        let expected = corrupt_literals(left, right, "main", span);
        let invalid_source = source.replace("1.5", "true").replace("2.5", "missing");
        let source_error = compiler.compile_source(&invalid_source).unwrap_err();
        for level in [
            OptimizationLevel::None,
            OptimizationLevel::Basic,
            OptimizationLevel::Standard,
            OptimizationLevel::Aggressive,
        ] {
            compiler.set_optimization_level(level);
            assert_eq!(compiler.compile(&program).unwrap_err(), expected);
            assert_eq!(
                compiler.compile_source(&invalid_source).unwrap_err(),
                source_error
            );
        }
        assert_eq!(
            compile(&program, FoldingPolicy::Disabled).unwrap_err(),
            expected
        );
    }
}
