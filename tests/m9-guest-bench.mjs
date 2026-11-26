export function guestSamples(bytes) {
    const module = new WebAssembly.Module(bytes);
    const { hot } = new WebAssembly.Instance(module, {
        z: { unused() {}, bool() {} }, a: { record() {}, change() {} },
    }).exports;
    function batch() {
        let sum = 0;
        for (let i = 0; i < 2000; i++) sum += hot(i);
        if (sum !== 3399000) throw new Error(`unexpected hot result: ${sum}`);
    }
    batch();
    return Array.from({ length: 7 }, () => {
        const start = performance.now();
        batch();
        return performance.now() - start;
    });
}

if (import.meta.main) {
    const { readFile, writeFile } = await import('node:fs/promises');
    const { join } = await import('node:path');
    const { levels } = await import('./m9-corpus.mjs');
    const [, , dir, ...extra] = process.argv;
    if (!dir || extra.length) throw new Error('usage: node tests/m9-guest-bench.mjs <artifact-dir>');
    const measurements = [];
    for (const level of levels) {
        const prefix = `contracts-O${level}`;
        const input = await readFile(join(dir, `${prefix}-input.wasm`));
        const output = await readFile(join(dir, `${prefix}-native.wasm`));
        measurements.push({ level, offMs: guestSamples(input), onMs: guestSamples(output) });
    }
    await writeFile(join(dir, 'node-guest.json'), JSON.stringify(measurements, null, 2) + '\n');
}
