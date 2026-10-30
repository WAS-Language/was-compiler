'use strict';
// Ordinary modules only. This is timing evidence, never allocation accounting.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const crypto = require('node:crypto');
const { createAbi } = require('../../runtime/abi.cjs');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const COUNTS = [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 64, 128, 256];
const REPETITIONS = 3, SAMPLES = 5, ITERATIONS = 8, WARMUPS = 2;

function cases() {
    const result = [];
    for (const count of COUNTS) {
        const parts = Array.from({ length: count }, (_, i) => ['é', '', '🦀', 'a'][i % 4]);
        for (const separator of ['', '·']) {
            for (const entry of ['repeated', 'accumulate', 'join_existing']) {
                result.push({ name: `${entry}_${separator ? 'separated' : 'empty'}_${count}`,
                    entry, types: ['string[]', 'string'], args: [parts, separator],
                    expected: parts.join(separator), n: count,
                    B: parts.reduce((n, p) => n + Buffer.byteLength(p), 0) +
                        Math.max(0, count - 1) * Buffer.byteLength(separator) });
            }
        }
    }
    for (const length of [64, 256, 1024]) {
        for (const [label, needle, replacement] of [
            ['no_match', 'a'.repeat(32) + 'b', 'x'],
            ['overlap', 'a'.repeat(16), 'b'],
            ['expand', 'aa', 'aaéa'],
            ['shrink', 'aa', ''],
        ]) {
            const source = 'a'.repeat(length - 1); // Odd tails exercise nonoverlapping leftovers.
            result.push(replacementCase(`${label}_${length}`, source, needle, replacement));
        }
    }
    result.push(replacementCase('sentinel_nonoverlap', 'aaa', 'aa', 'b', 'ba'));
    result.push(replacementCase('sentinel_case_sensitive', 'AaAa', 'a', 'xx', 'AxxAxx'));
    result.push(replacementCase('sentinel_utf8', 'éé🦀é', 'é', 'λ', 'λλ🦀λ'));
    assert.equal(result.length, 123);
    return result;
}
function replacementCase(name, source, needle, replacement, expected) {
    return { name: `replace_${name}`, entry: 'replace', types: ['string', 'string', 'string'],
        args: [source, needle, replacement], expected: expected ?? source.replaceAll(needle, replacement),
        sourceBytes: Buffer.byteLength(source), needleBytes: Buffer.byteLength(needle),
        replacementBytes: Buffer.byteLength(replacement) };
}
function fresh(module, test) {
    const instance = new WebAssembly.Instance(module);
    const abi = createAbi(() => instance);
    const args = test.args.map((value, i) => abi.encode(test.types[i], value));
    return { e: instance.exports, abi, args };
}
function verify(session, test, outputs) {
    for (const handle of outputs) {
        assert.equal(session.abi.decode('string', handle), test.expected, test.name);
    }
    // Retention and immutable inputs checked after possible guest growth.
    test.args.forEach((value, i) => assert.deepEqual(session.abi.decode(test.types[i], session.args[i]), value));
}
function check(module, tests) {
    for (const test of tests) {
        const session = fresh(module, test);
        const outputs = Array.from({ length: ITERATIONS }, () => session.e[test.entry](...session.args));
        verify(session, test, outputs);
        assert.equal(Buffer.byteLength(test.expected), test.B ?? Buffer.byteLength(test.expected));
    }
    const emptyNeedle = replacementCase('empty_needle', '', '', 'x');
    const session = fresh(module, emptyNeedle);
    assert.throws(() => session.e.replace(...session.args), WebAssembly.RuntimeError);
}
function timing(module, tests) {
    const samples = [];
    for (let repetition = 1; repetition <= REPETITIONS; repetition++) {
        const ordered = repetition % 2 === 1 ? tests : tests.toReversed();
        for (const test of ordered) {
            // Warmup runs on separate instances. Engine/module tier state is shared,
            // but every measured batch starts with a fresh instance and fresh inputs.
            for (let i = 0; i < WARMUPS; i++) {
                const warm = fresh(module, test);
                const outputs = Array.from({ length: ITERATIONS }, () => warm.e[test.entry](...warm.args));
                verify(warm, test, outputs);
            }
            for (let sample = 1; sample <= SAMPLES; sample++) {
                const session = fresh(module, test);
                const entry = session.e[test.entry], args = session.args;
                const outputs = new Array(ITERATIONS);
                const start = process.hrtime.bigint();
                for (let i = 0; i < ITERATIONS; i++) outputs[i] = entry(...args);
                const elapsedNs = Number(process.hrtime.bigint() - start);
                verify(session, test, outputs);
                samples.push({ case: test.name, repetition, sample, iterations: ITERATIONS, elapsedNs });
            }
        }
    }
    assert.equal(samples.length, tests.length * REPETITIONS * SAMPLES);
    return samples;
}
function main() {
    const [mode, file] = process.argv.slice(2);
    assert.equal(process.argv.length, 4, 'usage: check|timing MODULE.wasm (check also accepts stdin: -)');
    assert(['check', 'timing'].includes(mode));
    assert(Number(process.versions.node.split('.')[0]) >= 24, 'Node 24+ required');
    assert(mode === 'check' || file !== '-');
    const bytes = fs.readFileSync(file === '-' ? 0 : file);
    assert(WebAssembly.validate(bytes));
    const module = new WebAssembly.Module(bytes);
    assert.deepEqual(WebAssembly.Module.imports(module), []);
    assert.deepEqual(WebAssembly.Module.exports(module).map(e => e.name).sort(),
        ['__alloc', 'accumulate', 'join_existing', 'memory', 'repeated', 'replace']);
    const tests = cases();
    check(module, tests); // Independent correctness pass, entirely outside timing.
    const samples = mode === 'timing' ? timing(module, tests) : [];
    const record = { mode, checkedCases: tests.length, module: { bytes: bytes.length, sha256: hash(bytes) },
        environment: { node: process.version, v8: process.versions.v8, os: os.type(), release: os.release(),
            arch: os.arch(), cpu: os.cpus()[0]?.model, logicalCpus: os.cpus().length },
        methodology: { repetitions: REPETITIONS, samplesPerCase: SAMPLES, iterations: ITERATIONS,
            warmupBatchesPerCasePerRepetition: WARMUPS, order: 'forward/reverse/forward',
            instance: 'fresh per measured batch; warmup uses separate fresh instances',
            numericPreset: 'None', observations: false, timer: 'process.hrtime.bigint',
            included: 'JS loop, spread call/crossing and handle stores; guest validation/allocation/copy/growth',
            excluded: 'source/native/engine compilation, instance/input/oracle setup, decoding/assertions/reporting',
            tiering: 'module/engine state shared; warmup does not guarantee stable JIT tier or GC scheduling' },
        cases: tests.map(({ args, types, ...test }) => ({ ...test,
            inputUtf8BytesByOccurrence: args.reduce((n, arg) => n + (Array.isArray(arg) ?
                arg.reduce((m, p) => m + Buffer.byteLength(p), 0) : Buffer.byteLength(arg)), 0) })),
        samples };
    console.log(JSON.stringify(record));
}
if (require.main === module) main();
module.exports = { cases };
