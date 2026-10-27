use std::{process::Command, time::Duration};

use super::*;

use crate::optimization::{OptimizationLevel, optimize};

fn modules(program: &crate::typed_ast::TypedProgram) -> Vec<(OptimizationLevel, Vec<u8>)> {
    let mut enabled = None;
    [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ]
    .into_iter()
    .map(|level| {
        let optimized = optimize(program, level);
        let policy = level.numeric_policy();
        let ordinary =
            super::super::compile_with_policy(&optimized, &ExportOptions::All, policy).unwrap();
        let bytes = compile(&optimized, &ExportOptions::All, policy).unwrap();
        assert_eq!(
            bytes,
            compile(&optimized, &ExportOptions::All, policy).unwrap(),
            "{level:?}: deterministic region bytes"
        );
        assert_eq!(
            ordinary,
            super::super::compile_with_policy(&optimized, &ExportOptions::All, policy).unwrap(),
            "{level:?}: ordinary bytes unchanged after region compilation"
        );
        if level != OptimizationLevel::None {
            if let Some((previous_region, previous_ordinary)) = &enabled {
                assert_eq!(
                    &bytes, previous_region,
                    "{level:?}: enabled region byte identity"
                );
                assert_eq!(
                    &ordinary, previous_ordinary,
                    "{level:?}: enabled ordinary byte identity"
                );
            } else {
                enabled = Some((bytes.clone(), ordinary));
            }
        }
        (level, bytes)
    })
    .collect()
}

fn run(source: &str, script: &str) {
    let program = crate::Compiler::new().analyze(source).unwrap();
    for (level, bytes) in modules(&program) {
        run_bytes(&bytes, script, level);
    }
}

fn run_bytes(bytes: &[u8], script: &str, level: OptimizationLevel) {
    wasmparser::Validator::new().validate_all(bytes).unwrap();
    let output = crate::test_process::output(
        Command::new("node").args(["-e", script]),
        Some(bytes),
        Duration::from_secs(15),
    );
    assert!(
        output.status.success(),
        "{level:?}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn cap_memory(bytes: &[u8], maximum: u64) -> Vec<u8> {
    let mut capped = wasm_encoder::Module::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let Some((id, range)) = payload.unwrap().as_section() {
            if id == 5 {
                let mut memory = wasm_encoder::MemorySection::new();
                memory.memory(wasm_encoder::MemoryType {
                    minimum: 1,
                    maximum: Some(maximum),
                    memory64: false,
                    shared: false,
                    page_size_log2: None,
                });
                capped.section(&memory);
            } else {
                capped.section(&wasm_encoder::RawSection {
                    id,
                    data: &bytes[usize::try_from(range.start).unwrap()
                        ..usize::try_from(range.end).unwrap()],
                });
            }
        }
    }
    capped.finish()
}

#[test]
#[ignore = "requires Node.js"]
fn reentrant_recursive_callers_keep_retained_arrays_and_captures_isolated() {
    use crate::{ArrayElementType, Compiler, Type};

    let mut compiler = Compiler::new();
    compiler
        .register_host_function(
            "observe",
            "env",
            "observe",
            vec![
                Type::Number,
                Type::Array(ArrayElementType::Number),
                Type::Array(ArrayElementType::Bool),
                Type::Array(ArrayElementType::String),
            ],
            Type::Void,
        )
        .unwrap();
    let program = compiler
        .analyze(
            r#"
        fn temporary(n: number, i: number) -> number {
            let a = [n, n + 1]; a[0] += 5; return a[i];
        }
        fn caller(n: number) -> number {
            let nums = [n]; let flags = [true]; let strings = ["retained"];
            let captured = n + 10;
            let read = () => captured;
            observe(n, nums, flags, strings);
            if n > 0 { return caller(n - 1) + nums[0] + read(); }
            return nums[0] + read();
        }
    "#,
        )
        .unwrap();
    let selections = select(&program);
    assert!(matches!(selections[0], Selection::Region(_)));
    assert!(matches!(selections[1], Selection::Ineligible));
    for (level, bytes) in modules(&program) {
        run_bytes(
            &bytes,
            r#"
            const assert = require('node:assert/strict');
            let e;
            const saved = [];
            const view = () => new DataView(e.memory.buffer);
            const word = p => view().getUint32(p, true);
            const value = h => view().getFloat64(word(h), true);
            const readString = h => {
                const s = word(word(h));
                return Buffer.from(e.memory.buffer, word(s), word(s + 4)).toString();
            };
            e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)), {
                env: { observe(n, nums, flags, strings) {
                    saved.push([nums, flags, strings, n]);
                    assert.equal(value(nums), n);
                    assert.equal(view().getUint8(word(flags)), 1);
                    assert.equal(readString(strings), 'retained');
                    assert.throws(() => e.temporary(n, 99), WebAssembly.RuntimeError);
                    assert.equal(e.temporary(n, 0), n + 5);
                    const cursor = e.__alloc(8);
                    const pages = e.memory.buffer.byteLength;
                    for (let i = 0; i < 40; i++) {
                        assert.equal(e.temporary(n + i, 0), n + i + 5);
                        assert.equal(e.temporary(n + i, 1), n + i + 1);
                        for (const [a, b, c, expected] of saved) {
                            assert.equal(value(a), expected);
                            assert.equal(view().getUint8(word(b)), 1);
                            assert.equal(readString(c), 'retained');
                        }
                    }
                    assert.equal(e.__alloc(8), cursor + 8);
                    assert.equal(e.memory.buffer.byteLength, pages);
                    if (n === 2) {
                        e.__alloc(70000);
                        assert.equal(e.caller(0), 10);
                    }
                    assert.equal(value(nums), n);
                }}
            }).exports;
            assert.equal(e.caller(2), 36);
            assert.equal(saved.length, 4);
            assert.equal(new Set(saved.map(x => x[0])).size, 4);
            for (let i = 0; i < 40; i++) {
                assert.equal(e.temporary(i, 0), i + 5);
                assert.equal(e.caller(0), 10);
            }
            assert.equal(saved.length, 44);
            for (const column of [0, 1, 2]) {
                assert.equal(new Set(saved.map(x => x[column])).size, saved.length);
            }
            for (const [nums, flags, strings, n] of saved) {
                assert.equal(value(nums), n);
                assert.equal(view().getUint8(word(flags)), 1);
                assert.equal(readString(strings), 'retained');
            }
        "#,
            level,
        );
    }
}

#[test]
#[ignore = "requires Node.js"]
fn front_loaded_reservation_precedes_earlier_initializer_trap() {
    let program = crate::Compiler::new()
        .analyze(
            r#"
        fn early(n: number) -> number {
            let a = [n]; let b = [a[99]]; let c = [1, 2]; return b[0] + c[0];
        }
    "#,
        )
        .unwrap();
    assert!(matches!(select(&program)[0], Selection::Region(_)));
    for (level, region) in modules(&program) {
        let policy = level.numeric_policy();
        for (bytes, consumed) in [
            (region, 56),
            (
                super::super::compile_with_policy(&program, &ExportOptions::All, policy).unwrap(),
                32,
            ),
        ] {
            let script = format!(
                r#"
                const assert = require('node:assert/strict');
                const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0))).exports;
                const base = e.__alloc(8) + 8;
                assert.throws(() => e.early(17), WebAssembly.RuntimeError);
                const view = new DataView(e.memory.buffer);
                assert.equal(view.getFloat64(base + 8, true), 17);
                assert.equal(view.getUint32(base + 20, true), 1);
                assert.equal(e.__alloc(8), base + {consumed});
            "#
            );
            run_bytes(&bytes, &script, level);
        }
    }
}

#[test]
#[ignore = "requires Node.js"]
fn failed_front_loaded_reservation_preserves_cursor_and_ordinary_storage() {
    let program = crate::Compiler::new()
        .analyze(
            r#"
        fn retained(n: number) -> number[] { return [n]; }
        fn early(n: number) -> number {
            let a = [n]; return a[0]; let b = [a[99], 2, 3];
        }
        fn skipped() { return; let a = [1]; }
    "#,
        )
        .unwrap();
    for (level, bytes) in modules(&program) {
        run_bytes(
            &cap_memory(&bytes, 1),
            r#"
            const assert = require('node:assert/strict');
            const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0))).exports;
            const h = e.retained(321);
            const v = new DataView(e.memory.buffer);
            const payload = v.getUint32(h, true);
            const p = e.__alloc(8);
            e.__alloc(65536 - p - 8 - 32);
            e.skipped();
            // A 16-byte first literal fits, but the full 48-byte block does not.
            assert.throws(() => e.early(9), WebAssembly.RuntimeError);
            assert.equal(e.memory.buffer.byteLength, 65536);
            assert.equal(e.__alloc(8), 65504, 'failed reservation did not advance cursor');
            assert.equal(v.getFloat64(payload, true), 321);
            assert.equal(v.getUint32(h, true), payload);
            assert.throws(() => e.early(10), WebAssembly.RuntimeError);
            assert.equal(e.__alloc(8), 65512, 'no per-literal fallback or failed-provision leak');
        "#,
            level,
        );
    }
}

#[test]
#[ignore = "requires Node.js"]
fn capped_later_literal_reservation_changes_failure_and_growth_before_access_traps() {
    let program = crate::Compiler::new()
        .analyze(
            r#"
            fn early(n: number, i: number) -> number {
                let a = [n]; let scalar = a[i];
                let later = [1, 2, 3]; return scalar + later[0];
            }
        "#,
        )
        .unwrap();
    assert!(matches!(select(&program)[0], Selection::Region(_)));
    for (level, region) in modules(&program) {
        let ordinary = super::super::compile_with_policy(
            &program,
            &ExportOptions::All,
            level.numeric_policy(),
        )
        .unwrap();
        for maximum in [1, 2] {
            for (bytes, is_region) in [(&ordinary, false), (&region, true)] {
                let consumed = if is_region {
                    if maximum == 1 { 0 } else { 48 }
                } else {
                    16
                };
                let pages = if is_region && maximum == 2 { 2 } else { 1 };
                let script = format!(
                    r#"
                    const assert = require('node:assert/strict');
                    const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0))).exports;
                    const p = e.__alloc(8);
                    e.__alloc(65504 - p - 8);
                    new Uint8Array(e.memory.buffer, 65504, 32).fill(0xa5);
                    assert.throws(() => e.early(17, 99), WebAssembly.RuntimeError);
                    assert.equal(e.memory.buffer.byteLength, {pages} * 65536);
                    const v = new DataView(e.memory.buffer);
                    if ({consumed} === 0) {{
                        assert.ok(new Uint8Array(e.memory.buffer, 65504, 32).every(x => x === 0xa5),
                            'full reservation fails before even the first descriptor is written');
                    }} else {{
                        assert.equal(v.getUint32(65504, true), 65512);
                        assert.equal(v.getUint32(65508, true), 1);
                        assert.equal(v.getFloat64(65512, true), 17);
                        assert.equal(v.getUint32(65520, true), 0xa5a5a5a5,
                            'later literal reserved but not initialized before earlier scalar access trap');
                    }}
                    assert.equal(e.__alloc(8), 65504 + {consumed});
                "#
                );
                run_bytes(&cap_memory(bytes, maximum), &script, level);
            }
        }
    }
}

#[test]
#[ignore = "requires Node.js"]
fn acquisition_is_lazy_after_scalar_contract_and_absent_for_unreachable_literals() {
    let program = crate::Compiler::new()
        .analyze(
            r#"
            fn guarded(n: number) -> number requires n > 0 {
                let a = [n]; return a[0];
            }
            fn skipped() -> number { return 7; let a = [1, 2, 3]; }
        "#,
        )
        .unwrap();
    assert!(
        select(&program)
            .iter()
            .all(|s| matches!(s, Selection::Region(_)))
    );
    for (level, bytes) in modules(&program) {
        run_bytes(
            &cap_memory(&bytes, 1),
            r#"
            const assert = require('node:assert/strict');
            const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0))).exports;
            const start = e.__alloc(8);
            for (let i = 0; i < 20; i++) {
                assert.throws(() => e.guarded(0), WebAssembly.RuntimeError);
                assert.equal(e.skipped(), 7);
            }
            assert.equal(e.__alloc(8), start + 8, 'no acquisition before scalar contract or unreachable literal');
            assert.equal(e.guarded(9), 9);
            const cursor = e.__alloc(8);
            for (let i = 0; i < 20; i++) {
                assert.throws(() => e.guarded(-1), WebAssembly.RuntimeError);
                assert.equal(e.guarded(i + 1), i + 1);
            }
            assert.equal(e.__alloc(8), cursor + 8, 'pre-acquisition traps do not quarantine cached storage');
            const p = e.__alloc(8);
            e.__alloc(65536 - p - 8);
            for (let i = 0; i < 20; i++) {
                assert.equal(e.skipped(), 7);
                assert.throws(() => e.guarded(0), WebAssembly.RuntimeError);
                assert.equal(e.guarded(123), 123);
            }
            assert.equal(e.memory.buffer.byteLength, 65536);
        "#,
            level,
        );
    }
}

#[test]
#[ignore = "requires Node.js"]
fn reuse_reinitializes_distinct_aligned_literals_and_preserves_ordinary_handles() {
    run(
        r#"
        fn numbers(n: number) -> number {
            let a = [n, n + 1];
            let b = [a[0] + a[1]];
            let empty: number[] = [];
            a[0] += 3;
            a[0] -= 1;
            a[0] *= 2;
            a[0] /= 2;
            a[0] %= 7;
            a[1] = b[0];
            return a[0] + a[1] + len(empty);
        }
        fn flags(flag: bool) -> bool {
            let empty: bool[] = [];
            let a = [flag, false, true, false, true, false, true, false];
            let b = [!flag, true, false, true, false, true, false, true, false];
            a[1] = !a[0];
            len(empty);
            return a[1] == b[0];
        }
        fn bare(flag: bool) { let a = [flag]; a[0] = !flag; return; }
        fn fallthrough(flag: bool) { let a = [flag]; a[0] = !flag; }
        fn early(n: number) -> number {
            let a = [n]; return a[0];
            let b = [a[99], 2, 3];
        }
        fn never_allocates() { return; let a = [1, 2]; }
        fn retained(n: number) -> number[] { return [n, n + 1]; }
    "#,
        r#"
        const assert = require('node:assert/strict');
        const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0))).exports;
        const view = () => new DataView(e.memory.buffer);
        const word = p => view().getUint32(p, true);
        const number = p => view().getFloat64(p, true);
        const retained = e.retained(123);
        const payload = word(retained);
        const marker = e.__alloc(8), base = marker + 8;
        assert.equal(e.numbers(1), 6);
        // Independent offsets: number[2]=24, number[1]=16, empty=8.
        assert.equal(word(base), base + 8);
        assert.equal(word(base + 4), 2);
        assert.equal(word(base + 24), base + 32);
        assert.equal(word(base + 28), 1);
        assert.equal(number(base + 32), 3);
        assert.equal(word(base + 40), base + 48);
        assert.equal(word(base + 44), 0);
        assert.equal(e.__alloc(8), base + 48);
        const flagsBase = e.__alloc(8) + 8;
        assert.equal(e.flags(1), 1);
        // Empty=8, bool[8]=16, bool[9]=24, each individually aligned.
        assert.equal(word(flagsBase + 8), flagsBase + 16);
        assert.equal(word(flagsBase + 24), flagsBase + 32);
        assert.equal(e.__alloc(8), flagsBase + 48);
        e.bare(1); e.fallthrough(1);
        const earlyBase = e.__alloc(8) + 8;
        assert.equal(e.early(9), 9);
        assert.equal(e.__alloc(8), earlyBase + 48, 'unreachable suffix reserves capacity without executing');
        const before = e.__alloc(8);
        for (let i = 0; i < 200; i++) {
            assert.equal(e.numbers(i), (i + 2) % 7 + 2 * i + 1);
            // Retired temporary bytes have no retention rights. Poison only while
            // idle; the next call must replace every descriptor and payload byte
            // before any bool is consumed (this is not an invalid-bool input test).
            new Uint8Array(e.memory.buffer, flagsBase, 48).fill(0xa5);
            assert.equal(e.flags(i % 2), 1);
            assert.equal(word(flagsBase), flagsBase + 8);
            assert.equal(word(flagsBase + 4), 0);
            assert.equal(word(flagsBase + 8), flagsBase + 16);
            assert.equal(word(flagsBase + 12), 8);
            assert.equal(word(flagsBase + 24), flagsBase + 32);
            assert.equal(word(flagsBase + 28), 9);
            assert.deepEqual(Array.from(new Uint8Array(e.memory.buffer, flagsBase + 16, 8)),
                [i % 2, 1 - i % 2, 1, 0, 1, 0, 1, 0]);
            assert.deepEqual(Array.from(new Uint8Array(e.memory.buffer, flagsBase + 32, 9)),
                [1 - i % 2, 1, 0, 1, 0, 1, 0, 1, 0]);
            e.bare(i % 2); e.fallthrough(i % 2);
            assert.equal(e.early(i), i);
            e.never_allocates();
        }
        assert.equal(e.__alloc(8), before + 8, 'no region-driven heap growth after warmup');
        assert.equal(word(retained), payload);
        assert.equal(number(payload), 123);
        assert.equal(number(payload + 8), 124);
        const fresh = e.retained(456);
        assert.notEqual(fresh, retained);
        assert.equal(number(word(fresh)), 456);
        assert.equal(number(payload), 123);
    "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn traps_quarantine_owned_blocks_and_keep_completed_initialization_and_writes() {
    run(
        r#"
        fn read(n: number, i: number) -> number {
            let a = [n, n + 1]; a[0] += 5; return a[i];
        }
        fn initialize(n: number, i: number) -> number {
            let a = [n]; let b = [a[0], a[i]]; return b[1];
        }
        fn flags(flag: bool, i: number) -> bool {
            let a = [flag, false]; a[1] = true; return a[i];
        }
    "#,
        r#"
        const assert = require('node:assert/strict');
        const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0))).exports;
        const view = () => new DataView(e.memory.buffer);
        const word = p => view().getUint32(p, true);
        const number = p => view().getFloat64(p, true);
        const quarantined = [];
        const remember = (base, size) => {
            assert.ok(quarantined.every(([other, bytes]) =>
                base + size <= other || other + bytes.length <= base), 'distinct quarantined blocks');
            quarantined.push([base, Buffer.from(new Uint8Array(e.memory.buffer, base, size))]);
        };
        const verify = () => {
            for (const [base, bytes] of quarantined) {
                assert.deepEqual(Buffer.from(new Uint8Array(e.memory.buffer, base, bytes.length)), bytes,
                    `entire quarantined block at ${base} remains intact`);
            }
        };
        for (let t = 0; t < 4; t++) {
            const base = e.__alloc(8) + 8;
            assert.throws(() => e.read(10 + t, 99), WebAssembly.RuntimeError);
            assert.equal(word(base), base + 8);
            assert.equal(number(base + 8), 15 + t);
            remember(base, 24);
            verify();
            assert.equal(e.__alloc(8), base + 24);
            const cached = base + 32;
            assert.equal(e.read(100, 0), 105);
            assert.equal(e.read(200, 1), 201);
            assert.equal(word(cached), cached + 8);
            assert.equal(number(base + 8), 15 + t);
            // Trap using the cached block: it must also be removed from reuse.
            assert.throws(() => e.read(300 + t, 99), WebAssembly.RuntimeError);
            assert.equal(number(cached + 8), 305 + t);
            remember(cached, 24);
            verify();
            assert.equal(e.read(400 + t, 0), 405 + t);
            assert.equal(number(cached + 8), 305 + t);
            assert.throws(() => e.read(500 + t, 99), WebAssembly.RuntimeError);
            const third = cached + 24;
            assert.equal(word(third), third + 8);
            assert.equal(number(third + 8), 505 + t);
            remember(third, 24);
            verify();
        }
        assert.equal(quarantined.length, 12);
        const success = e.__alloc(8) + 8;
        assert.equal(e.read(600, 0), 605);
        assert.equal(e.__alloc(8), success + 24);
        const cursor = e.__alloc(8);
        for (let i = 0; i < 40; i++) {
            assert.equal(e.read(700 + i, 1), 701 + i);
            assert.equal(word(success), success + 8);
            assert.equal(number(success + 8), 705 + i);
            verify();
        }
        assert.equal(e.__alloc(8), cursor + 8, 'only the successful block is reused');
        const base = e.__alloc(8) + 8;
        assert.throws(() => e.initialize(42, 99), WebAssembly.RuntimeError);
        assert.equal(word(base + 16), base + 24, 'second descriptor initialized before its elements');
        assert.equal(word(base + 20), 2);
        assert.equal(number(base + 24), 42, 'completed first element survives second-element trap');
        remember(base, 40);
        assert.equal(e.__alloc(8), base + 40);
        assert.equal(e.initialize(90, 0), 90);
        assert.equal(e.initialize(91, 0), 91);
        assert.equal(number(base + 24), 42);
        const boolBase = e.__alloc(8) + 8;
        assert.throws(() => e.flags(1, 99), WebAssembly.RuntimeError);
        assert.equal(view().getUint8(boolBase + 8), 1);
        assert.equal(view().getUint8(boolBase + 9), 1);
        remember(boolBase, 16);
        assert.equal(e.flags(0, 0), 0);
        assert.equal(e.flags(1, 0), 1);
        assert.equal(view().getUint8(boolBase + 8), 1);
        e.__alloc(70000);
        for (let i = 0; i < 40; i++) {
            assert.equal(e.initialize(i, 0), i);
            assert.equal(e.flags(i % 2, 0), i % 2);
            assert.equal(e.read(i, 0), i + 5);
            verify();
        }
    "#,
    );
}
