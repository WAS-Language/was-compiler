'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const names = ['allocations', 'requested_bytes', 'reserved_bytes',
  'string_payload_allocations', 'string_payload_bytes', 'string_descriptor_bytes',
  'string_copied_bytes'];
const sessions = ['ordinary', 'observed'].map(variant => {
  let exports;
  const effects = [], retained = [];
  const instance = new WebAssembly.Instance(new WebAssembly.Module(
    fs.readFileSync(path.join(process.argv[2], `${variant}.wasm`))), {
    env: {
      produce(value) {
        effects.push(value);
        return value === 1 ? exports.left() : value === 2 ? exports.right() : 0;
      },
      retain(handle) { retained.push(handle); },
    },
  });
  exports = instance.exports;
  return { exports, effects, retained };
});
const [ordinary, observed] = sessions;
function counters() {
  return names.map(name => observed.exports[`__m5_${name}`].value);
}
function text(session, handle) {
  // Refresh on every decode: neither growth nor movable array payloads permit
  // retaining a view. Decoding is host observation, never guest copying.
  const memory = session.exports.memory.buffer;
  const view = new DataView(memory);
  const pointer = view.getUint32(handle, true), length = view.getUint32(handle + 4, true);
  return new TextDecoder('utf-8', { fatal: true }).decode(new Uint8Array(memory, pointer, length));
}
function equalMemory() {
  assert.deepEqual(new Uint8Array(ordinary.exports.memory.buffer),
    new Uint8Array(observed.exports.memory.buffer));
}
function operation(call, expected, delta) {
  const before = counters();
  const cursor = observed.exports.__m5_cursor.value;
  const results = sessions.map(call);
  assert.deepEqual(results[0], results[1]);
  assert.deepEqual(results[0], expected);
  assert.deepEqual(counters().map((n, i) => n - before[i]), delta.map(BigInt));
  assert.equal(observed.exports.__m5_cursor.value - cursor, delta[2]);
  equalMemory();
  return results[0];
}
const zero = [0, 0, 0, 0, 0, 0, 0];
assert.deepEqual(counters(), zero.map(BigInt));
const handles = sessions.map(s => [s.exports.left(), s.exports.right(), s.exports.empty()]);
assert.deepEqual(handles[0], handles[1]);
const [left, right, empty] = handles[0];
const joined = operation(s => {
  const h = s.exports.concat(left, right);
  assert.equal(text(s, h), 'λ猫!');
  assert.equal(text(s, left), 'λ');
  assert.equal(text(s, right), '猫!');
  assert.notEqual(h, left);
  assert.notEqual(h, right);
  return h;
}, observed.exports.__m5_cursor.value, [1, 14, 16, 1, 6, 8, 6]);
operation(s => s.exports.concat(empty, left), left, zero);
operation(s => s.exports.concat(right, empty), right, zero);
operation(s => s.exports.concat(empty, empty), empty, zero);
// Reuse must still validate the other operand; no allocation on failure.
for (const args of [[empty, 0], [0, empty], [0, right]]) {
  operation(s => { assert.throws(() => s.exports.concat(...args), WebAssembly.RuntimeError); return 'trap'; }, 'trap', zero);
}
operation(s => {
  const h = s.exports.effects();
  assert.equal(text(s, h), 'λ猫!');
  assert.deepEqual(s.effects, [1, 2]);
  s.effects.length = 0;
  return 'effects';
}, 'effects', [1, 14, 16, 1, 6, 8, 6]);
operation(s => {
  assert.throws(() => s.exports.trapped(), WebAssembly.RuntimeError);
  // A malformed host result traps at the host-call boundary before operand 4;
  // the earlier completed concat remains observable after the caught trap.
  assert.deepEqual(s.effects, [1, 2, 3]);
  assert.equal(s.retained.length, 1);
  assert.equal(text(s, s.retained[0]), 'λ猫!');
  s.effects.length = 0;
  return 'trap';
}, 'trap', [1, 14, 16, 1, 6, 8, 6]);
// Exponential doubling reaches real memory growth in a bounded 15 calls.
let current = left, bytes = 2;
const initialPages = observed.exports.memory.buffer.byteLength / 65536;
for (let i = 0; i < 15; i++) {
  bytes *= 2;
  const expectedHandle = observed.exports.__m5_cursor.value;
  const next = operation(s => {
    const h = s.exports.concat(current, current);
    assert.equal(text(s, h), 'λ'.repeat(2 ** (i + 1)));
    assert.equal(text(s, joined), 'λ猫!');
    assert.equal(text(s, s.retained[0]), 'λ猫!');
    return h;
  }, expectedHandle, [1, bytes + 8, Math.ceil((bytes + 8) / 8) * 8, 1, bytes, 8, bytes]);
  current = next;
}
assert.ok(observed.exports.memory.buffer.byteLength / 65536 > initialPages);
// Mixed attribution tracer: string[] payloads contain handles, not copied text.
// Independent M4 layout decoding distinguishes physical payload/record storage
// from descriptor/authority overhead; no page-derived copying estimate is used.
const fragments = operation(s => s.exports.fragments(), observed.exports.__m5_cursor.value + 48,
  [3, 96, 96, 0, 0, 0, 0]);
function arrayState(s) {
  const view = new DataView(s.exports.memory.buffer);
  const payload = view.getUint32(fragments, true);
  const length = view.getUint32(fragments + 4, true);
  assert.equal(view.getUint32(fragments + 20, true), length);
  assert.equal(view.getUint32(fragments + 32, true), payload);
  const capacity = view.getBigUint64(fragments + 24, true);
  const identity = view.getBigUint64(fragments + 40, true);
  const handles = Array.from({ length }, (_, i) => view.getUint32(payload + i * 4, true));
  return { payload, length, capacity, identity, handles };
}
const firstArrayState = arrayState(observed);
assert.equal(firstArrayState.capacity, 8n);
operation(s => { s.exports.append(fragments, left); return arrayState(s); },
  { ...firstArrayState, length: 1, handles: [left] }, zero);
operation(s => { s.exports.append(fragments, right); return arrayState(s); },
  { ...firstArrayState, length: 2, handles: [left, right] }, zero);
const beforeRelocation = arrayState(observed);
const replacement = observed.exports.__m5_cursor.value + 32;
operation(s => { s.exports.append(fragments, empty); return arrayState(s); },
  { payload: replacement, length: 3, capacity: 16n, identity: beforeRelocation.identity + 1n,
    handles: [left, right, empty] }, [1, 48, 48, 0, 0, 0, 0]);
assert.notEqual(arrayState(observed).payload, beforeRelocation.payload);
// At completed-call boundaries this fixture retains both old and new capacity:
// 24 array-payload bytes, 64 allocator-record bytes, 8 stable descriptor bytes,
// 40 authority bytes and 8 source-literal descriptor bytes = 144 bump bytes.
// The one relocation implies 2 * 4 = 8 handle bytes copied, NOT string bytes.
for (const s of sessions) {
  assert.deepEqual(arrayState(s).handles.map(h => text(s, h)), ['λ', '猫!', '']);
  assert.equal(text(s, joined), 'λ猫!');
}
// Fixture counters use modular i64 addition, not a guard that could add traps.
observed.exports.__m5_string_copied_bytes.value = 9223372036854775807n;
for (const s of sessions) s.exports.concat(left, right);
assert.equal(observed.exports.__m5_string_copied_bytes.value, -9223372036854775803n);
equalMemory();
// Capped variants differ only in a test-fixture memory maximum, equally applied
// to both sides. No control export or simulated allocator failure is required.
const capped = ['ordinary', 'observed'].map(variant => new WebAssembly.Instance(
  new WebAssembly.Module(fs.readFileSync(path.join(process.argv[2], `${variant}-capped.wasm`))),
  { env: { produce() { throw Error('unexpected callback'); }, retain() { throw Error('unexpected callback'); } } },
).exports);
let cappedHandle = capped[0].left();
let successful = 0;
for (let i = 0; i < 16; i++) {
  const before = names.map(name => capped[1][`__m5_${name}`].value);
  const cursor = capped[1].__m5_cursor.value;
  if (i < 13) {
    const result = capped.map(e => e.concat(cappedHandle, cappedHandle));
    assert.equal(result[0], result[1]);
    cappedHandle = result[0];
    successful++;
  } else {
    for (const e of capped) assert.throws(() => e.concat(cappedHandle, cappedHandle), WebAssembly.RuntimeError);
    assert.deepEqual(names.map(name => capped[1][`__m5_${name}`].value), before);
    assert.equal(capped[1].__m5_cursor.value, cursor);
    // Earlier success remains valid; failed allocation cannot publish a result.
    for (const e of capped) {
      const view = new DataView(e.memory.buffer);
      assert.equal(view.getUint32(cappedHandle + 4, true), 16384);
    }
    break;
  }
  assert.deepEqual(new Uint8Array(capped[0].memory.buffer), new Uint8Array(capped[1].memory.buffer));
}
assert.equal(successful, 13);
console.log('M5 paired concat observations: exact events, reuse, growth, allocation failure and retained traps passed');
