use super::{Config, Error, compile};
use crate::{Compiler, exports::ExportOptions};
use wasmparser::{ExternalKind, Operator, Parser, Payload, TypeRef, ValType, Validator};

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
