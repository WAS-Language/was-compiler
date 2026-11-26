// Node 24+: run directly after rebuilding pkg. Optional arguments are local paths:
// node tests/playground-worker.mjs [binaryen@116.0.0/index.js [guard-pkg-directory]]
import assert from 'node:assert/strict';
import { cp, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { registerHooks } from 'node:module';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const [binaryenPath, guardPath, ...extra] = process.argv.slice(2);
assert.equal(extra.length, 0, 'usage: node tests/playground-worker.mjs [binaryen-index.js [guard-pkg]]');
const source = 'fn main() -> number { let a = [40, 2]; return a[0] + a[1]; }';
const phase = 'Binaryen post-processing';
const assets = ['./binaryen-116.mjs', './binaryen-check/was_compiler.js'];

async function stage(t, { optimizer, guard } = {}) {
    const dir = await mkdtemp(join(tmpdir(), 'playground-worker-'));
    t.after(() => rm(dir, { recursive: true, force: true }));
    const browser = join(dir, 'examples/browser');
    await cp(join(root, 'examples/browser'), browser, { recursive: true,
        filter: path => !['binaryen-116.mjs', 'binaryen-check'].includes(basename(path)),
    });
    await writeFile(join(dir, 'package.json'), '{"type":"module"}');
    await symlink(join(root, 'pkg'), join(dir, 'pkg'));
    if (optimizer) await cp(resolve(optimizer), join(browser, assets[0]));
    if (guard) await cp(resolve(guard), join(browser, 'binaryen-check'), { recursive: true });

    const loads = [];
    const hooks = registerHooks({ resolve(specifier, context, next) {
        if (assets.includes(specifier)) loads.push(specifier);
        return next(specifier, context);
    } });
    t.after(() => hooks.deregister());
    // wasm-pack's web target fetches sibling Wasm. Never fall back to the network.
    t.mock.method(globalThis, 'fetch', async url => new Response(await readFile(fileURLToPath(url)), {
        headers: { 'Content-Type': 'application/wasm' },
    }));
    let handler;
    const messages = [];
    globalThis.self = {
        addEventListener(type, callback) { assert.equal(type, 'message'); handler = callback; },
        postMessage(value, transfer) { messages.push(structuredClone(value, { transfer })); },
    };
    t.after(() => { delete globalThis.self; });
    await import(pathToFileURL(join(browser, 'worker.js')));
    async function send(fields) {
        messages.length = 0;
        await handler({ data: { source, hostPrint: true, showWat: true, ...fields } });
        return messages.slice();
    }
    return { send, loads, browser };
}

function onlyError(messages, expectedPhase) {
    assert.deepEqual(messages.map(m => m.type), ['error']);
    assert.equal(messages[0].phase, expectedPhase);
    assert.ok(messages[0].message);
    for (const field of ['bytes', 'wat', 'result']) assert.ok(!(field in messages[0]));
    assert.deepEqual(messages[0].logs, []);
}

test('requested but unavailable optimizer publishes only a post-processing error', async t => {
    const { send } = await stage(t, { guard: guardPath });
    onlyError(await send({ binaryen: true, optimization: 1 }), phase);
});

function success(messages) {
    assert.deepEqual(messages.map(m => m.type), ['compiled', 'result'], messages.at(-1)?.message);
    assert.equal(messages[1].result, '42');
    assert.deepEqual(messages[1].logs, []);
    return messages[0];
}

async function baseline(level) {
    const { Compiler } = await import('../pkg/was_compiler.js');
    const compiler = new Compiler(); // initialized by the actual worker
    try {
        compiler.setOptimizationLevel(level);
        compiler.setExports(['main']);
        compiler.registerHostFunction('print', 'env', 'log_number', ['number'], 'void');
        return { bytes: compiler.compile(source).slice(), wat: compiler.compileWat(source) };
    } finally {
        compiler.free();
    }
}

for (const [reason, fields] of [
    ['Binaryen off', { binaryen: false, optimization: 3 }],
    ['O0', { binaryen: true, optimization: 0 }],
    ['default request', {}],
]) {
    test(`${reason}: publish compiler output without resolving either optional asset`, async t => {
        const { send, loads } = await stage(t);
        const { bytes, wat } = success(await send(fields));
        assert.deepEqual({ bytes, wat }, await baseline(fields.optimization ?? 1));
        assert.deepEqual(loads, []);
    });
}

test('a source error precedes any optional asset resolution', async t => {
    const { send, loads } = await stage(t);
    onlyError(await send({ source: 'fn', binaryen: true, optimization: 2 }), 'Compilation');
    assert.deepEqual(loads, []);
});

test('missing guard does not publish the unchecked output or fall back', {
    skip: !binaryenPath && 'supply the Binaryen index.js path',
}, async t => {
    const { send } = await stage(t, { optimizer: binaryenPath });
    onlyError(await send({ binaryen: true, optimization: 2 }), phase);
});

const needsPackages = !(binaryenPath && guardPath) && 'supply both Binaryen index.js and guard pkg';

test('real guard prevents publication when valid optimized Wasm changes the unused import', {
    skip: needsPackages,
}, async t => {
    const { send, browser } = await stage(t, { optimizer: binaryenPath, guard: guardPath });
    // Inject a valid, same-length ABI change at the optimizer's output boundary.
    const entry = join(browser, assets[0]);
    await cp(entry, join(browser, 'original.mjs'));
    await writeFile(entry, `import binaryen from './original.mjs';
        const read = binaryen.readBinary;
        binaryen.readBinary = (...args) => {
            const module = read(...args);
            const emit = module.emitBinary.bind(module);
            module.emitBinary = (...args) => {
                const bytes = emit(...args);
                const index = Buffer.from(bytes).indexOf('log_number');
                if (index < 0) throw new Error('unused import missing from fixture');
                bytes[index] = 'X'.charCodeAt(0);
                return bytes;
            };
            return module;
        };
        export default binaryen;`);
    const messages = await send({ binaryen: true, optimization: 1 });
    onlyError(messages, phase);
    assert.match(messages[0].message, /interface/i);
    assert.match(messages[0].message, /uncheck Binaryen/i);
});

test('default and explicit O1–O3 publish and run the import-preserving profile output', {
    skip: needsPackages,
}, async t => {
    const { send, loads } = await stage(t, { optimizer: binaryenPath, guard: guardPath });
    const { default: reference } = await import(pathToFileURL(resolve(binaryenPath)));
    const printer = await import(pathToFileURL(join(resolve(guardPath), 'was_compiler.js')));
    await printer.default();
    const instantiate = t.mock.method(WebAssembly, 'instantiate');
    for (const fields of [{ optimization: 1 }, { optimization: 2 }, { optimization: 3 }, {}]) {
        const level = fields.optimization ?? 1;
        instantiate.mock.resetCalls();
        const { bytes, wat } = success(await send({ binaryen: true, ...fields }));
        const { bytes: input, wat: ordinaryWat } = await baseline(level);
        const module = reference.readBinary(input);
        try {
            module.setFeatures(reference.Features.MVP);
            reference.setOptimizeLevel(level);
            reference.setShrinkLevel(0);
            reference.setDebugInfo(false);
            module.runPasses(['optimize-instructions']);
            const expected = module.emitBinary();
            assert.notDeepEqual(input, expected, 'fixture must distinguish optimized and ordinary modules');
            assert.deepEqual(bytes, expected);
            assert.notEqual(wat, ordinaryWat);
            assert.equal(wat, printer.checkBinaryenOutput(input, expected, true));
        } finally {
            module.dispose();
        }
        assert.deepEqual(WebAssembly.Module.imports(new WebAssembly.Module(bytes)), [
            { module: 'env', name: 'log_number', kind: 'function' },
        ]);

        const executed = instantiate.mock.calls.at(-1).arguments[0];
        assert.deepEqual(new Uint8Array(executed), bytes, 'worker must instantiate the downloaded bytes');
    }
    assert.deepEqual(new Set(loads), new Set(assets));
});
