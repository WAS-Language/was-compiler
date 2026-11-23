'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

// Corpus and independent raw Wasm results, in #91 plan order.
const workloads = [
    {name: 'fraction_closed', expected: 24.375},
    {name: 'fraction_open', expected: 24.375},
    {name: 'wide_closed', expected: 390},
    {name: 'wide_open', expected: 390},
    {name: 'bool_closed', input: 3, expected: 1},
    {name: 'bool_open', input: 3, expected: 1},
    {name: 'negative_zero', expected: -0, verifyOnly: true},
    {name: 'subnormal_tie', expected: +0, verifyOnly: true},
    {name: 'nonfinite_root', expected: Infinity, verifyOnly: true},
    {name: 'division_root', expected: 0.75},
    {name: 'integer_local', expected: 1},
    {name: 'range_direct', input: 4, expected: 6, memory: true},
    {name: 'array_growth', expected: 2.25, memory: true},
];
const presets = ['None', 'Basic', 'Standard', 'Aggressive'];
const samples = 20;
const warmupCalls = 2048;
const warmedCalls = 4096;
const checkpoints = [0, 1, warmupCalls, warmupCalls + warmedCalls, 8192];

function loadSchedule(directory, mode, isolated) {
    const variants = isolated ? ['FoldOff', 'FoldOn'] : mode === 'verify' ? presets : presets.slice(0, 2);
    const schedule = [];
    for (const {input = 1.5, ...workload} of workloads) {
        if (mode !== 'verify' && workload.verifyOnly) continue;
        for (const preset of variants) {
            const file = `${workload.name}-${preset}.wasm`;
            const module = new WebAssembly.Module(fs.readFileSync(path.join(directory, file)));
            assert.deepEqual(WebAssembly.Module.imports(module), [], file);
            const expectedExports = workload.memory ? ['__alloc:function', 'main:function', 'memory:memory']
                : ['main:function'];
            const exports = WebAssembly.Module.exports(module).map(({name, kind}) => `${name}:${kind}`);
            assert.deepEqual(exports.sort(), expectedExports, file);
            schedule.push({...workload, preset, module, input});
        }
    }
    return mode === 'reverse' ? schedule.reverse() : schedule;
}

function verify(schedule, rows) {
    for (const {name, preset, module, input, expected} of schedule) {
        const label = `${name}/${preset}`;
        const {main, memory} = new WebAssembly.Instance(module).exports;
        const initial = memory?.buffer.byteLength;
        const shouldGrow = name === 'array_growth' || (name === 'range_direct' && preset === 'None');
        let calls = 0;
        for (const checkpoint of checkpoints) {
            for (; calls < checkpoint; calls++) {
                // Strict assert.equal uses Object.is, including for both signs of zero.
                assert.equal(main(input), expected, label);
            }
            const bytes = memory?.buffer.byteLength;
            if (shouldGrow && checkpoint === checkpoints.at(-1)) {
                assert(bytes > initial, `${label}: memory must grow`);
            } else if (!shouldGrow) {
                assert.equal(bytes, initial, `${label}: memory must not grow`);
            }
            rows.push(`${name},${preset},${checkpoint},${bytes ?? 'none'}`);
        }
        if (name === 'bool_open') {
            const alternate = new WebAssembly.Instance(module).exports.main;
            assert.equal(alternate(1), 0, `${label}: alternate input`);
        }
    }
}

function sumBatch(main, input, calls) {
    let sum = 0;
    for (let i = 0; i < calls; i++) sum += main(input);
    return sum;
}

function timeGuests(schedule, rows) {
    // Keep phases separate. In each phase the reverse session reverses the
    // entire workload/preset schedule, not just the order of the workloads.
    for (const boundary of ['instantiation', 'first_call', 'warmed']) {
        for (const {name, preset, module, input, expected} of schedule) {
            for (let sample = 1; sample <= samples; sample++) {
                const label = `${name}/${preset}/${boundary}/${sample}`;
                // Instantiation counts one constructor invocation, not the post-timer probe.
                let calls = 1;
                let elapsed;
                if (boundary === 'instantiation') {
                    const start = process.hrtime.bigint();
                    const instance = new WebAssembly.Instance(module);
                    elapsed = process.hrtime.bigint() - start;
                    assert.equal(instance.exports.main(input), expected, label);
                } else {
                    const {main} = new WebAssembly.Instance(module).exports;
                    if (boundary === 'first_call') {
                        const start = process.hrtime.bigint();
                        const result = main(input);
                        elapsed = process.hrtime.bigint() - start;
                        assert.equal(result, expected, label);
                    } else {
                        assert.equal(sumBatch(main, input, warmupCalls), expected * warmupCalls, label);
                        calls = warmedCalls;
                        const start = process.hrtime.bigint();
                        const checksum = sumBatch(main, input, calls);
                        elapsed = process.hrtime.bigint() - start;
                        assert.equal(checksum, expected * calls, label);
                    }
                }
                rows.push(`${name},${preset},${boundary},${sample},${calls},${elapsed}`);
            }
        }
    }
}

const [directory, mode, experiment, ...extra] = process.argv.slice(2);
assert(extra.length === 0 && (experiment === undefined || experiment === 'isolated') &&
    ['verify', 'forward', 'reverse'].includes(mode),
    'usage: node tests/support/m8_scalar_timing.cjs DIRECTORY verify|forward|reverse [isolated]');

const schedule = loadSchedule(directory, mode, experiment === 'isolated');
const verifying = mode === 'verify';
const rows = [verifying ? 'workload,preset,calls,memory_bytes'
    : 'workload,preset,boundary,sample,calls,elapsed_ns'];
if (verifying) verify(schedule, rows);
else timeGuests(schedule, rows);
fs.writeFileSync(path.join(directory, verifying ? 'growth.csv' : 'guest.csv'),
    rows.join('\n') + '\n', {flag: 'wx'});
