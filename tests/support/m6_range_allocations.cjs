const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const dir = process.argv[2];
// Each checkpoint is [tag, range records since last checkpoint, other bump bytes].
// Tag 0 is the return/trap boundary. Operand snapshots occur after their allocation.
const operand = tag => [tag, 0, 16];
const direct = (records, yields) => [operand(1), operand(2),
    ...Array.from({length: yields}, (_, i) => [10, i === 0 ? records : 0, 0]),
    [0, yields === 0 ? records : 0, 0]];
const local = (records, yields) => [operand(1), operand(2), operand(3),
    ...Array.from({length: yields}, (_, i) => [[10, i === 0 ? records : 0, 8],
        [11, 0, 8], [12, 0, 16]]).flat(), // promoted cell, environment, body array
    [0, yields === 0 ? records : 0, 0]];

for (let level = 0; level < 4; ++level) {
    const records = level === 0 ? 1 : 0;
    const cases = [
        ['direct', [1, 4], 6, direct(records, 3)],
        ['direct', [4, 4], 0, direct(records, 0)],
        ['local', [5, 1, -2], 9, local(records, 3)],
        ['local', [1, 5, -2], 0, local(records, 0)],
        ['local', [5, 5, 2], 5, local(records, 1)],
        ['indirect_binding', [1, 4], 6, [[0, 1, 0]]],
        ['indirect_binding', [4, 4], 0, [[0, 1, 0]]],
        ['indirect_call', [5, 1], 9, [[0, 1, 0]]],
        ['indirect_call', [1, 5], 0, [[0, 1, 0]]],
        ['abrupt', [1], 71, [operand(1), [0, 0, 0]]],
        ['abrupt', [2], 72, [operand(1), operand(2), [0, 0, 0]]],
        ['abrupt', [3], 73, [operand(1), operand(2), operand(3), [0, 0, 0]]],
        ['direct', [1.5, 1], 'trap', [operand(1), operand(2), [0, 0, 0]]],
        ...[[NaN, 4, 1], [4, Infinity, 1], [4, 1, -0], [4, 1, 2 ** 53]].map(args =>
            ['local', args, 'trap', [operand(1), operand(2), operand(3), [0, 0, 0]]]),
    ];
    for (const variant of ['ordinary', 'observed']) {
        const bytes = fs.readFileSync(path.join(dir, `o${level}-${variant}.wasm`));
        const observed = variant === 'observed';
        let trace = [], cursor, count;
        function checkpoint(tag) {
            assert.ok(trace.length < 16, 'fixture checkpoint limit');
            if (!observed) {
                trace.push(tag);
                return;
            }
            const nextCursor = guest.__m6_cursor.value >>> 0;
            const nextCount = Number(guest.__m6_records.value);
            const records = nextCount - count;
            trace.push([tag, records, nextCursor - cursor - 32 * records]);
            cursor = nextCursor;
            count = nextCount;
        }
        const guest = new WebAssembly.Instance(new WebAssembly.Module(bytes), {env: {checkpoint}}).exports;
        assert.equal('__m6_cursor' in guest, observed);
        assert.equal('__m6_records' in guest, observed);
        if (observed) {
            cursor = guest.__m6_cursor.value >>> 0;
            count = Number(guest.__m6_records.value);
            assert.equal(count, 0);
        }
        // Repeat on the same instance, including successful calls after caught traps.
        for (let repeat = 0; repeat < 2; ++repeat) {
            for (const [index, [name, args, expected, phases]] of cases.entries()) {
                trace = [];
                if (expected === 'trap') assert.throws(() => guest[name](...args), WebAssembly.RuntimeError);
                else assert.equal(guest[name](...args), expected, name);
                checkpoint(0);
                assert.deepEqual(trace, observed ? phases : phases.map(([tag]) => tag),
                    `o${level}/${variant}/${name}/${index}`);
                if (observed) console.log(JSON.stringify({level, repeat, index, name, trace}));
            }
        }
    }
}
