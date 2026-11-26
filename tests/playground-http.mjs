// Opt-in Linux smoke; no installs or external HTTP. Run from the repository root:
// node tests/playground-http.mjs <binaryen@116.0.0/index.js> <guard-pkg-directory>
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { access, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { corpus, levels } from './m9-corpus.mjs';

const [optimizer, guard, evidence, mode, ...extra] = process.argv.slice(2);
assert.ok(optimizer && guard && (!mode || (evidence && mode === '--measure')) && !extra.length,
    'usage: node tests/playground-http.mjs <binaryen@116.0.0/index.js> <guard-pkg-directory> [matrix-artifact-dir [--measure]]');
const query = mode ? '?measure' : evidence ? '?matrix' : '';
const root = fileURLToPath(new URL('../', import.meta.url));
const pkg = JSON.parse(await readFile(join(dirname(optimizer), 'package.json')));
assert.equal(`${pkg.name}@${pkg.version}`, 'binaryen@116.0.0');
const optional = 'examples/browser/binaryen-116.mjs';
const guardPrefix = 'examples/browser/binaryen-check/';
const postProcessor = 'examples/browser/binaryen-post-process.js';
const paths = new Map(['index.html', 'main.js', 'worker.js', 'binaryen-post-process.js'].map(file => {
    const path = `examples/browser/${file}`;
    return [path, join(root, path)];
}));
paths.set(optional, resolve(optimizer));
paths.set('examples/browser/worker-observer.mjs', join(root, 'tests/playground-worker-observer.mjs'));
for (const file of ['was_compiler.js', 'was_compiler_bg.wasm']) {
    paths.set(`pkg/${file}`, join(root, 'pkg', file));
    paths.set(guardPrefix + file, join(guard, file));
    paths.set(`verifier/${file}`, join(guard, file));
}
await Promise.all([...paths.values()].map(path => access(path)));
const cases = ['off', 'o0', 'missing', 'active'];
const hits = new Map(cases.map(name => [name, new Set()]));
const prefixes = Object.keys(corpus).flatMap(name => [0, ...levels].map(level => `${name}-O${level}`));
const matrixPaths = new Map();
if (evidence) {
    for (const file of ['m9-chromium.mjs', 'm9-corpus.mjs', 'm9-guest-bench.mjs']) matrixPaths.set(`/${file}`, join(root, 'tests', file));
    for (const name of Object.keys(corpus)) matrixPaths.set(`/evidence/${name}.was`, join(root, 'tests/fixtures/m9', `${name}.was`));
    matrixPaths.set('/evidence/native-status.json', join(evidence, 'native-status.json'));
    for (const prefix of prefixes) {
        for (const suffix of ['input', 'native', 'chromium']) {
            const file = `${prefix}-${suffix}.wasm`;
            matrixPaths.set(`/evidence/${file}`, join(evidence, file));
        }
    }
}
const { promise: report, resolve: received, reject: fail } = Promise.withResolvers();
const server = createServer(async (req, res) => {
    try {
        if (req.url === '/result' && req.method === 'POST') {
            let body = '';
            for await (const chunk of req) body += chunk;
            received(JSON.parse(body));
            return res.end();
        }
        // Restrict page and module-worker fetches to this loopback origin.
        res.setHeader('Content-Security-Policy', "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; connect-src 'self' blob:");
        if (req.url === `/${query}`) {
            res.setHeader('Content-Type', 'text/html');
            return res.end('<body><script type="module" src="/driver.js"></script>');
        }
        const [, name, ...parts] = req.url.split('/');
        const path = parts.join('/');
        if (name === 'active' && path === 'examples/browser/observed-worker.mjs') {
            res.setHeader('Content-Type', 'text/javascript');
            return res.end('import "./worker-observer.mjs"; import "./worker.js";');
        }
        const file = req.url === '/driver.js' ? join(root, 'tests/playground-http-page.js')
            : matrixPaths.get(req.url) ?? ((hits.has(name) || (evidence && name === 'matrix')) && paths.get(path));
        if (file && hits.has(name)) hits.get(name).add(path);
        if (!file || (name === 'missing' && path === optional)) return res.writeHead(404).end();
        const mime = file.endsWith('.wasm') ? 'application/wasm'
            : file.endsWith('.html') ? 'text/html'
            : file.endsWith('.mjs') || file.endsWith('.js') ? 'text/javascript' : 'text/plain';
        res.setHeader('Content-Type', mime);
        res.end(await readFile(file));
    } catch (error) { res.destroy(); fail(error); }
});

const profile = await mkdtemp(join(tmpdir(), 'playground-http-'));
let child, deadline;
try {
    server.listen(0, '127.0.0.1');
    await once(server, 'listening');
    child = spawn('chromium', ['--headless', '--disable-background-networking', '--no-first-run',
        `--user-data-dir=${profile}`, `http://127.0.0.1:${server.address().port}/${query}`],
    { detached: true, env: { ...process.env, XDG_CONFIG_HOME: profile, XDG_CACHE_HOME: profile }, stdio: ['ignore', 'ignore', 'pipe'] });
    let stderr = '';
    child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-2000); });
    child.once('error', fail);
    child.once('exit', (code, signal) => fail(new Error(`Chromium exited (${signal ?? code}): ${stderr}`)));
    deadline = setTimeout(() => fail(new Error('Chromium smoke exceeded 90 seconds')), 90_000);
    const { workers: rows, matrix, matrixError } = await report;
    assert.deepEqual(rows.map(([name]) => name), mode ? [] : cases);
    for (const [name, error] of rows) {
        const requests = hits.get(name);
        const optionalLoads = [...requests].filter(path => path === postProcessor || path === optional || path.startsWith(guardPrefix));
        let failure = error;
        if (failure === null) {
            try {
                for (const path of ['examples/browser/worker.js', 'pkg/was_compiler_bg.wasm']) assert.ok(requests.has(path), `not fetched: ${path}`);
                if (name === 'active') assert.deepEqual(new Set(optionalLoads), new Set([postProcessor, optional, guardPrefix + 'was_compiler.js', guardPrefix + 'was_compiler_bg.wasm']));
                else if (name === 'missing') assert.ok(requests.has(optional), 'missing optimizer never requested');
                else assert.deepEqual(optionalLoads, []);
            } catch (error) { failure = error.message; }
        }
        console.log(`${name}: ${failure === null ? 'ok' : `failed: ${failure}`}`);
        if (failure !== null) process.exitCode = 1;
    }
    if (evidence) assert.equal(matrixError, null, matrixError);
    if (mode === '--measure') {
        assert.deepEqual(matrix.map(row => row.level), levels);
        await writeFile(join(evidence, 'chromium-measurements.json'), JSON.stringify(matrix, null, 2) + '\n');
    } else if (evidence) {
        assert.deepEqual(matrix.map(row => row.prefix), prefixes);
        for (const { prefix, error, rejection, output } of matrix) {
            if (output) await writeFile(join(evidence, `${prefix}-chromium.wasm`), Buffer.from(output, 'base64'));
            if (rejection) await writeFile(join(evidence, `${prefix}-chromium-error.txt`), rejection + '\n');
            console.log(`${prefix}: ${error ?? 'ok'} (Chromium)`);
            if (error !== null) process.exitCode = 1;
        }

    }
} finally {
    clearTimeout(deadline);
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
        const stopped = once(child, 'exit');
        process.kill(-child.pid, 'SIGTERM');
        const kill = setTimeout(() => process.kill(-child.pid, 'SIGKILL'), 2000);
        try { await stopped; } finally { clearTimeout(kill); }
    }
    server.closeAllConnections();
    server.close();
    await rm(profile, { recursive: true, force: true, maxRetries: 3 });
}
