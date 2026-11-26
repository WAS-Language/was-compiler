use super::binaryen_guard::{GuardError, InspectError, check};
use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
    Instruction, MemorySection, MemoryType, Module, StartSection, TypeSection, ValType,
};

fn imports(params: &[ValType], entries: &[(&str, &str)], shifted: bool) -> Vec<u8> {
    let mut types = TypeSection::new();
    if shifted {
        types.ty().function([], [ValType::F64]);
    }
    types.ty().function(params.iter().copied(), []);
    let mut imports = ImportSection::new();
    for &(module, name) in entries {
        imports.import(module, name, EntityType::Function(u32::from(shifted)));
    }
    let mut module = Module::new();
    module.section(&types).section(&imports);
    module.finish()
}

#[test]
fn imports_compare_order_names_and_signature_not_type_indices() {
    let names = [("z", "unused"), ("a", "used")];
    let input = imports(&[ValType::F64], &names, false);
    check(&input, &imports(&[ValType::F64], &names, true)).unwrap();
    for changed in [
        imports(&[ValType::F64], &names[1..], false),
        imports(&[ValType::F64], &[names[1], names[0]], false),
        imports(&[ValType::F64], &[("x", "unused"), names[1]], false),
        imports(&[ValType::F64], &[("z", "other"), names[1]], false),
        imports(&[ValType::I32], &names, false),
    ] {
        assert!(matches!(
            check(&input, &changed),
            Err(GuardError::InterfaceChanged)
        ));
    }
}

fn exports(maximum: u64, result: ValType, entries: &[(&str, ExportKind)]) -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function([], [result]);
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 1,
        maximum: Some(maximum),
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    let mut exports = ExportSection::new();
    for &(name, kind) in entries {
        exports.export(name, kind, 0);
    }
    let mut body = Function::new([]);
    body.instruction(&Instruction::Unreachable)
        .instruction(&Instruction::End);
    let mut code = CodeSection::new();
    code.function(&body);
    let mut module = Module::new();
    module
        .section(&types)
        .section(&functions)
        .section(&memory)
        .section(&exports)
        .section(&code);
    module.finish()
}

#[test]
fn exports_compare_order_names_kinds_signatures_and_memory_limits() {
    let names = [("memory", ExportKind::Memory), ("main", ExportKind::Func)];
    let input = exports(2, ValType::I32, &names);
    for changed in [
        exports(2, ValType::I32, &names[1..]),
        exports(2, ValType::I32, &[names[1], names[0]]),
        exports(2, ValType::I32, &[("heap", ExportKind::Memory), names[1]]),
        exports(2, ValType::I32, &[("memory", ExportKind::Func), names[1]]),
        exports(2, ValType::F64, &names),
        exports(3, ValType::I32, &names),
    ] {
        assert!(matches!(
            check(&input, &changed),
            Err(GuardError::InterfaceChanged)
        ));
    }
}

#[test]
fn changing_an_imported_global_to_mutable_is_outside_mvp() {
    let global = |mutable| {
        let mut imports = ImportSection::new();
        imports.import(
            "env",
            "counter",
            EntityType::Global(wasm_encoder::GlobalType {
                val_type: ValType::I32,
                mutable,
                shared: false,
            }),
        );
        let mut module = Module::new();
        module.section(&imports);
        module.finish()
    };

    let input = global(false);
    check(&input, &input).unwrap();
    assert!(matches!(
        check(&input, &global(true)),
        Err(GuardError::InvalidOutput(InspectError::InvalidWasm(_)))
    ));
}

fn private_body(instructions: &[Instruction<'_>], start: bool) -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function([], []);
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut body = Function::new([]);
    for instruction in instructions {
        body.instruction(instruction);
    }
    body.instruction(&Instruction::End);
    let mut code = CodeSection::new();
    code.function(&body);
    let mut module = Module::new();
    module.section(&types).section(&functions);
    if start {
        module.section(&StartSection { function_index: 0 });
    }
    module.section(&code);
    module.finish()
}

#[test]
fn validates_both_complete_modules_under_mvp_without_start() {
    let empty = Module::new().finish();
    check(&empty, &private_body(&[], false)).unwrap();
    let post_mvp = private_body(
        &[
            Instruction::I32Const(1),
            Instruction::I32Extend8S,
            Instruction::Drop,
        ],
        false,
    );
    wasmparser::Validator::new()
        .validate_all(&post_mvp)
        .unwrap();
    for rejected in [post_mvp, private_body(&[Instruction::Drop], false), vec![]] {
        assert!(matches!(
            check(&empty, &rejected),
            Err(GuardError::InvalidOutput(InspectError::InvalidWasm(_)))
        ));
        assert!(matches!(
            check(&rejected, &empty),
            Err(GuardError::InvalidInput(InspectError::InvalidWasm(_)))
        ));
    }
    let start = private_body(&[], true);
    assert!(matches!(
        check(&empty, &start),
        Err(GuardError::InvalidOutput(InspectError::UnexpectedStart))
    ));
    assert!(matches!(
        check(&start, &empty),
        Err(GuardError::InvalidInput(InspectError::UnexpectedStart))
    ));
}
