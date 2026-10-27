use std::{process::Command, time::Duration};

use super::{Config, compile};
use crate::{Compiler, exports::ExportOptions};

const SOURCE: &str = "fn retained() -> number[] { return [17, 29]; }";

fn run(config: Config, script: &str) {
    let program = Compiler::new().analyze(SOURCE).unwrap();
    let bytes = compile(&program, &ExportOptions::All, config).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let script = format!(
        r#"
        const assert = require('node:assert/strict');
        const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0))).exports;
        const acquire = n => {{ const [p, id] = e.__m3_acquire(n); return [p >>> 0, id]; }};
        const observe = (selector, argument = 0) => e.__m3_observe(selector, argument);
        const bytes = () => new Uint8Array(e.memory.buffer);
        const view = () => new DataView(e.memory.buffer);
        const word = p => view().getUint32(p, true);
        const wide = p => view().getBigUint64(p, true);
        const fill = (p, n, value) => bytes().fill(value, p, p + n);
        const check = (p, n, value) => assert.deepEqual(Array.from(bytes().slice(p, p + n)), Array(n).fill(value));
        {script}
        "#
    );
    let output = crate::test_process::output(
        Command::new("node").args(["-e", &script]),
        Some(&bytes),
        Duration::from_secs(15),
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires Node.js"]
fn live_blocks_reuse_with_new_identity_and_preserve_other_storage() {
    run(
        Config::default(),
        r#"
        const retained = e.retained() >>> 0;
        const ordinary = word(retained);
        const start = observe(0);
        const [a, first] = acquire(9n);
        const [b, second] = acquire(13n);
        assert.equal(a % 8, 0);
        assert.equal(b % 8, 0);
        assert.ok(a + 16 <= b || b + 16 <= a);
        assert.equal(first, 1n);
        assert.equal(second, 2n);
        fill(a, 9, 0x31);
        fill(b, 13, 0x72);
        assert.equal(observe(2), 2n);
        assert.equal(observe(3, a), 1n);
        assert.equal(observe(4, a), first);
        assert.equal(observe(5, a), 16n);
        assert.equal(observe(6, a), 9n);
        // Independent management-record offsets, not compiler constants.
        assert.equal(BigInt(a), start + 32n);
        assert.equal(word(a - 32), 0);
        assert.equal(word(a - 28), 0);
        assert.equal(word(a - 24), 1);
        assert.equal(word(a - 20), 1);
        assert.equal(wide(a - 16), first);
        assert.equal(wide(a - 8), 9n);
        assert.equal(word(b - 32), a - 32);
        e.__m3_release(a, first);
        assert.equal(observe(3, a), 2n);
        assert.equal(observe(7, 1), BigInt(a - 32));
        const cursor = observe(0);
        const [again, renewed] = acquire(15n);
        assert.equal(again, a);
        assert.equal(renewed, 3n);
        assert.equal(observe(0), cursor);
        assert.equal(observe(2), 2n);
        assert.equal(observe(6, again), 15n);
        fill(again, 15, 0x53);
        check(again, 15, 0x53);
        check(b, 13, 0x72);
        assert.equal(view().getFloat64(ordinary, true), 17);
        assert.equal(view().getFloat64(ordinary + 8, true), 29);
        e.__m3_release(again, renewed);
        e.__m3_release(b, second);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn zero_requests_are_distinct_and_classification_rejects_wide_invalid_requests() {
    run(
        Config::default(),
        r#"
        const initial = [observe(0), observe(1), observe(2), observe(8)];
        assert.equal(e.__m3_class(0n), 0);
        assert.equal(e.__m3_class(1n), 0);
        for (let classIndex = 0; classIndex < 29; classIndex++) {
            const capacity = 8n << BigInt(classIndex);
            const firstRequest = classIndex === 0 ? 0n : capacity / 2n + 1n;
            for (const request of [firstRequest, capacity - 1n, capacity]) {
                assert.equal(e.__m3_class(request), classIndex, `class ${classIndex}, request ${request}`);
            }
            if (classIndex < 28) {
                assert.equal(e.__m3_class(capacity + 1n), classIndex + 1);
            } else {
                assert.throws(() => e.__m3_class(capacity + 1n), WebAssembly.RuntimeError);
            }
        }
        assert.deepEqual([observe(0), observe(1), observe(2), observe(8)], initial);
        const [a, first] = acquire(0n);
        const [b, second] = acquire(0n);
        assert.notEqual(a, b);
        assert.notEqual(first, second);
        assert.equal(observe(5, a), 8n);
        assert.equal(observe(6, a), 0n);
        const before = [observe(0), observe(1), observe(2), observe(8)];
        for (const request of [-1n, -9223372036854775808n, 2147483649n, 4294967296n, 9223372036854775807n]) {
            assert.throws(() => e.__m3_class(request), WebAssembly.RuntimeError);
            assert.throws(() => acquire(request), WebAssembly.RuntimeError);
            assert.deepEqual([observe(0), observe(1), observe(2), observe(8)], before);
        }
        for (const selector of [3, 4, 5, 6]) {
            assert.equal(observe(selector, a + 1), 0n);
            assert.equal(observe(selector, 0), 0n);
        }
        assert.throws(() => observe(7, -1), WebAssembly.RuntimeError);
        assert.throws(() => observe(7, 29), WebAssembly.RuntimeError);
        assert.equal(observe(3, a), 1n);
        assert.equal(observe(3, b), 1n);
        e.__m3_release(a, first);
        e.__m3_release(b, second);
    "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn double_release_and_stale_identity_trap_without_corrupting_same_instance() {
    run(
        Config::default(),
        r#"
        const [a, old] = acquire(8n);
        const [live, liveId] = acquire(8n);
        fill(live, 8, 0x71);
        e.__m3_release(a, old);
        const cursor = observe(0);
        const head = observe(7, 0);
        assert.throws(() => e.__m3_release(a, old), WebAssembly.RuntimeError);
        assert.equal(observe(7, 0), head);
        assert.equal(observe(3, a), 2n);
        const [again, renewed] = acquire(8n);
        assert.equal(again, a);
        assert.notEqual(renewed, old);
        fill(again, 8, 0x29);
        assert.throws(() => e.__m3_release(again, old), WebAssembly.RuntimeError);
        assert.equal(observe(3, again), 1n);
        assert.equal(observe(4, again), renewed);
        assert.equal(observe(7, 0), 0n);
        assert.equal(observe(0), cursor);
        assert.equal(observe(2), 2n);
        assert.equal(observe(8), renewed);
        check(again, 8, 0x29);
        check(live, 8, 0x71);
        e.__m3_release(again, renewed);
        const [reused, next] = acquire(8n);
        assert.equal(reused, again);
        assert.equal(next, renewed + 1n);
        assert.notEqual(reused, live);
        check(live, 8, 0x71);
        e.__m3_release(reused, next);
        e.__m3_release(live, liveId);
    "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn unknown_interior_and_wrong_identity_release_preserve_live_and_free_state() {
    run(
        Config::default(),
        r#"
        const hostStorage = e.__alloc(8) >>> 0;
        fill(hostStorage, 8, 0x45);
        const [live, liveId] = acquire(9n);
        const [free, freeId] = acquire(9n);
        fill(live, 9, 0x61);
        fill(free, 9, 0x27);
        e.__m3_release(free, freeId);
        const snapshot = () => [
            observe(0), observe(1), observe(2), observe(8),
            ...Array.from({length: 29}, (_, classIndex) => observe(7, classIndex)),
            ...[live, free].flatMap(p => [
                ...[3, 4, 5, 6].map(selector => observe(selector, p)),
                Array.from(bytes().slice(p - 32, p + 16))
            ])
        ];
        const before = snapshot();
        for (const [address, identity] of [
            [0, liveId], [hostStorage, liveId], [0xfffffff8, liveId],
            [live + 1, liveId], [free + 1, freeId], [live - 32, liveId],
            [live, freeId], [live, 0n]
        ]) {
            assert.throws(() => e.__m3_release(address, identity), WebAssembly.RuntimeError);
            assert.deepEqual(snapshot(), before);
            check(live, 9, 0x61);
            check(hostStorage, 8, 0x45);
        }
        const [again, renewed] = acquire(9n);
        assert.equal(again, free);
        assert.equal(renewed, 3n);
        assert.notEqual(again, live);
        assert.equal(observe(2), 2n);
        fill(again, 9, 0x38);
        check(live, 9, 0x61);
        check(again, 9, 0x38);
        e.__m3_release(again, renewed);
        e.__m3_release(live, liveId);
    "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn acquisition_reuses_only_exact_class_without_borrowing_larger_free_block() {
    run(
        Config::default(),
        r#"
        const [large, largeId] = acquire(17n);
        fill(large, 17, 0x72);
        e.__m3_release(large, largeId);
        const cursor = observe(0);
        const largeHead = observe(7, 2);
        assert.equal(largeHead, BigInt(large - 32));
        const [small, smallId] = acquire(9n);
        assert.notEqual(small, large);
        assert.equal(observe(0), cursor + 48n);
        assert.equal(observe(2), 2n);
        assert.equal(observe(5, small), 16n);
        assert.equal(observe(3, large), 2n);
        assert.equal(observe(7, 2), largeHead);
        assert.equal(observe(7, 1), 0n);
        fill(small, 9, 0x43);
        const afterFresh = observe(0);
        const [again, renewed] = acquire(31n);
        assert.equal(again, large);
        assert.notEqual(renewed, largeId);
        assert.equal(observe(0), afterFresh);
        assert.equal(observe(2), 2n);
        assert.equal(observe(5, again), 32n);
        assert.equal(observe(6, again), 31n);
        fill(again, 31, 0x19);
        check(again, 31, 0x19);
        check(small, 9, 0x43);
        e.__m3_release(again, renewed);
        e.__m3_release(small, smallId);
    "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn identity_exhaustion_preserves_live_and_free_blocks_without_wrap() {
    run(
        Config {
            last_identity: u64::MAX - 2,
            backing_limit: None,
        },
        r#"
        const [live, liveId] = acquire(8n);
        const [free, freeId] = acquire(8n);
        assert.equal(BigInt.asUintN(64, liveId), 18446744073709551614n);
        assert.equal(BigInt.asUintN(64, freeId), 18446744073709551615n);
        fill(live, 8, 0x41);
        e.__m3_release(free, freeId);
        const before = [observe(0), observe(1), observe(2), observe(7, 0), observe(8)];
        assert.throws(() => acquire(8n), WebAssembly.RuntimeError);
        assert.throws(() => acquire(16n), WebAssembly.RuntimeError);
        assert.deepEqual([observe(0), observe(1), observe(2), observe(7, 0), observe(8)], before);
        assert.equal(observe(3, live), 1n);
        assert.equal(observe(3, free), 2n);
        check(live, 8, 0x41);
        e.__m3_release(live, liveId);
    "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn backing_limit_failure_preserves_free_availability_and_live_contents() {
    // No static payload: initial cursor 8, then two 32+8 byte reservations end at 88.
    run(
        Config {
            last_identity: 0,
            backing_limit: Some(88),
        },
        r#"
        assert.equal(observe(0), 8n);
        const [live, liveId] = acquire(8n);
        const [free, freeId] = acquire(8n);
        fill(live, 8, 0x62);
        e.__m3_release(free, freeId);
        const before = [observe(0), observe(1), observe(2), observe(7, 0), observe(8)];
        assert.equal(observe(0), 88n);
        assert.throws(() => acquire(9n), WebAssembly.RuntimeError);
        assert.deepEqual([observe(0), observe(1), observe(2), observe(7, 0), observe(8)], before);
        check(live, 8, 0x62);
        const [again, renewed] = acquire(8n);
        assert.equal(again, free);
        assert.equal(renewed, 3n);
        assert.equal(observe(0), 88n);
        assert.notEqual(again, live);
        fill(again, 8, 0x19);
        check(live, 8, 0x62);
        check(again, 8, 0x19);
        e.__m3_release(again, renewed);
        e.__m3_release(live, liveId);
    "#,
    );
}
