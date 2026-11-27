// Loaded by playground-http-page.js only for the integrated release matrix.
import { corpus, exercise, levels } from './m9-corpus.mjs';
import { guestSamples } from './m9-guest-bench.mjs';
import init, { Compiler } from '/matrix/pkg/was_compiler.js';
import { postProcess } from '/matrix/examples/browser/binaryen-post-process.js';

async function load(path) {
    const result = await fetch(`/evidence/${path}`);
    if (!result.ok) throw new Error(`missing matrix input: ${path}`);
    return result;
}

function equal(actual, expected, what) {
    if (actual.length !== expected.length || actual.some((value, i) => !Object.is(value, expected[i]))) {
        throw new Error(`${what} differs (${actual.length} vs ${expected.length} entries)`);
    }
}

export async function matrix() {
    await init();
    const status = await (await load('native-status.json')).json();
    const rows = [];
    for (const [name, { exports, hosts }] of Object.entries(corpus)) {
        const source = await (await load(`${name}.was`)).text();
        const compiler = new Compiler();
        try {
            for (const host of hosts) compiler.registerHostFunction(...host);
            if (exports?.length === 0) compiler.exportNone();
            else if (exports) compiler.setExports(exports);
            for (const level of [0, ...levels]) {
                const prefix = `${name}-O${level}`;
                const row = { prefix, error: null };
                rows.push(row);
                try {
                    compiler.setOptimizationLevel(level);
                    const input = compiler.compile(source);
                    const nativeInput = new Uint8Array(await (await load(`${prefix}-input.wasm`)).arrayBuffer());
                    equal(input, nativeInput, 'browser/native input');
                    const before = exercise(name, input);
                    if (level === 0) {
                        // The actual worker O0 bypass is checked separately.
                        if (status[prefix] !== 'published') throw new Error('native O0 did not publish');
                        const native = new Uint8Array(await (await load(`${prefix}-native.wasm`)).arrayBuffer());
                        equal(input, native, 'browser/native O0 bypass bytes');
                        row.output = input.toBase64();
                        continue;
                    }
                    const run = async () => (await postProcess(input, level, false)).bytes;
                    let output;
                    try {
                        output = await run();
                    } catch (error) {
                        row.rejection = error.message;
                        throw error;
                    }
                    row.output = output.toBase64();
                    equal(await run(), output, 'repeated Chromium output');
                    equal(exercise(name, output), before, 'Chromium ABI observations');
                    if (status[prefix] !== 'published') throw new Error('native adapter did not publish');
                    const native = new Uint8Array(await (await load(`${prefix}-native.wasm`)).arrayBuffer());
                    equal(output, native, 'Chromium/native optimized bytes');

                } catch (error) {
                    row.error = error.message;
                }
            }
        } finally {
            compiler.free();
        }
    }
    return rows;
}

export async function measure() {
    const measurements = [];
    for (const level of levels) {
        const prefix = `contracts-O${level}`;
        const input = new Uint8Array(await (await load(`${prefix}-input.wasm`)).arrayBuffer());
        const expected = new Uint8Array(await (await load(`${prefix}-chromium.wasm`)).arrayBuffer());
        const run = async () => (await postProcess(input, level, false)).bytes;
        equal(await run(), expected, 'warmup Chromium output');
        const postMs = [];
        for (let sample = 0; sample < 7; sample++) {
            const start = performance.now();
            const output = await run();
            postMs.push(performance.now() - start);
            equal(output, expected, 'timed Chromium output');
        }
        measurements.push({ level, postMs, offMs: guestSamples(input), onMs: guestSamples(expected) });
    }
    return measurements;
}
