'use strict';
// #79: bounded deterministic evidence, not a benchmark or public runtime adapter.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const directory = process.argv[2];
assert.equal(process.argv.length, 3);
const variants = ['ordinary', 'observed'];
const modules = variants.map(name => new WebAssembly.Module(
  fs.readFileSync(path.join(directory, `${name}.wasm`))));
const sizes = [0, 1, 2, 3, 4, 5, 8, 9, 16, 17, 32, 33, 64, 65, 129, 257, 1025];
const names = ['allocations', 'requested_bytes', 'reserved_bytes',
  'string_payload_allocations', 'string_payload_bytes', 'string_descriptor_bytes',
  'string_copied_bytes', 'array_acquired_bytes', 'array_released_bytes',
  'array_active_bytes', 'array_peak_active_bytes', 'array_copied_bytes', 'array_relocations'];
const decoder = new TextDecoder('utf-8', { fatal: true });
const byteLength = text => Buffer.byteLength(text, 'utf8');
const align = bytes => Math.max(8, Math.ceil(bytes / 8) * 8);
const view = e => new DataView(e.memory.buffer);
const u32 = (e, p) => view(e).getUint32(p, true);
function descriptor(e, h) {
  assert(h >= 8 && h % 8 === 0);
  const pointer = u32(e, h), length = u32(e, h + 4);
  assert(pointer >= 8 && pointer + length <= e.memory.buffer.byteLength);
  return { pointer, length };
}
function text(e, h) {
  const { pointer, length } = descriptor(e, h);
  return decoder.decode(new Uint8Array(e.memory.buffer, pointer, length));
}
function counters(e) {
  return Object.fromEntries(names.map(name => {
    const value = BigInt.asUintN(64, e[`__m5_${name}`].value);
    // All cases have <=1025 fragments of <=5 bytes. Baselines copy <3M bytes;
    // growth adds <1M bytes. A deliberately loose 64MiB bound covers cumulative
    // counters in every fresh session; no seeded/wrapped value is measurement.
    assert(value < 67108864n, `${name} exceeds independently bounded workload`);
    return [name, value];
  }));
}
const delta = (after, before) => Object.fromEntries(names.map(n => [n, after[n] - before[n]]));
function paired() {
  const exports = modules.map(m => new WebAssembly.Instance(m).exports);
  const observed = exports[1];
  for (const name of names) assert.equal(exports[0][`__m5_${name}`], undefined);
  assert.equal(exports[0].__m5_allocator_registry, undefined);
  const origin = observed.__m5_cursor.value >>> 0;
  const same = () => assert.deepEqual(new Uint8Array(exports[0].memory.buffer),
    new Uint8Array(observed.memory.buffer));
  function call(name, ...args) {
    const result = exports.map(e => e[name](...args));
    assert.equal(result[0], result[1]);
    same();
    counters(observed);
    assert.equal(BigInt((observed.__m5_cursor.value >>> 0) - origin), counters(observed).reserved_bytes);
    return result[0];
  }
  function trap(name, ...args) {
    for (const e of exports) assert.throws(() => e[name](...args), WebAssembly.RuntimeError);
    same();
  }
  return { exports, observed, origin, call, trap, same };
}
function assertStrings(e, expected) {
  const c = counters(e);
  for (const [name, value] of Object.entries(expected)) assert.equal(c[name], BigInt(value), name);
}
// Traverse physical records, not acquisition occurrences: free storage is retained
// once, and reuse must never manufacture another physical extent.
function registry(session) {
  const e = session.observed, records = [], seen = new Set();
  let p = e.__m5_allocator_registry.value >>> 0;
  while (p) {
    assert(p >= session.origin && p % 8 === 0 && !seen.has(p));
    seen.add(p);
    const cls = u32(e, p + 8), active = u32(e, p + 12);
    assert(cls < 29 && (active === 0 || active === 1));
    const capacity = 8 * 2 ** cls;
    assert.equal(view(e).getBigUint64(p + 24, true), BigInt(capacity));
    assert(p + 32 + capacity <= (e.__m5_cursor.value >>> 0));
    records.push({ address: p, capacity, active, end: p + 32 + capacity });
    p = u32(e, p);
  }
  records.sort((a, b) => a.address - b.address);
  for (let i = 1; i < records.length; i++) assert(records[i - 1].end <= records[i].address);
  const active = records.filter(r => r.active).reduce((n, r) => n + r.capacity, 0);
  assert.equal(BigInt(active), counters(e).array_active_bytes);
  return { records, active, physicalCapacity: records.reduce((n, r) => n + r.capacity, 0),
    freeCapacity: records.filter(r => !r.active).reduce((n, r) => n + r.capacity, 0),
    recordBytes: records.length * 32 };
}
function inputs(session) {
  const e = session.observed;
  const definitions = [['empty', ''], ['fragment', 'a'], ['multibyte', 'λ猫'],
    ['separator', '界'], ['bar', '|'], ['b', 'b']];
  const handles = {}, ranges = [];
  for (const [name, expected] of definitions) {
    const h = session.call(name);
    assert.equal(text(e, h), expected);
    const d = descriptor(e, h);
    assert(h < session.origin && d.pointer + d.length <= session.origin);
    handles[expected] = h;
    ranges.push([h, h + 8], [d.pointer, d.pointer + d.length]);
  }
  // Count the union of byte ranges, not fragment occurrences or duplicate handles.
  ranges.sort((a, b) => a[0] - b[0]);
  const unique = [];
  for (const [start, end] of ranges) {
    if (end === start) continue;
    const previous = unique.at(-1);
    if (previous && start <= previous[1]) previous[1] = Math.max(previous[1], end);
    else unique.push([start, end]);
  }
  return { handles, staticUniqueBytes: unique.reduce((n, [a, b]) => n + b - a, 0), ranges: unique };
}
function arrayContents(e, h) {
  const p = u32(e, h), n = u32(e, h + 4);
  return Array.from({ length: n }, (_, i) => text(e, u32(e, p + i * 4)));
}
function model(n) {
  let capacity = 8, acquired = 8, released = 0, copied = 0, relocations = 0, peak = 8;
  const capacities = [8];
  for (let i = 0; i < n; i++) {
    if ((i + 1) * 4 > capacity) {
      const next = capacity * 2;
      peak = Math.max(peak, capacity + next);
      acquired += next; released += capacity; copied += i * 4; relocations++;
      capacity = next; capacities.push(next);
    }
  }
  return { capacity, acquired, released, copied, relocations, peak, capacities };
}
const rows = [];
function construction(n, pattern, separator) {
  const s = paired(), e = s.observed, input = inputs(s);
  const fragments = Array.from({ length: n }, (_, i) => pattern[i % pattern.length]);
  const expected = fragments.join(separator), B = byteLength(expected);
  const parts = s.call('fragments');
  let completedCallPeak = 8;
  for (const fragment of fragments) {
    s.call('append', parts, input.handles[fragment]);
    completedCallPeak = Math.max(completedCallPeak, registry(s).active);
  }
  assert.deepEqual(arrayContents(e, parts), fragments);
  assertStrings(e, { string_payload_allocations: 0, string_payload_bytes: 0,
    string_descriptor_bytes: 0, string_copied_bytes: 0 });
  const m = model(n), r = registry(s);
  assert.deepEqual(r.records.map(r => r.capacity), m.capacities);
  assert.equal(r.active, m.capacity);
  assert.equal(r.freeCapacity, m.released);
  const activeRecord = r.records.filter(record => record.active);
  assert.equal(activeRecord.length, 1);
  assert.equal(u32(e, parts), activeRecord[0].address + 32);
  assert.equal(u32(e, parts + 4), n);
  assert.equal(u32(e, parts + 20), n);
  assert.equal(view(e).getBigUint64(parts + 24, true), BigInt(m.capacity));
  assert.equal(u32(e, parts + 32), u32(e, parts));
  assert.equal(view(e).getBigUint64(parts + 40, true),
    view(e).getBigUint64(activeRecord[0].address + 16, true));
  assertStrings(e, { array_acquired_bytes: m.acquired, array_released_bytes: m.released,
    array_active_bytes: m.capacity, array_peak_active_bytes: m.peak,
    array_copied_bytes: m.copied, array_relocations: m.relocations });
  assert.equal(completedCallPeak, m.capacity);
  if (m.relocations) assert(m.peak > completedCallPeak,
    'acquisition-time old+new peak differs from completed-call active peak');
  const beforeJoin = counters(e);
  const result = s.call('join', parts, input.handles[separator]);
  for (const instance of s.exports) assert.equal(text(instance, result), expected);
  const materialized = n !== 1;
  const stringReservation = materialized ? align(8 + B) : 0;
  assertStrings(e, { string_payload_allocations: +materialized,
    string_payload_bytes: materialized ? B : 0, string_descriptor_bytes: materialized ? 8 : 0,
    string_copied_bytes: materialized ? B : 0 });
  for (const name of names.filter(n => n.startsWith('array_'))) assert.equal(counters(e)[name], beforeJoin[name]);
  // The current implementation uses a source descriptor (8), stable descriptor
  // plus authority (48), and each distinct allocator record (32) plus capacity.
  // These are explicit implementation-accounting expectations, not identity or
  // allocation requirements imposed on future valid string implementations.
  const physical = 8 + 48 + r.recordBytes + r.physicalCapacity + stringReservation;
  assertStrings(e, { allocations: 2 + r.records.length + +materialized,
    requested_bytes: 56 + r.recordBytes + r.physicalCapacity + (materialized ? 8 + B : 0),
    reserved_bytes: physical });
  const extents = [[s.origin, s.origin + 8], [parts, parts + 48],
    ...r.records.map(r => [r.address, r.end])];
  if (materialized) extents.push([result, result + stringReservation]);
  extents.sort((a, b) => a[0] - b[0]);
  assert.equal(extents[0][0], s.origin);
  for (let i = 1; i < extents.length; i++) assert.equal(extents[i - 1][1], extents[i][0]);
  assert.equal(extents.at(-1)[1], e.__m5_cursor.value >>> 0);
  if (n > 0) assert(m.copied < 8 * n);
  if (n >= 2) {
    assert(physical <= 256 * (n + 1) + B + 8);
    assert(m.copied + B <= 8 * n + B);
  }
  for (const [value, h] of Object.entries(input.handles)) assert.equal(text(e, h), value);
  const first = counters(e);
  const second = s.call('join', parts, input.handles[separator]);
  assert.equal(text(e, result), expected);
  assert.equal(text(e, second), expected);
  const repeated = delta(counters(e), first);
  assert.equal(repeated.reserved_bytes, BigInt(stringReservation));
  assert.equal(repeated.string_copied_bytes, BigInt(materialized ? B : 0));
  assert.equal(repeated.array_copied_bytes, 0n);
  rows.push({ n, pattern, separator, B, fragmentProductionBytes: 0,
    preexistingInputs: { uniqueBytes: input.staticUniqueBytes, ranges: input.ranges,
      scope: 'six preexisting scalar input literals; union excludes unused corpus literals',
      initialLayoutEnd: s.origin },
    counters: first, physical: { sourceDescriptor: 8, stableDescriptorAndAuthority: 48,
      allocatorRecords: r.recordBytes, retainedArrayCapacity: r.physicalCapacity,
      activeCapacity: r.active, freeCapacity: r.freeCapacity, stringReservation,
      totalReserved: physical }, completedCallActivePeak: completedCallPeak,
    observedTransientActivePeak: m.peak, repeatedJoinDelta: repeated });
}
// First vertical slice: two literal bytes, no production during accumulation.
construction(2, ['a'], '');
for (const n of sizes) {
  for (const [pattern, separator] of [
    [['a'], ''], [['λ猫'], ''], [['', 'λ猫', '', 'λ猫'], '界'],
    [['', ''], ''], [['', ''], '界'], [['a', '', 'b', 'a'], '|'],
  ]) construction(n, pattern, separator);
}
const baselines = [];
for (const fragment of ['a', 'λ猫']) for (const n of sizes) {
  const s = paired(), input = inputs(s), f = byteLength(fragment);
  let current = input.handles[''];
  const retained = [];
  for (let i = 0; i < n; i++) {
    current = s.call('concat', current, input.handles[fragment]);
    retained.push(current);
    assert.equal(text(s.observed, current), fragment.repeat(i + 1));
  }
  const copied = n >= 1 ? f * (n * (n + 1) / 2 - 1) : 0;
  let reserved = 0;
  for (let k = 2; k <= n; k++) reserved += align(8 + k * f);
  const count = Math.max(0, n - 1);
  assertStrings(s.observed, { allocations: count, requested_bytes: copied + 8 * count,
    reserved_bytes: reserved, string_payload_allocations: count,
    string_payload_bytes: copied, string_descriptor_bytes: 8 * count, string_copied_bytes: copied,
    array_acquired_bytes: 0, array_copied_bytes: 0 });
  for (let i = 0; i < retained.length; i++) assert.equal(text(s.observed, retained[i]), fragment.repeat(i + 1));
  baselines.push({ n, fragment, fragmentBytes: f, B: f * n, copiedBytes: copied,
    retainedReservedBytes: reserved, firstEmptyOperandReuseRetained: true,
        fragmentProductionBytes: 0, preexistingInputUniqueBytes: input.staticUniqueBytes,
        counters: counters(s.observed) });
}
const correctness = [];
for (const [name, values] of [['fixed_zero', []], ['fixed_one', ['']],
  ['fixed_two', ['', '']], ['fixed_mixed', ['a', '', 'b']],
  ['fixed_duplicates', ['λ猫', 'λ猫', '']]]) {
  for (const separator of ['', '界', '|']) {
    const s = paired(), input = inputs(s), h = s.call(name);
    const before = counters(s.observed), result = s.call('join', h, input.handles[separator]);
    assert.equal(text(s.observed, result), values.join(separator));
    assert.deepEqual(arrayContents(s.observed, h), values);
    const d = delta(counters(s.observed), before);
    assert.equal(d.string_copied_bytes, BigInt(values.length === 1 ? 0 : byteLength(values.join(separator))));
    correctness.push({ name, separator, expected: values.join(separator), joinDelta: d });
  }
}
// Separator effects are guest nested captures: mutation and actual relocation
// affect the captured receiver, while rebinding the lexical variable does not.
for (const [name, expected] of [['separator_effects', 'λ猫界界b'],
  ['fixed_separator_effects', 'a|λ猫|b'], ['ordered_argument_effects', 'a|b']]) {
  const s = paired(), result = s.call(name);
  assert.equal(text(s.observed, result), expected);
  correctness.push({ name, expected });
}
// Reuse is not a bypass for validating the separator, singleton entry, or outer
// storage. Corruption is applied identically and restored before later calls.
for (const name of ['fixed_zero', 'fixed_one']) {
  const s = paired(), input = inputs(s), h = s.call(name), e = s.observed;
  const before = counters(e);
  s.trap('join', h, 0);
  assert.deepEqual(counters(e), before);
  const pointer = u32(e, h), length = u32(e, h + 4);
  for (const instance of s.exports) view(instance).setUint32(h, instance.memory.buffer.byteLength - 1, true);
  for (const instance of s.exports) view(instance).setUint32(h + 4, 2, true);
  s.trap('join', h, input.handles['']);
  assert.deepEqual(counters(e), before);
  for (const instance of s.exports) {
    view(instance).setUint32(h, pointer, true); view(instance).setUint32(h + 4, length, true);
  }
  if (length) {
    const saved = u32(e, pointer);
    for (const instance of s.exports) view(instance).setUint32(pointer, 0, true);
    s.trap('join', h, input.handles['']);
    assert.deepEqual(counters(e), before);
    for (const instance of s.exports) view(instance).setUint32(pointer, saved, true);
  }
  s.same();
}
// Retain inputs, array entries and separately retained results across actual
// memory growth, then a caught trap. Numeric offsets survive refreshed views.
const lifetime = (() => {
  const s = paired(), input = inputs(s), parts = s.call('fragments');
  for (const value of ['λ猫', '', 'a']) s.call('append', parts, input.handles[value]);
  const result = s.call('join', parts, input.handles['界']);
  const repeated = s.call('join', parts, input.handles['界']);
  const initialBytes = s.observed.memory.buffer.byteLength;
  let large = input.handles['λ猫'];
  for (let i = 0; i < 15; i++) large = s.call('concat', large, large);
  assert(s.observed.memory.buffer.byteLength > initialBytes);
  const before = counters(s.observed);
  s.trap('reserve', parts, -1);
  assert.deepEqual(counters(s.observed), before);
  for (const e of s.exports) {
    assert.equal(text(e, result), 'λ猫界界a');
    assert.equal(text(e, repeated), 'λ猫界界a');
    assert.equal(byteLength(text(e, large)), 5 * 2 ** 15);
    assert.deepEqual(arrayContents(e, parts), ['λ猫', '', 'a']);
    for (const [value, h] of Object.entries(input.handles)) assert.equal(text(e, h), value);
  }
  registry(s);
  return { actualMemoryGrowth: true, caughtTrapRetainsEarlierResults: true,
    initialMemoryBytes: initialBytes, finalMemoryBytes: s.observed.memory.buffer.byteLength,
    retainedJoinResults: 2, counters: counters(s.observed) };
})();
// Optional fragment-production cost is a separate interval, never folded into
// the zero-production static-input construction rows above.
const productionExample = (() => {
  const s = paired(), input = inputs(s), before = counters(s.observed);
  const fragment = s.call('produced');
  assert.equal(text(s.observed, fragment), 'λ猫a');
  const production = delta(counters(s.observed), before);
  assert.equal(production.string_copied_bytes, 6n);
  const parts = s.call('fragments');
  s.call('append', parts, fragment); s.call('append', parts, fragment);
  const joinBefore = counters(s.observed), h = s.call('join', parts, input.handles['']);
  assert.equal(text(s.observed, h), 'λ猫aλ猫a');
  return { production, join: delta(counters(s.observed), joinBefore) };
})();
console.log(JSON.stringify({ schema: 'm5-construction-79-v1', sizes,
  policy: 'default None; current implementation accounting, not mandatory allocation semantics',
  counterBoundExclusive: '67108864', counterArithmetic: 'unsigned i64 BigInt; all exact totals below 2^64',
  rows, baselines, correctness, lifetime, productionExample },
  (_, value) => typeof value === 'bigint' ? value.toString() : value, 2));
