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
                Array.from(bytes().slice(p - 32, p + 9))
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
fn simultaneous_zero_requests_remain_active_until_explicit_release() {
    run(
        Config::default(),
        r#"
        const blocks = Array.from({length: 6}, () => acquire(0n));
        assert.equal(new Set(blocks.map(([p]) => p)).size, 6);
        assert.deepEqual(blocks.map(([, id]) => id), [1n, 2n, 3n, 4n, 5n, 6n]);
        const cursor = observe(0);
        for (const [p, id] of blocks) {
            assert.equal(p % 8, 0);
            assert.equal(observe(3, p), 1n);
            assert.equal(observe(4, p), id);
            assert.equal(observe(5, p), 8n);
            assert.equal(observe(6, p), 0n);
        }
        assert.equal(observe(7, 0), 0n);
        // Zero requested bytes permit no payload reads or writes, even though
        // each block reserves eight bytes and owns a separate record.
        const freed = new Set([blocks[1][0], blocks[4][0]]);
        for (const index of [1, 4]) e.__m3_release(...blocks[index]);
        const reused = [acquire(0n), acquire(0n)];
        assert.deepEqual(new Set(reused.map(([p]) => p)), freed);
        assert.deepEqual(reused.map(([, id]) => id), [7n, 8n]);
        assert.equal(observe(0), cursor);
        assert.equal(observe(2), 6n);
        assert.equal(observe(7, 0), 0n);
        for (const index of [0, 2, 3, 5]) {
            const [p, id] = blocks[index];
            assert.equal(observe(3, p), 1n);
            assert.equal(observe(4, p), id);
            e.__m3_release(p, id);
        }
        for (const block of reused) e.__m3_release(...block);
        for (const [p] of blocks) assert.equal(observe(3, p), 2n);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn mixed_classes_preserve_live_extents_and_metadata_across_unordered_reuse() {
    run(
        Config::default(),
        r#"
        const groups = [
            {capacity: 8, classIndex: 0, requests: [1, 8, 5, 7, 2], renew: [6, 8]},
            {capacity: 16, classIndex: 1, requests: [9, 16, 13, 15, 10], renew: [12, 16]},
            {capacity: 32, classIndex: 2, requests: [17, 32, 21, 31, 18], renew: [29, 32]},
            {capacity: 128, classIndex: 4, requests: [65, 128, 93, 127, 66], renew: [100, 128]}
        ];
        const records = new Map(), live = new Map();
        let nextIdentity = 1n;
        function verify() {
            const active = [...live.values()];
            for (const block of active) {
                const {p, id, n, capacity, classIndex, expected} = block;
                assert.equal(observe(3, p), 1n);
                assert.equal(observe(4, p), id);
                assert.equal(observe(5, p), BigInt(capacity));
                assert.equal(observe(6, p), BigInt(n));
                assert.equal(word(p - 24), classIndex);
                assert.equal(word(p - 20), 1);
                assert.equal(wide(p - 16), id);
                assert.equal(wide(p - 8), BigInt(n));
                assert.deepEqual(bytes().slice(p, p + n), expected);
            }
            for (let i = 0; i < active.length; i++) {
                for (const other of active.slice(i + 1)) {
                    const a = active[i];
                    // Include records and the entire reserved capacity in the
                    // disjointness check, but never read unused payload bytes.
                    assert.ok(a.p + a.capacity <= other.p - 32 ||
                              other.p + other.capacity <= a.p - 32);
                }
            }
        }
        function take(group, n) {
            const [p, id] = acquire(BigInt(n));
            assert.equal(id, nextIdentity++);
            assert.equal(p % 8, 0);
            assert.ok(!live.has(p));
            if (records.has(p)) assert.notEqual(id, records.get(p));
            records.set(p, id);
            const headers = [...records.keys()].map(address =>
                [address, bytes().slice(address - 32, address)]);
            const expected = Uint8Array.from({length: n}, (_, i) =>
                (Number(id) * 37 + i * 19) % 256);
            // Initialize exactly the requested extent on every acquisition;
            // reuse provides no zero-initialization or spare-capacity rights.
            bytes().set(expected, p);
            for (const [address, header] of headers) {
                assert.deepEqual(bytes().slice(address - 32, address), header);
            }
            const block = {p, id, n, ...group, expected};
            live.set(p, block);
            verify();
            return block;
        }
        const blocks = groups.map(() => []);
        for (let i = 0; i < 5; i++) {
            for (const g of [2, 0, 3, 1]) blocks[g].push(take(groups[g], groups[g].requests[i]));
        }
        const cursor = observe(0);
        assert.equal(observe(2), 20n);
        const freed = groups.map(() => new Set());
        for (const [g, i] of [[3, 1], [0, 3], [2, 1], [1, 3], [0, 1], [3, 3], [1, 1], [2, 3]]) {
            const {p, id} = blocks[g][i];
            e.__m3_release(p, id);
            live.delete(p);
            freed[g].add(p);
            assert.equal(observe(3, p), 2n);
            verify();
        }
        for (const g of [1, 3, 0, 2]) {
            // Each class has multiple free AND live blocks before reacquisition.
            assert.equal(freed[g].size, 2);
            assert.equal([...live.values()].filter(b => b.classIndex === groups[g].classIndex).length, 3);
        }
        for (let i = 0; i < 2; i++) {
            for (const g of [1, 3, 0, 2]) {
                const {p} = take(groups[g], groups[g].renew[i]);
                assert.ok(freed[g].delete(p), 'reuse must be a member of this class, not a prescribed order');
                assert.equal(observe(0), cursor);
                assert.equal(observe(2), 20n);
            }
        }
        for (const group of groups) assert.equal(observe(7, group.classIndex), 0n);
        verify();
        for (const {p, id} of [...live.values()].reverse()) e.__m3_release(p, id);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn adjacent_free_blocks_are_not_coalesced_split_or_borrowed() {
    run(
        Config::default(),
        r#"
        const [a, aId] = acquire(9n), [b, bId] = acquire(16n);
        assert.equal(b - 32, a + 16, 'fresh physical blocks are adjacent');
        fill(a, 9, 0x31);
        fill(b, 16, 0x72);
        check(a, 9, 0x31);
        check(b, 16, 0x72);
        e.__m3_release(a, aId);
        e.__m3_release(b, bId);
        const smallHead = observe(7, 1), before = observe(0);
        const [large, largeId] = acquire(17n);
        assert.equal(BigInt(large), before + 32n);
        assert.equal(observe(0), before + 64n);
        assert.equal(observe(2), 3n);
        assert.equal(observe(7, 1), smallHead);
        assert.equal(observe(3, a), 2n);
        assert.equal(observe(3, b), 2n);
        fill(large, 17, 0x19);
        check(large, 17, 0x19);
        e.__m3_release(large, largeId);
        const largeHead = observe(7, 2), afterLarge = observe(0);
        const [tiny, tinyId] = acquire(1n);
        assert.equal(BigInt(tiny), afterLarge + 32n);
        assert.equal(observe(0), afterLarge + 40n);
        assert.equal(observe(2), 4n);
        assert.equal(observe(7, 1), smallHead);
        assert.equal(observe(7, 2), largeHead);
        assert.equal(observe(3, large), 2n);
        fill(tiny, 1, 0xff);
        const cursor = observe(0);
        const reused = [acquire(15n), acquire(10n), acquire(31n)];
        assert.deepEqual(new Set(reused.slice(0, 2).map(([p]) => p)), new Set([a, b]));
        assert.equal(reused[2][0], large);
        assert.deepEqual(reused.map(([, id]) => id), [5n, 6n, 7n]);
        assert.equal(observe(0), cursor);
        assert.equal(observe(2), 4n);
        assert.equal(observe(7, 1), 0n);
        assert.equal(observe(7, 2), 0n);
        for (const [i, [p, id]] of reused.entries()) {
            const n = [15, 10, 31][i];
            fill(p, n, 0x40 + i);
            check(p, n, 0x40 + i);
            assert.equal(observe(4, p), id);
            assert.equal(observe(6, p), BigInt(n));
        }
        check(tiny, 1, 0xff);
        for (const block of reused) e.__m3_release(...block);
        e.__m3_release(tiny, tinyId);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn real_memory_growth_preserves_initialized_live_blocks_identities_and_free_records() {
    run(
        Config::default(),
        r#"
        const retained = e.retained() >>> 0, ordinary = word(retained);
        const descriptor = bytes().slice(retained, retained + 8);
        const live = [], records = new Set();
        let nextIdentity = 1n, growths = 0;
        function verify() {
            assert.deepEqual(bytes().slice(retained, retained + 8), descriptor);
            assert.equal(view().getFloat64(ordinary, true), 17);
            assert.equal(view().getFloat64(ordinary + 8, true), 29);
            for (const {p, id, n, capacity, expected} of live) {
                assert.equal(observe(3, p), 1n);
                assert.equal(observe(4, p), id);
                assert.equal(observe(5, p), BigInt(capacity));
                assert.equal(observe(6, p), BigInt(n));
                assert.deepEqual(bytes().slice(p, p + n), expected);
            }
        }
        function take(n, capacity, mustGrow) {
            const oldBuffer = e.memory.buffer, oldView = bytes();
            const pages = observe(1), cursor = observe(0);
            const headers = [...records].map(p => [p, bytes().slice(p - 32, p)]);
            const [p, id] = acquire(BigInt(n));
            assert.equal(id, nextIdentity++);
            assert.equal(p % 8, 0);
            if (mustGrow) {
                growths++;
                assert.ok(observe(1) > pages);
                assert.notEqual(e.memory.buffer, oldBuffer);
                assert.equal(oldBuffer.byteLength, 0);
                assert.equal(oldView.byteLength, 0);
            }
            // Every view below is recreated from the current memory buffer.
            for (const [address, header] of headers) {
                assert.deepEqual(bytes().slice(address - 32, address), header);
            }
            assert.equal(BigInt(p), cursor + 32n);
            assert.equal(observe(0), cursor + BigInt(32 + capacity));
            for (const block of live) {
                assert.ok(block.p + block.capacity <= p - 32 || p + capacity <= block.p - 32);
            }
            const header = bytes().slice(p - 32, p);
            const expected = Uint8Array.from({length: n}, (_, i) => (i * 13 + Number(id) * 41) % 256);
            bytes().set(expected, p);
            assert.deepEqual(bytes().slice(p - 32, p), header);
            records.add(p);
            const block = {p, id, n, capacity, expected};
            live.push(block);
            verify();
            return block;
        }
        take(9, 16, false);
        take(127, 128, false);
        const free = take(33, 64, false);
        live.pop();
        e.__m3_release(free.p, free.id);
        const freeHeader = bytes().slice(free.p - 32, free.p);
        take(65537, 131072, true);
        take(17, 32, false);
        take(131073, 262144, true);
        assert.equal(growths, 2);
        assert.deepEqual(bytes().slice(free.p - 32, free.p), freeHeader);
        assert.equal(observe(3, free.p), 2n);
        const cursor = observe(0), population = observe(2);
        const [p, id] = acquire(63n);
        assert.equal(p, free.p);
        assert.equal(id, 7n);
        assert.notEqual(id, free.id);
        assert.equal(observe(0), cursor);
        assert.equal(observe(2), population);
        fill(p, 63, 0xa7);
        check(p, 63, 0xa7);
        verify();
        e.__m3_release(p, id);
        for (const block of live) e.__m3_release(block.p, block.id);
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
