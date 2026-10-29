//! Private M4 observation transport; ordinary compilation never selects it.
use super::*;
use crate::{Compiler, exports::ExportOptions};
use wasm_encoder::{Encode, ExportKind, ExportSection, Module, RawSection};
use wasmparser::{Operator, Parser, Payload, Validator};

const OBSERVATIONS: [(&str, u32); 4] = [
    ("__m4_cursor", 0),
    ("__m4_last_identity", LAST_IDENTITY_GLOBAL),
    ("__m4_allocator_registry", ALLOCATOR_REGISTRY_GLOBAL),
    ("__m4_array_registry", ARRAY_REGISTRY_GLOBAL),
];
const COUNTERS: [&str; 2] = ["__m4_allocator_comparisons", "__m4_authority_comparisons"];

fn modules(source: &str, comparisons: bool) -> [Vec<u8>; 2] {
    let program = Compiler::new().analyze(source).unwrap();
    assert!(
        program.host_functions.is_empty(),
        "fixture indices exclude imports"
    );
    let validated =
        super::super::validation::ValidatedProgram::new(&program, &ExportOptions::All).unwrap();
    let ordinary = super::super::assemble(&validated);
    let observed = observe(
        &ordinary,
        validated.layout().resizable.as_ref().unwrap(),
        comparisons,
    )
    .unwrap();
    for bytes in [&ordinary, &observed] {
        Validator::new().validate_all(bytes).unwrap();
    }
    [ordinary, observed]
}

// Add exports and, optionally, stack-neutral i64 increments after each registry
// null guard. Raw body splicing preserves every existing instruction and local.
fn observe(original: &[u8], plan: &Plan, comparisons: bool) -> Result<Vec<u8>, CodegenError> {
    let counter = global_count(original);
    let mut targets = vec![(plan.core_helper(AllocatorHelper::Find).raw(), counter)];
    targets.extend(plan.elements.iter().map(|&element| {
        (
            plan.authority_helper(element, AuthorityHelper::Lookup)
                .raw(),
            counter + 1,
        )
    }));
    let mut module = Module::new();
    for payload in Parser::new(0).parse_all(original) {
        match payload.unwrap() {
            Payload::GlobalSection(reader) if comparisons => {
                let mut globals = GlobalSection::new();
                for global in reader {
                    let global = global.unwrap();
                    let mut ops = global.init_expr.get_operators_reader();
                    let (val_type, init) = match ops.read().unwrap() {
                        Operator::I32Const { value } => (ValType::I32, ConstExpr::i32_const(value)),
                        Operator::I64Const { value } => (ValType::I64, ConstExpr::i64_const(value)),
                        other => panic!("unexpected fixture global: {other:?}"),
                    };
                    assert!(matches!(ops.read().unwrap(), Operator::End));
                    assert!(ops.eof());
                    globals.global(
                        GlobalType {
                            val_type,
                            mutable: global.ty.mutable,
                            shared: global.ty.shared,
                        },
                        &init,
                    );
                }
                for _ in COUNTERS {
                    globals.global(
                        GlobalType {
                            val_type: ValType::I64,
                            mutable: true,
                            shared: false,
                        },
                        &ConstExpr::i64_const(0),
                    );
                }
                module.section(&globals);
            }
            Payload::ExportSection(reader) => {
                let mut section = ExportSection::new();
                for export in reader {
                    let export = export.unwrap();
                    if OBSERVATIONS.iter().any(|(name, _)| *name == export.name)
                        || (comparisons && COUNTERS.contains(&export.name))
                    {
                        return Err(CodegenError::DuplicateExportName {
                            name: export.name.to_owned(),
                        });
                    }
                    let kind = match export.kind {
                        wasmparser::ExternalKind::Func => ExportKind::Func,
                        wasmparser::ExternalKind::Memory => ExportKind::Memory,
                        other => panic!("unexpected fixture export: {other:?}"),
                    };
                    section.export(export.name, kind, export.index);
                }
                for (name, index) in OBSERVATIONS {
                    section.export(name, ExportKind::Global, index);
                }
                if comparisons {
                    for (offset, name) in COUNTERS.iter().enumerate() {
                        section.export(
                            name,
                            ExportKind::Global,
                            counter + u32::try_from(offset).unwrap(),
                        );
                    }
                }
                module.section(&section);
            }
            Payload::CodeSectionStart { .. } if comparisons => {
                let mut code = CodeSection::new();
                let mut index = 0;
                for payload in Parser::new(0).parse_all(original) {
                    if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                        if let Some((_, global)) =
                            targets.iter().find(|(target, _)| *target == index)
                        {
                            let mut ops = body.get_operators_reader().unwrap();
                            for _ in 0..7 {
                                ops.read().unwrap();
                            }
                            let offset =
                                usize::try_from(ops.original_position() - body.range().start)
                                    .unwrap();
                            let bytes = body.as_bytes();
                            let mut observed = bytes[..offset].to_vec();
                            for instruction in increment(*global) {
                                instruction.encode(&mut observed);
                            }
                            observed.extend_from_slice(&bytes[offset..]);
                            code.raw(&observed);
                        } else {
                            code.raw(body.as_bytes());
                        }
                        index += 1;
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

fn increment(global: u32) -> [Instruction<'static>; 4] {
    [
        Instruction::GlobalGet(global),
        Instruction::I64Const(1),
        Instruction::I64Add,
        Instruction::GlobalSet(global),
    ]
}

fn global_count(bytes: &[u8]) -> u32 {
    Parser::new(0)
        .parse_all(bytes)
        .find_map(|payload| {
            if let Payload::GlobalSection(reader) = payload.unwrap() {
                Some(reader.count())
            } else {
                None
            }
        })
        .unwrap()
}

fn globals(bytes: &[u8]) -> Vec<String> {
    let mut result = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::GlobalSection(reader) = payload.unwrap() {
            for global in reader {
                let global = global.unwrap();
                let init = global
                    .init_expr
                    .get_operators_reader()
                    .into_iter()
                    .map(|op| format!("{:?}", op.unwrap()))
                    .collect::<Vec<_>>();
                result.push(format!("{:?}: {init:?}", global.ty));
            }
        }
    }
    result
}

fn exports(bytes: &[u8]) -> Vec<(String, wasmparser::ExternalKind, u32)> {
    let mut result = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::ExportSection(reader) = payload.unwrap() {
            for export in reader {
                let export = export.unwrap();
                result.push((export.name.to_owned(), export.kind, export.index));
            }
        }
    }
    result
}

#[test]
fn observations_are_private_and_ordinary_compilation_is_unchanged() {
    let source = "fn make() -> number[] { return resizable([1]); }";
    let [ordinary, observed] = modules(source, false);
    assert_eq!(ordinary, Compiler::new().compile_source(source).unwrap());
    assert!(exports(&ordinary).iter().all(|(name, kind, _)| {
        !name.starts_with("__m4_") && *kind != wasmparser::ExternalKind::Global
    }));
    let expected = OBSERVATIONS
        .map(|(name, index)| (name.to_owned(), wasmparser::ExternalKind::Global, index));
    assert_eq!(&exports(&observed)[exports(&ordinary).len()..], &expected);
    assert_eq!(sections(&ordinary, &[7]), sections(&observed, &[7]));
    assert_eq!(modules(source, false), [ordinary, observed]);
}

fn sections(bytes: &[u8], omitted: &[u8]) -> Vec<(u8, Vec<u8>)> {
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
        .collect()
}

fn bodies(bytes: &[u8]) -> Vec<(Vec<String>, Vec<String>)> {
    Parser::new(0)
        .parse_all(bytes)
        .filter_map(|payload| {
            if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                let locals = body
                    .get_locals_reader()
                    .unwrap()
                    .into_iter()
                    .map(|local| format!("{:?}", local.unwrap()))
                    .collect();
                let ops = body
                    .get_operators_reader()
                    .unwrap()
                    .into_iter()
                    .map(|op| format!("{:?}", op.unwrap()))
                    .collect();
                Some((locals, ops))
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn counters_only_add_exact_increments_after_live_registry_guards() {
    // Cover all typed LOOKUP helpers, both with and without constructors. The
    // latter exercises M4 discovery driven solely by management operations.
    for source in [
        r#"
        fn numbers() -> number[] { return resizable([1]); }
        fn bools() -> bool[] { return resizable([true]); }
        fn strings() -> string[] { return resizable(["a"]); }
        "#,
        r#"
        fn numbers(a: number[]) { a.reserve(1); a.push(2); a.pop(); a.remove(0); }
        fn bools(a: bool[]) { a.reserve(1); a.push(true); a.pop(); a.remove(0); }
        fn strings(a: string[]) { a.reserve(1); a.push("a"); a.pop(); a.remove(0); }
        "#,
    ] {
        let [ordinary, observed] = modules(source, true);
        assert_eq!(
            sections(&ordinary, &[6, 7, 10]),
            sections(&observed, &[6, 7, 10])
        );
        let counter = global_count(&ordinary);
        assert_eq!(global_count(&observed), counter + 2);
        let original_globals = globals(&ordinary);
        let observed_globals = globals(&observed);
        assert_eq!(
            &observed_globals[..original_globals.len()],
            &original_globals
        );
        let expected_counter = "GlobalType { content_type: I64, mutable: true, shared: false }: [\"I64Const { value: 0 }\", \"End\"]";
        assert_eq!(
            &observed_globals[original_globals.len()..],
            &[expected_counter, expected_counter]
        );
        let original_bodies = bodies(&ordinary);
        let mut observed_bodies = bodies(&observed);
        assert_eq!(original_bodies.len(), observed_bodies.len());
        let mut sites = [0, 0];
        for ((_, original), (_, observed)) in original_bodies.iter().zip(&mut observed_bodies) {
            let kind = if original.first() == Some(&"GlobalGet { global_index: 2 }".to_owned()) {
                Some(0)
            } else if original.first() == Some(&"GlobalGet { global_index: 32 }".to_owned()) {
                Some(1)
            } else {
                None
            };
            if let Some(kind) = kind {
                // Independent decoded expectation: only visited non-null records
                // count. No entry count, empty-list count or helper-call count.
                assert_eq!(
                    &original[1..7],
                    &[
                        "LocalSet { local_index: 1 }",
                        "Block { blockty: Empty }",
                        "Loop { blockty: Empty }",
                        "LocalGet { local_index: 1 }",
                        "I32Eqz",
                        "BrIf { relative_depth: 1 }",
                    ]
                );
                let global = counter + u32::try_from(kind).unwrap();
                assert_eq!(
                    &observed[7..11],
                    &[
                        format!("GlobalGet {{ global_index: {global} }}"),
                        "I64Const { value: 1 }".to_owned(),
                        "I64Add".to_owned(),
                        format!("GlobalSet {{ global_index: {global} }}"),
                    ]
                );
                observed.drain(7..11);
                sites[kind] += 1;
            }
        }
        assert_eq!(sites, [1, 3]);
        // Removing exactly those four increments recovers every decoded body
        // and local declaration, including all calls, memory ops and indices.
        assert_eq!(original_bodies, observed_bodies);
        let ordinary_exports = exports(&ordinary);
        assert!(
            ordinary_exports
                .iter()
                .all(|(name, _, _)| { !name.starts_with("__m4_") && !name.starts_with("__m3_") })
        );
        let mut expected = ordinary_exports;
        expected.extend(
            OBSERVATIONS
                .map(|(name, index)| (name.to_owned(), wasmparser::ExternalKind::Global, index)),
        );
        expected.extend(COUNTERS.iter().enumerate().map(|(offset, name)| {
            (
                name.to_string(),
                wasmparser::ExternalKind::Global,
                counter + u32::try_from(offset).unwrap(),
            )
        }));
        assert_eq!(exports(&observed), expected);
        assert_eq!(ordinary, Compiler::new().compile_source(source).unwrap());
        assert_eq!(modules(source, true), [ordinary, observed]);
    }
}

#[test]
fn observation_export_collisions_are_private_to_the_requested_variant() {
    for name in OBSERVATIONS.iter().map(|(name, _)| *name).chain(COUNTERS) {
        let source = format!("fn {name}() -> number[] {{ return resizable([1]); }}");
        let program = Compiler::new().analyze(&source).unwrap();
        let validated =
            super::super::validation::ValidatedProgram::new(&program, &ExportOptions::All).unwrap();
        let ordinary = super::super::assemble(&validated);
        assert_eq!(ordinary, Compiler::new().compile_source(&source).unwrap());
        let plan = validated.layout().resizable.as_ref().unwrap();
        assert!(matches!(observe(&ordinary, plan, true),
            Err(CodegenError::DuplicateExportName { name: duplicate }) if duplicate == name));
        if COUNTERS.contains(&name) {
            Validator::new()
                .validate_all(&observe(&ordinary, plan, false).unwrap())
                .unwrap();
        }
    }
}

fn record(mode: &str) {
    use std::{path::PathBuf, process::Command, sync::Mutex, time::Duration};
    // Both recorders write the same module pair. Full ignored runs must not race
    // these two modes; measurement evidence still requires an otherwise idle run.
    static RECORDING: Mutex<()> = Mutex::new(());
    let _recording = RECORDING.lock().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source =
        std::fs::read_to_string(root.join("tests/support/m4_resizable_corpus.was")).unwrap();
    let [ordinary, observed] = modules(&source, true);
    let directory = root.join("target/m4-resizable");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("ordinary.wasm"), ordinary).unwrap();
    std::fs::write(directory.join("observed.wasm"), observed).unwrap();
    let output = crate::test_process::output(
        Command::new("node")
            .arg(root.join("tests/support/m4_resizable_measure.cjs"))
            .arg(mode)
            .arg(&directory),
        None,
        Duration::from_secs(120),
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(directory.join(format!("{mode}.json")), &output.stdout).unwrap();
    println!("{}", String::from_utf8_lossy(&output.stdout));
}

#[test]
#[ignore = "requires Node.js and M4 corpus/harness; explicit isolated accounting"]
fn record_m4_accounting() {
    record("accounting");
}

#[test]
#[ignore = "requires Node.js and M4 corpus/harness; explicit isolated ordinary-module timing"]
fn record_m4_guest_timing() {
    record("timing");
}
