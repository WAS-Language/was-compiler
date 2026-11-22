use crate::{Compiler, ExportOptions, OptimizationLevel, Type};
use std::{fs, path::PathBuf, process::Command, time::Duration};
use wasm_encoder::{
    CodeSection, ConstExpr, Encode, ExportKind, ExportSection, GlobalSection, GlobalType,
    Instruction, Module, RawSection, ValType,
};
use wasmparser::{ExternalKind, FunctionBody, Operator, Parser, Payload, Validator};

const CURSOR: &str = "__m6_cursor";
const RECORDS: &str = "__m6_records";

const SOURCE: &str = include_str!("../../../../tests/support/m6_range_allocations.was");
const LEVELS: [OptimizationLevel; 4] = [
    OptimizationLevel::None,
    OptimizationLevel::Basic,
    OptimizationLevel::Standard,
    OptimizationLevel::Aggressive,
];
const EXPORTS: [&str; 5] = [
    "direct",
    "local",
    "indirect_binding",
    "indirect_call",
    "abrupt",
];

struct Decoded<'a> {
    bodies: Vec<FunctionBody<'a>>,
    exports: Vec<wasmparser::Export<'a>>,
    imports: u32,
}

impl<'a> Decoded<'a> {
    fn read(bytes: &'a [u8]) -> Self {
        Validator::new().validate_all(bytes).unwrap();
        let mut decoded = Self {
            bodies: Vec::new(),
            exports: Vec::new(),
            imports: 0,
        };
        for payload in Parser::new(0).parse_all(bytes) {
            match payload.unwrap() {
                Payload::ImportSection(reader) => {
                    for import in reader.into_imports() {
                        assert!(matches!(import.unwrap().ty, wasmparser::TypeRef::Func(_)));
                        decoded.imports += 1;
                    }
                }
                Payload::ExportSection(reader) => decoded
                    .exports
                    .extend(reader.into_iter().map(Result::unwrap)),
                Payload::CodeSectionEntry(body) => decoded.bodies.push(body),
                _ => {}
            }
        }
        decoded
    }

    fn exported_function(&self, name: &str) -> u32 {
        let export = self
            .exports
            .iter()
            .find(|export| export.name == name)
            .unwrap();
        assert_eq!(export.kind, ExternalKind::Func);
        export.index
    }

    fn body(&self, index: u32) -> &FunctionBody<'a> {
        let slot = index
            .checked_sub(self.imports)
            .expect("expected a defined function");
        &self.bodies[usize::try_from(slot).unwrap()]
    }

    fn calls(&self, index: u32) -> Vec<u32> {
        operators(self.body(index))
            .filter_map(|op| match op {
                Operator::Call { function_index } => Some(function_index),
                _ => None,
            })
            .collect()
    }
}

fn operators<'a>(body: &FunctionBody<'a>) -> impl Iterator<Item = Operator<'a>> + use<'a> {
    body.get_operators_reader()
        .unwrap()
        .into_iter()
        .map(Result::unwrap)
}

fn assert_lowering(decoded: &Decoded<'_>, baseline: bool) -> (usize, u32) {
    let allocator = decoded.exported_function("__alloc");
    // Identify the constructor by the private record's three integer field stores,
    // independently of the compiler's helper plan and ABI constants.
    let constructors: Vec<_> = decoded
        .bodies
        .iter()
        .enumerate()
        .filter(|(_, body)| {
            operators(body)
                .filter_map(|op| match op {
                    Operator::I64Store { memarg } => Some(memarg.offset),
                    _ => None,
                })
                .eq([0, 8, 16])
        })
        .collect();
    let &[(slot, body)] = constructors.as_slice() else {
        panic!("expected one range constructor")
    };
    let constructor = decoded
        .imports
        .checked_add(u32::try_from(slot).unwrap())
        .unwrap();
    assert_eq!(decoded.calls(constructor), [allocator]);
    let ops: Vec<_> = operators(body).collect();
    assert!(ops.windows(2).any(|pair| matches!(pair,
        [Operator::I32Const { value: 32 }, Operator::Call { function_index }] if *function_index == allocator
    )));

    let index = |name| decoded.exported_function(name);
    let run = decoded.calls(index("local"));
    let [run] = run.as_slice() else {
        panic!("expected a direct call to the local helper")
    };
    let count = |function, callee| {
        decoded
            .calls(function)
            .into_iter()
            .filter(|&call| call == callee)
            .count()
    };
    for function in [index("direct"), *run, index("abrupt")] {
        assert_eq!(count(function, constructor), usize::from(baseline));
    }
    assert_eq!(count(index("indirect_binding"), constructor), 1);
    let indirect = decoded.calls(index("indirect_call"));
    let make = indirect[0];
    assert_ne!(make, constructor);
    assert_eq!(count(index("indirect_call"), constructor), 0);
    assert_eq!(count(make, constructor), 1);
    (slot, allocator)
}

fn module_pairs() -> [[Vec<u8>; 2]; 4] {
    let mut compiler = Compiler::new();
    compiler.set_exports(ExportOptions::Only(EXPORTS.map(String::from).to_vec()));
    compiler
        .register_host_function(
            "checkpoint",
            "env",
            "checkpoint",
            vec![Type::Number],
            Type::Void,
        )
        .unwrap();
    let mut enabled = None;
    LEVELS.map(|level| {
        let baseline = level == OptimizationLevel::None;
        compiler.set_optimization_level(level);
        let ordinary = compiler.compile_source(SOURCE).unwrap();
        assert_eq!(compiler.compile_source(SOURCE).unwrap(), ordinary);
        if !baseline {
            assert_eq!(enabled.get_or_insert_with(|| ordinary.clone()), &ordinary);
        }
        let decoded = Decoded::read(&ordinary);
        assert!(
            decoded
                .exports
                .iter()
                .all(|export| !matches!(export.name, CURSOR | RECORDS))
        );
        let (constructor, allocator) = assert_lowering(&decoded, baseline);
        let observed = observe(&ordinary, &decoded, constructor, allocator);
        let traced = Decoded::read(&observed);
        assert_eq!(decoded.imports, traced.imports);
        assert_eq!(decoded.bodies.len(), traced.bodies.len());
        for (index, (before, after)) in decoded.bodies.iter().zip(traced.bodies).enumerate() {
            if index != constructor {
                assert_eq!(before.as_bytes(), after.as_bytes());
            }
        }
        assert_eq!(compiler.compile_source(SOURCE).unwrap(), ordinary);
        [ordinary, observed]
    })
}

#[test]
fn ordinary_lowering_and_private_observation_are_deterministic() {
    assert_eq!(module_pairs(), module_pairs());
}

#[test]
#[ignore = "requires Node.js on PATH"]
fn attributable_range_record_elimination() {
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/m6-range-allocations");
    fs::create_dir_all(&directory).unwrap();
    for (preset, [ordinary, observed]) in module_pairs().into_iter().enumerate() {
        fs::write(directory.join(format!("o{preset}-ordinary.wasm")), ordinary).unwrap();
        fs::write(directory.join(format!("o{preset}-observed.wasm")), observed).unwrap();
    }
    let output = crate::test_process::output(
        Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/support/m6_range_allocations.cjs"
            ))
            .arg(directory),
        None,
        Duration::from_secs(30),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}

// Patch a copy of the fixture's ordinary bytes, never the compiler pipeline.
fn observe(original: &[u8], decoded: &Decoded<'_>, constructor: usize, allocator: u32) -> Vec<u8> {
    let cursor = 0;
    let records = 1;
    let alloc_body = decoded.body(allocator);
    assert!(
        operators(alloc_body)
            .filter_map(|op| match op {
                Operator::GlobalSet { global_index } => Some(global_index),
                _ => None,
            })
            .eq([cursor])
    );

    let mut module = Module::new();
    for payload in Parser::new(0).parse_all(original) {
        match payload.unwrap() {
            Payload::GlobalSection(reader) => {
                // This fixture only uses the ordinary bump allocator.
                assert_eq!(reader.count(), 1);
                let heap = reader.into_iter().next().unwrap().unwrap();
                assert_eq!(heap.ty.content_type, wasmparser::ValType::I32);
                assert!(heap.ty.mutable && !heap.ty.shared);
                let mut globals = GlobalSection::new();
                let Operator::I32Const { value: heap_start } =
                    heap.init_expr.get_operators_reader().read().unwrap()
                else {
                    panic!("expected an i32 heap start");
                };
                globals.global(
                    GlobalType {
                        val_type: ValType::I32,
                        mutable: true,
                        shared: false,
                    },
                    &ConstExpr::i32_const(heap_start),
                );
                globals.global(
                    GlobalType {
                        val_type: ValType::I64,
                        mutable: true,
                        shared: false,
                    },
                    &ConstExpr::i64_const(0),
                );
                module.section(&globals);
            }
            Payload::ExportSection(_) => {
                let mut exports = ExportSection::new();
                for export in &decoded.exports {
                    let kind = match export.kind {
                        ExternalKind::Func => ExportKind::Func,
                        ExternalKind::Memory => ExportKind::Memory,
                        other => panic!("unexpected fixture export: {other:?}"),
                    };
                    exports.export(export.name, kind, export.index);
                }
                exports.export(CURSOR, ExportKind::Global, cursor);
                exports.export(RECORDS, ExportKind::Global, records);
                module.section(&exports);
            }
            Payload::CodeSectionStart { .. } => {
                let mut code = CodeSection::new();
                for (index, body) in decoded.bodies.iter().enumerate() {
                    if index != constructor {
                        code.raw(body.as_bytes());
                        continue;
                    }
                    let mut ops = body.get_operators_reader().unwrap();
                    let offset = loop {
                        if matches!(ops.read().unwrap(), Operator::Call { function_index } if function_index == allocator)
                        {
                            break ops
                                .original_position()
                                .checked_sub(body.range().start)
                                .unwrap();
                        }
                    };
                    let (prefix, suffix) =
                        body.as_bytes().split_at(usize::try_from(offset).unwrap());
                    let mut bytes = prefix.to_vec();
                    // Count only after successful allocation; keep the returned handle on the stack.
                    for op in [
                        Instruction::GlobalGet(records),
                        Instruction::I64Const(1),
                        Instruction::I64Add,
                        Instruction::GlobalSet(records),
                    ] {
                        op.encode(&mut bytes);
                    }
                    bytes.extend_from_slice(suffix);
                    code.raw(&bytes);
                }
                module.section(&code);
            }
            Payload::CodeSectionEntry(_) => {}
            payload => {
                if let Some((id, range)) = payload.as_section() {
                    module.section(&RawSection {
                        id,
                        data: &original[usize::try_from(range.start).unwrap()
                            ..usize::try_from(range.end).unwrap()],
                    });
                }
            }
        }
    }
    module.finish()
}
