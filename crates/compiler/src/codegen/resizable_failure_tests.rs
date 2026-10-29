//! Bounded M4 failure injection; no production compilation path selects this transport.
use super::*;
use crate::{Compiler, exports::ExportOptions};
use std::{process::Command, time::Duration};
use wasm_encoder::{ExportKind, ExportSection, Module, RawSection};
use wasmparser::{Operator, Parser, Payload, Validator};

// Each source fills the minimum eight-byte payload capacity exactly.
const FULL_MINIMUM_CAPACITY_CASES: [(&str, &str, &str); 3] = [
    ("number", "11", "23"),
    (
        "bool",
        "true, false, true, false, true, false, true, false",
        "true",
    ),
    ("string", "\"a\", \"b\"", "\"c\""),
];

fn fixture_source(ty: &str, values: &str, item: &str) -> String {
    format!(
        r#"
        fn source() -> {ty}[] {{ return [{values}]; }}
        fn make(a: {ty}[]) -> {ty}[] {{ return resizable(a); }}
        fn reserve(a: {ty}[], n: number) {{ a.reserve(n); }}
        fn push(a: {ty}[]) {{ a.push({item}); }}
        fn pop(a: {ty}[]) -> {ty} {{ return a.pop(); }}
        fn remove(a: {ty}[], n: number) -> {ty} {{ return a.remove(n); }}
        fn write(a: {ty}[]) {{ a[0] = {item}; }}
        fn failed(a: {ty}[], n: number) {{ a.reserve({{ a.remove(0); n }}); }}
        "#
    )
}

// Rewrite only test fixtures after the normal checked backend has planned every
// index and lowered every body. Default emission remains the production path;
// the acquire replacement uses the existing M3 core's before-backing seam.
fn compile_fixture(source: &str, last_identity: u64, backing_limit: Option<u64>) -> Vec<u8> {
    let program = Compiler::new().analyze(source).unwrap();
    assert!(
        program.host_functions.is_empty(),
        "fixture body indices exclude imports"
    );
    let validated =
        super::super::validation::ValidatedProgram::new(&program, &ExportOptions::All).unwrap();
    let plan = validated.layout().resizable.as_ref().unwrap();
    let original = super::super::assemble(&validated);
    let mut module = Module::new();
    let mut global_count = 0;
    let mut body_index = 0;
    for payload in Parser::new(0).parse_all(&original) {
        match payload.unwrap() {
            Payload::GlobalSection(reader) => {
                let mut globals = GlobalSection::new();
                for (index, global) in reader.into_iter().enumerate() {
                    let global = global.unwrap();
                    let mut ops = global.init_expr.get_operators_reader();
                    let (ty, init) = match ops.read().unwrap() {
                        Operator::I32Const { value } => (ValType::I32, ConstExpr::i32_const(value)),
                        Operator::I64Const { value } => (
                            ValType::I64,
                            ConstExpr::i64_const(if index == LAST_IDENTITY_GLOBAL as usize {
                                i64::from_ne_bytes(last_identity.to_ne_bytes())
                            } else {
                                value
                            }),
                        ),
                        other => panic!("unexpected fixture global: {other:?}"),
                    };
                    assert!(matches!(ops.read().unwrap(), Operator::End));
                    assert!(ops.eof());
                    globals.global(
                        GlobalType {
                            val_type: ty,
                            mutable: global.ty.mutable,
                            shared: global.ty.shared,
                        },
                        &init,
                    );
                    global_count += 1;
                }
                module.section(&globals);
            }
            Payload::ExportSection(reader) => {
                let mut exports = ExportSection::new();
                for export in reader {
                    let export = export.unwrap();
                    let kind = match export.kind {
                        wasmparser::ExternalKind::Func => ExportKind::Func,
                        wasmparser::ExternalKind::Memory => ExportKind::Memory,
                        other => panic!("unexpected fixture export: {other:?}"),
                    };
                    exports.export(export.name, kind, export.index);
                }
                for index in 0..global_count {
                    exports.export(
                        &format!("__m4_test_global_{index}"),
                        ExportKind::Global,
                        index,
                    );
                }
                module.section(&exports);
            }
            Payload::CodeSectionStart { .. } => {
                let mut code = CodeSection::new();
                for payload in Parser::new(0).parse_all(&original) {
                    if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                        if body_index == plan.core_helper(AllocatorHelper::Acquire).raw()
                            && let Some(limit) = backing_limit
                        {
                            use Instruction::*;
                            let guard = [
                                GlobalGet(0),
                                I64ExtendI32U,
                                LocalGet(4),
                                I64Add,
                                I64Const(i64::try_from(limit).unwrap()),
                                I64GtU,
                                If(BlockType::Empty),
                                Unreachable,
                                End,
                            ];
                            code.function(&plan.core.acquire(&guard, &[]));
                        } else {
                            code.raw(body.as_bytes());
                        }
                        body_index += 1;
                    }
                }
                module.section(&code);
            }
            Payload::CodeSectionEntry(_) => {}
            other => {
                if let Some((id, range)) = other.as_section() {
                    module.section(&RawSection {
                        id,
                        data: &original[usize::try_from(range.start).unwrap()
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
        const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0))).exports;
        const view = () => new DataView(e.memory.buffer);
        const data = h => view().getUint32(h, true);
        const length = h => view().getUint32(h + 4, true);
        const width = h => [8, 1, 4][view().getUint32(h + 16, true)];
        const logical = h => Array.from(new Uint8Array(e.memory.buffer, data(h), length(h) * width(h)));
        const state = () => [
            Object.entries(e).filter(([k]) => k.startsWith('__m4_test_global_')).map(([, g]) => g.value),
            Array.from(new Uint8Array(e.memory.buffer))
        ];
        const unchangedTrap = operation => {{
            const before = state();
            assert.throws(operation, WebAssembly.RuntimeError);
            assert.deepEqual(state(), before);
        }};
        {script}
        "#
    );
    let output = crate::test_process::output(
        Command::new("node").arg("-e").arg(script),
        Some(bytes),
        Duration::from_secs(30),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_transport_changes_only_requested_fixture_sections_and_never_ordinary_compilation() {
    let source = fixture_source("number", "11", "23");
    let ordinary = Compiler::new().compile_source(&source).unwrap();
    let fixture = compile_fixture(&source, 0, None);
    let sections = |bytes: &[u8], omitted: &[u8]| {
        Parser::new(0)
            .parse_all(bytes)
            .filter_map(|payload| {
                let (id, range) = payload.unwrap().as_section()?;
                (!omitted.contains(&id)).then(|| {
                    (
                        id,
                        bytes[usize::try_from(range.start).unwrap()
                            ..usize::try_from(range.end).unwrap()]
                            .to_vec(),
                    )
                })
            })
            .collect::<Vec<_>>()
    };
    // Default unit transport appends exports, not instructions, state or options.
    assert_eq!(sections(&ordinary, &[7]), sections(&fixture, &[7]));
    let configured = compile_fixture(&source, u64::MAX - 3, Some(1024));
    assert_eq!(
        sections(&ordinary, &[6, 7, 10]),
        sections(&configured, &[6, 7, 10])
    );
    for payload in Parser::new(0).parse_all(&ordinary) {
        if let Payload::ExportSection(exports) = payload.unwrap() {
            for export in exports {
                let export = export.unwrap();
                assert_ne!(export.kind, wasmparser::ExternalKind::Global);
                assert!(!export.name.starts_with("__m4_test_"));
                assert!(!export.name.starts_with("__m3_"));
            }
        }
    }
    assert_eq!(ordinary, Compiler::new().compile_source(&source).unwrap());
}

#[test]
#[ignore = "requires Node.js"]
fn failed_constructor_keeps_unpublished_acquisition_without_sweeping_published_arrays() {
    for (ty, values, item) in [
        (
            "bool",
            "true, false, true, false, true, false, true, false",
            "true",
        ),
        ("string", "\"a\", \"b\"", "\"c\""),
    ] {
        run(
            &compile_fixture(&fixture_source(ty, values, item), 0, None),
            r#"
            const source = e.source(), a = e.make(source);
            const before = logical(a), w = width(a), n = length(a);
            const slot = data(source) + (n - 1) * w;
            const saved = new Uint8Array(e.memory.buffer).slice(slot, slot + w);
            if (w === 1) view().setUint8(slot, 2);
            else view().setUint32(slot, 0, true);
            const cursor = e.__m4_test_global_0.value;
            const authorityHead = e.__m4_test_global_32.value;
            assert.throws(() => e.make(source), WebAssembly.RuntimeError);
            assert.equal(e.__m4_test_global_32.value, authorityHead, 'failed copy publishes no array');
            assert.equal(e.__m4_test_global_0.value, cursor + 88, 'successful payload and object allocations remain');
            assert.equal(e.__m4_test_global_1.value, 2n);
            const abandonedRecord = e.__m4_test_global_2.value, abandoned = abandonedRecord + 32;
            assert.equal(view().getUint32(abandonedRecord + 12, true), 1);
            assert.equal(view().getBigUint64(abandonedRecord + 24, true), 8n);
            assert.deepEqual(logical(a), before);
            assert.equal(w === 1 ? view().getUint8(slot) : view().getUint32(slot, true), w === 1 ? 2 : 0);
            new Uint8Array(e.memory.buffer).set(saved, slot);
            const b = e.make(source);
            assert.notEqual(data(b), abandoned, 'unpublished active payload is not swept into reuse');
            assert.deepEqual(logical(b), before);
            e.reserve(a, 1);
            const c = e.make(source);
            assert.notEqual(data(c), abandoned);
            assert.deepEqual(logical(c), before);
            assert.deepEqual(logical(a), before);
            assert.equal(view().getUint32(abandonedRecord + 12, true), 1);
            e.write(a);
            e.pop(b);
            e.push(b);
            assert.equal(length(b), n);
            "#,
        );
    }
}

#[test]
#[ignore = "requires Node.js"]
fn exact_authority_preflight_rejects_fabrication_and_inconsistent_records_before_mutation() {
    for (ty, values, item) in FULL_MINIMUM_CAPACITY_CASES {
        run(
            &compile_fixture(&fixture_source(ty, values, item), 0, None),
            r#"
            const source = e.source(), a = e.make(source), b = e.make(source);
            const p = data(a), n = length(a), kind = view().getUint32(a + 16, true);
            const reject = h => {
                for (const operation of [() => e.reserve(h, 0), () => e.reserve(h, 100),
                                        () => e.push(h), () => e.pop(h), () => e.remove(h, -0)]) {
                    unchangedTrap(operation);
                }
            };
            // A complete copied descriptor/record is structurally valid but is
            // not an exact registry member, even with a genuine payload/identity.
            const fake = e.__alloc(48);
            new Uint8Array(e.memory.buffer).copyWithin(fake, a, a + 48);
            view().setUint32(fake + 12, fake, true);
            reject(fake);
            reject(source);
            assert.equal(data(a), p);
            // Deliberate private metadata corruption is fault injection, NOT a
            // supported host operation or a security/isolation guarantee.
            const edits = [
                [a, 'u32', data(b)], [a + 4, 'u32', n - 1],
                [a + 16, 'u32', (kind + 1) % 3], [a + 20, 'u32', n - 1],
                [a + 24, 'u64', 0n], [a + 24, 'u64', 2147483649n],
                [a + 24, 'u64', 16n], [a + 32, 'u32', data(b)],
                [a + 36, 'u32', 1], [a + 40, 'u64', 999n],
                [p - 20, 'u32', 0], [p - 16, 'u64', 999n],
                [p - 8, 'u64', 7n], [p - 24, 'u32', 1]
            ];
            for (const [address, type, value] of edits) {
                const saved = new Uint8Array(e.memory.buffer).slice(address, address + 8);
                if (type === 'u64') view().setBigUint64(address, value, true);
                else view().setUint32(address, value, true);
                reject(a);
                new Uint8Array(e.memory.buffer).set(saved, address);
                // Same-instance recovery after restoring the intentionally damaged
                // field: writes and take/append remain usable without acquisition.
                e.reserve(a, -0);
                e.write(a);
                e.remove(a, -0);
                e.push(a);
                assert.equal(data(a), p);
                assert.equal(length(a), n);
                assert.equal(length(b), n);
            }
            "#,
        );
    }
}

#[test]
#[ignore = "requires Node.js"]
fn backing_failure_preserves_post_argument_state_free_reuse_and_later_fresh_acquisition() {
    for (ty, values, item) in FULL_MINIMUM_CAPACITY_CASES {
        run(
            &compile_fixture(&fixture_source(ty, values, item), 0, Some(1024)),
            r#"
            const source = e.source(), a = e.make(source), b = e.make(source);
            const displaced = data(a);
            e.reserve(a, 1);
            const pointer = data(a), initial = logical(a), n = length(a), w = width(a);
            const globals = state()[0];
            assert.throws(() => e.failed(a, 2048), WebAssembly.RuntimeError);
            assert.equal(length(a), n - 1, 'argument removal survives failed replacement');
            assert.equal(data(a), pointer);
            assert.deepEqual(logical(a), initial.slice(w));
            assert.deepEqual(state()[0], globals, 'failed growth commits no allocator state');
            unchangedTrap(() => e.reserve(b, 2048));
            const afterArgument = logical(a);
            e.push(a);
            e.pop(a);
            assert.deepEqual(logical(a), afterArgument);
            // Failure does not drain a different free class or poison smaller fresh growth.
            const c = e.make(source);
            assert.equal(data(c), displaced);
            assert.deepEqual(logical(c), Array.from(new Uint8Array(e.memory.buffer, data(source), length(source) * w)));
            const before = logical(b);
            e.push(b);
            assert.notEqual(data(b), displaced);
            e.pop(b);
            assert.deepEqual(logical(b), before);
            assert.deepEqual(logical(a), afterArgument);
            unchangedTrap(() => e.reserve(c, 2048));
            e.reserve(c, -0);
            e.write(c);
            "#,
        );
    }
}

#[test]
#[ignore = "requires Node.js"]
fn exhausted_identity_preserves_free_payloads_and_allows_later_nonallocating_mutations() {
    for (ty, values, item) in FULL_MINIMUM_CAPACITY_CASES {
        let source = fixture_source(ty, values, item);
        run(
            &compile_fixture(&source, u64::MAX - 3, None),
            r#"
            const source = e.source(), a = e.make(source), b = e.make(source);
            const old = data(a);
            e.reserve(a, 1);
            assert.notEqual(data(a), old);
            assert.equal(BigInt.asUintN(64, e.__m4_test_global_1.value), 18446744073709551615n);
            const free = e.__m4_test_global_3.value;
            assert.equal(free, old - 32);
            for (const operation of [() => e.reserve(a, 100), () => e.push(b), () => e.make(source)]) {
                unchangedTrap(operation);
                assert.equal(e.__m4_test_global_3.value, free, 'exhaustion never drains the free class');
                // No new acquisition is needed for these successful later operations.
                e.reserve(b, -0);
                const before = logical(a);
                e.push(a);
                e.pop(a);
                assert.deepEqual(logical(a), before);
                e.write(b);
                assert.equal(length(b), length(source));
            }
            "#,
        );
    }
}
