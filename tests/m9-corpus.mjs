// Shared by the native runner and Chromium. These are raw revision-1 ABI checks,
// independent of the compiler's descriptor constants and JavaScript marshaler.
export const corpus = {
    scalar: { exports: ['main'], hosts: [] },
    resizable: { exports: ['main'], hosts: [] },
    'memory-only': { exports: [], hosts: [] },
    contracts: { exports: null, hosts: [
        ['unused', 'z', 'unused', ['bool'], 'bool'],
        ['record', 'a', 'record', ['number'], 'number'],
        ['change', 'a', 'change', ['number[]'], 'number'],
        ['from_host', 'z', 'bool', ['bool'], 'bool'],
    ] },
    'resizable-reentry': { exports: null, hosts: [
        ['change', 'a', 'change', ['number[]', 'bool'], 'number'],
        ['record', 'a', 'record', ['number'], 'number'],
    ] },
};
export const levels = [1, 2, 3];

function check(ok, detail) {
    if (!ok) throw new Error(detail);
}

function trap(fn, detail) {
    try { fn(); } catch (error) {
        if (error instanceof WebAssembly.RuntimeError) return;
        throw error;
    }
    throw new Error(`expected Wasm trap: ${detail}`);
}

export function exercise(name, bytes) {
    const module = new WebAssembly.Module(bytes);
    const imports = WebAssembly.Module.imports(module).map(i => `${i.module}.${i.name}:${i.kind}`);
    const names = WebAssembly.Module.exports(module).map(({ name, kind }) => `${name}:${kind}`);
    const expectedImports = name === 'contracts' ? ['z.unused:function', 'a.record:function', 'a.change:function', 'z.bool:function']
        : name === 'resizable-reentry' ? ['a.change:function', 'a.record:function'] : [];
    check(imports.join() === expectedImports.join(), `wrong ordered imports: ${imports}`);
    if (name === 'resizable-reentry') return exerciseResizableReentry(module, names);
    const seen = [];
    let guestExports;
    function view() { return new DataView(guestExports.memory.buffer); }
    function descriptor(handle) { return [view().getUint32(handle, true), view().getUint32(handle + 4, true)]; }
    guestExports = new WebAssembly.Instance(module, { z: {
        unused() { throw new Error('unused import was called'); },
        bool(n) { check(n === 0 || n === 1, 'noncanonical bool passed to host'); return n * -7; },
    }, a: {
        record(n) { seen.push(n); return n; },
        change(handle) {
            seen.push('change');
            guestExports.overwrite(handle, 40);
            check(view().getFloat64(descriptor(handle)[0], true) === 40, 'host reentry did not mutate the payload');
            return 5;
        },
    } }).exports;
    if (name === 'scalar') {
        check(names.join() === 'main:function' && guestExports.main(25) === 42 && guestExports.main(-3) === 3, 'scalar surface or results');
        return [];
    }
    check(names.slice(-2).join() === 'memory:memory,__alloc:function', 'wrong revision-1 memory exports');
    check(guestExports.memory.buffer.byteLength === 65536, 'wrong initial memory size');
    if (name === 'memory-only') {
        check(names.length === 2, 'unselected function was exported');
        const address = guestExports.__alloc(70000);
        check(address >= 8 && address % 8 === 0 && guestExports.memory.buffer.byteLength === 131072, 'allocation or growth failed');
        return [address];
    }
    if (name === 'resizable') {
        check(names.length === 3 && names[0] === 'main:function', 'resizable export surface');
        const handle = guestExports.main();
        const [data, len] = descriptor(handle);
        check(len === 1 && view().getFloat64(data, true) === 42, 'resizable constructor result');
        return [handle, data, len];
    }
    const exported = ['remainder', 'calculate', 'composition', 'guarded', 'create', 'overwrite', 'update', 'invalid_index', 'read_bool', 'pass_bool', 'words', 'membership', 'short_circuit', 'array_equal', 'hot'];
    check(names.slice(0, -2).join() === exported.map(name => `${name}:function`).join(), 'wrong user exports');
    for (const [lhs, rhs] of [[5.5, 2], [-0, 2], [-4, 2], [NaN, 3], [3, 0], [Infinity, 2], [1, Infinity],
        [Number.MIN_VALUE * 3, Number.MIN_VALUE * 2], [Number.MAX_VALUE, 3]]) {
        seen.length = 0;
        check(Object.is(guestExports.remainder(lhs, rhs), lhs % rhs), 'incorrect floating remainder');
        check(seen.length === 2 && Object.is(seen[0], lhs) && Object.is(seen[1], rhs), 'incorrect host callback order');
    }
    seen.length = 0;
    check(guestExports.composition(2) === 26 && guestExports.composition(1) === 14, 'pipeline, default, range, match or capture result');
    trap(() => guestExports.guarded(2), 'second precondition');
    check(seen.join() === '2,12', 'normalized call effects before contract trap');
    seen.length = 0;
    check(guestExports.membership(3) === 1 && guestExports.membership(2) === 0 && guestExports.membership(5.5) === 0,
        'stepped range membership');
    check(guestExports.short_circuit(0) === 0 && seen.length === 0, 'short-circuited callback ran');
    check(guestExports.short_circuit(3) === 1 && seen.join() === '3', 'non-skipped callback did not run');
    check(guestExports.array_equal(-0, 0) === 1 && guestExports.array_equal(NaN, NaN) === 0,
        'numeric array equality for signed zero and NaN');
    seen.length = 0;
    const values = guestExports.create();
    const [old, initial] = descriptor(values);
    check(values % 8 === 0 && old % 8 === 0 && initial === 2, 'wrong number[] descriptor');
    check(guestExports.update(values) === 9 && seen.join() === 'change', 'compound write or reentry effects');
    const [data, len] = descriptor(values);
    const contents = () => Array.from({ length: len }, (_, i) => view().getFloat64(data + i * 8, true));
    check(data === old && len === 2 && contents().join() === '7,4', 'fixed payload or old-value semantics');
    seen.length = 0;
    trap(() => guestExports.invalid_index(values), 'checked place before RHS');
    trap(() => guestExports.invalid_index(0), 'invalid descriptor before RHS');
    check(seen.length === 0 && contents().join() === '7,4', 'trapping place changed state or called RHS');
    const bools = guestExports.__alloc(16);
    view().setUint32(bools, bools + 8, true);
    view().setUint32(bools + 4, 2, true);
    view().setUint8(bools + 8, 1);
    view().setUint8(bools + 9, 2);
    check(guestExports.read_bool(bools, -0) === 1 && guestExports.pass_bool(1) === 1 && guestExports.pass_bool(0) === 0, 'bool boundary');
    trap(() => guestExports.read_bool(bools, 1), 'consumed invalid bool');
    const words = guestExports.words();
    const beforeGrowth = guestExports.memory.buffer;
    guestExports.__alloc(70000);
    check(guestExports.memory.buffer !== beforeGrowth && contents().join() === '7,4', 'growth lost prior array payload');
    const [handles, count] = descriptor(words);
    const strings = Array.from({ length: count }, (_, i) => {
        const [start, size] = descriptor(view().getUint32(handles + 4 * i, true));
        return new TextDecoder('utf-8', { fatal: true }).decode(new Uint8Array(guestExports.memory.buffer, start, size));
    });
    check(count === 2 && strings.join() === '😀,α!', 'UTF-8 or string[] layout after growth');
    return [values, old, ...descriptor(values), bools, words, handles, guestExports.memory.buffer.byteLength];
}

function exerciseResizableReentry(module, names) {
    check(names.join() === 'create:function,update:function,grow:function,shrink:function,read:function,memory:memory,__alloc:function',
        'wrong resizable reentry export surface');
    const effects = [];
    const { exports } = new WebAssembly.Instance(module, { a: {
        change(handle, grow) {
            check(handle === values, 'callback did not receive the original handle');
            check(grow === 0 || grow === 1, 'callback received a noncanonical bool');
            effects.push(grow ? 'grow' : 'shrink');
            if (grow) {
                const previous = payload([10, 20]);
                check(exports.grow(handle) === values, 'growth changed the handle');
                check(payload([10, 20, 30]) !== previous, 'fixture did not relocate the payload');
            } else {
                check(payload([10, 42, 30]) === relocated, 'pending write did not target the relocated payload');
                check(exports.shrink(handle) === 72, 'reentrant pop returned the wrong values');
                payload([11]);
            }
            effects.push('rhs-ready');
            return 42;
        },
        record(value) { effects.push(value); return value; },
    } });
    check(exports.memory.buffer.byteLength === 65536, 'wrong reentry fixture initial memory size');
    const values = exports.create();
    function payload(expected) {
        const view = new DataView(exports.memory.buffer);
        const data = view.getUint32(values, true);
        const len = view.getUint32(values + 4, true);
        check(len === expected.length && expected.every((value, i) =>
            view.getFloat64(data + i * 8, true) === value), `expected array contents [${expected}]`);
        return data;
    }
    check(exports.update(values, 1) === 42, 'pending write after growth returned the wrong value');
    const relocated = payload([10, 42, 30]);
    trap(() => exports.update(values, 0), 'shrink invalidated the pending index');
    payload([11]);
    const recovered = exports.read(values);
    check(recovered === 12, 'same-instance call after trap did not observe the completed shrink');
    check(effects.join() === 'grow,rhs-ready,42,shrink,rhs-ready', 'wrong callback order or effects after trap');
    return [...effects, recovered];
}
