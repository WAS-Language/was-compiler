const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {checkModule} = require('./m6_adapter_runtime.cjs');

async function main() {
    const pkg = process.argv[2];
    const js = path.join(pkg, 'was_compiler.js');
    const wasm = path.join(pkg, 'was_compiler_bg.wasm');
    assert.ok(fs.existsSync(js) && fs.existsSync(wasm), 'rebuild browser bindings offline before this test');
    const {default: init, Compiler} = await import(pathToFileURL(js).href);
    await init({module_or_path: fs.readFileSync(wasm)});

    const input = fs.readFileSync(0);
    let offset = 0;
    function field() {
        const length = input.readUInt32LE(offset);
        offset += 4;
        const value = input.subarray(offset, offset + length);
        offset += length;
        return value;
    }

    function compiler() {
        const c = new Compiler();
        c.registerHostFunction('checkpoint', 'range', 'for', ['number'], 'void');
        c.registerHostFunction('unused', 'by', 'in', [], 'void');
        return c;
    }

    const fixtures = Number(field().toString());
    for (let index = 0; index < fixtures; index++) {
        const fixture = field().toString();
        const source = field().toString();
        const selection = field().toString();
        const c = compiler();
        try {
            if (selection === '') c.exportNone();
            else if (selection !== '*') c.setExports(selection.split(','));
            for (let level = 0; level < 4; level++) {
                const expectedBytes = field();
                const expectedWat = field().toString();
                if (level === 0) assert.deepEqual(Buffer.from(c.compile(source)), expectedBytes, 'default');
                c.setOptimizationLevel(level);
                const bytes = c.compile(source);
                assert.deepEqual(Buffer.from(bytes), expectedBytes, `fixture ${fixture}, O${level}`);
                assert.equal(c.compileWat(source), expectedWat);
                checkModule(bytes, fixture);
            }
        } finally {
            c.free();
        }
    }

    const diagnostics = Array.from({length: Number(field().toString())}, () => [field().toString(), field().toString()]);
    const hostErrors = [field().toString(), field().toString()];
    assert.equal(offset, input.length);
    const matches = expected => error => error instanceof Error && error.message === expected;
    const c = compiler();
    try {
        for (let level = 0; level < 4; level++) {
            c.setOptimizationLevel(level);
            for (const [source, expected] of diagnostics) {
                assert.throws(() => c.compile(source), matches(expected));
                assert.throws(() => c.compileWat(source), matches(expected));
            }
            assert.throws(() => c.registerHostFunction('bad', 'range', 'for', ['range'], 'void'), matches(hostErrors[0]));
            assert.throws(() => c.registerHostFunction('bad', 'range', 'for', [], 'range'), matches(hostErrors[1]));
        }
    } finally {
        c.free();
    }
}

main().catch(error => { console.error(error); process.exitCode = 1; });
