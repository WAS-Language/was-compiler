'use strict';
// #71 private evidence, not a runtime adapter or public management interface.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const os = require('node:os');
const [mode, directory] = process.argv.slice(2);
assert(['accounting', 'timing'].includes(mode), 'expected accounting|timing ABS_DIRECTORY');
assert(directory && path.isAbsolute(directory), 'module directory must be absolute');
assert.equal(process.argv.length, 4);
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const source = fs.readFileSync(path.join(__dirname, 'm4_resizable_corpus.was'));
const moduleBytes = Object.fromEntries(['ordinary', 'observed'].map(name =>
  [name, fs.readFileSync(path.join(directory, `${name}.wasm`))]));
const modules = Object.fromEntries(Object.entries(moduleBytes).map(([name, bytes]) => {
  assert(WebAssembly.validate(bytes));
  return [name, new WebAssembly.Module(bytes)];
}));
const observations = ['cursor', 'last_identity', 'allocator_registry', 'array_registry',
  'allocator_comparisons', 'authority_comparisons'];
const fresh = name => {
  const e = new WebAssembly.Instance(modules[name]).exports;
  for (const observation of observations) {
    if (name === 'ordinary') assert.equal(e[`__m4_${observation}`], undefined);
    else assert(e[`__m4_${observation}`] instanceof WebAssembly.Global);
  }
  return e;
};
const types = ['number', 'bool', 'string'];
const widths = { number: 8, bool: 1, string: 4 };
const small = { number: [0.25, -1.75, 4.25], bool: [true, false, true],
  string: ['even', 'odd-λ', 'even'] };
const item = (type, index) => type === 'number' ? index * 3 - 7.25 :
  type === 'bool' ? index % 2 === 0 : index % 2 === 0 ? 'even' : 'odd-λ';
const capacityFor = bytes => {
  let capacity = 8;
  while (capacity < bytes) capacity *= 2;
  return capacity;
};
const global = (e, name) => e[`__m4_${name}`].value;
const counters = e => ({ allocator: global(e, 'allocator_comparisons'),
  authority: global(e, 'authority_comparisons') });
const subtract = (a, b) => Object.fromEntries(Object.keys(a).map(k => [k, a[k] - b[k]]));
function view(e) { return new DataView(e.memory.buffer); }
function range(e, address, bytes, alignment = 1) {
  assert(Number.isSafeInteger(address) && address >= 8 && address % alignment === 0);
  assert(Number.isSafeInteger(bytes) && bytes >= 0 && address + bytes <= e.memory.buffer.byteLength);
}
const u32 = (e, p) => { range(e, p, 4, 4); return view(e).getUint32(p, true); };
const u64 = (e, p) => {
  range(e, p, 8, 8);
  const value = view(e).getBigUint64(p, true);
  assert(value <= BigInt(Number.MAX_SAFE_INTEGER), 'bounded fixture observation must convert exactly');
  return Number(value);
};
function string(e, handle) {
  range(e, handle, 8, 8);
  const p = u32(e, handle), length = u32(e, handle + 4);
  range(e, p, length);
  return new TextDecoder('utf-8', { fatal: true }).decode(new Uint8Array(e.memory.buffer, p, length));
}
function contents(e, handle, type) {
  range(e, handle, 8, 8);
  const p = u32(e, handle), length = u32(e, handle + 4), width = widths[type];
  range(e, p, length * width, width);
  // Never decode spare capacity, free payloads, or stale post-remove slots.
  return Array.from({ length }, (_, i) => {
    const address = p + i * width;
    if (type === 'number') return view(e).getFloat64(address, true);
    if (type === 'string') return string(e, u32(e, address));
    const value = view(e).getUint8(address);
    assert(value === 0 || value === 1);
    return value === 1;
  });
}
function bytes(e, start, length) {
  range(e, start, length);
  return Buffer.from(new Uint8Array(e.memory.buffer, start, length));
}
function disjoint(extents) {
  extents.sort((a, b) => a.start - b.start);
  for (let i = 1; i < extents.length; i++) {
    assert(extents[i - 1].end <= extents[i].start,
      `overlap: ${JSON.stringify(extents.slice(i - 1, i + 1))}`);
  }
}

function accountingSession(label) {
  const ordinary = fresh('ordinary'), observed = fresh('observed');
  const origin = global(observed, 'cursor') >>> 0;
  const arrays = [], sequence = [], physical = new Map();
  let acquisitions = 0, cumulativeAcquiredCapacityBytes = 0, relocations = 0;
  let copiedLiveBytes = 0, reusedAcquisitions = 0;
  const peak = {};
  function inspect() {
    const before = counters(observed);
    const cursor = global(observed, 'cursor') >>> 0;
    assert.equal(ordinary.memory.buffer.byteLength, observed.memory.buffer.byteLength);
    const allocator = new Map(), authorities = new Map(), extents = [];
    // Frozen independent layouts: allocator 32 bytes; descriptor + authority 48.
    let p = global(observed, 'allocator_registry') >>> 0;
    while (p !== 0) {
      assert(!allocator.has(p), 'allocator registry cycle');
      range(observed, p, 32, 8);
      const cls = u32(observed, p + 8), active = u32(observed, p + 12);
      assert(cls < 29 && (active === 0 || active === 1));
      const capacity = 8 * 2 ** cls;
      const record = { address: p, next: u32(observed, p), freeNext: u32(observed, p + 4),
        capacity, active, identity: u64(observed, p + 16), requested: u64(observed, p + 24) };
      assert.equal(record.requested, capacity, 'M4 acquires full capacity');
      assert(record.identity > 0 && record.identity <= acquisitions);
      range(observed, p, 32 + capacity, 8);
      assert(p + 32 + capacity <= cursor);
      assert.deepEqual(bytes(ordinary, p, 32), bytes(observed, p, 32));
      allocator.set(p, record);
      extents.push({ start: p, end: p + 32 + capacity, kind: 'allocator record + payload' });
      if (physical.has(p)) assert.equal(physical.get(p), capacity);
      physical.set(p, capacity);
      p = record.next;
    }
    assert.equal(allocator.size, physical.size, 'registry never drops physical records');
    p = global(observed, 'array_registry') >>> 0;
    while (p !== 0) {
      assert(!authorities.has(p), 'authority registry cycle');
      range(observed, p - 8, 48, 8);
      const handle = u32(observed, p + 4), kind = u32(observed, p + 8);
      assert.equal(handle, p - 8);
      assert(kind < 3);
      assert.equal(u32(observed, p + 28), 0);
      const record = { handle, kind, length: u32(observed, p + 12),
        capacity: u64(observed, p + 16), payload: u32(observed, p + 24),
        identity: u64(observed, p + 32) };
      assert.equal(u32(observed, handle), record.payload);
      assert.equal(u32(observed, handle + 4), record.length);
      assert(record.length * widths[types[kind]] <= record.capacity);
      const allocation = allocator.get(record.payload - 32);
      assert(allocation && allocation.active === 1);
      assert.equal(allocation.capacity, record.capacity);
      assert.equal(allocation.identity, record.identity);
      assert.deepEqual(bytes(ordinary, handle, 48), bytes(observed, handle, 48));
      authorities.set(p, record);
      extents.push({ start: handle, end: handle + 48, kind: 'descriptor + authority' });
      p = u32(observed, p);
    }
    disjoint(extents);
    assert.equal(authorities.size, arrays.length);
    const owned = new Set();
    let liveBytes = 0, liveElements = 0, activeCapacityBytes = 0, capacityElements = 0;
    for (const a of arrays) {
      const r = authorities.get(a.handle + 8);
      assert(r);
      assert.equal(r.kind, types.indexOf(a.type));
      assert.equal(r.capacity, a.capacity);
      assert.equal(r.length, a.values.length);
      assert(!owned.has(r.payload));
      owned.add(r.payload);
      for (const e of [ordinary, observed]) assert.deepEqual(contents(e, a.handle, a.type), a.values);
      liveElements += r.length;
      liveBytes += r.length * widths[a.type];
      activeCapacityBytes += r.capacity;
      capacityElements += r.capacity / widths[a.type];
    }
    const free = [...allocator.values()].filter(r => !r.active);
    const active = [...allocator.values()].filter(r => r.active);
    assert.equal(active.length, arrays.length);
    for (const r of active) { assert(owned.has(r.address + 32)); assert.equal(r.freeNext, 0); }
    // Heads are deliberately not exported. Reconstruct each nonempty free chain
    // from its unique zero-incoming record and require every free record exactly once.
    const freeByClass = {};
    for (const capacity of new Set(free.map(r => r.capacity))) {
      const group = free.filter(r => r.capacity === capacity);
      const incoming = new Set(group.map(r => r.freeNext).filter(Boolean));
      const heads = group.filter(r => !incoming.has(r.address));
      assert.equal(heads.length, 1);
      const seen = new Set();
      let next = heads[0].address;
      while (next) {
        assert(!seen.has(next));
        seen.add(next);
        const r = allocator.get(next);
        assert(r && !r.active && r.capacity === capacity);
        next = r.freeNext;
      }
      assert.equal(seen.size, group.length);
      freeByClass[capacity] = { records: group.length, bytes: capacity * group.length };
    }
    const freeCapacityBytes = free.reduce((sum, r) => sum + r.capacity, 0);
    const allocatorRecordBytes = allocator.size * 32, authorityObjectBytes = arrays.length * 48;
    const physicalCapacityBytes = activeCapacityBytes + freeCapacityBytes;
    const bumpGrowthBytes = cursor - origin;
    const otherBumpBytes = bumpGrowthBytes - physicalCapacityBytes - allocatorRecordBytes - authorityObjectBytes;
    assert(otherBumpBytes >= 0, 'other bump storage includes retained constructor source literals/alignment');
    assert.equal(Number(global(observed, 'last_identity')), acquisitions);
    assert.deepEqual(counters(observed), before, 'host observation must not execute FIND/LOOKUP');
    const current = { liveElements, liveBytes, activeCapacityBytes, capacityElements,
      spareCapacityBytes: activeCapacityBytes - liveBytes, activeAllocatorRecords: active.length,
      freeAllocatorRecords: free.length, allocatorRecords: allocator.size,
      authorityDescriptors: arrays.length, allocatorRecordBytes, authorityObjectBytes,
      physicalCapacityBytes, freeCapacityBytes, cursor, bumpGrowthBytes, otherBumpBytes,
      pages: observed.memory.buffer.byteLength / 65536 };
    for (const [key, value] of Object.entries(current)) peak[key] = Math.max(peak[key] || 0, value);
    return { current, freeByClass, allocator, authorities };
  }
  let state = inspect();
  function operation(op, a, argument) {
    const beforeCounters = counters(observed), previous = state;
    const old = a && previous.authorities.get(a.handle + 8);
    let result;
    if (op === 'new' || op === 'small') {
      const type = a;
      const handles = [ordinary, observed].map(e => e[`${op}_${type}`]() >>> 0);
      assert.equal(handles[0], handles[1]);
      const values = op === 'small' ? [...small[type]] : [];
      a = { id: arrays.length, type, handle: handles[0], values,
        capacity: capacityFor(values.length * widths[type]), appendRelocations: 0, appendCopiedBytes: 0 };
      arrays.push(a);
      acquisitions++;
      cumulativeAcquiredCapacityBytes += a.capacity;
      const payload = u32(observed, a.handle);
      if (previous.allocator.has(payload - 32)) reusedAcquisitions++;
    } else {
      const returns = [ordinary, observed].map(e => e[`${op}_${a.type}`](a.handle, argument));
      if (op === 'push') a.values.push(item(a.type, argument));
      else if (op === 'remove' || op === 'pop') {
        const index = op === 'pop' ? a.values.length - 1 : argument;
        result = a.values.splice(index, 1)[0];
        for (let i = 0; i < returns.length; i++) {
          const decoded = a.type === 'string' ? string([ordinary, observed][i], returns[i] >>> 0) :
            a.type === 'bool' ? returns[i] === 1 : returns[i];
          assert.equal(decoded, result);
          if (a.type === 'bool') assert(returns[i] === 0 || returns[i] === 1);
        }
      }
      const required = (a.values.length + (op === 'reserve' ? argument : 0)) * widths[a.type];
      const expectedCapacity = Math.max(a.capacity, capacityFor(required));
      const payload = u32(observed, a.handle);
      if (expectedCapacity !== a.capacity) {
        assert.notEqual(payload, old.payload);
        acquisitions++;
        cumulativeAcquiredCapacityBytes += expectedCapacity;
        relocations++;
        copiedLiveBytes += old.length * widths[a.type];
        if (op === 'push') {
          assert.equal(expectedCapacity, a.capacity * 2, 'append grows geometrically');
          a.appendRelocations++;
          a.appendCopiedBytes += old.length * widths[a.type];
        }
        if (previous.allocator.has(payload - 32)) {
          assert.equal(previous.allocator.get(payload - 32).active, 0);
          assert.equal(previous.allocator.get(payload - 32).capacity, expectedCapacity);
          reusedAcquisitions++;
        }
      } else {
        assert.equal(payload, old.payload);
        assert.equal(u64(observed, a.handle + 8 + 32), old.identity);
      }
      a.capacity = expectedCapacity;
    }
    const comparisons = subtract(counters(observed), beforeCounters);
    state = inspect();
    sequence.push({ step: sequence.length, op, array: a.id, type: a.type,
      ...(argument === undefined ? {} : { argument }), ...(result === undefined ? {} : { result }),
      comparisons, ...state.current });
    return a;
  }
  function report() {
    return { label, initialCursor: origin, sequence, current: state.current,
      operationBoundaryPeak: peak, freeByClass: state.freeByClass,
      cumulative: { acquisitions, acquiredCapacityBytes: cumulativeAcquiredCapacityBytes,
        relocations, inferredCopiedLiveBytes: copiedLiveBytes, reusedAcquisitions },
      arrays: arrays.map(a => ({ ...a, finalValues: a.values, values: undefined })),
      comparisons: counters(observed), ordinaryObservedParity: true };
  }
  return { operation, report, get state() { return state; } };
}
function accounting() {
  const main = accountingSession('mixed-types');
  for (const type of types) {
    const a = main.operation('new', type);
    const b = main.operation('small', type);
    // Isolated append prefix: derive exact geometric count and sum independently.
    const count = 129, width = widths[type];
    for (let i = 0; i < count; i++) main.operation('push', a, i);
    const finalCapacity = capacityFor(count * width);
    assert.equal(a.appendRelocations, Math.log2(finalCapacity / 8));
    assert.equal(a.appendCopiedBytes, finalCapacity - 8);
    assert(a.appendCopiedBytes < 2 * count * width);
    for (let i = 0; i < 65; i++) main.operation('push', b, i + 200);
    main.operation('reserve', b, 257);
    main.operation('reserve', b, 0);
    main.operation('reserve', b, 1);
    // Spare append, then ordered front/middle/last removal and pop, capacity retained.
    main.operation('push', b, 999);
    main.operation('remove', b, 0);
    main.operation('remove', b, Math.floor(b.values.length / 2));
    main.operation('remove', b, b.values.length - 1);
    main.operation('pop', b);
    // Repeated fill/shrink transitions; all earlier arrays remain live and verified.
    for (let cycle = 0; cycle < 3; cycle++) {
      const c = main.operation('new', type);
      for (let i = 0; i < 33; i++) main.operation('push', c, cycle * 100 + i);
      for (let i = 0; i < 17; i++) main.operation('pop', c);
      main.operation('reserve', c, 65);
      for (let i = 0; i < 17; i++) main.operation('push', c, 500 + i);
    }
  }
  const mixed = main.report();
  assert(mixed.cumulative.reusedAcquisitions > 0);
  const stranded = [];
  for (const type of types) {
    const s = accountingSession(`exact-class-stranding-${type}`), width = widths[type];
    const arrays = Array.from({ length: 10 }, () => s.operation('new', type));
    for (const a of arrays) s.operation('reserve', a, 64 / width);
    for (const a of arrays) s.operation('reserve', a, 128 / width);
    const before = { ...s.state.current };
    assert.equal(s.state.freeByClass[64].bytes, 640);
    assert(before.freeCapacityBytes >= 512);
    assert.equal(s.state.freeByClass[512], undefined);
    s.operation('reserve', arrays[0], 512 / width);
    const after = s.state.current;
    assert.equal(after.allocatorRecords, before.allocatorRecords + 1);
    assert.equal(after.bumpGrowthBytes - before.bumpGrowthBytes, 512 + 32);
    assert.equal(after.freeCapacityBytes, before.freeCapacityBytes + 128);
    stranded.push({ ...s.report(), witness: { freeBefore: before.freeCapacityBytes,
      unavailableExactClassBytes: 512, freshBackingBytes: 544 } });
  }
  return { mixed, stranded };
}

// Timed cases use only ordinary instances. Setups and independent expected values
// are outside the timer; the JS loop and JS/Wasm crossings are intentionally inside.
const timingCases = types.flatMap(type => ['append', 'spare-push-pop', 'ordered-remove-push',
  'reserve-noop-oldest-32'].map(kind => ({ type, kind,
    iterations: kind === 'append' ? 512 : kind === 'ordered-remove-push' ? 2048 : 4096 })));
function timedCase(spec, measure) {
  const { type, kind, iterations } = spec, e = fresh('ordinary');
  const handle = e[`new_${type}`]() >>> 0;
  const push = e[`push_${type}`], pop = e[`pop_${type}`];
  const reserve = e[`reserve_${type}`], remove = e[`remove_${type}`];
  let expected = [];
  const retained = [];
  if (kind !== 'append') reserve(handle, 128);
  if (kind === 'ordered-remove-push') {
    for (let i = 0; i < 64; i++) push(handle, i);
    expected = Array.from({ length: 64 }, (_, i) => item(type, i));
  }
  if (kind === 'reserve-noop-oldest-32') {
    for (let i = 0; i < 31; i++) retained.push(e[`small_${type}`]() >>> 0);
  }
  // Build the oracle before the timed region, with no guest execution.
  if (kind === 'append') expected = Array.from({ length: iterations }, (_, i) => item(type, i));
  if (kind === 'ordered-remove-push') {
    for (let i = 0; i < iterations; i++) {
      expected.splice(i % 3 === 0 ? 0 : i % 3 === 1 ? 32 : 63, 1);
      expected.push(item(type, i + 1000));
    }
  }
  const start = measure ? process.hrtime.bigint() : 0n;
  if (kind === 'append') {
    for (let i = 0; i < iterations; i++) push(handle, i);
  } else if (kind === 'spare-push-pop') {
    for (let i = 0; i < iterations; i++) { push(handle, i); pop(handle); }
  } else if (kind === 'ordered-remove-push') {
    for (let i = 0; i < iterations; i++) {
      remove(handle, i % 3 === 0 ? 0 : i % 3 === 1 ? 32 : 63);
      push(handle, i + 1000);
    }
  } else {
    for (let i = 0; i < iterations; i++) reserve(handle, i % 2);
  }
  const elapsedNs = measure ? process.hrtime.bigint() - start : 0n;
  assert.deepEqual(contents(e, handle, type), expected);
  for (const h of retained) assert.deepEqual(contents(e, h, type), small[type]);
  return { elapsedNs, nsPerIteration: Number(elapsedNs) / iterations,
    finalLength: expected.length, finalPages: e.memory.buffer.byteLength / 65536 };
}
function timing() {
  const samples = [];
  // Warmup uses different instances and the same finite operation sequence.
  for (const spec of timingCases) timedCase(spec, false);
  for (let repetition = 0; repetition < 3; repetition++) {
    const ordered = repetition % 2 ? [...timingCases].reverse() : timingCases;
    for (const spec of ordered) {
      for (let sample = 0; sample < 5; sample++) {
        samples.push({ repetition, sample, ...spec, ...timedCase(spec, true) });
      }
    }
  }
  return { samples, repetitions: 3, samplesPerCasePerRepetition: 5,
    warmup: 'one full untimed case per type/kind on separate discarded ordinary instances',
    boundary: 'hrtime around JS operation loop, JS/Wasm calls, guest parameter validation, item selection, authority/allocator searches, mutation, allocation/copy/growth if needed, and returned scalar transport (ignored by timed loop); excludes compilation, instantiation, setup, oracle construction, decoding, assertions, and hashing',
    operations: { append: '512 push calls from empty; includes geometric transitions',
      'spare-push-pop': '4096 push/pop pairs from empty with 128 elements reserved',
      'ordered-remove-push': '2048 remove/push pairs at constant length 64; remove indices cycle 0,32,63',
      'reserve-noop-oldest-32': '4096 reserve calls alternating additional 0/1 on oldest of 32 arrays; selected empty array has 128 elements reserved' } };
}
const results = mode === 'accounting' ? accounting() : timing();
console.log(JSON.stringify({ schema: 1, mode, recordedUtc: new Date().toISOString(),
  environment: { node: process.version, v8: process.versions.v8, platform: process.platform,
    release: os.release(), arch: process.arch, cpu: os.cpus()[0]?.model, logicalCpus: os.cpus().length },
  source: { sha256: hash(source), bytes: source.length },
  harness: { sha256: hash(fs.readFileSync(__filename)) },
  modules: Object.fromEntries(Object.entries(moduleBytes).map(([name, b]) =>
    [name, { sha256: hash(b), bytes: b.length }])),
  definitions: {
    sequence: 'accounting records every guest call in order; array IDs are constructor order per fresh session; push arguments are deterministic item indices',
    comparisons: 'observed global deltas: one increment per non-null record visited by allocator FIND or per-type authority LOOKUP; includes all nested searches in the operation, not just one public helper; host loads execute no searches',
    capacity: 'full acquired power-of-two payload bytes, minimum 8; capacityElements sums per-array capacity divided by its element width; free capacity is untyped bytes',
    cumulative: 'sum over successful constructor/relocation acquisitions including reuse; copied live bytes inferred from verified relocations and pre-operation length times width, not measured memory-load/store counts; constructor copy bytes excluded',
    peaks: 'maxima at completed guest-operation boundaries only, not transient acquire-before-release high-water marks',
    backing: 'cursor minus fresh-instance initial cursor; includes allocator records, payloads, retained 48-byte authority objects, and other bump storage (constructor fixed literals/alignment); pages are total committed Wasm memory pages',
    parity: 'fresh ordinary and observed instances execute identical sequence; equal handles, public contents, returned scalars, pages and every initialized allocator/authority record; ordinary registry roots/cursor inferred from paired observed addresses, not independently exported',
  },
  limitations: [
    'Private fixture layouts, not a public introspection or management ABI; trusted-host loads only.',
    'No spare/free payload bytes are decoded or compared. This harness does not prove the absence of guest spare-byte loads; decoded lowering tests own that claim.',
    'Disjointness covers full allocator record/payload and retained descriptor/authority extents, not a census of fixed literals/static string storage.',
    'Peaks omit intra-operation transient double-capacity residency; no general reclamation, unreachable-array collection, trap rollback, or heap plateau claim.',
    'Free-chain reconstruction verifies links and membership without exported class heads; it cannot independently verify hidden head globals.',
    'Timing is ordinary-module host-driven execution, includes JS overhead and Wasm tiering/GC/system noise; no native execution or compiler-time inference, statistical significance, or release acceptance claim.',
  ], results }, (_, value) => typeof value === 'bigint' ? value.toString() : value, 2));
