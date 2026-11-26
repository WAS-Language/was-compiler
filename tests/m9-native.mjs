// Run from the project root with feature-disabled and feature-enabled CLI binaries.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, unlinkSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { corpus, exercise, levels } from './m9-corpus.mjs';

assert.equal(process.argv.length, 5, 'usage: node tests/m9-native.mjs <ordinary-cli> <binaryen-cli> <fresh-artifact-dir>');
const [ordinary, enabled, out] = process.argv.slice(2);
mkdirSync(out);
const inheritedSettings = new Set(['BINARYEN_CORES', 'BINARYEN_PASS_DEBUG', 'BINARYEN_PRINT_FULL']);
const env = Object.fromEntries(Object.entries(process.env).filter(([name]) => !inheritedSettings.has(name.toUpperCase())));
env.BINARYEN_CORES = '1';
env.BINARYEN_PASS_DEBUG = '0';
const options = { timeout: 20_000, killSignal: 'SIGKILL', stdio: ['ignore', 'pipe', 'pipe'], env };
const status = {};
for (const [name, { exports, hosts }] of Object.entries(corpus)) {
    const flags = hosts.map(([name, module, field, params, result]) => `--host=${name}=${module}.${field}(${params.join(',')})->${result}`);
    if (exports?.length === 0) flags.push('--no-exports');
    else for (const name of exports ?? []) flags.push('--export', name);
    const source = `tests/fixtures/m9/${name}.was`;
    for (const level of [0, ...levels]) {
        const prefix = `${name}-O${level}`;
        function compile(cli, output, ...activation) {
            execFileSync(cli, [source, `-O${level}`, ...flags, ...activation, '-o', output], options);
            return readFileSync(output);
        }
        const inputPath = join(out, `${prefix}-input.wasm`);
        const nativePath = join(out, `${prefix}-native.wasm`);
        const input = compile(ordinary, inputPath);
        assert.deepEqual(compile(enabled, nativePath), input, `${prefix}: inactive enabled build`);
        unlinkSync(nativePath);
        const before = exercise(name, input);
        let active;
        try {
            active = compile(enabled, nativePath, '--binaryen');
        } catch (error) {
            assert.equal(error.status, 1, `${prefix}: abnormal CLI failure`);
            assert.ok(!existsSync(nativePath), `${prefix}: failed request published an artifact`);
            writeFileSync(join(out, `${prefix}-native-error.txt`), error.stderr.toString().replace(/[ \t]+$/gm, ''));
            status[prefix] = 'rejected';
            process.exitCode = 1;
            console.log(`${prefix}: ${input.length} bytes -> rejected (native)`);
            continue;
        }
        status[prefix] = 'published';
        if (level === 0) {
            assert.deepEqual(active, input, `${prefix}: O0 bypass`);
        } else {
            assert.deepEqual(compile(enabled, nativePath, '--binaryen'), active, `${prefix}: repeated native output`);
        }
        assert.deepEqual(exercise(name, active), before, `${prefix}: native ABI observations`);
        console.log(`${prefix}: ${input.length} -> ${active.length} bytes (native)`);
    }
}
writeFileSync(join(out, 'native-status.json'), JSON.stringify(status, null, 2) + '\n');
