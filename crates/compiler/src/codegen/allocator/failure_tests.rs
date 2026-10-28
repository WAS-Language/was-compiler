use std::{process::Command, time::Duration};

use super::{Config, compile};
use crate::{Compiler, exports::ExportOptions};
use wasm_encoder::{ConstExpr, GlobalSection, GlobalType, MemorySection, MemoryType, RawSection};
use wasmparser::{Operator, Parser, Payload, Validator};

#[test]
#[ignore = "requires Node.js"]
fn rejected_request_domain_preserves_all_free_classes_and_live_contents() {
    run(
        &bounded_module(SOURCE, Config::default(), None),
        r#"
        const [a, aid] = acquire(8n);
        const [b, bid] = acquire(0n);
        const [c, cid] = acquire(16n);
        blocks.push(a, b, c);
        fill(a, 8, 0x61);
        e.__m3_release(b, bid);
        e.__m3_release(c, cid);
        const initial = snapshot();
        for (const [request, expected] of [[0n, 0], [1n, 0], [2147483647n, 28], [2147483648n, 28]]) {
            assert.equal(e.__m3_class(request), expected);
            assert.deepEqual(snapshot(), initial);
        }
        for (const request of [-9223372036854775808n, -1n, 2147483649n,
                               4294967295n, 4294967296n, 9223372036854775807n]) {
            assert.throws(() => e.__m3_class(request), WebAssembly.RuntimeError);
            rejected(request);
            check(a, 8, 0x61);
            // Drain both free classes after every failure in the same instance.
            const cursor = observe(0), population = observe(2);
            const [x, xid] = acquire(0n), [y, yid] = acquire(9n);
            assert.equal(x, b);
            assert.equal(y, c);
            assert.ok(x + 8 <= y || y + 16 <= x);
            assert.ok(a + 8 <= x || x + 8 <= a);
            assert.ok(a + 8 <= y || y + 16 <= a);
            fill(x, 8, 0x72);
            fill(y, 9, 0x83);
            check(a, 8, 0x61);
            assert.equal(observe(0), cursor);
            assert.equal(observe(2), population);
            e.__m3_release(x, xid);
            e.__m3_release(y, yid);
        }
        e.__m3_release(a, aid);
        assert.equal(observe(3, a), 2n);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn exhausted_identity_never_consumes_free_or_fresh_storage_even_after_releases() {
    run(
        &bounded_module(
            SOURCE,
            Config {
                last_identity: u64::MAX - 3,
                backing_limit: None,
            },
            None,
        ),
        r#"
        const [a, aid] = acquire(8n);
        const [b, bid] = acquire(8n);
        blocks.push(a, b);
        fill(a, 8, 0x41);
        e.__m3_release(b, bid);
        const [again, last] = acquire(0n);
        assert.equal(again, b);
        assert.equal(BigInt.asUintN(64, aid), 18446744073709551613n);
        assert.equal(BigInt.asUintN(64, bid), 18446744073709551614n);
        assert.equal(BigInt.asUintN(64, last), 18446744073709551615n);
        fill(again, 8, 0x52);
        e.__m3_release(again, last);
        for (let i = 0; i < 3; i++) {
            rejected(8n);  // Matching free block must not be removed.
            rejected(9n);  // Empty class must not reserve backing.
            check(a, 8, 0x41);
            assert.equal(observe(3, a), 1n);
            assert.equal(observe(3, b), 2n);
            assert.equal(observe(7, 0), BigInt(b - 32));
        }
        e.__m3_release(a, aid); // Release does not require another identity.
        assert.equal(observe(3, a), 2n);
        assert.equal(observe(7, 0), BigInt(a - 32));
        assert.equal(view().getUint32(a - 28, true), b - 32);
        rejected(0n);
        rejected(16n);
        assert.equal(BigInt.asUintN(64, observe(8)), 18446744073709551615n);
        assert.equal(observe(2), 2n);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn metadata_plus_payload_address_limit_traps_without_large_memory_or_partial_commit() {
    // Ordinary maximum is 65,535 pages (4,294,901,760 bytes). Even a zero
    // request needs a 32-byte record plus eight-byte payload, not just eight.
    run(
        &bounded_module(SOURCE, Config::default(), Some(-65544)),
        r#"
        assert.equal(observe(0), 4294901752n);
        assert.equal(observe(1), 1n);
        for (const request of [0n, 8n, 2147483647n, 2147483648n, 0n]) rejected(request);
        assert.equal(observe(2), 0n);
        assert.equal(observe(8), 0n);
        assert.equal(observe(1), 1n, 'address failure precedes any giant memory.grow');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn bounded_memory_preserves_a_multi_page_static_minimum() {
    let source = format!(
        "fn retained() -> string {{ return \"{}\"; }}",
        "x".repeat(70000)
    );
    run(
        &bounded_module(&source, Config::default(), None),
        r#"
        assert.equal(observe(1), 2n);
        const p = e.retained() >>> 0;
        assert.equal(view().getUint32(p + 4, true), 70000);
        const data = view().getUint32(p, true);
        check(data, 70000, 120);
        const [a, id] = acquire(65536n);
        fill(a, 65536, 0x74);
        assert.equal(observe(1), 3n);
        blocks.push(a);
        rejected(131072n);
        check(a, 65536, 0x74);
        check(data, 70000, 120);
        e.__m3_release(a, id);
        "#,
    );
}

const SOURCE: &str = "fn retained() -> number[] { return [17, 29]; }";

#[test]
#[ignore = "requires Node.js"]
fn completed_release_survives_later_traps_while_active_blocks_are_never_swept() {
    use crate::{ArrayElementType, Type};

    let mut compiler = Compiler::new();
    compiler
        .register_host_function(
            "effect",
            "env",
            "effect",
            vec![Type::Array(ArrayElementType::Number)],
            Type::Void,
        )
        .unwrap();
    let program = compiler
        .analyze(
            r#"
        fn retained() -> number[] { return [17, 29]; }
        fn fail(values: number[]) -> number {
            values[0] = 61;
            effect(values);
            return values[2];
        }
    "#,
        )
        .unwrap();
    let bytes = compile(&program, &ExportOptions::All, Config::default()).unwrap();
    run(
        &bounded_bytes(&bytes, None),
        r#"
        const ordinary = e.retained() >>> 0;
        const payload = view().getUint32(ordinary, true);
        const held = [];
        let effects = 0;
        let b, bid;
        // The callback completes a release while an ordinary guest invocation
        // is suspended. Returning then traps on the guest's checked index load.
        onEffect = p => {
            assert.equal(p, ordinary);
            assert.equal(view().getFloat64(payload, true), 61);
            effects++;
            const [a, aid] = acquire(8n);
            [b, bid] = acquire(8n);
            blocks.push(a);
            if (!blocks.includes(b)) blocks.push(b);
            fill(a, 8, 0x30 + effects);
            fill(b, 8, 0x70);
            held.push([a, aid, 0x30 + effects]);
            e.__m3_release(b, bid);
        };
        for (let i = 0; i < 4; i++) {
            assert.throws(() => e.fail(ordinary), WebAssembly.RuntimeError);
            assert.equal(effects, i + 1, 'completed host effects are not rolled back');
            assert.equal(view().getFloat64(payload, true), 61);
            assert.equal(view().getFloat64(payload + 8, true), 29);
            assert.equal(observe(3, b), 2n);
            const before = observe(0);
            const [reuse, id] = acquire(8n);
            assert.equal(reuse, b, 'completed release stays available across the trap');
            assert.equal(observe(0), before);
            fill(reuse, 8, 0x81);
            for (const [a, aid, value] of held) {
                assert.notEqual(reuse, a, 'active-at-trap allocations never enter the pool');
                assert.equal(observe(3, a), 1n);
                assert.equal(observe(4, a), aid);
                check(a, 8, value);
            }
            assert.equal(new Set(held.map(([p]) => p)).size, held.length);
            e.__m3_release(reuse, id);
            // Repeated acquisition traps also leave all retained blocks alone.
            rejected(-1n);
            rejected(2147483648n);
        }
        assert.equal(observe(2), 5n, 'finite sequence retains four active blocks plus one reusable block');
        // Further successful calls neither sweep nor reclaim the held blocks.
        const next = e.retained() >>> 0;
        assert.notEqual(next, ordinary);
        for (const [a, aid, value] of held) {
            assert.equal(observe(3, a), 1n);
            check(a, 8, value);
        }
        "#,
    );
}

// Change only the memory maximum, retaining the module's actual minimum. The
// optional cursor seed reaches address overflow without reserving multi-GiB memory.
fn bounded_module(source: &str, config: Config, cursor: Option<i32>) -> Vec<u8> {
    let program = Compiler::new().analyze(source).unwrap();
    let bytes = compile(&program, &ExportOptions::All, config).unwrap();
    bounded_bytes(&bytes, cursor)
}

fn bounded_bytes(bytes: &[u8], cursor: Option<i32>) -> Vec<u8> {
    let mut module = wasm_encoder::Module::new();
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.unwrap();
        match payload {
            Payload::MemorySection(reader) => {
                assert_eq!(reader.count(), 1);
                let original = reader.into_iter().next().unwrap().unwrap();
                assert!(!original.memory64 && !original.shared);
                let mut memory = MemorySection::new();
                memory.memory(MemoryType {
                    minimum: original.initial,
                    maximum: Some(original.initial + 1),
                    memory64: original.memory64,
                    shared: original.shared,
                    page_size_log2: original.page_size_log2,
                });
                module.section(&memory);
            }
            Payload::GlobalSection(reader) if cursor.is_some() => {
                let mut globals = GlobalSection::new();
                for (index, global) in reader.into_iter().enumerate() {
                    let global = global.unwrap();
                    let mut ops = global.init_expr.get_operators_reader();
                    let init = match ops.read().unwrap() {
                        Operator::I32Const { value } => {
                            ConstExpr::i32_const(if index == 0 { cursor.unwrap() } else { value })
                        }
                        Operator::I64Const { value } => ConstExpr::i64_const(value),
                        other => panic!("unexpected fixture initializer: {other:?}"),
                    };
                    assert!(matches!(ops.read().unwrap(), Operator::End));
                    assert!(ops.eof());
                    globals.global(
                        GlobalType {
                            val_type: match global.ty.content_type {
                                wasmparser::ValType::I32 => wasm_encoder::ValType::I32,
                                wasmparser::ValType::I64 => wasm_encoder::ValType::I64,
                                other => panic!("unexpected fixture global: {other:?}"),
                            },
                            mutable: global.ty.mutable,
                            shared: global.ty.shared,
                        },
                        &init,
                    );
                }
                module.section(&globals);
            }
            other => {
                if let Some((id, range)) = other.as_section() {
                    module.section(&RawSection {
                        id,
                        data: &bytes[usize::try_from(range.start).unwrap()
                            ..usize::try_from(range.end).unwrap()],
                    });
                }
            }
        }
    }
    let bytes = module.finish();
    Validator::new().validate_all(&bytes).unwrap();
    bytes
}

fn run(bytes: &[u8], script: &str) {
    let script = format!(
        r#"
        const assert = require('node:assert/strict');
        let onEffect = () => {{}};
        const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {{env: {{effect: p => onEffect(p >>> 0)}}}}).exports;
        const observe = (s, p = 0) => e.__m3_observe(s, p);
        const acquire = n => {{ const [p, id] = e.__m3_acquire(n); return [p >>> 0, id]; }};
        const view = () => new DataView(e.memory.buffer);
        const bytes = () => new Uint8Array(e.memory.buffer);
        const fill = (p, n, value) => bytes().fill(value, p, p + n);
        const check = (p, n, value) => assert.deepEqual(Array.from(bytes().slice(p, p + n)), Array(n).fill(value));
        const blocks = [];
        function snapshot() {{
            return [observe(0), observe(1), observe(2), observe(8),
                ...Array.from({{length: 29}}, (_, c) => observe(7, c)),
                ...blocks.map(p => [
                    ...[3, 4, 5, 6].map(s => observe(s, p)),
                    Array.from(bytes().slice(p - 32, p))
                ])];
        }}
        function rejected(request) {{
            const before = snapshot();
            assert.throws(() => acquire(request), WebAssembly.RuntimeError);
            assert.deepEqual(snapshot(), before, 'failed acquisition must not commit metadata or consume free capacity');
        }}
        {script}
        "#
    );
    let output = crate::test_process::output(
        Command::new("node").args(["-e", &script]),
        Some(bytes),
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
#[ignore = "requires Node.js"]
fn real_backing_growth_failure_preserves_live_free_and_unpublished_storage() {
    let bytes = bounded_module(SOURCE, Config::default(), None);
    run(
        &bytes,
        r#"
        const retained = e.retained() >>> 0;
        const ordinary = view().getUint32(retained, true);
        const [a, aid] = acquire(13n);
        const [b, bid] = acquire(7n);
        const [large, lid] = acquire(65536n);
        blocks.push(a, b, large);
        fill(a, 13, 0x31);
        fill(b, 7, 0x42);
        fill(large, 65536, 0x53);
        assert.equal(observe(1), 2n, 'successful real growth precedes failed growth');
        e.__m3_release(b, bid);
        const cursor = observe(0);
        const unreserved = Number(cursor);
        bytes().fill(0xa7, unreserved, unreserved + 64);
        for (const request of [131072n, 2147483647n, 2147483648n, 131072n]) {
            rejected(request);
            check(unreserved, 64, 0xa7);
            check(a, 13, 0x31);
            check(large, 65536, 0x53);
            assert.equal(view().getFloat64(ordinary, true), 17);
            assert.equal(view().getFloat64(ordinary + 8, true), 29);
            assert.equal(observe(3, a), 1n);
            assert.equal(observe(3, large), 1n);
            assert.equal(observe(3, b), 2n);
        }
        assert.equal(observe(2), 3n);
        assert.equal(observe(8), 3n, 'failures consume no identity');
        const [again, id] = acquire(0n);
        assert.equal(again, b);
        assert.equal(id, 4n);
        assert.equal(observe(0), cursor);
        assert.equal(observe(2), 3n);
        fill(again, 8, 0x64);
        assert.ok(a + 16 <= again || again + 8 <= a);
        assert.ok(large + 65536 <= again || again + 8 <= large);
        check(a, 13, 0x31);
        check(large, 65536, 0x53);
        e.__m3_release(a, aid);
        e.__m3_release(again, id);
        e.__m3_release(large, lid);
        assert.equal(observe(1), 2n, 'release does not shrink grown memory');
        "#,
    );
}
