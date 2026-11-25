// Opt-in Linux smoke; no installs or external HTTP. Run from the repository root:
// node tests/playground-http.mjs <binaryen@116.0.0/index.js> <guard-pkg-directory>
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { access, mkdtemp, readFile, rm } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const [optimizer, guard] = process.argv.slice(2);
assert.ok(process.argv.length === 4, 'usage: node tests/playground-http.mjs <binaryen@116.0.0/index.js> <guard-pkg-directory>');
const root = fileURLToPath(new URL('../', import.meta.url));
const pkg = JSON.parse(await readFile(join(dirname(optimizer), 'package.json')));
assert.equal(`${pkg.name}@${pkg.version}`, 'binaryen@116.0.0');
const optional = 'examples/browser/binaryen-116.mjs';
const guardPrefix = 'examples/browser/binaryen-check/';
const paths = new Map(['index.html', 'main.js', 'worker.js'].map(file => {
    const path = `examples/browser/${file}`;
    return [path, join(root, path)];
}));
paths.set(optional, resolve(optimizer));
for (const file of ['was_compiler.js', 'was_compiler_bg.wasm']) {
    paths.set(`pkg/${file}`, join(root, 'pkg', file));
    paths.set(guardPrefix + file, join(guard, file));
        paths.set(`verifier/${file}`, join(guard, file));
}
await Promise.all([...paths.values()].map(path => access(path)));
const cases = ['off', 'o0', 'missing', 'active'];
const hits = new Map(cases.map(name => [name, new Set()]));
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
        if (req.url === '/') {
            res.setHeader('Content-Type', 'text/html');
            return res.end('<body><script type="module" src="/driver.js"></script>');
        }
        const [, name, ...parts] = req.url.split('/');
        const path = parts.join('/');
        const file = req.url === '/driver.js' ? join(root, 'tests/playground-http-page.js') : hits.has(name) && paths.get(path);
        if (file && hits.has(name)) hits.get(name).add(path);
        if (!file || (name === 'missing' && path === optional)) return res.writeHead(404).end();
        const mime = file.endsWith('.wasm') ? 'application/wasm' : file.endsWith('.html') ? 'text/html' : 'text/javascript';
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
        `--user-data-dir=${profile}`, `http://127.0.0.1:${server.address().port}/`],
    { detached: true, env: { ...process.env, XDG_CONFIG_HOME: profile, XDG_CACHE_HOME: profile }, stdio: ['ignore', 'ignore', 'pipe'] });
    let stderr = '';
    child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-2000); });
    child.once('error', fail);
    child.once('exit', (code, signal) => fail(new Error(`Chromium exited (${signal ?? code}): ${stderr}`)));
    deadline = setTimeout(() => fail(new Error('Chromium smoke exceeded 90 seconds')), 90_000);
    const rows = await report;
    assert.deepEqual(rows.map(([name]) => name), cases);
    for (const [name, error] of rows) {
        const requests = hits.get(name);
        const optionalLoads = [...requests].filter(path => path === optional || path.startsWith(guardPrefix));
        let failure = error;
        if (failure === null) {
            try {
                for (const path of ['examples/browser/worker.js', 'pkg/was_compiler_bg.wasm']) assert.ok(requests.has(path), `not fetched: ${path}`);
                if (name === 'active') assert.deepEqual(new Set(optionalLoads), new Set([optional, guardPrefix + 'was_compiler.js', guardPrefix + 'was_compiler_bg.wasm']));
                else if (name === 'missing') assert.ok(requests.has(optional), 'missing optimizer never requested');
                else assert.deepEqual(optionalLoads, []);
            } catch (error) { failure = error.message; }
        }
        console.log(`${name}: ${failure === null ? 'ok' : `failed: ${failure}`}`);
        if (failure !== null) process.exitCode = 1;
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
