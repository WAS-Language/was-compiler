const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const selected = ['direct', 'local', 'indirect_binding', 'indirect_call', 'abrupt'];
const imports = [
    {module: 'range', name: 'for', kind: 'function'},
    {module: 'by', name: 'in', kind: 'function'},
];

const surfaces = {
    selected: {functions: selected, memory: true},
    internal: {functions: [], memory: true},
    indirect: {functions: ['main'], memory: true},
    scalar: {functions: ['main'], memory: false},
    direct: {functions: ['main'], memory: true},
};

function checkModule(bytes, fixture) {
    const module = new WebAssembly.Module(bytes);
    assert.deepEqual(WebAssembly.Module.imports(module), imports);
    const {functions, memory} = surfaces[fixture];
    const expected = functions.map(name => ({name, kind: 'function'}));
    if (memory) expected.push({name: 'memory', kind: 'memory'}, {name: '__alloc', kind: 'function'});
    const byName = (a, b) => a.name.localeCompare(b.name);
    assert.deepEqual(WebAssembly.Module.exports(module).sort(byName), expected.sort(byName));

    let effects = [];
    const host = {range: {for(tag) {
        assert.ok(effects.length < 16, 'fixture effect limit');
        effects.push(tag);
    }}};
    assert.throws(() => new WebAssembly.Instance(module, host), TypeError);
    host.by = {in() { assert.fail('unused import invoked'); }};
    const guest = new WebAssembly.Instance(module, host).exports;
    if (memory) {
        assert.equal(guest.memory.buffer.byteLength, 65_536);
        const first = guest.__alloc(0);
        assert.ok(first >= 8 && first % 8 === 0);
        assert.equal(guest.__alloc(1), first + 8);
    }
    if (fixture === 'internal') return;
    if (fixture === 'indirect' || fixture === 'direct') {
        assert.equal(guest.main(1, 4), 7);
        return;
    }
    if (fixture === 'scalar') {
        assert.equal(guest.main(10), 4);
        return;
    }

    const max = Number.MAX_SAFE_INTEGER;
    const cases = [
        ['direct', [1, 4], 6, [1, 2, 10, 10, 10]],
        ['direct', [4, 4], 0, [1, 2]],
        ['direct', [max - 1, max], max - 1, [1, 2, 10]],
        ['local', [5, 1, -2], 9, [1, 2, 3, 10, 11, 12, 10, 11, 12, 10, 11, 12]],
        ['local', [1, 5, -2], 0, [1, 2, 3]],
        ['local', [max, max, max], max, [1, 2, 3, 10, 11, 12]],
        ['local', [-max, -max, -max], -max, [1, 2, 3, 10, 11, 12]],
        ['local', [-0, -0, 1], 0, [1, 2, 3, 10, 11, 12]],
        ['indirect_binding', [1, 4], 6, []],
        ['indirect_call', [5, 1], 9, []],
        ['abrupt', [2], 72, [1, 2]],
        ['direct', [1.5, 1], 'trap', [1, 2]],
        ...[[NaN, 4, 1], [4, Infinity, 1], [4, 1, -0], [4, 1, 2 ** 53]]
            .map(args => ['local', args, 'trap', [1, 2, 3]]),
        ['local', [2, 2, 1], 2, [1, 2, 3, 10, 11, 12]], // Recovery on the same instance.
    ];
    for (const [name, args, expectedResult, expectedEffects] of cases) {
        effects = [];
        if (expectedResult === 'trap') {
            assert.throws(() => guest[name](...args), WebAssembly.RuntimeError, name);
        } else {
            assert.equal(guest[name](...args), expectedResult, name);
        }
        assert.deepEqual(effects, expectedEffects, name);
    }
}

module.exports = {checkModule};

if (require.main === module) {
    const dir = process.argv[2];
    for (const fixture of Object.keys(surfaces)) {
        for (let level = 0; level < 4; level++) {
            checkModule(fs.readFileSync(path.join(dir, `${fixture}-${level}.wasm`)), fixture);
        }
    }
}
