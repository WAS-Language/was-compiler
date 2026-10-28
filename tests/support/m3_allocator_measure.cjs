'use strict';
// Private M3 transport only. No ordinary array release or host ownership API.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const crypto = require('node:crypto');
const os = require('node:os');
const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const modules = Object.fromEntries(['prototype', 'instrumented'].map(name => {
  const bytes = Buffer.from(input[name]);
  assert(WebAssembly.validate(bytes));
  return [name, new WebAssembly.Module(bytes)];
}));
const fresh = (variant = 'prototype') => new WebAssembly.Instance(modules[variant]).exports;
const requests = [0, 9, 17, 33, 65];
const capacities = [8, 16, 32, 64, 128];

function workload(e) {
  const live = new Map();
  const physical = new Map();
  let acquisitions = 0n, requestedSum = 0n, capacitySum = 0n;
  let peakRequested = 0n, peakCapacity = 0n, peakBlocks = 0n;
  const origin = e.__m3_observe(0, 0);
  const totals = () => {
    let requested = 0n, capacity = 0n;
    for (const b of live.values()) {
      requested += BigInt(b.n);
      capacity += BigInt(b.capacity);
    }
    return { requested, capacity };
  };
  function verify() {
    const bytes = new Uint8Array(e.memory.buffer);
    const view = new DataView(e.memory.buffer);
    for (const b of live.values()) {
      assert.equal(e.__m3_observe(3, b.p), 1n);
      assert.equal(e.__m3_observe(4, b.p), b.id);
      assert.equal(e.__m3_observe(5, b.p), BigInt(b.capacity));
      assert.equal(e.__m3_observe(6, b.p), BigInt(b.n));
      for (let i = 0; i < b.n; i++) assert.equal(bytes[b.p + i], b.value);
      // Independent frozen record layout; payload initialization must not alter metadata.
      assert.equal(view.getBigUint64(b.p - 16, true), BigInt.asUintN(64, b.id));
      assert.equal(view.getBigUint64(b.p - 8, true), BigInt(b.n));
    }
    const extents = [...physical].sort((a, b) => a[0] - b[0]);
    for (let i = 1; i < extents.length; i++) {
      assert(extents[i - 1][0] + extents[i - 1][1] <= extents[i][0] - 32);
    }
  }
  function acquire(n, capacity) {
    const [raw, id] = e.__m3_acquire(BigInt(n));
    const p = raw >>> 0;
    assert(!live.has(p));
    const oldCapacity = physical.get(p);
    if (oldCapacity !== undefined) assert.equal(oldCapacity, capacity);
    physical.set(p, capacity);
    const value = Number((id % 251n) + 1n);
    new Uint8Array(e.memory.buffer).fill(value, p, p + n);
    const b = { p, id, n, capacity, value };
    live.set(p, b);
    acquisitions++;
    requestedSum += BigInt(n);
    capacitySum += BigInt(capacity);
    const t = totals();
    peakRequested = t.requested > peakRequested ? t.requested : peakRequested;
    peakCapacity = t.capacity > peakCapacity ? t.capacity : peakCapacity;
    peakBlocks = BigInt(live.size) > peakBlocks ? BigInt(live.size) : peakBlocks;
    verify();
    return b;
  }
  function release(b) {
    verify();
    e.__m3_release(b.p, b.id);
    live.delete(b.p);
    assert.equal(e.__m3_observe(3, b.p), 2n);
    verify();
  }
  function snapshot(label) {
    verify();
    const { requested, capacity } = totals();
    const population = e.__m3_observe(2, 0);
    assert.equal(population, BigInt(physical.size));
    let physicalCapacity = 0n;
    for (const c of physical.values()) physicalCapacity += BigInt(c);
    const backing = e.__m3_observe(0, 0) - origin;
    assert.equal(backing, physicalCapacity + 32n * population);
    assert.equal(e.__m3_observe(8, 0), acquisitions);
    return { label, cursor: e.__m3_observe(0, 0), backing, population,
      recordBytes: 32n * population, physicalCapacity,
      activeBlocks: BigInt(live.size), activeRequested: requested, activeCapacity: capacity,
      activeRoundingWaste: capacity - requested, freeCapacity: physicalCapacity - capacity,
      pages: e.__m3_observe(1, 0), acquisitions, cumulativeRequested: requestedSum,
      cumulativeCapacity: capacitySum, peakRequested, peakCapacity, peakBlocks };
  }
  return { acquire, release, snapshot };
}

function correctness(variant) {
  const e = fresh(variant);
  const retained = e.retained() >>> 0;
  const w = workload(e);
  const a = w.acquire(9, 16), b = w.acquire(17, 32);
  w.release(a);
  const renewed = w.acquire(9, 16);
  assert.equal(renewed.p, a.p);
  assert.notEqual(renewed.id, a.id);
  assert.throws(() => e.__m3_release(a.p, a.id), WebAssembly.RuntimeError);
  w.snapshot('after-stale-trap');
  w.release(b);
  w.release(renewed);
  const v = new DataView(e.memory.buffer);
  assert.equal(v.getUint32(retained + 4, true), 2);
  const p = v.getUint32(retained, true);
  assert.equal(v.getFloat64(p, true), 17);
  assert.equal(v.getFloat64(p + 8, true), 29);
}
// Correctness instances are discarded before any measurement or timer.
for (const variant of ['prototype', 'instrumented']) correctness(variant);

function accounting(variant) {
  const w = workload(fresh(variant));
  const snapshots = [w.snapshot('initial')];
  const persistent = requests.map((n, i) => w.acquire(n, capacities[i]));
  const batch = () => requests.flatMap((n, i) => Array.from({ length: 3 }, () => w.acquire(n, capacities[i])));
  const warmup = batch();
  snapshots.push(w.snapshot('warmup-full'));
  // Unordered release and reacquisition; no dependence on a preferred reuse address.
  for (const b of warmup.reverse()) w.release(b);
  const plateau = w.snapshot('warmup-released');
  snapshots.push(plateau);
  for (let cycle = 1; cycle <= 256; cycle++) {
    const blocks = batch();
    const full = w.snapshot(`full-${cycle}`);
    for (const field of ['cursor', 'backing', 'population', 'recordBytes', 'physicalCapacity', 'pages']) {
      assert.equal(full[field], plateau[field]);
    }
    for (const b of blocks.reverse()) w.release(b);
    const sample = w.snapshot(`cycle-${cycle}`);
    for (const field of ['cursor', 'backing', 'population', 'recordBytes', 'physicalCapacity', 'pages']) {
      assert.equal(sample[field], plateau[field]);
    }
    if ((cycle & (cycle - 1)) === 0) snapshots.push(sample);
  }
  for (const b of persistent) w.release(b);
  snapshots.push(w.snapshot('all-released'));
  assert.equal(plateau.backing, 1632n);
  assert.equal(plateau.recordBytes, 640n);
  assert.equal(plateau.freeCapacity, 744n);
  assert.equal(plateau.peakRequested, 496n);
  assert.equal(plateau.peakCapacity, 992n);

  const s = workload(fresh(variant));
  const small = Array.from({ length: 8 }, () => s.acquire(9, 16));
  for (const b of small) s.release(b);
  const stranded = [s.snapshot('128-free-in-class-16')];
  const other = s.acquire(17, 32);
  stranded.push(s.snapshot('class-32-must-provision'));
  assert.equal(stranded[1].backing - stranded[0].backing, 64n);
  assert.equal(stranded[1].freeCapacity, 128n);
  assert.equal(stranded[1].activeRoundingWaste, 15n);
  s.release(other);
  stranded.push(s.snapshot('all-free'));
  return { snapshots, stranded };
}

function registrySearch() {
  const samples = [];
  for (const population of [0, 1, 8, 32, 128, 512]) {
    const positions = population === 0 ? [] : [
      ['head', 1], ['middle', Math.floor(population / 2) + 1], ['tail', population],
      ['wrong-identity', Math.floor(population / 2) + 1], ['double-release', population],
    ];
    for (const [position, rank] of [...positions, ['absent', population]]) {
      const e = fresh('instrumented');
      const blocks = Array.from({ length: population }, () => e.__m3_acquire(9n));
      for (const [p] of blocks) new Uint8Array(e.memory.buffer).fill(0x57, p, p + 9);
      const block = position === 'absent' ? [0, 1n] : blocks[population - rank];
      if (position === 'double-release') e.__m3_release(...block);
      const identity = position === 'wrong-identity' ? block[1] + 1n : block[1];
      const before = e.__m3_registry_comparisons.value;
      const cursor = e.__m3_observe(0, 0);
      const snapshot = () => ({
        globals: [0, 1, 2, 8].map(selector => e.__m3_observe(selector, 0)).concat(
          Array.from({ length: 29 }, (_, index) => e.__m3_observe(7, index))),
        blocks: blocks.map(([p]) => ({
          record: Buffer.from(new Uint8Array(e.memory.buffer, p - 32, 32)),
          requested: Buffer.from(new Uint8Array(e.memory.buffer, p, 9)),
        })),
      });
      const memory = snapshot();
      const traps = ['absent', 'wrong-identity', 'double-release'].includes(position);
      if (traps) assert.throws(() => e.__m3_release(block[0], identity), WebAssembly.RuntimeError);
      else e.__m3_release(block[0], identity);
      const comparisons = e.__m3_registry_comparisons.value - before;
      assert.equal(comparisons, BigInt(rank));
      assert.equal(e.__m3_observe(0, 0), cursor);
      assert.equal(e.__m3_observe(2, 0), BigInt(population));
      if (traps) assert.deepEqual(snapshot(), memory);
      for (const [p, id] of blocks) {
        assert.deepEqual(Array.from(new Uint8Array(e.memory.buffer, p, 9)), Array(9).fill(0x57));
        const view = new DataView(e.memory.buffer);
        const released = p === block[0] && (!traps || position === 'double-release');
        assert.equal(view.getUint32(p - 20, true), released ? 0 : 1);
        assert.equal(view.getBigUint64(p - 16, true), BigInt.asUintN(64, id));
        assert.equal(view.getBigUint64(p - 8, true), 9n);
      }
      // Observation lookups share FIND and are counted too; isolate them after the release delta.
      if (population > 0) {
        const observationBefore = e.__m3_registry_comparisons.value;
        e.__m3_observe(3, blocks[0][0]);
        assert.equal(e.__m3_registry_comparisons.value - observationBefore, BigInt(population));
      }
      samples.push({ population, position, rank, comparisons, traps });
    }
  }
  return { samples };
}

function guestTiming() {
  const samples = [];
  const pairs = 4096;
  const warmupPairs = 2048;
  const cases = [1, 32, 128, 512].flatMap(population =>
    [...new Set([1, Math.floor(population / 2) + 1, population])].map(rank => ({ population, rank })));
  for (let repetition = 0; repetition < 3; repetition++) {
    const ordered = repetition % 2 === 0 ? cases : [...cases].reverse();
    for (const { population, rank } of ordered) {
      for (let sample = 0; sample < 5; sample++) {
        const e = fresh();
        assert.equal(e.__m3_registry_comparisons, undefined);
        const blocks = Array.from({ length: population }, () => e.__m3_acquire(9n));
        for (const [p] of blocks) new Uint8Array(e.memory.buffer).fill(0x36, p, p + 9);
        const selected = blocks[population - rank];
        let [p, identity] = selected;
        const release = e.__m3_release, acquire = e.__m3_acquire;
        for (let i = 0; i < warmupPairs; i++) {
          release(p, identity);
          [p, identity] = acquire(9n);
        }
        assert.equal(p, selected[0]);
        const cursor = e.__m3_observe(0, 0);
        const pages = e.__m3_observe(1, 0);
        const start = process.hrtime.bigint();
        for (let i = 0; i < pairs; i++) {
          release(p, identity);
          [p, identity] = acquire(9n);
        }
        const elapsedNs = process.hrtime.bigint() - start;
        assert.equal(p, selected[0]);
        assert.equal(identity, BigInt(population + warmupPairs + pairs));
        assert.equal(e.__m3_observe(0, 0), cursor);
        assert.equal(e.__m3_observe(1, 0), pages);
        assert.equal(e.__m3_observe(2, 0), BigInt(population));
        // Initialize the newly acquired requested extent before any post-timing read.
        new Uint8Array(e.memory.buffer).fill(0x36, p, p + 9);
        for (const [address] of blocks) {
          assert.deepEqual(Array.from(new Uint8Array(e.memory.buffer, address, 9)), Array(9).fill(0x36));
        }
        samples.push({ repetition, sample, population, rank, pairs, warmupPairs,
          elapsedNs, nsPerPair: Number(elapsedNs) / pairs });
      }
    }
  }
  return { samples, boundary: 'JS loop + two JS/Wasm calls + release search + exact-class reuse + multivalue transport per pair; no payload initialization in timer' };
}

assert(['accounting', 'search', 'timing'].includes(input.mode));
let results;
if (input.mode === 'accounting') {
  results = accounting('prototype');
  assert.deepEqual(accounting('instrumented'), results);
  results.instrumentedParity = true;
} else {
  results = input.mode === 'search' ? registrySearch() : guestTiming();
}
console.log(JSON.stringify({ schema: 1, mode: input.mode, recordedUtc: new Date().toISOString(),
  environment: { node: process.version, v8: process.versions.v8, platform: process.platform,
    release: os.release(), arch: process.arch, cpu: os.cpus()[0].model },
  source: { sha256: hash(input.source), bytes: Buffer.byteLength(input.source) },
  harness: { sha256: hash(fs.readFileSync(__filename)) },
  modules: Object.fromEntries(['prototype', 'instrumented'].map(name => [name,
    { sha256: hash(Buffer.from(input[name])), bytes: input[name].length }])),
  workload: { requests, capacities, persistentPerClass: 1, transientPerClass: 3, cycles: 256 },
  results }, (_, value) => typeof value === 'bigint' ? value.toString() : value, 2));
