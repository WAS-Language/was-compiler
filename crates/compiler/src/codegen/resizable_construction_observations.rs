//! Private string-array construction observations, layered over the M5 string tracer.
//!
//! Acquired/released bytes count full allocator payload capacities, including reuse.
//! Active bytes include unreleased acquisitions even after a later trap; the peak
//! must be updated at acquisition, before reserve releases the displaced payload.
//! Copied bytes count handle stores (four per constructor entry, one per reserve
//! byte store), never immutable string payload bytes. Relocations count successful
//! reserve acquisitions, not calls that fit existing capacity. All arithmetic is
//! nontrapping modular i64 arithmetic; exact totals require bounded fixtures.
use super::super::{CodegenError, validation::ValidatedProgram};
use super::{AllocatorHelper, ArrayElementType, Plan};
use crate::{Compiler, exports::ExportOptions};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, Encode, ExportKind, ExportSection, GlobalType, Instruction,
    Module, RawSection, ValType,
};
use wasmparser::{Operator, Parser, Payload, Validator};

const COUNTERS: [&str; 6] = [
    "__m5_array_acquired_bytes",
    "__m5_array_released_bytes",
    "__m5_array_active_bytes",
    "__m5_array_peak_active_bytes",
    "__m5_array_copied_bytes",
    "__m5_array_relocations",
];
const REGISTRY: &str = "__m5_allocator_registry";

/// Returns ordinary and observed modules for the same string-element fixture.
/// The observed module must retain the aggregate string observations and expose
/// the allocator registry for independent host attribution, not allocator control.
pub(in crate::codegen) fn modules(compiler: &Compiler, source: &str) -> [Vec<u8>; 2] {
    let [ordinary, strings] = super::super::string_observation_tests::modules(compiler, source);
    let program = compiler.analyze(source).unwrap();
    let checked = ValidatedProgram::new(&program, &ExportOptions::All).unwrap();
    let plan = checked.layout().resizable.as_ref().unwrap();
    assert_eq!(
        plan.elements,
        [ArrayElementType::String],
        "string-element fixtures only"
    );
    let observed = observe(&strings, plan).unwrap();
    Validator::new().validate_all(&observed).unwrap();
    [ordinary, observed]
}

fn observe(original: &[u8], plan: &Plan) -> Result<Vec<u8>, CodegenError> {
    use Instruction::*;
    let base = Parser::new(0)
        .parse_all(original)
        .find_map(|payload| {
            if let Payload::GlobalSection(reader) = payload.unwrap() {
                Some(reader.count())
            } else {
                None
            }
        })
        .unwrap();
    let acquire = plan.core_helper(AllocatorHelper::Acquire).raw();
    let release = plan.core_helper(AllocatorHelper::Release).raw();
    let constructor = plan
        .constructors
        .contains(&ArrayElementType::String)
        .then(|| plan.constructor(ArrayElementType::String).raw());
    let reserve = plan
        .reserves
        .contains(&ArrayElementType::String)
        .then(|| plan.reserve(ArrayElementType::String).raw());
    let mut function_index = 0;
    for payload in Parser::new(0).parse_all(original) {
        if let Payload::ImportSection(reader) = payload.unwrap() {
            for import in reader.into_imports() {
                if matches!(import.unwrap().ty, wasmparser::TypeRef::Func(_)) {
                    function_index += 1;
                }
            }
        }
    }
    let mut module = Module::new();
    for payload in Parser::new(0).parse_all(original) {
        match payload.unwrap() {
            Payload::GlobalSection(reader) => {
                let range = reader.range();
                let mut offset = usize::try_from(range.start).unwrap();
                while original[offset] & 0x80 != 0 {
                    offset += 1;
                }
                offset += 1;
                let mut data = Vec::new();
                (base + 6).encode(&mut data);
                data.extend_from_slice(&original[offset..usize::try_from(range.end).unwrap()]);
                for _ in COUNTERS {
                    GlobalType {
                        val_type: ValType::I64,
                        mutable: true,
                        shared: false,
                    }
                    .encode(&mut data);
                    ConstExpr::i64_const(0).encode(&mut data);
                }
                module.section(&RawSection { id: 6, data: &data });
            }
            Payload::ExportSection(reader) => {
                let mut exports = ExportSection::new();
                for export in reader {
                    let export = export.unwrap();
                    if COUNTERS.contains(&export.name) || export.name == REGISTRY {
                        return Err(CodegenError::DuplicateExportName {
                            name: export.name.to_owned(),
                        });
                    }
                    let kind = match export.kind {
                        wasmparser::ExternalKind::Func => ExportKind::Func,
                        wasmparser::ExternalKind::Memory => ExportKind::Memory,
                        wasmparser::ExternalKind::Global => ExportKind::Global,
                        other => panic!("unexpected fixture export: {other:?}"),
                    };
                    exports.export(export.name, kind, export.index);
                }
                for (offset, name) in COUNTERS.iter().enumerate() {
                    exports.export(
                        name,
                        ExportKind::Global,
                        base + u32::try_from(offset).unwrap(),
                    );
                }
                exports.export(REGISTRY, ExportKind::Global, plan.core.registry);
                module.section(&exports);
            }
            Payload::CodeSectionStart { .. } => {
                let mut code = CodeSection::new();
                for payload in Parser::new(0).parse_all(original) {
                    if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                        let mut ops = body.get_operators_reader().unwrap();
                        let mut rewritten = Vec::new();
                        let mut previous_offset = 0;
                        let mut sites = 0;
                        let mut previous = None;
                        while !ops.eof() {
                            let op = ops.read().unwrap();
                            let mut additions = Vec::new();
                            if function_index == acquire
                                && matches!(op, Operator::GlobalSet { global_index } if global_index == plan.core.last_identity)
                            {
                                // Local 3 is the classified full capacity on both
                                // fresh and reused success paths. Count before returning
                                // to reserve, while the displaced buffer is still active.
                                update(&mut additions, base, &[LocalGet(3)], I64Add);
                                update(&mut additions, base + 2, &[LocalGet(3)], I64Add);
                                encode(
                                    &mut additions,
                                    &[
                                        GlobalGet(base + 2),
                                        GlobalGet(base + 3),
                                        I64GtU,
                                        If(BlockType::Empty),
                                        GlobalGet(base + 2),
                                        GlobalSet(base + 3),
                                        End,
                                    ],
                                );
                            } else if function_index == release
                                && matches!(op, Operator::Call { function_index } if function_index == plan.core.set_head.raw())
                            {
                                // Class local 3 remains available after the fully
                                // verified release commits; no memory load is added.
                                let capacity = [
                                    I64Const(i64::from(super::super::abi::MIN_ALLOCATION_BYTES)),
                                    LocalGet(3),
                                    I64ExtendI32U,
                                    I64Shl,
                                ];
                                update(&mut additions, base + 1, &capacity, I64Add);
                                update(&mut additions, base + 2, &capacity, I64Sub);
                            } else if Some(function_index) == constructor
                                && matches!(op, Operator::I32Store { memarg } if memarg.offset == 0 && memarg.align == 2)
                                && matches!(previous, Some(Operator::Call { .. }))
                            {
                                // Only the consumed string-handle store follows a
                                // call. Descriptor/authority stores do not.
                                update(
                                    &mut additions,
                                    base + 4,
                                    &[I64Const(i64::from(
                                        super::super::abi::STRING_ARRAY_ELEMENT_SIZE,
                                    ))],
                                    I64Add,
                                );
                            } else if Some(function_index) == reserve {
                                if matches!(op, Operator::Call { function_index } if function_index == acquire)
                                {
                                    update(&mut additions, base + 5, &[I64Const(1)], I64Add);
                                } else if matches!(op, Operator::I32Store8 { .. }) {
                                    update(&mut additions, base + 4, &[I64Const(1)], I64Add);
                                }
                            }
                            if !additions.is_empty() {
                                sites += 1;
                                let offset =
                                    usize::try_from(ops.original_position() - body.range().start)
                                        .unwrap();
                                rewritten
                                    .extend_from_slice(&body.as_bytes()[previous_offset..offset]);
                                rewritten.extend(additions);
                                previous_offset = offset;
                            }
                            previous = Some(op);
                        }
                        let expected = if function_index == acquire
                            || function_index == release
                            || Some(function_index) == constructor
                        {
                            1
                        } else if Some(function_index) == reserve {
                            2
                        } else {
                            0
                        };
                        assert_eq!(
                            sites, expected,
                            "array observation success-site contract changed in function {function_index}"
                        );
                        rewritten.extend_from_slice(&body.as_bytes()[previous_offset..]);
                        code.raw(&rewritten);
                        function_index += 1;
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
    Ok(module.finish())
}

fn encode(bytes: &mut Vec<u8>, instructions: &[Instruction<'_>]) {
    for instruction in instructions {
        instruction.encode(bytes);
    }
}

fn update(bytes: &mut Vec<u8>, global: u32, value: &[Instruction<'_>], operation: Instruction<'_>) {
    Instruction::GlobalGet(global).encode(bytes);
    encode(bytes, value);
    operation.encode(bytes);
    Instruction::GlobalSet(global).encode(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmparser::{ExternalKind, Operator, Parser, Payload, Validator};

    const COUNTERS: [&str; 6] = [
        "__m5_array_acquired_bytes",
        "__m5_array_released_bytes",
        "__m5_array_active_bytes",
        "__m5_array_peak_active_bytes",
        "__m5_array_copied_bytes",
        "__m5_array_relocations",
    ];

    // One user body, eleven ordinary memory helpers, seven allocator helpers,
    // constructor, lookup, validate, reserve, push. Imports shift function indices
    // but not these code-body indices. No production Plan selects test sites.
    const SOURCE: &str = "fn fixture() -> string[] { let parts: string[] = resizable([]); parts.push(\"λ\"); parts.push(\"猫!\"); parts.push(\"\"); return parts; }";

    #[test]
    fn removing_array_observations_restores_layered_bodies_locals_and_sections() {
        for imported in [false, true] {
            let mut compiler = Compiler::new();
            if imported {
                compiler
                    .register_host_function("effect", "env", "effect", vec![], crate::Type::Void)
                    .unwrap();
            }
            let source = if imported {
                SOURCE.replace("let parts:", "effect(); let parts:")
            } else {
                SOURCE.to_owned()
            };
            let [ordinary, baseline] =
                super::super::super::string_observation_tests::modules(&compiler, &source);
            let pair = modules(&compiler, &source);
            assert_eq!(pair[0], ordinary);
            assert_eq!(ordinary, compiler.compile_source(&source).unwrap());
            for bytes in &pair {
                Validator::new().validate_all(bytes).unwrap();
            }
            assert_isolation(&baseline, &pair[1], u32::from(imported));
            assert_eq!(pair, modules(&compiler, &source));
            assert!(
                exports(&ordinary)
                    .iter()
                    .all(|(name, _, _)| !name.starts_with("__m5_"))
            );
        }
    }

    fn exports(bytes: &[u8]) -> Vec<(String, ExternalKind, u32)> {
        Parser::new(0)
            .parse_all(bytes)
            .find_map(|payload| match payload.unwrap() {
                Payload::ExportSection(reader) => Some(
                    reader
                        .into_iter()
                        .map(|export| {
                            let export = export.unwrap();
                            (export.name.to_owned(), export.kind, export.index)
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap()
    }

    fn bodies(bytes: &[u8]) -> Vec<wasmparser::FunctionBody<'_>> {
        Parser::new(0)
            .parse_all(bytes)
            .filter_map(|payload| match payload.unwrap() {
                Payload::CodeSectionEntry(body) => Some(body),
                _ => None,
            })
            .collect()
    }

    fn unaffected_sections(bytes: &[u8]) -> Vec<(u8, Vec<u8>)> {
        Parser::new(0)
            .parse_all(bytes)
            .filter_map(|payload| {
                let (id, range) = payload.unwrap().as_section()?;
                (![6, 7, 10].contains(&id)).then(|| {
                    (
                        id,
                        bytes[usize::try_from(range.start).unwrap()
                            ..usize::try_from(range.end).unwrap()]
                            .to_vec(),
                    )
                })
            })
            .collect()
    }

    fn globals(bytes: &[u8]) -> Vec<(wasmparser::GlobalType, Vec<u8>)> {
        Parser::new(0)
            .parse_all(bytes)
            .find_map(|payload| match payload.unwrap() {
                Payload::GlobalSection(reader) => Some(
                    reader
                        .into_iter()
                        .map(|global| {
                            let global = global.unwrap();
                            let range = global.init_expr.get_binary_reader().range();
                            (
                                global.ty,
                                bytes[usize::try_from(range.start).unwrap()
                                    ..usize::try_from(range.end).unwrap()]
                                    .to_vec(),
                            )
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap()
    }

    fn assert_isolation(baseline: &[u8], observed: &[u8], imports: u32) {
        assert_eq!(unaffected_sections(baseline), unaffected_sections(observed));
        let old_globals = globals(baseline);
        // Cursor + 32 M4 globals + seven existing aggregate string counters.
        assert_eq!(old_globals.len(), 40);
        let new_globals = globals(observed);
        assert_eq!(new_globals.len(), 46, "six array counters must be added");
        assert_eq!(&new_globals[..40], old_globals);
        for (ty, init) in &new_globals[40..] {
            assert_eq!(ty.content_type, wasmparser::ValType::I64);
            assert!(ty.mutable);
            assert!(!ty.shared);
            assert_eq!(init, &[0x42, 0x00, 0x0b]);
        }

        let old_exports = exports(baseline);
        let new_exports = exports(observed);
        assert_eq!(new_exports.len(), old_exports.len() + 7);
        assert_eq!(&new_exports[..old_exports.len()], old_exports);
        for (offset, name) in COUNTERS.iter().enumerate() {
            assert_eq!(
                new_exports[old_exports.len() + offset],
                (
                    name.to_string(),
                    ExternalKind::Global,
                    40 + u32::try_from(offset).unwrap()
                )
            );
        }
        assert_eq!(
            new_exports.last().unwrap(),
            &(
                "__m5_allocator_registry".to_owned(),
                ExternalKind::Global,
                2
            )
        );

        let old_bodies = bodies(baseline);
        let new_bodies = bodies(observed);
        assert_eq!(old_bodies.len(), new_bodies.len());
        let mut sites = Vec::new();
        for (index, (old, new)) in old_bodies.iter().zip(&new_bodies).enumerate() {
            let mut ops = new.get_operators_reader().unwrap();
            let prefix = usize::try_from(ops.original_position() - new.range().start).unwrap();
            // Keep the complete local-declaration prefix, not just instructions.
            let mut restored = new.as_bytes()[..prefix].to_vec();
            let mut previous = None;
            while !ops.eof() {
                let start = ops.original_position();
                let op = ops.read().unwrap();
                if let Operator::GlobalGet { global_index } = op
                    && (40..46).contains(&global_index)
                {
                    let tail = match (index, global_index) {
                        (13, 40) => {
                            // Identity has committed on both fresh and reuse branches;
                            // the payload and acquisition identity are not returned yet.
                            assert!(matches!(
                                previous,
                                Some(Operator::GlobalSet { global_index: 1 })
                            ));
                            vec![
                                "LocalGet { local_index: 3 }".into(),
                                "I64Add".into(),
                                set(40),
                                get(42),
                                "LocalGet { local_index: 3 }".into(),
                                "I64Add".into(),
                                set(42),
                                get(42),
                                get(43),
                                "I64GtU".into(),
                                "If { blockty: Empty }".into(),
                                get(42),
                                set(43),
                                "End".into(),
                            ]
                        }
                        (18, 41) => {
                            // No decrement until verified release publishes its free-list head.
                            assert!(
                                matches!(previous, Some(Operator::Call { function_index }) if function_index == 16 + imports)
                            );
                            vec![
                                "I64Const { value: 8 }".into(),
                                "LocalGet { local_index: 3 }".into(),
                                "I64ExtendI32U".into(),
                                "I64Shl".into(),
                                "I64Add".into(),
                                set(41),
                                get(42),
                                "I64Const { value: 8 }".into(),
                                "LocalGet { local_index: 3 }".into(),
                                "I64ExtendI32U".into(),
                                "I64Shl".into(),
                                "I64Sub".into(),
                                set(42),
                            ]
                        }
                        (19, 44) => {
                            // Constructor copies a validated four-byte string handle.
                            assert!(
                                matches!(previous, Some(Operator::I32Store { memarg }) if memarg.offset == 0 && memarg.align == 2)
                            );
                            vec!["I64Const { value: 4 }".into(), "I64Add".into(), set(44)]
                        }
                        (22, 44) => {
                            assert!(
                                matches!(previous, Some(Operator::I32Store8 { memarg }) if memarg.offset == 0 && memarg.align == 0)
                            );
                            vec!["I64Const { value: 1 }".into(), "I64Add".into(), set(44)]
                        }
                        (22, 45) => {
                            // Count successful acquisition before validation/copy/release,
                            // while leaving both returned stack values intact.
                            assert!(
                                matches!(previous, Some(Operator::Call { function_index }) if function_index == 13 + imports)
                            );
                            vec!["I64Const { value: 1 }".into(), "I64Add".into(), set(45)]
                        }
                        _ => {
                            panic!("unexpected observation in body {index}, global {global_index}")
                        }
                    };
                    for expected in tail {
                        assert_eq!(format!("{:?}", ops.read().unwrap()), expected);
                    }
                    sites.push((index, global_index));
                    continue;
                }
                let end = ops.original_position();
                restored.extend_from_slice(
                    &new.as_bytes()[usize::try_from(start - new.range().start).unwrap()
                        ..usize::try_from(end - new.range().start).unwrap()],
                );
                previous = Some(op);
            }
            assert_eq!(restored, old.as_bytes(), "body {index}, including locals");
        }
        assert_eq!(sites, [(13, 40), (18, 41), (19, 44), (22, 45), (22, 44)]);
    }

    #[test]
    fn array_export_collisions_do_not_affect_ordinary_compilation() {
        for name in COUNTERS.into_iter().chain([REGISTRY]) {
            let source = format!("fn {name}() -> string[] {{ return resizable([]); }}");
            let compiler = Compiler::new();
            let program = compiler.analyze(&source).unwrap();
            let checked = ValidatedProgram::new(&program, &ExportOptions::All).unwrap();
            let [ordinary, strings] =
                super::super::super::string_observation_tests::modules(&compiler, &source);
            assert_eq!(ordinary, compiler.compile_source(&source).unwrap());
            assert_eq!(
                observe(&strings, checked.layout().resizable.as_ref().unwrap()),
                Err(CodegenError::DuplicateExportName {
                    name: name.to_owned()
                })
            );
        }
    }

    #[test]
    #[ignore = "requires Node.js"]
    fn paired_array_capacity_reuse_transient_peaks_copy_stores_and_failures() {
        use std::{path::PathBuf, process::Command, time::Duration};
        let source = r#"
            fn fragments() -> string[] { return resizable([]); }
            fn copy(parts: string[]) -> string[] { return resizable(parts); }
            fn append(parts: string[], item: string) -> void { parts.push(item); }
            fn reserve(parts: string[], additional: number) -> void { parts.reserve(additional); }
            fn left() -> string { return "λ"; }
            fn right() -> string { return "猫!"; }
            fn empty() -> string { return ""; }
            fn join(parts: string[]) -> string { return string_join(parts, "|"); }
        "#;
        let [ordinary, observed] = modules(&Compiler::new(), source);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let directory = root.join("target/m5-array-construction-observations");
        std::fs::create_dir_all(&directory).unwrap();
        for (name, bytes) in [("ordinary", &ordinary), ("observed", &observed)] {
            std::fs::write(directory.join(format!("{name}.wasm")), bytes).unwrap();
            std::fs::write(
                directory.join(format!("{name}-capped.wasm")),
                super::super::super::string_observation_tests::cap_memory(bytes),
            )
            .unwrap();
        }
        // Keep the host decoder and literal event expectations independent of
        // production constants and the instrumenter's site-selection machinery.
        let script = r#"
            const assert = require('node:assert/strict');
            const fs = require('node:fs');
            const path = require('node:path');
            const dir = process.argv[1];
            const load = name => new WebAssembly.Instance(new WebAssembly.Module(
                fs.readFileSync(path.join(dir, name + '.wasm')))).exports;
            const a = load('ordinary'), b = load('observed');
            const names = ['acquired_bytes', 'released_bytes', 'active_bytes',
                'peak_active_bytes', 'copied_bytes', 'relocations'];
            const counters = e => names.map(n => BigInt.asUintN(64, e['__m5_array_' + n].value));
            const check = expected => assert.deepEqual(counters(b), expected.map(BigInt));
            const view = e => new DataView(e.memory.buffer);
            const descriptor = (e, h) => [view(e).getUint32(h, true), view(e).getUint32(h + 4, true)];
            const text = (e, h) => {
                const [p, n] = descriptor(e, h);
                return new TextDecoder('utf-8', {fatal:true}).decode(new Uint8Array(e.memory.buffer, p, n));
            };
            const same = () => assert.deepEqual(new Uint8Array(a.memory.buffer), new Uint8Array(b.memory.buffer));
            const call = (name, ...args) => {
                const x = a[name](...args), y = b[name](...args);
                assert.equal(x, y); same(); return x;
            };
            // Private record layout decoded independently. Registry includes
            // retained free records and orphaned active acquisitions after traps.
            const registry = e => {
                const result = []; const seen = new Set();
                let p = e.__m5_allocator_registry.value >>> 0;
                while (p) {
                    assert(!seen.has(p)); seen.add(p);
                    const d = view(e), cls = d.getUint32(p + 8, true);
                    assert(cls < 29);
                    result.push({p, capacity: 8n << BigInt(cls), active: d.getUint32(p + 12, true)});
                    p = d.getUint32(p, true);
                }
                assert.equal(result.filter(r => r.active === 1).reduce((s,r) => s+r.capacity, 0n), counters(e)[2]);
                return result;
            };
            check([0,0,0,0,0,0]);
            const first = call('fragments'); check([8,0,8,8,0,0]);
            call('append', first, call('left')); call('append', first, call('right'));
            check([8,0,8,8,0,0]);
            call('append', first, call('empty')); check([24,8,16,24,8,1]);
            assert.equal(registry(b).length, 2);
            const second = call('fragments'); check([32,8,24,24,8,1]);
            assert.equal(registry(b).length, 2, 'constructor reused the free eight-byte payload');
            call('reserve', first, 6); check([96,24,72,88,20,2]);
            call('reserve', second, 3); check([112,32,80,88,20,3]);
            assert.equal(registry(b).length, 3, 'reserve reused the free sixteen-byte payload');
            call('reserve', first, 0); check([112,32,80,88,20,3]);
            const copy = call('copy', first); check([128,32,96,96,32,3]);
            assert.equal(descriptor(b, copy)[1], 3);
            const [payload] = descriptor(b, copy);
            assert.deepEqual([0,1,2].map(i => text(b, view(b).getUint32(payload + 4*i, true))), ['λ','猫!','']);
            const result = call('join', first);
            assert.equal(text(b, result), 'λ|猫!|');
            assert.equal(b.__m5_string_payload_allocations.value, 1n);
            assert.equal(b.__m5_string_payload_bytes.value, 8n);
            assert.equal(b.__m5_string_copied_bytes.value, 8n);
            check([128,32,96,96,32,3]);
            // A later constructor failure retains its successful acquisition and
            // the first completed handle store, not the invalid second one.
            const [sourcePayload] = descriptor(b, first);
            const saved = view(b).getUint32(sourcePayload + 4, true);
            for (const e of [a,b]) view(e).setUint32(sourcePayload + 4, 0, true);
            for (const e of [a,b]) assert.throws(() => e.copy(first), WebAssembly.RuntimeError);
            same(); check([144,32,112,112,36,3]); registry(b);
            for (const e of [a,b]) view(e).setUint32(sourcePayload + 4, saved, true);
            const before = counters(b);
            for (const e of [a,b]) assert.throws(() => e.reserve(first, -1), WebAssembly.RuntimeError);
            same(); assert.deepEqual(counters(b), before);
            // Actual backing allocation failure, not a speculative capacity
            // estimate or an invalid-input trap, leaves success counters unchanged.
            const ca = load('ordinary-capped'), cb = load('observed-capped');
            const ha = ca.fragments(), hb = cb.fragments(); assert.equal(ha,hb);
            const prior = counters(cb);
            for (const [e,h] of [[ca,ha],[cb,hb]]) {
                assert.throws(() => e.reserve(h, 1000000), WebAssembly.RuntimeError);
            }
            assert.deepEqual(counters(cb), prior); registry(cb);
            assert.deepEqual(new Uint8Array(ca.memory.buffer), new Uint8Array(cb.memory.buffer));
            // Counter additions wrap without new traps; this seeded value is
            // arithmetic evidence, not an exact storage observation.
            b.__m5_array_acquired_bytes.value = 9223372036854775807n;
            call('fragments');
            assert.equal(b.__m5_array_acquired_bytes.value, -9223372036854775801n);
        "#;
        let output = crate::test_process::output(
            Command::new("node").arg("-e").arg(script).arg(&directory),
            None,
            Duration::from_secs(60),
        );
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn get(global: u32) -> String {
        format!("GlobalGet {{ global_index: {global} }}")
    }

    fn set(global: u32) -> String {
        format!("GlobalSet {{ global_index: {global} }}")
    }
}
