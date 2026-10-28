use super::{Config, Error, compile};
use crate::{Compiler, exports::ExportOptions};
use wasmparser::{BlockType, ExternalKind, Operator, Parser, Payload, TypeRef, ValType, Validator};

#[derive(Debug, PartialEq, Eq)]
enum Effect {
    Call(u32),
    GlobalSet(u32),
    MemoryGrow,
    Load {
        width: u8,
        offset: u64,
        align: u8,
        memory: u32,
    },
    Store {
        width: u8,
        offset: u64,
        align: u8,
        memory: u32,
    },
    Other,
}

#[derive(Debug, Default)]
struct Decoded {
    types: Vec<(Vec<ValType>, Vec<ValType>)>,
    imports: Vec<(String, String, u32)>,
    functions: Vec<u32>,
    exports: Vec<(String, ExternalKind, u32)>,
    bodies: Vec<Vec<String>>,
    raw_bodies: Vec<Vec<u8>>,
    effects: Vec<Vec<Effect>>,
    data_section: Vec<u8>,
    globals: Vec<(ValType, bool, Vec<String>)>,
    memory: Vec<(u64, Option<u64>)>,
    data: Vec<Vec<u8>>,
}

fn decode(bytes: &[u8]) -> Decoded {
    Validator::new().validate_all(bytes).unwrap();
    let mut module = Decoded::default();
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.unwrap();
        if let Some((11, range)) = payload.as_section() {
            module.data_section = bytes
                [usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap()]
                .to_vec();
        }
        match payload {
            Payload::TypeSection(reader) => {
                for ty in reader.into_iter_err_on_gc_types() {
                    let ty = ty.unwrap();
                    module
                        .types
                        .push((ty.params().to_vec(), ty.results().to_vec()));
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import.unwrap();
                    let TypeRef::Func(index) = import.ty else {
                        panic!("expected function import");
                    };
                    module
                        .imports
                        .push((import.module.into(), import.name.into(), index));
                }
            }
            Payload::FunctionSection(reader) => {
                module.functions = reader.into_iter().map(Result::unwrap).collect();
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.unwrap();
                    module
                        .exports
                        .push((export.name.into(), export.kind, export.index));
                }
            }
            Payload::CodeSectionEntry(body) => {
                let range = body.range();
                module.raw_bodies.push(
                    bytes[usize::try_from(range.start).unwrap()
                        ..usize::try_from(range.end).unwrap()]
                        .to_vec(),
                );
                module.effects.push(
                    body.get_operators_reader()
                        .unwrap()
                        .into_iter()
                        .map(|op| match op.unwrap() {
                            Operator::Call { function_index } => Effect::Call(function_index),
                            Operator::GlobalSet { global_index } => Effect::GlobalSet(global_index),
                            Operator::MemoryGrow { .. } => Effect::MemoryGrow,
                            Operator::I32Load { memarg } => Effect::Load {
                                width: 32,
                                offset: memarg.offset,
                                align: memarg.align,
                                memory: memarg.memory,
                            },
                            Operator::I64Load { memarg } => Effect::Load {
                                width: 64,
                                offset: memarg.offset,
                                align: memarg.align,
                                memory: memarg.memory,
                            },
                            Operator::I32Store { memarg } => Effect::Store {
                                width: 32,
                                offset: memarg.offset,
                                align: memarg.align,
                                memory: memarg.memory,
                            },
                            Operator::I64Store { memarg } => Effect::Store {
                                width: 64,
                                offset: memarg.offset,
                                align: memarg.align,
                                memory: memarg.memory,
                            },
                            _ => Effect::Other,
                        })
                        .collect(),
                );
                module.bodies.push(
                    body.get_operators_reader()
                        .unwrap()
                        .into_iter()
                        .map(|op| format!("{:?}", op.unwrap()))
                        .collect(),
                );
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    let global = global.unwrap();
                    let init = global
                        .init_expr
                        .get_operators_reader()
                        .into_iter()
                        .map(|op| format!("{:?}", op.unwrap()))
                        .collect();
                    module
                        .globals
                        .push((global.ty.content_type, global.ty.mutable, init));
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = memory.unwrap();
                    module.memory.push((memory.initial, memory.maximum));
                }
            }
            Payload::DataSection(reader) => {
                module.data = reader
                    .into_iter()
                    .map(|data| data.unwrap().data.to_vec())
                    .collect();
            }
            _ => {}
        }
    }
    module
}

fn export_index(module: &Decoded, name: &str) -> u32 {
    module
        .exports
        .iter()
        .find(|(export, kind, _)| export == name && *kind == ExternalKind::Func)
        .unwrap_or_else(|| panic!("missing function export {name}"))
        .2
}

fn operators<'a>(bytes: &'a [u8], module: &Decoded, function: u32) -> Vec<Operator<'a>> {
    Parser::new(0)
        .parse_all(bytes)
        .filter_map(|payload| match payload.unwrap() {
            Payload::CodeSectionEntry(body) => Some(body),
            _ => None,
        })
        .nth(usize::try_from(function).unwrap() - module.imports.len())
        .unwrap()
        .get_operators_reader()
        .unwrap()
        .into_iter()
        .map(Result::unwrap)
        .collect()
}

// Independent contract table, deliberately not derived from allocator constants
// or its emitted shift. Large classes are checked without reserving their payloads.
const EXPECTED_CAPACITIES: [u64; 29] = [
    8,
    16,
    32,
    64,
    128,
    256,
    512,
    1_024,
    2_048,
    4_096,
    8_192,
    16_384,
    32_768,
    65_536,
    131_072,
    262_144,
    524_288,
    1_048_576,
    2_097_152,
    4_194_304,
    8_388_608,
    16_777_216,
    33_554_432,
    67_108_864,
    134_217_728,
    268_435_456,
    536_870_912,
    1_073_741_824,
    2_147_483_648,
];

#[test]
fn private_compilation_emits_valid_management_exports() {
    let program = Compiler::new()
        .analyze("fn main() -> number { let values = [1]; return values[0]; }")
        .unwrap();
    let bytes = compile(&program, &ExportOptions::All, Config::default()).unwrap();
    let module = decode(&bytes);
    for (name, params, results) in [
        (
            "__m3_acquire",
            vec![ValType::I64],
            vec![ValType::I32, ValType::I64],
        ),
        ("__m3_release", vec![ValType::I32, ValType::I64], vec![]),
        ("__m3_class", vec![ValType::I64], vec![ValType::I32]),
        (
            "__m3_observe",
            vec![ValType::I32, ValType::I32],
            vec![ValType::I64],
        ),
    ] {
        let index = export_index(&module, name) as usize - module.imports.len();
        assert_eq!(
            module.types[module.functions[index] as usize],
            (params, results),
            "{name}"
        );
    }
}

#[test]
fn appended_management_preserves_existing_indices_bodies_and_ordinary_bytes() {
    let mut compiler = Compiler::new();
    compiler
        .register_host_function(
            "host",
            "env",
            "host",
            vec![crate::ast::Type::Number],
            crate::ast::Type::Number,
        )
        .unwrap();
    let program = compiler
        .analyze(
            r#"
        fn main(n: number) -> number {
            let values = [n, 7];
            let step = (x: number) => x + values[0];
            let label = "m3";
            if label != "m3" { return 0; }
            let span = 1..=5;
            if n in span { return host(step(n % 3)); }
            return values[1];
        }
    "#,
        )
        .unwrap();
    let ordinary = compiler.compile(&program).unwrap();
    let baseline = decode(&ordinary);
    let bytes = compile(&program, &ExportOptions::All, Config::default()).unwrap();
    let managed = decode(&bytes);
    assert_eq!(managed.imports, baseline.imports);
    assert_eq!(&managed.types[..baseline.types.len()], baseline.types);
    assert_eq!(
        &managed.functions[..baseline.functions.len()],
        baseline.functions
    );
    assert_eq!(&managed.bodies[..baseline.bodies.len()], baseline.bodies);
    assert_eq!(
        &managed.raw_bodies[..baseline.raw_bodies.len()],
        baseline.raw_bodies
    );
    assert_eq!(managed.data_section, baseline.data_section);
    assert!(!baseline.data_section.is_empty());
    assert_eq!(&managed.globals[..baseline.globals.len()], baseline.globals);
    assert_eq!(&managed.exports[..baseline.exports.len()], baseline.exports);
    assert_eq!(managed.memory, baseline.memory);
    assert_eq!(managed.data, baseline.data);
    for name in ["__m3_acquire", "__m3_release", "__m3_class", "__m3_observe"] {
        assert!(
            export_index(&managed, name) as usize
                >= baseline.imports.len() + baseline.functions.len()
        );
    }
    assert_eq!(
        bytes,
        compile(&program, &ExportOptions::All, Config::default()).unwrap()
    );
    assert_eq!(ordinary, compiler.compile(&program).unwrap());
    assert!(
        baseline
            .exports
            .iter()
            .all(|(name, _, _)| !name.starts_with("__m3_"))
    );
}

#[test]
fn private_compilation_requires_planned_memory_and_preserves_checked_body_errors() {
    let compiler = Compiler::new();
    let scalar = compiler
        .analyze("fn main() -> number { return 1; }")
        .unwrap();
    assert!(matches!(
        compile(&scalar, &ExportOptions::All, Config::default()),
        Err(Error::MissingMemory)
    ));
    let mut program = compiler
        .analyze("fn main() -> number[] { return [1]; }")
        .unwrap();
    program.functions[0].id = crate::typed_ast::FunctionId(9);
    let expected = compiler.compile(&program).unwrap_err();
    let Error::Checked(actual) =
        compile(&program, &ExportOptions::All, Config::default()).unwrap_err()
    else {
        panic!("expected the checked backend diagnostic");
    };
    assert_eq!(actual, expected);
    assert_eq!(actual.to_string(), expected.to_string());
    assert_eq!(actual.span(), expected.span());
}

#[test]
fn management_uses_one_backing_call_and_registry_authority_before_header_access() {
    let program = Compiler::new()
        .analyze("fn main() -> number[] { return [1]; }")
        .unwrap();
    let baseline = decode(&crate::codegen::compile(&program).unwrap());
    let module = decode(&compile(&program, &ExportOptions::All, Config::default()).unwrap());
    let acquire = export_index(&module, "__m3_acquire") as usize - module.imports.len();
    let release = export_index(&module, "__m3_release") as usize - module.imports.len();
    let backing = export_index(&baseline, "__alloc");
    assert_eq!(
        module.effects[acquire]
            .iter()
            .filter(|op| **op == Effect::Call(backing))
            .count(),
        1
    );
    // Only the unchanged ordinary allocator may grow memory or update its cursor.
    for body in &module.effects[baseline.effects.len()..] {
        assert!(!body.contains(&Effect::MemoryGrow));
        assert!(!body.contains(&Effect::GlobalSet(0)));
    }
    let stores = |body: usize| {
        module.effects[body]
            .iter()
            .filter_map(|op| {
                if let Effect::Store {
                    width,
                    offset,
                    align,
                    memory,
                } = op
                {
                    Some((*width, *offset, *align, *memory))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        stores(acquire),
        [
            (32, 0, 2, 0),
            (32, 8, 2, 0),
            (32, 4, 2, 0),
            (32, 12, 2, 0),
            (64, 16, 3, 0),
            (64, 24, 3, 0)
        ]
    );
    assert_eq!(stores(release), [(32, 4, 2, 0), (32, 12, 2, 0)]);
    let release_effects = &module.effects[release];
    let (find_position, find) = release_effects
        .iter()
        .enumerate()
        .find_map(|(position, op)| {
            if let Effect::Call(index) = op {
                Some((position, *index))
            } else {
                None
            }
        })
        .expect("release calls registry lookup");
    assert!(
        release_effects[..find_position]
            .iter()
            .all(|op| *op == Effect::Other)
    );
    assert!(
        module.bodies[release][find_position + 1..]
            .iter()
            .position(|op| op == "Unreachable")
            .unwrap()
            < release_effects[find_position + 1..]
                .iter()
                .position(|op| matches!(op, Effect::Load { .. } | Effect::Store { .. }))
                .unwrap()
    );
    assert!(find as usize >= baseline.imports.len() + baseline.functions.len());
    let find = find as usize - module.imports.len();
    assert_eq!(
        module.types[module.functions[find] as usize],
        (vec![ValType::I32], vec![ValType::I32])
    );
    let lookup_accesses: Vec<_> = module.effects[find]
        .iter()
        .filter(|op| matches!(op, Effect::Load { .. } | Effect::Store { .. }))
        .collect();
    assert_eq!(
        lookup_accesses,
        [&Effect::Load {
            width: 32,
            offset: 0,
            align: 2,
            memory: 0
        }]
    );
    assert!(
        module.bodies[find]
            .windows(4)
            .any(|ops| ops[0] == "I32Const { value: 32 }"
                && ops[1] == "I32Add"
                && ops[2] == "LocalGet { local_index: 0 }"
                && ops[3] == "I32Eq")
    );
    let release_loads: Vec<_> = release_effects
        .iter()
        .filter_map(|op| {
            if let Effect::Load {
                width,
                offset,
                align,
                memory,
            } = op
            {
                Some((*width, *offset, *align, *memory))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        release_loads,
        [(64, 16, 3, 0), (32, 12, 2, 0), (32, 8, 2, 0)]
    );
}

#[test]
fn decoded_rounding_covers_every_capacity_and_adjacent_request() {
    use Operator::*;

    let program = Compiler::new()
        .analyze("fn main() -> number[] { return [1]; }")
        .unwrap();
    let bytes = compile(&program, &ExportOptions::All, Config::default()).unwrap();
    let module = decode(&bytes);
    let body = operators(&bytes, &module, export_index(&module, "__m3_class"));
    let maximum = body
        .windows(10)
        .find_map(|ops| match ops {
            [
                LocalGet { local_index: 0 },
                I64Const { value: 0 },
                I64LtS,
                LocalGet { local_index: 0 },
                I64Const { value: maximum },
                I64GtU,
                I32Or,
                If {
                    blockty: BlockType::Empty,
                },
                Unreachable,
                End,
            ] => Some(u64::try_from(*maximum).unwrap()),
            _ => None,
        })
        .expect("signed-negative and unsigned upper guards trap before rounding");
    assert_eq!(maximum, 2_147_483_648);
    let (minimum, capacity_local) = body
        .windows(4)
        .find_map(|ops| match ops {
            [
                I64Const { value },
                LocalSet { local_index },
                Block {
                    blockty: BlockType::Empty,
                },
                Loop {
                    blockty: BlockType::Empty,
                },
            ] => Some((u64::try_from(*value).unwrap(), *local_index)),
            _ => None,
        })
        .expect("rounding initializes capacity before its loop");
    assert_eq!(minimum, 8);
    let (shift, increment) = body
        .windows(18)
        .find_map(|ops| match ops {
            [
                Loop {
                    blockty: BlockType::Empty,
                },
                LocalGet { local_index: a },
                LocalGet { local_index: 0 },
                I64GeU,
                BrIf { relative_depth: 1 },
                LocalGet { local_index: b },
                I64Const { value: shift },
                I64Shl,
                LocalSet { local_index: c },
                LocalGet { local_index: d },
                I32Const { value: increment },
                I32Add,
                LocalSet { local_index: e },
                Br { relative_depth: 0 },
                End,
                End,
                LocalGet {
                    local_index: result,
                },
                End,
            ] if *a == capacity_local && a == b && b == c && d == e && e == result => Some((
                u32::try_from(*shift).unwrap(),
                u32::try_from(*increment).unwrap(),
            )),
            _ => None,
        })
        .expect("unsigned capacity test exits; otherwise capacity and class advance together");
    assert_eq!((shift, increment), (1, 1));
    assert!(
        body.iter()
            .all(|op| !matches!(op, Call { .. } | GlobalSet { .. }))
    );

    // Evaluate only the decoded rounding recurrence. Expected classes come from
    // table positions, not from that recurrence; Node tests execute the full helper.
    let mut cases = vec![(0, Some(0)), (1, Some(0))];
    for (class, capacity) in EXPECTED_CAPACITIES.into_iter().enumerate() {
        let class = u32::try_from(class).unwrap();
        cases.extend([
            (capacity - 1, Some(class)),
            (capacity, Some(class)),
            (capacity + 1, (class < 28).then_some(class + 1)),
        ]);
    }
    for (request, expected) in cases {
        let actual = if request > maximum {
            None
        } else {
            let mut capacity = minimum;
            let mut class = 0;
            while capacity < request {
                assert!(class < 29, "rounding must terminate");
                capacity = capacity.checked_shl(shift).unwrap();
                class += increment;
            }
            assert_eq!(
                capacity,
                EXPECTED_CAPACITIES[usize::try_from(class).unwrap()]
            );
            Some(class)
        };
        assert_eq!(actual, expected, "request {request}");
    }
}

#[test]
fn decoded_free_list_dispatch_has_one_distinct_global_per_class() {
    use Operator::*;

    let program = Compiler::new()
        .analyze("fn main() -> number[] { return [1]; }")
        .unwrap();
    let bytes = compile(&program, &ExportOptions::All, Config::default()).unwrap();
    let module = decode(&bytes);
    let acquire = operators(&bytes, &module, export_index(&module, "__m3_acquire"));
    let (head, set_head) = (
        acquire
            .windows(3)
            .find_map(|ops| match ops {
                [LocalGet { .. }, Call { function_index }, LocalTee { .. }] => {
                    Some(*function_index)
                }
                _ => None,
            })
            .expect("acquire reads its exact class head"),
        acquire
            .windows(3)
            .find_map(|ops| match ops {
                [I32Load { memarg }, Call { function_index }, End] if memarg.offset == 4 => {
                    Some(*function_index)
                }
                _ => None,
            })
            .expect("reuse pops that class's free link"),
    );
    let reads = operators(&bytes, &module, head);
    let writes = operators(&bytes, &module, set_head);
    assert_eq!(reads.len(), 29 * 7 + 2);
    assert_eq!(writes.len(), 29 * 8 + 2);
    let mut globals = Vec::new();
    for (class, _) in EXPECTED_CAPACITIES.iter().enumerate() {
        let read_global = match &reads[class * 7..class * 7 + 7] {
            [
                LocalGet { local_index: 0 },
                I32Const { value },
                I32Eq,
                If {
                    blockty: BlockType::Empty,
                },
                GlobalGet { global_index },
                Return,
                End,
            ] if *value == i32::try_from(class).unwrap() => *global_index,
            other => panic!("class {class} head read: {other:?}"),
        };
        match &writes[class * 8..class * 8 + 8] {
            [
                LocalGet { local_index: 0 },
                I32Const { value },
                I32Eq,
                If {
                    blockty: BlockType::Empty,
                },
                LocalGet { local_index: 1 },
                GlobalSet { global_index },
                Return,
                End,
            ] if *value == i32::try_from(class).unwrap() => {
                assert_eq!(*global_index, read_global, "class {class}");
            }
            other => panic!("class {class} head write: {other:?}"),
        }
        assert!(
            !globals.contains(&read_global),
            "classes must not alias heads"
        );
        globals.push(read_global);
        let global = &module.globals[usize::try_from(read_global).unwrap()];
        assert_eq!(
            global,
            &(
                ValType::I32,
                true,
                vec!["I32Const { value: 0 }".into(), "End".into()]
            )
        );
    }
    assert!(matches!(&reads[29 * 7..], [Unreachable, End]));
    assert!(matches!(&writes[29 * 8..], [Unreachable, End]));
}

#[test]
fn decoded_reservation_math_preserves_large_capacities_and_unsigned_addresses() {
    use Operator::*;

    let program = Compiler::new()
        .analyze("fn main() -> number[] { return [1]; }")
        .unwrap();
    let bytes = compile(
        &program,
        &ExportOptions::All,
        Config {
            last_identity: 0,
            backing_limit: Some(4_294_901_760),
        },
    )
    .unwrap();
    let module = decode(&bytes);
    let acquire = operators(&bytes, &module, export_index(&module, "__m3_acquire"));
    let (base, class_local, capacity_local) = acquire
        .windows(5)
        .find_map(|ops| match ops {
            [
                I64Const { value },
                LocalGet { local_index: class },
                I64ExtendI32U,
                I64Shl,
                LocalSet {
                    local_index: capacity,
                },
            ] => Some((u64::try_from(*value).unwrap(), *class, *capacity)),
            _ => None,
        })
        .expect("capacity is shifted in i64, including class 28");
    assert_eq!(base, 8);
    let (record_bytes, total_local) = acquire
        .windows(5)
        .find_map(|ops| match ops {
            [
                LocalGet {
                    local_index: capacity,
                },
                I64Const { value },
                I64Add,
                LocalSet { local_index: total },
                LocalGet {
                    local_index: checked,
                },
            ] if *capacity == capacity_local && total == checked => {
                Some((u64::try_from(*value).unwrap(), *total))
            }
            _ => None,
        })
        .expect("record sizing adds to capacity before checking the total");
    assert_eq!(record_bytes, 32);
    assert!(acquire.windows(6).any(|ops| matches!(ops,
        [LocalGet { local_index }, I64Const { value: 4_294_967_295 }, I64GtU,
            If { blockty: BlockType::Empty }, Unreachable, End] if *local_index == total_local
    )));
    assert!(acquire.windows(9).any(|ops| matches!(ops,
        [GlobalGet { global_index: 0 }, I64ExtendI32U, LocalGet { local_index }, I64Add,
            I64Const { value: 4_294_901_760 }, I64GtU,
            If { blockty: BlockType::Empty }, Unreachable, End] if *local_index == total_local
    )));
    let backing = export_index(&module, "__alloc");
    assert!(acquire.windows(3).any(|ops| matches!(ops,
        [LocalGet { local_index }, I32WrapI64, Call { function_index }]
            if *local_index == total_local && *function_index == backing
    )));
    assert!(acquire.windows(5).any(|ops| matches!(
        ops,
        [
            LocalGet { .. },
            I32Const { value: 32 },
            I32Add,
            LocalGet { .. },
            End
        ]
    )));
    let observe = operators(&bytes, &module, export_index(&module, "__m3_observe"));
    assert!(observe.windows(5).any(|ops| matches!(ops,
        [I64Const { value: 8 }, LocalGet { .. }, I32Load { memarg }, I64ExtendI32U, I64Shl]
            if memarg.offset == 8 && memarg.align == 2 && memarg.memory == 0
    )));
    // The backing allocator, not M3, enforces the memory ceiling. Its request
    // must remain unsigned after the i32 transport sets its high bit.
    let backing_body = operators(&bytes, &module, backing);
    assert!(backing_body.windows(6).any(|ops| matches!(
        ops,
        [
            LocalGet { local_index: 0 },
            I64ExtendI32U,
            I64Const { value: 7 },
            I64Add,
            I64Const { value: -8 },
            I64And
        ]
    )));
    assert!(backing_body.windows(7).any(|ops| matches!(
        ops,
        [
            LocalGet { .. },
            LocalGet { .. },
            I64Add,
            LocalTee { .. },
            I64Const {
                value: 4_294_901_760
            },
            I64GtU,
            If {
                blockty: BlockType::Empty
            }
        ]
    )));
    assert!(class_local != capacity_local);

    for (class, expected) in EXPECTED_CAPACITIES.into_iter().enumerate() {
        let capacity = base << class;
        assert_eq!(capacity, expected, "class {class}");
        let total = capacity + record_bytes;
        assert_eq!(total, expected + 32);
        assert_eq!(total % 8, 0, "combined reservation remains aligned");
        assert!(u32::try_from(total).is_ok());
    }
    // Synthetic cursor/capacity cases cross signed-i32 and memory32 boundaries;
    // no backing storage is acquired. Expected addresses/ends are literal oracles.
    for (cursor, class, payload, end, within_memory) in [
        (8_u32, 28, 40_u32, 2_147_483_688_u64, true),
        (0x7fff_ffe0, 0, 0x8000_0000, 0x8000_0008, true),
        (0xfffe_ffd8, 0, 0xfffe_fff8, 0xffff_0000, true),
        (0xfffe_ffe0, 0, 0xffff_0000, 0xffff_0008, false),
        (0x8000_0000, 28, 0x8000_0020, 0x1_0000_0020, false),
    ] {
        let capacity = base << class;
        let actual_payload = u64::from(cursor) + record_bytes;
        let actual_end = actual_payload + capacity;
        assert_eq!(actual_payload, u64::from(payload));
        assert_eq!(actual_end, end);
        assert_eq!(actual_payload % 8, 0);
        assert_eq!(actual_end <= 4_294_901_760, within_memory);
        if within_memory {
            assert!(actual_payload + EXPECTED_CAPACITIES[class] <= actual_end);
        }
    }
}

#[test]
fn management_export_collisions_reject_only_selected_user_exports() {
    for name in ["__m3_class", "__m3_acquire", "__m3_release", "__m3_observe"] {
        let program = Compiler::new()
            .analyze(&format!("fn {name}() -> number[] {{ return [1]; }}"))
            .unwrap();
        let ordinary = crate::codegen::compile(&program).unwrap();
        let Error::Checked(actual) =
            compile(&program, &ExportOptions::All, Config::default()).unwrap_err()
        else {
            panic!("expected export collision for {name}");
        };
        assert_eq!(
            actual,
            crate::codegen::CodegenError::DuplicateExportName { name: name.into() }
        );
        let hidden = decode(&compile(&program, &ExportOptions::None, Config::default()).unwrap());
        assert_eq!(
            hidden
                .exports
                .iter()
                .filter(|(export, _, _)| export == name)
                .count(),
            1
        );
        assert!(export_index(&hidden, name) as usize >= program.functions.len());
        assert_eq!(ordinary, crate::codegen::compile(&program).unwrap());
    }
}

#[test]
fn class_helper_checks_wide_domain_before_narrowing_and_globals_are_private() {
    let program = Compiler::new()
        .analyze("fn main() -> number[] { return [1]; }")
        .unwrap();
    let baseline = decode(&crate::codegen::compile(&program).unwrap());
    let module = decode(
        &compile(
            &program,
            &ExportOptions::All,
            Config {
                last_identity: 42,
                backing_limit: None,
            },
        )
        .unwrap(),
    );
    let class = export_index(&module, "__m3_class") as usize - module.imports.len();
    let body = &module.bodies[class];
    let narrowing = body.iter().position(|op| op == "I32WrapI64");
    let trap = body
        .iter()
        .position(|op| op == "Unreachable")
        .expect("invalid requests trap");
    assert!(narrowing.is_none_or(|position| trap < position));
    assert!(body.iter().any(|op| op == "I64Const { value: 2147483648 }"));
    assert!(
        body.iter()
            .any(|op| op.starts_with("I64Gt") || op.starts_with("I64Lt"))
    );
    let appended = &module.globals[baseline.globals.len()..];
    assert!(
        appended
            .iter()
            .filter(|(ty, mutable, init)| *ty == ValType::I32
                && *mutable
                && init == &["I32Const { value: 0 }", "End"])
            .count()
            >= 29
    );
    assert!(
        appended
            .iter()
            .any(|(ty, mutable, init)| *ty == ValType::I64
                && *mutable
                && init == &["I64Const { value: 42 }", "End"])
    );
    assert!(
        module
            .exports
            .iter()
            .all(|(_, kind, _)| *kind != ExternalKind::Global)
    );
}
