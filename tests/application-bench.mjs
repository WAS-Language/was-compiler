import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const workloads = ['report', 'aggregate'];
const sizes = [16, 128, 1024];
const presets = ['o0', 'default'];
const orders = ['forward', 'reverse', 'forward'];
const warmupsPerCase = 2;
const samplesPerCase = 5;
const statuses = ['ok', 'alerta', 'ação', 'revisão'];
const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
const encoder = new TextEncoder();
const descriptorBytes = 8;
const minimumAddress = 8;

function expectedResult(workload, size) {
    if (workload === 'report') {
        const items = Array.from({ length: size }, (_, i) => `item=${statuses[i % 4]}`);
        return ['status', ...items].join('; ');
    }

    let sum = 0;
    for (let i = 0; i < size; i++) {
        const value = (i * 37 + 11) % 100;
        sum += value * (value >= 80 ? 3 : value >= 50 ? 2 : 1);
    }
    return sum;
}

function loadCases(directory) {
    const cases = [];
    for (const workload of workloads) {
        for (const size of sizes) {
            for (const preset of presets) {
                const file = `${workload}-${size}-${preset}.wasm`;
                const bytes = readFileSync(join(directory, file));
                const expected = expectedResult(workload, size);
                const expectedBytes = workload === 'report' ? encoder.encode(expected).byteLength : null;
                cases.push({ workload, size, preset, file, bytes, expected, expectedBytes });
            }
        }
    }
    return cases;
}

function requireCase(condition, file, message) {
    if (!condition) throw new Error(`${file}: ${message}`);
}

function validateResult(entry, exports, result) {
    const { file, workload, expected, expectedBytes } = entry;
    if (workload === 'aggregate') {
        requireCase(result === expected, file, `expected ${expected}, got ${result}`);
        return;
    }

    requireCase(Number.isInteger(result) && (result | 0) === result, file, 'expected an i32 string handle');
    requireCase(exports.memory instanceof WebAssembly.Memory, file, 'missing memory export');

    // Read the current buffer after main(), which may have grown guest memory.
    const buffer = exports.memory.buffer;
    const handle = result >>> 0;
    requireCase(handle >= minimumAddress && handle % descriptorBytes === 0 &&
        handle + descriptorBytes <= buffer.byteLength, file, 'invalid string descriptor');
    const descriptor = new DataView(buffer, handle, descriptorBytes);
    const data = descriptor.getUint32(0, true);
    const length = descriptor.getUint32(4, true);
    requireCase(data >= minimumAddress && data + length <= buffer.byteLength, file, 'invalid string payload bounds');
    requireCase(length === expectedBytes, file, `expected ${expectedBytes} string bytes, got ${length}`);
    requireCase(decoder.decode(new Uint8Array(buffer, data, length)) === expected, file, 'unexpected report text');
}

function validateCase(entry, preflight = false) {
    try {
        const module = new WebAssembly.Module(entry.bytes);
        if (preflight && WebAssembly.Module.imports(module).length !== 0) {
            throw new Error('unexpected Wasm imports');
        }
        const { exports } = new WebAssembly.Instance(module);
        if (preflight && (typeof exports.main !== 'function' || exports.main.length !== 0)) {
            throw new Error('expected a zero-argument main function export');
        }
        validateResult(entry, exports, exports.main());
    } catch (error) {
        throw new Error(`validation failed for ${entry.file}`, { cause: error });
    }
}

function measure(entry) {
    const bytes = entry.bytes;
    const compileStart = process.hrtime.bigint();
    const module = new WebAssembly.Module(bytes);
    const compileEnd = process.hrtime.bigint();

    const instantiateStart = process.hrtime.bigint();
    const instance = new WebAssembly.Instance(module);
    const instantiateEnd = process.hrtime.bigint();
    const exports = instance.exports;
    const { main } = exports;

    const executeStart = process.hrtime.bigint();
    const result = main();
    const executeEnd = process.hrtime.bigint();

    validateResult(entry, exports, result);
    return {
        compileNs: Number(compileEnd - compileStart),
        instantiateNs: Number(instantiateEnd - instantiateStart),
        executeNs: Number(executeEnd - executeStart),
    };
}

function main() {
    const [mode, directory, ...extra] = process.argv.slice(2);
    if ((mode !== 'check' && mode !== 'timing') || !directory || extra.length !== 0) {
        throw new Error('usage: node tests/application-bench.mjs check|timing DIRECTORY');
    }
    if (Number(process.versions.node.split('.')[0]) < 24) {
        throw new Error('Node.js 24 or newer is required');
    }

    const cases = loadCases(directory);
    for (const entry of cases) validateCase(entry, true);
    if (mode === 'check') return;

    const repetitions = orders.map((order) => ({
        order,
        cases: (order === 'reverse' ? cases.toReversed() : cases).map((entry) => {
            for (let i = 0; i < warmupsPerCase; i++) validateCase(entry);
            const { workload, size, preset, file, bytes } = entry;
            return {
                workload, size, preset, file, wasmBytes: bytes.byteLength,
                samples: Array.from({ length: samplesPerCase }, () => measure(entry)),
            };
        }),
    }));

    process.stdout.write(JSON.stringify({
        node: process.versions.node,
        v8: process.versions.v8,
        engineCache: 'potential (not cold compilation)',
        warmupsPerCase,
        samplesPerCase,
        repetitions,
    }, null, 2) + '\n');
}

main();
