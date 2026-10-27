use std::{process::Command, time::Duration};

use super::*;

fn run(source: &str, script: &str) {
    let program = crate::Compiler::new().analyze(source).unwrap();
    for policy in [NumericPolicy::Baseline, NumericPolicy::ProvenIntegers] {
        let bytes = compile(&program, &ExportOptions::All, policy).unwrap();
        run_bytes(&bytes, script, policy);
    }
}

fn run_bytes(bytes: &[u8], script: &str, policy: NumericPolicy) {
    wasmparser::Validator::new().validate_all(bytes).unwrap();
    let output = crate::test_process::output(
        Command::new("node").args(["-e", script]),
        Some(bytes),
        Duration::from_secs(15),
    );
    assert!(
        output.status.success(),
        "{policy:?}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
    for policy in [NumericPolicy::Baseline, NumericPolicy::ProvenIntegers] {
        let bytes = compile(&program, &ExportOptions::All, policy).unwrap();
        let mut capped = wasm_encoder::Module::new();
        for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
            let payload = payload.unwrap();
            if let Some((id, range)) = payload.as_section() {
                if id == 5 {
                    let mut memory = wasm_encoder::MemorySection::new();
                    memory.memory(wasm_encoder::MemoryType {
                        minimum: 1,
                        maximum: Some(1),
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
        run_bytes(
            &capped.finish(),
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
            policy,
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
            assert.equal(e.flags(i % 2), 1);
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
        for (let t = 0; t < 4; t++) {
            const base = e.__alloc(8) + 8;
            assert.throws(() => e.read(10 + t, 99), WebAssembly.RuntimeError);
            assert.equal(word(base), base + 8);
            assert.equal(number(base + 8), 15 + t);
            assert.equal(e.__alloc(8), base + 24);
            assert.equal(e.read(100, 0), 105);
            assert.equal(e.read(200, 1), 201);
            assert.equal(number(base + 8), 15 + t);
            // Trap using the cached block: it must also be removed from reuse.
            assert.throws(() => e.read(300 + t, 99), WebAssembly.RuntimeError);
        }
        const base = e.__alloc(8) + 8;
        assert.throws(() => e.initialize(42, 99), WebAssembly.RuntimeError);
        assert.equal(word(base + 16), base + 24, 'second descriptor initialized before its elements');
        assert.equal(word(base + 20), 2);
        assert.equal(number(base + 24), 42, 'completed first element survives second-element trap');
        assert.equal(e.__alloc(8), base + 40);
        assert.equal(e.initialize(90, 0), 90);
        assert.equal(e.initialize(91, 0), 91);
        assert.equal(number(base + 24), 42);
        const boolBase = e.__alloc(8) + 8;
        assert.throws(() => e.flags(1, 99), WebAssembly.RuntimeError);
        assert.equal(view().getUint8(boolBase + 8), 1);
        assert.equal(view().getUint8(boolBase + 9), 1);
        assert.equal(e.flags(0, 0), 0);
        assert.equal(e.flags(1, 0), 1);
        assert.equal(view().getUint8(boolBase + 8), 1);
    "#,
    );
}
