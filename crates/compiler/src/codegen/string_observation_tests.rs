//! Private concat/join tracer. Integration dependencies cannot select this module.
use super::{CodegenError, memory::Helper, validation::ValidatedProgram};
use crate::{Compiler, exports::ExportOptions};
use wasm_encoder::{
    CodeSection, ConstExpr, Encode, ExportKind, ExportSection, GlobalType, Instruction, Module,
    RawSection, ValType,
};
use wasmparser::{Operator, Parser, Payload, Validator};

const COUNTERS: [&str; 7] = [
    "__m5_allocations",
    "__m5_requested_bytes",
    "__m5_reserved_bytes",
    "__m5_string_payload_allocations",
    "__m5_string_payload_bytes",
    "__m5_string_descriptor_bytes",
    "__m5_string_copied_bytes",
];

pub(super) fn modules(compiler: &Compiler, source: &str) -> [Vec<u8>; 2] {
    let program = compiler.analyze(source).unwrap();
    let checked = ValidatedProgram::new(&program, &ExportOptions::All).unwrap();
    let memory = checked.layout().memory.as_ref().unwrap();
    let index = |helper| memory.first_function.raw() + memory.plan.helper_index(helper);
    let ordinary = super::assemble(&checked);
    let helpers = memory.plan.helpers();
    let optional_index = |helper| helpers.contains(&helper).then(|| index(helper));
    let observed = observe(
        &ordinary,
        index(Helper::Alloc),
        optional_index(Helper::StringConcat),
        optional_index(Helper::StringJoin),
    )
    .unwrap();
    for bytes in [&ordinary, &observed] {
        Validator::new().validate_all(bytes).unwrap();
    }
    [ordinary, observed]
}

fn observe(
    original: &[u8],
    alloc: u32,
    concat: Option<u32>,
    join: Option<u32>,
) -> Result<Vec<u8>, CodegenError> {
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
    let mut module = Module::new();
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
                (base + u32::try_from(COUNTERS.len()).unwrap()).encode(&mut data);
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
                    if COUNTERS.contains(&export.name) || export.name == "__m5_cursor" {
                        return Err(CodegenError::DuplicateExportName {
                            name: export.name.to_owned(),
                        });
                    }
                    let kind = match export.kind {
                        wasmparser::ExternalKind::Func => ExportKind::Func,
                        wasmparser::ExternalKind::Memory => ExportKind::Memory,
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
                exports.export("__m5_cursor", ExportKind::Global, 0);
                module.section(&exports);
            }
            Payload::CodeSectionStart { .. } => {
                let mut code = CodeSection::new();
                for payload in Parser::new(0).parse_all(original) {
                    if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                        let mut rewritten = Vec::new();
                        let mut previous = 0;
                        let mut ops = body.get_operators_reader().unwrap();
                        let mut sites = 0;
                        while !ops.eof() {
                            let op = ops.read().unwrap();
                            let additions = if function_index == alloc
                                && matches!(op, Operator::GlobalSet { global_index: 0 })
                            {
                                sites += 1;
                                let mut additions = Vec::new();
                                add(&mut additions, base, &[Instruction::I64Const(1)]);
                                add(
                                    &mut additions,
                                    base + 1,
                                    &[Instruction::LocalGet(0), Instruction::I64ExtendI32U],
                                );
                                add(&mut additions, base + 2, &[Instruction::LocalGet(2)]);
                                additions
                            } else if (Some(function_index) == concat
                                || Some(function_index) == join)
                                && matches!(op, Operator::LocalTee { local_index: 4 })
                            {
                                sites += 1;
                                let mut additions = Vec::new();
                                add(&mut additions, base + 3, &[Instruction::I64Const(1)]);
                                let length = if Some(function_index) == join { 14 } else { 9 };
                                add(&mut additions, base + 4, &[Instruction::LocalGet(length)]);
                                add(&mut additions, base + 5, &[Instruction::I64Const(8)]);
                                additions
                            } else if (Some(function_index) == concat
                                || Some(function_index) == join)
                                && matches!(op, Operator::I32Store8 { .. })
                            {
                                sites += 1;
                                let mut additions = Vec::new();
                                add(&mut additions, base + 6, &[Instruction::I64Const(1)]);
                                additions
                            } else {
                                Vec::new()
                            };
                            if !additions.is_empty() {
                                let offset =
                                    usize::try_from(ops.original_position() - body.range().start)
                                        .unwrap();
                                rewritten.extend_from_slice(&body.as_bytes()[previous..offset]);
                                rewritten.extend(additions);
                                previous = offset;
                            }
                        }
                        if function_index == alloc {
                            assert_eq!(sites, 1, "allocator observation contract changed");
                        }
                        if Some(function_index) == concat {
                            assert_eq!(sites, 3, "concat observation contract changed");
                        }
                        if Some(function_index) == join {
                            assert_eq!(sites, 3, "join observation contract changed");
                        }
                        rewritten.extend_from_slice(&body.as_bytes()[previous..]);
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

fn add(bytes: &mut Vec<u8>, global: u32, value: &[Instruction<'_>]) {
    Instruction::GlobalGet(global).encode(bytes);
    for instruction in value {
        instruction.encode(bytes);
    }
    Instruction::I64Add.encode(bytes);
    Instruction::GlobalSet(global).encode(bytes);
}

fn export_names(bytes: &[u8]) -> Vec<String> {
    Parser::new(0)
        .parse_all(bytes)
        .flat_map(|payload| {
            if let Payload::ExportSection(exports) = payload.unwrap() {
                exports
                    .into_iter()
                    .map(|e| e.unwrap().name.to_owned())
                    .collect()
            } else {
                Vec::new()
            }
        })
        .collect()
}

fn bodies(bytes: &[u8]) -> Vec<wasmparser::FunctionBody<'_>> {
    Parser::new(0)
        .parse_all(bytes)
        .filter_map(|p| {
            if let Payload::CodeSectionEntry(body) = p.unwrap() {
                Some(body)
            } else {
                None
            }
        })
        .collect()
}

fn sections(bytes: &[u8]) -> Vec<(u8, Vec<u8>)> {
    Parser::new(0)
        .parse_all(bytes)
        .filter_map(|p| {
            p.unwrap().as_section().map(|(id, range)| {
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

#[test]
fn removing_only_expected_counter_sites_restores_every_body_and_local() {
    let [ordinary, observed] = modules(
        &Compiler::new(),
        "fn concat(a: string, b: string) -> string { return string_concat(a, b); }",
    );
    assert_isolation(&ordinary, &observed, 1, 1, Some(8), None);
    let mut compiler = Compiler::new();
    compiler
        .register_host_function("effect", "env", "effect", vec![], crate::Type::Void)
        .unwrap();
    let [ordinary, observed] = modules(
        &compiler,
        "fn concat(a: string, b: string) -> string { effect(); return string_concat(a, b); } fn fragments() -> string[] { return resizable([]); }",
    );
    // Imported functions occupy function indices, but not code-body indices.
    // M4 contributes 32 globals after the ordinary cursor and four typed helpers.
    assert_isolation(&ordinary, &observed, 33, 2, Some(13), None);
}

fn assert_isolation(
    ordinary: &[u8],
    observed: &[u8],
    base: u32,
    alloc_body: usize,
    concat_body: Option<usize>,
    join_body: Option<usize>,
) {
    let untouched = |bytes: &[u8]| {
        sections(bytes)
            .into_iter()
            .filter(|(id, _)| ![6, 7, 10].contains(id))
            .collect::<Vec<_>>()
    };
    assert_eq!(untouched(ordinary), untouched(observed));
    let exports = |bytes: &[u8]| {
        Parser::new(0)
            .parse_all(bytes)
            .find_map(|payload| {
                if let Payload::ExportSection(reader) = payload.unwrap() {
                    Some(
                        reader
                            .into_iter()
                            .map(|export| {
                                let export = export.unwrap();
                                (export.name.to_owned(), export.kind, export.index)
                            })
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                }
            })
            .unwrap()
    };
    let original_exports = exports(ordinary);
    let observed_exports = exports(observed);
    assert_eq!(original_exports, observed_exports[..original_exports.len()]);
    let added_exports = &observed_exports[original_exports.len()..];
    assert_eq!(added_exports.len(), 8);
    for (offset, (name, kind, index)) in added_exports[..7].iter().enumerate() {
        assert_eq!(name, COUNTERS[offset]);
        assert_eq!(*kind, wasmparser::ExternalKind::Global);
        assert_eq!(*index, base + u32::try_from(offset).unwrap());
    }
    assert_eq!(
        added_exports[7],
        (
            "__m5_cursor".to_owned(),
            wasmparser::ExternalKind::Global,
            0
        )
    );
    let original_bodies = bodies(ordinary);
    let observed_bodies = bodies(observed);
    assert_eq!(original_bodies.len(), observed_bodies.len());
    let mut removed = Vec::new();
    for (index, (original, observed)) in original_bodies.iter().zip(&observed_bodies).enumerate() {
        let mut ops = observed.get_operators_reader().unwrap();
        let mut restored = observed.as_bytes()
            [..usize::try_from(ops.original_position() - observed.range().start).unwrap()]
            .to_vec();
        let mut previous = None;
        while !ops.eof() {
            let start = ops.original_position();
            let op = ops.read().unwrap();
            if let Operator::GlobalGet { global_index } = op
                && (base..base + 7).contains(&global_index)
            {
                let counter = global_index - base + 1;
                match counter {
                    1 => assert!(matches!(
                        previous,
                        Some(Operator::GlobalSet { global_index: 0 })
                    )),
                    4 => assert!(matches!(
                        previous,
                        Some(Operator::LocalTee { local_index: 4 })
                    )),
                    7 => assert!(matches!(previous, Some(Operator::I32Store8 { .. }))),
                    _ => {}
                }
                let value = ops.read().unwrap();
                match counter {
                    1 | 4 | 7 => assert!(matches!(value, Operator::I64Const { value: 1 })),
                    2 => {
                        assert!(matches!(value, Operator::LocalGet { local_index: 0 }));
                        assert!(matches!(ops.read().unwrap(), Operator::I64ExtendI32U));
                    }
                    3 => assert!(matches!(value, Operator::LocalGet { local_index: 2 })),
                    5 => {
                        let expected = if Some(index) == join_body { 14 } else { 9 };
                        assert!(
                            matches!(value, Operator::LocalGet { local_index } if local_index == expected)
                        );
                    }
                    6 => assert!(matches!(value, Operator::I64Const { value: 8 })),
                    _ => unreachable!(),
                }
                assert!(matches!(ops.read().unwrap(), Operator::I64Add));
                assert!(
                    matches!(ops.read().unwrap(), Operator::GlobalSet { global_index: target } if target == global_index)
                );
                removed.push((index, global_index));
                continue;
            }
            let start = usize::try_from(start - observed.range().start).unwrap();
            let end = usize::try_from(ops.original_position() - observed.range().start).unwrap();
            restored.extend_from_slice(&observed.as_bytes()[start..end]);
            previous = Some(op);
        }
        assert_eq!(original.as_bytes(), restored, "body {index}");
    }
    // Body indices and complete site lists come from the fixture contracts,
    // independently of the helper selection and rewriting in observe.
    let mut expected = vec![
        (alloc_body, base),
        (alloc_body, base + 1),
        (alloc_body, base + 2),
    ];
    for body in [concat_body, join_body].into_iter().flatten() {
        expected.extend([
            (body, base + 3),
            (body, base + 4),
            (body, base + 5),
            (body, base + 6),
            (body, base + 6),
        ]);
    }
    assert_eq!(removed, expected);
    let globals = |bytes: &[u8]| {
        Parser::new(0)
            .parse_all(bytes)
            .find_map(|p| {
                if let Payload::GlobalSection(r) = p.unwrap() {
                    Some(
                        r.into_iter()
                            .map(|g| {
                                let g = g.unwrap();
                                (
                                    format!("{:?}", g.ty),
                                    format!(
                                        "{:?}",
                                        g.init_expr.get_operators_reader().read().unwrap()
                                    ),
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                }
            })
            .unwrap()
    };
    let original_globals = globals(ordinary);
    let observed_globals = globals(observed);
    let base = usize::try_from(base).unwrap();
    assert_eq!(original_globals, observed_globals[..base]);
    assert_eq!(observed_globals.len(), base + 7);
    for (ty, init) in &observed_globals[base..] {
        assert!(ty.contains("I64") && ty.contains("mutable: true"));
        assert_eq!(init, "I64Const { value: 0 }");
    }
}

#[test]
fn join_observation_isolation_and_direct_final_payload_copying() {
    let compiler = Compiler::new();
    let source = "fn join(parts: string[], separator: string) -> string { return string_join(parts, separator); }";
    let [ordinary, observed] = modules(&compiler, source);
    // One user body, seven baseline helpers, four string-array helpers, join.
    assert_isolation(&ordinary, &observed, 1, 1, None, Some(12));
    assert_join_lowering(&ordinary, 12, 1, 2, 8);
    assert!(
        !export_names(&ordinary)
            .iter()
            .any(|name| name.starts_with("__m5_"))
    );
    assert_eq!(ordinary, compiler.compile_source(source).unwrap());
    assert_eq!([ordinary, observed], modules(&compiler, source));

    let mut compiler = Compiler::new();
    compiler
        .register_host_function("effect", "env", "effect", vec![], crate::Type::Void)
        .unwrap();
    let source = "fn join(parts: string[], separator: string) -> string { effect(); return string_concat(string_join(parts, separator), separator); } fn fragments() -> string[] { return resizable([]); }";
    let [ordinary, observed] = modules(&compiler, source);
    // The import shifts function indices by one, not body indices. M4 adds
    // 32 globals; concat precedes join after the four string-array helpers.
    assert_isolation(&ordinary, &observed, 33, 2, Some(13), Some(14));
    assert_join_lowering(&ordinary, 14, 3, 4, 10);
    assert_eq!(ordinary, compiler.compile_source(source).unwrap());
    assert_eq!([ordinary, observed], modules(&compiler, source));
}

fn assert_join_lowering(
    bytes: &[u8],
    join_body: usize,
    alloc: u32,
    string_validator: u32,
    array_validator: u32,
) {
    let bodies = bodies(bytes);
    let body = &bodies[join_body];
    let locals = body
        .get_locals_reader()
        .unwrap()
        .into_iter()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(
        locals,
        [
            (12, wasmparser::ValType::I32),
            (1, wasmparser::ValType::I64)
        ]
    );
    let ops = body
        .get_operators_reader()
        .unwrap()
        .into_iter()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    let calls = ops
        .iter()
        .filter_map(|op| match op {
            Operator::Call { function_index } => Some(*function_index),
            _ => None,
        })
        .collect::<Vec<_>>();
    // All static calls: separator, outer storage, each fragment, one final
    // allocation. In particular there is no concat or intermediate string call.
    assert_eq!(
        calls,
        [string_validator, array_validator, string_validator, alloc]
    );
    let allocation = ops
        .iter()
        .position(|op| matches!(op, Operator::Call { function_index } if *function_index == alloc))
        .unwrap();
    assert!(matches!(
        ops[allocation + 1],
        Operator::LocalTee { local_index: 4 }
    ));
    assert!(matches!(
        ops[allocation - 4],
        Operator::LocalGet { local_index: 14 }
    ));
    assert!(matches!(
        ops[allocation - 3],
        Operator::I64Const { value: 8 }
    ));
    assert!(matches!(ops[allocation - 2], Operator::I64Add));
    assert!(matches!(ops[allocation - 1], Operator::I32WrapI64));
    let sizing = &ops[..allocation];
    assert_eq!(
        sizing
            .iter()
            .filter(|op| matches!(op, Operator::LocalSet { local_index: 14 }))
            .count(),
        2
    );
    assert_eq!(
        sizing
            .iter()
            .filter(|op| matches!(op, Operator::I64Mul))
            .count(),
        1
    );
    assert_eq!(
        sizing
            .iter()
            .filter(|op| matches!(op, Operator::I64GtU))
            .count(),
        1
    );
    assert_eq!(
        sizing
            .iter()
            .filter(|op| matches!(op, Operator::Return))
            .count(),
        1
    );
    assert!(
        !sizing
            .iter()
            .any(|op| matches!(op, Operator::I32Store8 { .. }))
    );
    let stores = ops
        .iter()
        .enumerate()
        .filter_map(|(index, op)| matches!(op, Operator::I32Store8 { .. }).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), 2);
    for store in stores {
        assert!(store > allocation);
        assert!(matches!(ops[store - 1], Operator::I32Load8U { .. }));
        // Both copy loops write final payload + byte offset, not another
        // descriptor or an intermediate fragment buffer.
        assert!(matches!(
            ops[store - 7],
            Operator::LocalGet { local_index: 5 }
        ));
        assert!(matches!(
            ops[store - 6],
            Operator::LocalGet { local_index: 8 }
        ));
        assert!(matches!(ops[store - 5], Operator::I32Add));
    }
}

#[test]
fn join_collisions_are_rejected_only_by_the_private_variant() {
    for name in COUNTERS.into_iter().chain(["__m5_cursor"]) {
        let source = format!(
            "fn {name}(parts: string[], separator: string) -> string {{ return string_join(parts, separator); }}"
        );
        let compiler = Compiler::new();
        let program = compiler.analyze(&source).unwrap();
        let checked = ValidatedProgram::new(&program, &ExportOptions::All).unwrap();
        let ordinary = super::assemble(&checked);
        Validator::new().validate_all(&ordinary).unwrap();
        let memory = checked.layout().memory.as_ref().unwrap();
        let first = memory.first_function.raw();
        assert_eq!(
            observe(
                &ordinary,
                first,
                None,
                Some(first + memory.plan.helper_index(Helper::StringJoin)),
            ),
            Err(CodegenError::DuplicateExportName {
                name: name.to_owned()
            })
        );
        assert_eq!(ordinary, compiler.compile_source(&source).unwrap());
    }
}

#[test]
fn collisions_are_rejected_only_by_the_private_variant() {
    for name in COUNTERS.into_iter().chain(["__m5_cursor"]) {
        let source = format!("fn {name}(a: string) -> string {{ return string_concat(a, a); }}");
        let compiler = Compiler::new();
        let program = compiler.analyze(&source).unwrap();
        let checked = ValidatedProgram::new(&program, &ExportOptions::All).unwrap();
        let ordinary = super::assemble(&checked);
        Validator::new().validate_all(&ordinary).unwrap();
        let memory = checked.layout().memory.as_ref().unwrap();
        let first = memory.first_function.raw();
        assert_eq!(
            observe(
                &ordinary,
                first,
                Some(first + memory.plan.helper_index(Helper::StringConcat)),
                None,
            ),
            Err(CodegenError::DuplicateExportName {
                name: name.to_owned()
            })
        );
        assert_eq!(ordinary, compiler.compile_source(&source).unwrap());
    }
}

// A finite, fixture-only allocation failure: cap both variants at their initial
// pages rather than relying on machine exhaustion or attempting memory32's limit.
pub(super) fn cap_memory(original: &[u8]) -> Vec<u8> {
    let mut module = Module::new();
    for payload in Parser::new(0).parse_all(original) {
        match payload.unwrap() {
            Payload::MemorySection(reader) => {
                let mut section = wasm_encoder::MemorySection::new();
                for memory in reader {
                    let memory = memory.unwrap();
                    section.memory(wasm_encoder::MemoryType {
                        minimum: memory.initial,
                        maximum: Some(memory.initial),
                        memory64: false,
                        shared: false,
                        page_size_log2: None,
                    });
                }
                module.section(&section);
            }
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

#[test]
#[ignore = "requires Node.js"]
fn paired_concat_events_effects_growth_and_caught_trap_retention() {
    use crate::Type;
    use std::{path::PathBuf, process::Command, time::Duration};
    let mut compiler = Compiler::new();
    compiler
        .register_host_function(
            "produce",
            "env",
            "produce",
            vec![Type::Number],
            Type::String,
        )
        .unwrap();
    compiler
        .register_host_function("retain", "env", "retain", vec![Type::String], Type::Void)
        .unwrap();
    let source = r#"
        fn left() -> string { return "λ"; }
        fn right() -> string { return "猫!"; }
        fn empty() -> string { return ""; }
        fn concat(a: string, b: string) -> string { return string_concat(a, b); }
        fn fragments() -> string[] { return resizable([]); }
                fn append(parts: string[], item: string) -> void { parts.push(item); }
                fn effects() -> string { return string_concat(produce(1), produce(2)); }
        fn trapped() -> void {
            retain(string_concat(produce(1), produce(2)));
            string_concat(produce(3), produce(4));
        }
    "#;
    let [ordinary, observed] = modules(&compiler, source);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = root.join("target/m5-string-observations");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("ordinary-capped.wasm"),
        cap_memory(&ordinary),
    )
    .unwrap();
    std::fs::write(
        directory.join("observed-capped.wasm"),
        cap_memory(&observed),
    )
    .unwrap();
    std::fs::write(directory.join("ordinary.wasm"), ordinary).unwrap();
    std::fs::write(directory.join("observed.wasm"), observed).unwrap();
    let output = crate::test_process::output(
        Command::new("node")
            .arg(root.join("tests/support/m5_string_observations.cjs"))
            .arg(&directory),
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

#[test]
fn observations_are_private_and_deterministic() {
    let source = "fn concat(a: string, b: string) -> string { return string_concat(a, b); }";
    let compiler = Compiler::new();
    let [ordinary, observed] = modules(&compiler, source);
    assert_eq!(ordinary, compiler.compile_source(source).unwrap());
    assert_eq!(
        [ordinary.clone(), observed.clone()],
        modules(&compiler, source)
    );
    assert!(
        !export_names(&ordinary)
            .iter()
            .any(|name| name.starts_with("__m5_"))
    );
    for name in COUNTERS.into_iter().chain(["__m5_cursor"]) {
        assert!(
            export_names(&observed).iter().any(|export| export == name),
            "missing {name}"
        );
    }
}
