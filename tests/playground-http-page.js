// Runs inside Chromium. Each frame loads the unmodified playground and starts its own worker.
const results = [];
let ordinaryO1;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
function check(condition, message) { if (!condition) throw new Error(message); }

for (const [name, requested, level] of [['off', false, 1], ['o0', true, 0], ['missing', true, 1], ['active', true, 1]]) {
    const frame = document.createElement('iframe');
    try {
        const loaded = new Promise(resolve => { frame.onload = resolve; });
        frame.src = `/${name}/examples/browser/index.html`;
        document.body.append(frame);
        await loaded;
        const doc = frame.contentDocument;
        const el = id => doc.getElementById(id);
        const checkbox = [...doc.querySelectorAll('input[type=checkbox]')].find(input =>
            [...input.labels].some(label => /binaryen/i.test(label.textContent)));
        check(checkbox || !requested, 'missing playground Binaryen checkbox');
        if (checkbox) {
            check(!checkbox.checked, 'Binaryen must default to off');
            if (requested) checkbox.click();
        }
        el('optimization').value = String(level);
        el('host-print').checked = el('show-wat').checked = true;
        el('source').value = 'fn main() -> number { let a = [40, 2]; return a[0] + a[1]; }';
        el('run').click();
        check(el('run').disabled, 'page did not start worker');
        while (el('run').disabled) await sleep(30);
        const output = el('output').textContent;
        if (name === 'missing') {
            check(/^Binaryen post-processing: \S/.test(output), `wrong failure phase: ${output}`);
            check(el('download').disabled, 'failed run published a download');
        } else {
            check(output.includes('main() → 42') && output.includes('(no host calls)'), output);
            check(!el('download').disabled, 'no download');
            // Read the page's download without writing it to the user's Downloads directory.
            let url;
            doc.addEventListener('click', event => {
                if (event.target.matches('a[download]')) {
                    event.preventDefault();
                    url = event.target.href;
                }
            });
            el('download').click();
            check(url?.startsWith('blob:'), 'no Wasm blob');
            const bytes = new Uint8Array(await (await fetch(url)).arrayBuffer());
            const imports = WebAssembly.Module.imports(await WebAssembly.compile(bytes));
            check(imports.some(i => i.module === 'env' && i.name === 'log_number' && i.kind === 'function'), 'download lost unused import');
            if (name === 'off') ordinaryO1 = bytes;
            if (name === 'active') {
                check(ordinaryO1 && (bytes.length !== ordinaryO1.length ||
                    bytes.some((b, i) => b !== ordinaryO1[i])), 'active download unchanged from ordinary O1');
                // Separate URLs keep verifier loads out of the worker's guard request count.
                const guard = await import('/active/verifier/was_compiler.js');
                await guard.default();
                check(el('wat').textContent === guard.checkBinaryenOutput(bytes, bytes, true),
                    'displayed WAT differs from the downloaded Wasm');
            }
        }
        results.push([name, null]);
    } catch (error) {
        results.push([name, error?.message ?? String(error)]);
    } finally {
        frame.remove();
    }
}
await fetch('/result', { method: 'POST', body: JSON.stringify(results) });
