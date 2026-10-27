'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const crypto = require('node:crypto');
const os = require('node:os');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const { source, preset, mode } = input;
assert.equal(typeof source, 'string');
assert.equal(typeof preset, 'string');
assert.ok(['accounting', 'capacity', 'timing', 'failure'].includes(mode));
const capacities = { numbers: 24, aligned: 48, lazy: 16 };
assert.deepEqual(input.capacities.map(c => [c.name, c.capacity]).sort(),
    Object.entries(capacities).sort());
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const bytes = {};
const modules = {};
for (const name of ['baseline', 'prototype', 'instrumented']) {
    assert.ok(Array.isArray(input[name]), `${name} module byte array`);
    bytes[name] = Buffer.from(input[name]);
    modules[name] = new WebAssembly.Module(bytes[name]);
    assert.deepEqual(WebAssembly.Module.imports(modules[name]), [], `${name} imports`);
}
const fresh = name => new WebAssembly.Instance(modules[name]).exports;
const pages = e => e.memory.buffer.byteLength / 65536;
const fields = ['provisions', 'reuses', 'literals', 'retirements'];

function counters(e, name) {
    return Object.fromEntries(fields.map(field => {
        const global = e[`__m2_${name}_${field}`];
        assert.ok(global instanceof WebAssembly.Global, `${name} ${field} global`);
        assert.equal(typeof global.value, 'bigint', `${name} ${field} i64`);
        assert.ok(global.value >= 0n);
        return [field, global.value];
    }));
}

function assertCounters(e, name, expected) {
    assert.deepEqual(Object.values(counters(e, name)), expected.map(BigInt), name);
}

// Every measurement starts only after independent scalar and complete trap-effect
// checks. These marked observation instances are never measurement instances.
function verify(name) {
    const e = fresh(name);
    for (const n of [-13.5, 0, 19.25, 1024]) {
        assert.equal(e.numbers(n, 0), n + 5, name);
        assert.equal(e.numbers(n, 1), n + 1, name);
        assert.equal(e.aligned(n), n + 9, name);
        assert.equal(e.lazy(), 7, name);
    }
    const trapped = fresh(name);
    const markerBefore = trapped.__alloc(0) >>> 0;
    const block = markerBefore + 8;
    assert.throws(() => trapped.numbers(37.5, 99), WebAssembly.RuntimeError, name);
    const view = new DataView(trapped.memory.buffer);
    assert.equal(view.getUint32(block, true), block + 8, `${name} descriptor pointer`);
    assert.equal(view.getUint32(block + 4, true), 2, `${name} descriptor length`);
    assert.equal(view.getFloat64(block + 8, true), 42.5, `${name} completed compound`);
    assert.equal(view.getFloat64(block + 16, true), 38.5, `${name} second element`);
    const retained = Buffer.from(new Uint8Array(trapped.memory.buffer, block, 24));
    const markerAfter = trapped.__alloc(0) >>> 0;
    assert.equal(markerAfter - markerBefore - 8, 24, `${name} trapped allocation`);
    assert.equal(trapped.numbers(-8, 0), -3);
    assert.equal(trapped.aligned(2), 11);
    assert.equal(trapped.lazy(), 7);
    assert.deepEqual(Buffer.from(new Uint8Array(trapped.memory.buffer, block, 24)),
        retained, `${name} completed trap effects survive later calls`);
    return { scalarCases: 16, trapCases: 1, completedTrapBytes: 24,
        markerBefore, markerAfter, markerReservationBytes: 8,
        markerCalls: 2, markerOverheadBytes: 16 };
}

function checkedBatch(e, calls) {
    for (let i = 0; i < calls; i++) {
        assert.equal(e.numbers(i, 0), i + 5);
        assert.equal(e.aligned(i), i + 9);
    }
}

function accounting() {
    const e = fresh('instrumented');
    const snapshots = [];
    function snapshot(label, cached) {
        const functions = {};
        for (const [name, capacity] of Object.entries(capacities)) {
            const c = counters(e, name);
            const outstanding = c.provisions + c.reuses - c.retirements;
            assert.ok(outstanding >= 0n);
            const cachedBlocks = c.retirements - c.reuses;
            assert.ok(cachedBlocks === 0n || cachedBlocks === 1n);
            assert.equal(cachedBlocks, cached.includes(name) ? 1n : 0n);
            // At quiescent call boundaries, outstanding acquisitions are trapped
            // blocks, not active invocations. These are event-derived capacities.
            functions[name] = {
                ...Object.fromEntries(fields.map(field => [field, c[field].toString()])),
                provisionedBytes: (c.provisions * BigInt(capacity)).toString(),
                reusedBlockBytes: (c.reuses * BigInt(capacity)).toString(),
                cachedBlocks: cachedBlocks.toString(),
                cachedCapacityBytes: Number(cachedBlocks) * capacity,
                quarantinedBlocks: outstanding.toString(),
                quarantineBytes: (outstanding * BigInt(capacity)).toString(),
            };
        }
        snapshots.push({ label, functions });
    }
    for (const name of Object.keys(capacities)) assertCounters(e, name, [0, 0, 0, 0]);
    snapshot('initial', []);
    checkedBatch(e, 1);
    assertCounters(e, 'numbers', [1, 0, 1, 1]);
    assertCounters(e, 'aligned', [1, 0, 3, 1]);
    snapshot('after_1_pair', ['numbers', 'aligned']);
    checkedBatch(e, 255);
    assertCounters(e, 'numbers', [1, 255, 256, 256]);
    assertCounters(e, 'aligned', [1, 255, 768, 256]);
    snapshot('after_256_pairs', ['numbers', 'aligned']);
    for (let i = 0; i < 256; i++) assert.equal(e.lazy(), 7);
    assertCounters(e, 'lazy', [0, 0, 0, 0]);
    snapshot('after_256_lazy_calls', ['numbers', 'aligned']);
    for (let i = 1; i <= 8; i++) {
        assert.throws(() => e.numbers(i, 99), WebAssembly.RuntimeError);
        assertCounters(e, 'numbers', [i, 256, 256 + i, 256]);
        snapshot(`after_trap_${i}`, ['aligned']);
        assert.equal(snapshots.at(-1).functions.numbers.quarantineBytes, String(i * 24));
    }
    assert.equal(e.numbers(100, 0), 105);
    assertCounters(e, 'numbers', [9, 256, 265, 257]);
    snapshot('after_recovery', ['numbers', 'aligned']);
    assert.equal(e.numbers(-100, 0), -95);
    assertCounters(e, 'numbers', [9, 257, 266, 258]);
    assertCounters(e, 'aligned', [1, 255, 768, 256]);
    assertCounters(e, 'lazy', [0, 0, 0, 0]);
    snapshot('after_reuse', ['numbers', 'aligned']);
    assert.equal(snapshots.at(-1).functions.numbers.quarantineBytes, '192');

    const markers = {};
    for (const name of ['baseline', 'prototype']) {
        const marked = fresh(name);
        const before = marked.__alloc(0) >>> 0;
        checkedBatch(marked, 256);
        const after = marked.__alloc(0) >>> 0;
        assert.equal(before % 8, 0);
        assert.equal(after % 8, 0);
        const deltaBytes = after - before - 8;
        assert.equal(deltaBytes, name === 'baseline' ? 256 * (24 + 48) : 72);
        markers[name] = { before, after, deltaBytes, markerCalls: 2,
            markerReservationBytes: 8, markerOverheadBytes: 16, callsPerFunction: 256 };
    }
    // Independently establish __alloc(0)'s minimum reservation.
    const allocator = fresh('baseline');
    const offsets = [allocator.__alloc(0) >>> 0, allocator.__alloc(0) >>> 0];
    assert.equal(offsets[1] - offsets[0], 8);
    return { callsPerFunction: 256, lazyCalls: 256, traps: 8, recoveryCalls: 2,
        snapshots, markers, minimumMarker: { offsets, reservationBytes: 8 } };
}

function capacity() {
    const observations = {};
    for (const name of ['baseline', 'prototype']) {
        const e = fresh(name);
        const initialPages = pages(e);
        checkedBatch(e, 4096);
        const finalPages = pages(e);
        assert.ok(finalPages >= initialPages);
        observations[name] = { initialPages, finalPages };
    }
    assert.ok(observations.baseline.finalPages > observations.prototype.finalPages);
    return { callsPerFunction: 4096, markerCalls: 0, observations };
}

function median(samples) {
    const sorted = [...samples].sort((a, b) => a - b);
    return (sorted[(sorted.length - 1) >> 1] + sorted[sorted.length >> 1]) / 2;
}

function timing() {
    const repetitions = [];
    for (let repetition = 0; repetition < 3; repetition++) {
        const order = repetition % 2 === 0 ? ['baseline', 'prototype'] : ['prototype', 'baseline'];
        const observations = {};
        for (const name of order) {
            const warm = fresh(name);
            checkedBatch(warm, 4); // Eight guest calls, outside every timed sample.
            const rawNsPerCall = [];
            for (let sample = 0; sample < 10; sample++) {
                const e = fresh(name);
                let checksum = 0;
                const start = process.hrtime.bigint();
                for (let i = 0; i < 256; i++) {
                    checksum += e.numbers(i, 0);
                    checksum += e.aligned(i);
                }
                const elapsed = process.hrtime.bigint() - start;
                assert.equal(checksum, 68864);
                rawNsPerCall.push(Number(elapsed) / 512);
            }
            observations[name] = { rawNsPerCall, medianNsPerCall: median(rawNsPerCall) };
        }
        repetitions.push({ repetition, order, observations });
    }
    const aggregate = {};
    for (const name of ['baseline', 'prototype']) {
        aggregate[name] = { medianNsPerCall: median(repetitions.flatMap(
            repetition => repetition.observations[name].rawNsPerCall)) };
    }
    return { unit: 'ns/guest call', warmupCallsPerVariantPerRepetition: 8,
        samplesPerVariantPerRepetition: 10, pairsPerSample: 256, callsPerSample: 512,
        expectedChecksum: 68864, markerCalls: 0, repetitions, aggregate };
}

function failure() {
    const e = fresh('instrumented');
    assert.equal(pages(e), 1, 'failure module initially has one page');
    for (const name of Object.keys(capacities)) assertCounters(e, name, [0, 0, 0, 0]);
    const before = Object.fromEntries(Object.keys(capacities).map(name => [name, counters(e, name)]));
    const markerBefore = e.__alloc(0) >>> 0;
    const fillStart = markerBefore + 8;
    const targetCursor = 65536 - 16;
    const fillBytes = targetCursor - fillStart;
    assert.ok(fillBytes >= 8 && fillBytes % 8 === 0, 'room for bounded prefill');
    assert.equal(e.__alloc(fillBytes) >>> 0, fillStart);
    new Uint8Array(e.memory.buffer, fillStart, fillBytes).fill(0xa5);
    const retained = Buffer.from(new Uint8Array(e.memory.buffer));
    const initialPages = pages(e);
    assert.throws(() => e.numbers(11, 0), WebAssembly.RuntimeError, '24-byte reservation fails');
    const finalPages = pages(e);
    assert.equal(finalPages, 1, 'capped memory did not grow');
    assert.deepEqual(Buffer.from(new Uint8Array(e.memory.buffer)), retained,
        'failed reservation preserves all linear memory bytes');
    const snapshots = {};
    for (const [name, capacity] of Object.entries(capacities)) {
        const c = counters(e, name);
        assert.deepEqual(c, before[name], `${name} failed acquisition has no counter effects`);
        assert.equal((c.provisions + c.reuses - c.retirements) * BigInt(capacity), 0n);
        snapshots[name] = {
            ...Object.fromEntries(fields.map(field => [field, c[field].toString()])),
            cachedCapacityBytes: 0, quarantineBytes: '0',
        };
    }
    const markerAfter = e.__alloc(0) >>> 0;
    assert.equal(markerAfter, targetCursor, 'failed reservation preserves allocator cursor');
    return { attemptedCapacityBytes: 24, remainingBytesBeforeFailure: 16, traps: 1,
        markerBefore, markerAfter, markerCalls: 2, markerReservationBytes: 8,
        markerOverheadBytes: 16, fillStart, fillBytes, targetCursor,
        remainingBytesAfterObservation: 8, snapshots,
        capacityObservation: { initialPages, finalPages } };
}

const verification = Object.fromEntries(Object.keys(modules).map(name => [name, verify(name)]));
const result = { accounting, capacity, timing, failure }[mode]();
const cpus = os.cpus();
const harnessBytes = fs.readFileSync(__filename);
console.log(JSON.stringify({
    schemaVersion: 1, mode, preset, utc: new Date().toISOString(),
    source: { bytes: Buffer.byteLength(source, 'utf8'), sha256: hash(source) },
    modules: Object.fromEntries(Object.entries(bytes).map(([name, contents]) =>
        [name, { bytes: contents.length, sha256: hash(contents) }])),
    harness: { bytes: harnessBytes.length, sha256: hash(harnessBytes) },
    environment: { node: process.version, v8: process.versions.v8,
        platform: process.platform, arch: process.arch, osRelease: os.release(),
        cpu: cpus[0]?.model ?? null, logicalCpus: cpus.length,
        totalMemoryBytes: os.totalmem(), nodeFlags: process.execArgv },
    capacities: Object.entries(capacities).map(([name, capacity]) => ({ name, capacity })),
    verification, result,
}));
