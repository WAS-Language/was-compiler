use std::{process::Command, time::Duration};

use super::{Config, compile};
use crate::{Compiler, exports::ExportOptions};

const SOURCE: &str = r#"
fn retained() -> number[] { return [17, 29]; }
fn first(values: number[]) -> number { return values[0]; }
fn static_value() -> string { return "static"; }
fn is_static(value: string) -> bool { return value == "static"; }
"#;

fn module() -> Vec<u8> {
    let program = Compiler::new().analyze(SOURCE).unwrap();
    let bytes = compile(&program, &ExportOptions::All, Config::default()).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    bytes
}

fn run(script: &str) {
    let bytes = module();
    let script = format!(
        r#"
        const assert = require('node:assert/strict');
        const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0))).exports;
        const observe = (s, p = 0) => e.__m3_observe(s, p);
        const view = () => new DataView(e.memory.buffer);
        const bytes = () => new Uint8Array(e.memory.buffer);
        const word = p => view().getUint32(p, true);
        const acquire = n => {{ const [p, id] = e.__m3_acquire(BigInt(n)); return [p >>> 0, id]; }};
        const blocks = [];
        function block(n, capacity, value) {{
            const [p, id] = acquire(n);
            const b = {{p, id, n, capacity, expected: Array(n).fill(value)}};
            bytes().set(b.expected, p);
            blocks.push(b);
            return b;
        }}
        function contents(live) {{
            for (const b of live) {{
                assert.deepEqual(Array.from(bytes().slice(b.p, b.p + b.n)), b.expected);
                assert.equal(observe(3, b.p), 1n);
                assert.equal(observe(4, b.p), b.id);
                assert.equal(observe(5, b.p), BigInt(b.capacity));
                assert.equal(observe(6, b.p), BigInt(b.n));
            }}
            for (let i = 0; i < live.length; i++) {{
                for (const b of live.slice(i + 1)) {{
                    const a = live[i];
                    assert.ok(a.p + a.capacity <= b.p || b.p + b.capacity <= a.p,
                        'active rounded extents must not overlap');
                    assert.notEqual(a.id, b.id);
                }}
            }}
        }}
        function freeLists(free) {{
            const seen = new Set();
            for (let c = 0; c < 29; c++) {{
                let record = Number(observe(7, c));
                while (record !== 0) {{
                    assert.ok(!seen.has(record), 'no cycles or duplicate pool entries');
                    seen.add(record);
                    const b = free.find(b => b.p === record + 32);
                    assert.ok(b, 'only expected released blocks enter pools');
                    assert.equal(b.capacity, 8 * 2 ** c);
                    assert.equal(observe(3, b.p), 2n);
                    assert.equal(observe(4, b.p), b.id);
                    assert.equal(observe(5, b.p), BigInt(b.capacity));
                    assert.equal(observe(6, b.p), BigInt(b.n));
                    record = word(record + 4);
                }}
            }}
            assert.equal(seen.size, free.length);
        }}
        function snapshot() {{
            return [observe(0), observe(1), observe(2), observe(8),
                ...Array.from({{length: 29}}, (_, c) => observe(7, c)),
                ...blocks.map(b => [
                    ...[3, 4, 5, 6].map(s => observe(s, b.p)),
                    // Only management records, never stale released payload bytes.
                    Array.from(bytes().slice(b.p - 32, b.p))
                ])];
        }}
        function release(b) {{ e.__m3_release(b.p, b.id); }}
        function recover(live, free) {{
            contents(live);
            freeLists(free);
            const cursor = observe(0), pages = observe(1), population = observe(2);
            let identity = observe(8);
            const pending = new Set(free);
            const acquired = [];
            // Drain all known free capacity simultaneously; do not assume reuse order.
            for (const request of free) {{
                const [p, id] = acquire(request.n);
                const b = [...pending].find(b => b.p === p);
                assert.ok(b, 'reuse must neither grow backing nor duplicate an active block');
                assert.equal(b.capacity, request.capacity);
                pending.delete(b);
                assert.equal(id, ++identity);
                b.id = id;
                b.n = request.n;
                b.expected = Array(b.n).fill(0x93);
                bytes().set(b.expected, p);
                acquired.push(b);
                contents([...live, ...acquired]);
                assert.equal(observe(0), cursor);
                assert.equal(observe(1), pages);
                assert.equal(observe(2), population);
            }}
            assert.equal(pending.size, 0);
            freeLists([]);
            for (const b of acquired) release(b);
            contents(live);
            freeLists(free);
        }}
        function rejected(p, id, live, free, ordinary = () => {{}}) {{
            contents(live);
            freeLists(free);
            const before = snapshot();
            assert.throws(() => e.__m3_release(p, id), WebAssembly.RuntimeError);
            assert.deepEqual(snapshot(), before, 'rejected release must be nonmutating');
            ordinary();
            recover(live, free);
            ordinary();
        }}
        {script}
        "#
    );
    let output = crate::test_process::output(
        Command::new("node").args(["-e", &script]),
        Some(&bytes),
        Duration::from_secs(15),
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn decoded_release_guards_exact_provenance_identity_and_state_before_pool_writes() {
    use wasmparser::{Operator::*, Parser, Payload};

    let bytes = module();
    let mut release = None;
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            Payload::ImportSection(imports) => assert_eq!(imports.count(), 0),
            Payload::ExportSection(exports) => {
                for export in exports {
                    let export = export.unwrap();
                    if export.name == "__m3_release" {
                        release = Some(usize::try_from(export.index).unwrap());
                    }
                }
            }
            Payload::CodeSectionEntry(body) => {
                bodies.push(
                    body.get_operators_reader()
                        .unwrap()
                        .into_iter()
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap(),
                );
            }
            _ => {}
        }
    }
    let release = &bodies[release.unwrap()];
    // This focused sequence assertion includes the conditional traps, not just the
    // presence of loads/checks somewhere before a store. Offsets are ABI oracles.
    let [
        LocalGet { local_index: 0 },
        Call {
            function_index: find,
        },
        LocalTee { local_index: 2 },
        I32Eqz,
        If {
            blockty: provenance_guard,
        },
        Unreachable,
        End,
        LocalGet { local_index: 2 },
        I64Load { memarg: identity },
        LocalGet { local_index: 1 },
        I64Ne,
        LocalGet { local_index: 2 },
        I32Load { memarg: active },
        I32Const { value: 1 },
        I32Ne,
        I32Or,
        If {
            blockty: authority_guard,
        },
        Unreachable,
        End,
        LocalGet { local_index: 2 },
        I32Load { memarg: class },
        LocalSet { local_index: 3 },
        LocalGet { local_index: 2 },
        LocalGet { local_index: 3 },
        Call {
            function_index: head,
        },
        I32Store { memarg: next_free },
        LocalGet { local_index: 2 },
        I32Const { value: 0 },
        I32Store { memarg: state },
        LocalGet { local_index: 3 },
        LocalGet { local_index: 2 },
        Call {
            function_index: set_head,
        },
        End,
    ] = release.as_slice()
    else {
        panic!(
            "release must validate registry membership and authority before pool mutation: {release:?}"
        );
    };
    assert_eq!(*provenance_guard, wasmparser::BlockType::Empty);
    assert_eq!(*authority_guard, wasmparser::BlockType::Empty);
    for (memarg, offset, align) in [
        (identity, 16, 3),
        (active, 12, 2),
        (class, 8, 2),
        (next_free, 4, 2),
        (state, 12, 2),
    ] {
        assert_eq!(
            (memarg.offset, memarg.align, memarg.memory),
            (offset, align, 0)
        );
    }
    let lookup = &bodies[usize::try_from(*find).unwrap()];
    let [
        GlobalGet { global_index: _ },
        LocalSet { local_index: 1 },
        Block { blockty: _ },
        Loop { blockty: _ },
        LocalGet { local_index: 1 },
        I32Eqz,
        BrIf { relative_depth: 1 },
        LocalGet { local_index: 1 },
        I32Const { value: 32 },
        I32Add,
        LocalGet { local_index: 0 },
        I32Eq,
        If { blockty: _ },
        LocalGet { local_index: 1 },
        Return,
        End,
        LocalGet { local_index: 1 },
        I32Load {
            memarg: registry_next,
        },
        LocalSet { local_index: 1 },
        Br { relative_depth: 0 },
        End,
        End,
        I32Const { value: 0 },
        End,
    ] = lookup.as_slice()
    else {
        panic!(
            "lookup must traverse registered links, match exact payloads and terminate at null: {lookup:?}"
        );
    };
    assert_eq!(
        (
            registry_next.offset,
            registry_next.align,
            registry_next.memory
        ),
        (0, 2, 0)
    );
    // The only release callees are lookup, the read-only head dispatch and the
    // final head setter. In particular lookup cannot interpret caller-adjacent bytes.
    assert_ne!(find, head);
    assert_ne!(head, set_head);
    let head = &bodies[usize::try_from(*head).unwrap()];
    assert!(head.iter().all(|op| matches!(
        op,
        LocalGet { .. }
            | I32Const { .. }
            | I32Eq
            | If { .. }
            | GlobalGet { .. }
            | Return
            | End
            | Unreachable
    )));
    let setter = &bodies[usize::try_from(*set_head).unwrap()];
    assert!(setter.iter().all(|op| matches!(
        op,
        LocalGet { .. }
            | I32Const { .. }
            | I32Eq
            | If { .. }
            | GlobalSet { .. }
            | Return
            | End
            | Unreachable
    )));
}

#[test]
#[ignore = "requires Node.js"]
fn unmanaged_descriptors_boundaries_and_forged_interior_headers_have_no_authority() {
    run(r#"
        const staticHandle = e.static_value() >>> 0;
        const retained = e.retained() >>> 0;
        const retainedData = word(retained);
        const hostHandle = e.__alloc(8) >>> 0;
        const hostData = e.__alloc(16) >>> 0;
        view().setUint32(hostHandle, hostData, true);
        view().setUint32(hostHandle + 4, 2, true);
        view().setFloat64(hostData, 41, true);
        view().setFloat64(hostData + 8, 53, true);
        const fakeHost = e.__alloc(48) >>> 0;
        bytes().fill(0, fakeHost, fakeHost + 48);
        // Plausible management bytes outside the registry confer no authority.
        view().setUint32(fakeHost + 8, 1, true);
        view().setUint32(fakeHost + 12, 1, true);
        view().setBigUint64(fakeHost + 16, 1n, true);
        view().setBigUint64(fakeHost + 24, 16n, true);
        const ordinary = () => {
            assert.equal(e.is_static(staticHandle), 1);
            assert.equal(word(staticHandle + 4), 6);
            assert.deepEqual(Array.from(bytes().slice(word(staticHandle), word(staticHandle) + 6)),
                [115, 116, 97, 116, 105, 99]);
            assert.equal(e.first(retained), 17);
            assert.equal(word(retained), retainedData);
            assert.equal(word(retained + 4), 2);
            assert.equal(view().getFloat64(retainedData + 8, true), 29);
            assert.equal(e.first(hostHandle), 41);
            assert.equal(word(hostHandle), hostData);
            assert.equal(word(hostHandle + 4), 2);
            assert.equal(view().getFloat64(hostData + 8, true), 53);
        };
        ordinary();
        const live = block(128, 128, 0x37);
        const firstFree = block(9, 16, 0x48);
        const secondFree = block(15, 16, 0x59);
        const guard = block(8, 8, 0x6a);
        release(firstFree); release(secondFree);
        const free = [firstFree, secondFree];
        // Forge a complete header inside initialized live payload, not real metadata.
        // Build the expected bytes independently rather than snapshotting guest bytes.
        const expected = new Uint8Array(live.expected);
        const header = new DataView(expected.buffer);
        header.setUint32(32, 0, true);
        header.setUint32(36, 0, true);
        header.setUint32(40, 1, true);
        header.setUint32(44, 1, true);
        header.setBigUint64(48, live.id, true);
        header.setBigUint64(56, 16n, true);
        live.expected = Array.from(expected);
        bytes().set(live.expected, live.p);
        const end = e.memory.buffer.byteLength;
        for (const p of [
            0, 1, 7, 8, 0x7fffffff, 0x80000000, 0xfffffff8, 0xffffffff,
            end - 8, end, end + 8,
            staticHandle, word(staticHandle), retained, retainedData, hostHandle, hostData,
            fakeHost + 32, live.p - 32, live.p - 1, live.p + 1, live.p + 8,
            live.p + 64, live.p + 127, live.p + 128, firstFree.p + 1, secondFree.p + 8
        ]) {
            for (const s of [3, 4, 5, 6]) assert.equal(observe(s, p), 0n);
            rejected(p, live.id, [live, guard], free, ordinary);
        }
        release(live); release(guard);
        freeLists(blocks);
        "#);
}

#[test]
#[ignore = "requires Node.js"]
fn registry_positions_reject_wrong_double_and_aba_authority_in_same_instance() {
    run(r#"
        // Fresh records prepend: tail, middle, head. No later fresh M3 allocation.
        const tail = block(9, 16, 0x21);
        const middle = block(13, 16, 0x42);
        const head = block(15, 16, 0x63);
        assert.equal(word(head.p - 32), middle.p - 32);
        assert.equal(word(middle.p - 32), tail.p - 32);
        assert.equal(word(tail.p - 32), 0);
        const live = [tail, middle, head], free = [];
        function acquireAllFree(value) {
            while (free.length) {
                const request = free[0];
                const [p, id] = acquire(request.n);
                const b = free.find(b => b.p === p);
                assert.ok(b);
                b.id = id; b.n = request.n; b.expected = Array(b.n).fill(value);
                bytes().set(b.expected, p);
                free.splice(free.indexOf(b), 1); live.push(b);
            }
        }
        for (const target of [tail, middle, head]) {
            acquireAllFree(0x74);
            // Keep a known free block available at every trap.
            const spare = live.find(b => b !== target);
            release(spare); live.splice(live.indexOf(spare), 1); free.push(spare);
            rejected(target.p, 0n, live, free);
            rejected(target.p, -1n, live, free);
            rejected(target.p, live.find(b => b !== target).id, live, free);
            const obsolete = target.id;
            release(target); live.splice(live.indexOf(target), 1); free.push(target);
            rejected(target.p, obsolete, live, free);
            // Recovery renewed every free block, including target, then released it.
            rejected(target.p, target.id, live, free);
            acquireAllFree(0xa6);
            const other = live.find(b => b !== target);
            release(other); live.splice(live.indexOf(other), 1); free.push(other);
            rejected(target.p, obsolete, live, free);
            assert.equal(observe(3, target.p), 1n);
            contents(live);
        }
        for (const b of live) release(b);
        freeLists(blocks);
        assert.equal(observe(2), 3n);
        "#);
}
