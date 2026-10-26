use std::{process::Command, time::Duration};

use super::{Fact, Outcome, analyze};
use crate::{ArrayElementType, Compiler, OptimizationLevel, Type};

fn run_before_and_after(
    mut compiler: Compiler,
    source: &str,
    script: &str,
    check: impl Fn(&[Fact<'_>]),
) {
    let program = compiler.analyze(source).unwrap();
    let original = program.clone();
    let levels = [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ];
    // Compile every preset before the first explicit M1 analysis of this tree.
    let before = levels.map(|level| {
        compiler.set_optimization_level(level);
        compiler.compile(&program).unwrap()
    });
    check(&analyze(&program));
    assert_eq!(program, original);

    for (level, before) in levels.into_iter().zip(before) {
        compiler.set_optimization_level(level);
        let after = compiler.compile(&program).unwrap();
        assert_eq!(before, after, "M1 changed bytes at {level:?}");
        for (phase, bytes) in [("before", before), ("after", after)] {
            wasmparser::Validator::new().validate_all(&bytes).unwrap();
            let output = crate::test_process::output(
                Command::new("node").args(["-e", script]),
                Some(&bytes),
                Duration::from_secs(15),
            );
            assert!(
                output.status.success(),
                "{phase} M1 at {level:?}:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    assert_eq!(program, original);
}

#[test]
#[ignore = "requires Node.js"]
fn lifetime_candidates_can_trap_without_rolling_back_writes_or_allocations() {
    run_before_and_after(
        Compiler::new(),
        r#"
        fn number_read(i: number) -> number {
            let a = [10, 20];
            a[0] += 5;
            return a[i];
        }
        fn number_compound(i: number) -> number {
            let a = [10, 20];
            a[0] = 15;
            a[i] %= 6;
            return a[0];
        }
        fn bool_read(i: number) -> bool {
            let a = [false, false];
            a[0] = true;
            return a[i];
        }
        fn bool_write(i: number) -> bool {
            let a = [false, false];
            a[0] = true;
            a[i] = true;
            return a[1];
        }
        "#,
        r#"
        const assert = require('node:assert/strict');
        const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0))).exports;
        const view = () => new DataView(e.memory.buffer);
        const word = p => view().getUint32(p, true);
        for (const [name, size, read, initial, zero, one] of [
            ['number_read', 24, p => view().getFloat64(p, true), [15,20], 15, 20],
            ['number_compound', 24, p => view().getFloat64(p, true), [15,20], 3, 15],
            ['bool_read', 16, p => view().getUint8(p), [1,0], 1, 0],
            ['bool_write', 16, p => view().getUint8(p), [1,0], 0, 1],
        ]) {
            for (const i of [-1, 0.5, 2, NaN, Infinity]) {
                const marker = e.__alloc(8), h = marker + 8;
                assert.throws(() => e[name](i), WebAssembly.RuntimeError, name);
                // These literals reserve one descriptor plus an aligned two-element payload.
                assert.equal(word(h), h + 8);
                assert.equal(word(h + 4), 2);
                const stride = size === 24 ? 8 : 1;
                assert.deepEqual([read(h + 8), read(h + 8 + stride)], initial);
                assert.equal(e.__alloc(8), h + size, 'trapping allocation is not reclaimed');
                assert.equal(e[name](-0), zero);
                assert.equal(e[name](1), one);
                assert.deepEqual([read(h + 8), read(h + 8 + stride)], initial,
                    'later invocations do not reuse trapped storage');
            }
        }
        "#,
        |facts| {
            assert_eq!(facts.len(), 4);
            assert!(
                facts
                    .iter()
                    .all(|fact| fact.outcome == Outcome::TemporaryCandidate)
            );
            assert_eq!(
                facts
                    .iter()
                    .map(|fact| fact.origin.element)
                    .collect::<Vec<_>>(),
                [
                    ArrayElementType::Number,
                    ArrayElementType::Number,
                    ArrayElementType::Bool,
                    ArrayElementType::Bool
                ]
            );
        },
    );
}

#[test]
#[ignore = "requires Node.js"]
fn lifetime_aliases_across_user_calls_remain_conservative_and_shared() {
    run_before_and_after(
        Compiler::new(),
        r#"
        fn mutate(a: number[]) { a[1] = 7; }
        fn main() -> number {
            let a = [3, 4];
            let alias = a;
            alias[0] = 9;
            mutate(alias);
            return a[0] + alias[1];
        }
        "#,
        r#"
        const assert = require('node:assert/strict');
        const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0))).exports;
        assert.equal(e.main(), 16);
        assert.equal(e.main(), 16);
        "#,
        |facts| {
            assert_eq!(facts.len(), 1);
            assert!(matches!(facts[0].outcome, Outcome::Unproven(_)));
        },
    );
}

#[test]
#[ignore = "requires Node.js"]
fn lifetime_host_barrier_reentry_and_trap_preserve_distinct_live_arrays() {
    let mut compiler = Compiler::new();
    compiler
        .register_host_function(
            "observe",
            "env",
            "observe",
            vec![Type::Number, Type::Array(ArrayElementType::Bool)],
            Type::Void,
        )
        .unwrap();
    run_before_and_after(
        compiler,
        r#"
        fn main(tag: number, i: number) -> bool {
            let flags = [false, false];
            flags[0] = true;
            observe(tag, flags);
            flags[1] = true;
            return flags[i];
        }
        "#,
        r#"
        const assert = require('node:assert/strict');
        const events = [], retained = [];
        let e;
        const view = () => new DataView(e.memory.buffer);
        const word = p => view().getUint32(p, true);
        const read = h => [view().getUint8(word(h)), view().getUint8(word(h) + 1)];
        e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)), {
            env: {observe: (tag, h) => {
                events.push(tag); retained.push(h);
                assert.deepEqual(read(h), [1,0]);
                if (tag === 1) {
                    // Reentry may grow memory; every observation refreshes its DataView.
                    e.__alloc(70000);
                    assert.equal(e.main(2, 0), 1);
                    assert.deepEqual(read(h), [1,0]);
                    view().setUint8(word(h), 0);
                }
            }}
        }).exports;
        assert.throws(() => e.main(1, 2), WebAssembly.RuntimeError);
        assert.deepEqual(events, [1,2]);
        assert.notEqual(retained[0], retained[1]);
        assert.deepEqual(read(retained[0]), [0,1]);
        assert.deepEqual(read(retained[1]), [1,1]);
        assert.equal(e.main(3, 1), 1);
        assert.deepEqual(events, [1,2,3]);
        assert.equal(new Set(retained).size, 3);
        assert.deepEqual(read(retained[0]), [0,1]);
        assert.deepEqual(read(retained[1]), [1,1]);
        assert.deepEqual(read(retained[2]), [1,1]);
        "#,
        |facts| {
            assert_eq!(facts.len(), 1);
            assert!(!matches!(facts[0].outcome, Outcome::TemporaryCandidate));
        },
    );
}
