'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const workloads = ['direct', 'binding', 'call'];
const presets = ['None', 'Basic', 'Standard', 'Aggressive'];
const trips = [4, 1024];
const rounds = 2;
const samples = 7;
const warmupCalls = 2048;
const calls = 4096;

function sumBatch(sum, n, count) {
    let checksum = 0;
    for (let i = 0; i < count; i++) {
        checksum += sum(n);
    }
    return checksum;
}

function main(directory) {
    const schedule = [];
    for (const workload of workloads) {
        for (const preset of presets) {
            const bytes = fs.readFileSync(path.join(directory, `${workload}-${preset}.wasm`));
            const module = new WebAssembly.Module(bytes);
            assert.deepEqual(WebAssembly.Module.imports(module), []);
            assert.deepEqual(WebAssembly.Module.exports(module).map(x => x.name).sort(),
                ['__alloc', 'memory', 'sum']);
            for (const n of trips) {
                schedule.push({workload, preset, module, n});
            }
        }
    }

    const rows = ['workload,preset,trips,round,sample,calls,elapsed_ns'];
    for (let round = 1; round <= rounds; round++) {
        if (round % 2 === 0) schedule.reverse();
        for (const {workload, preset, module, n} of schedule) {
            // Arithmetic-series oracle; all checksums are exact safe integers.
            const expected = n * (n - 1) / 2;
            for (let sample = 1; sample <= samples; sample++) {
                const {sum} = new WebAssembly.Instance(module).exports;
                const label = `${workload}/${preset}/${n}/${round}/${sample}`;
                assert.equal(sum(0), 0, label);
                assert.equal(sum(n), expected, label);
                assert.equal(sumBatch(sum, n, warmupCalls), expected * warmupCalls, label);
                const start = process.hrtime.bigint();
                const result = sumBatch(sum, n, calls);
                const elapsed = process.hrtime.bigint() - start;
                assert.equal(result, expected * calls, label);
                rows.push(`${workload},${preset},${n},${round},${sample},${calls},${elapsed}`);
            }
        }
    }
    fs.writeFileSync(path.join(directory, 'guest.csv'), rows.join('\n') + '\n', {flag: 'wx'});
}

assert.equal(process.argv.length, 3, 'usage: node m6_range_timing.cjs OUTPUT_DIRECTORY');
main(process.argv[2]);
