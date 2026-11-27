// Explicit, offline execution gate. See docs/validation.md.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const cases = [
    { name: 'scalar', args: ['35'], expected: '65' },
    { name: 'resized', args: [], expected: '65' },
    { name: 'indexed', args: ['1'], expected: null },
];
const timeout = 10_000;
const fuel = 100_000;

function run(tool, args) {
    const result = spawnSync(tool, args, { encoding: 'utf8', timeout, killSignal: 'SIGKILL' });
    if (result.error?.code === 'ENOENT') throw new Error(`tool not found: ${tool}`);
    if (result.error?.code === 'ETIMEDOUT') throw new Error(`${tool} exceeded ${timeout} ms`);
    if (result.error) throw result.error;
    if (result.signal) throw new Error(`${tool} terminated by ${result.signal}`);
    return result;
}

export function checkInvocation({ status, stdout, stderr }, name, expected) {
    const out = stdout.trim();
    if (expected === null) {
        const causes = stderr.split(/\r?\n/).map(line => line.match(/\bwasm trap: (.*)$/)?.[1]).filter(Boolean);
        assert.ok(Number.isInteger(status) && status !== 0 && out === ''
            && stderr.includes(`failed to invoke \`${name}\``)
            && causes.length === 1 && causes[0] === 'wasm `unreachable` instruction executed',
        `${name}: expected an unreachable trap during invocation, got exit ${status}\n${stdout}${stderr}`);
    } else {
        assert.equal(status, 0, `${name}: Wasmtime failed\n${stderr}`);
        assert.equal(out, expected, `${name}: unexpected result`);
    }
}

function main([compiler, wasmtime = 'wasmtime', ...extra]) {
    assert.ok(compiler && extra.length === 0,
        'usage: node tests/wasmtime.mjs <was-compiler executable> [wasmtime executable]');
    const version = run(wasmtime, ['--version']);
    assert.equal(version.status, 0, version.stderr);
    assert.match(version.stdout, /^wasmtime \S+/);
    console.log(version.stdout.trim());

    const source = fileURLToPath(new URL('./fixtures/wasmtime.was', import.meta.url));
    const dir = mkdtempSync(join(tmpdir(), 'was-compiler-wasmtime-'));
    try {
        for (const [preset, flags] of [['O0', ['-O0']], ['default', []]]) {
            const path = join(dir, `${preset}.wasm`);
            const build = run(compiler, [source, ...flags, '--emit', 'wasm', '-o', path]);
            assert.equal(build.status, 0, `${preset}: compiler failed\n${build.stderr}`);
            const module = new WebAssembly.Module(readFileSync(path));
            assert.deepEqual(WebAssembly.Module.imports(module), [], `${preset}: expected raw Wasm without imports`);

            for (const { name, args, expected } of cases) {
                const result = run(wasmtime, ['run', '-C', 'cache=n', '-W', `fuel=${fuel}`, '--invoke', name, path, ...args]);
                checkInvocation(result, name, expected);
                console.log(`${preset} ${name}: ${expected ?? 'expected trap'}`);
            }
        }
    } finally {
        rmSync(dir, { recursive: true, force: true });
    }
}

if (import.meta.main) main(process.argv.slice(2));
