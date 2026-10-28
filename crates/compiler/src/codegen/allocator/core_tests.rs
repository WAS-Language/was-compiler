use super::core::{Emitter, HeadAccess};
use crate::codegen::indices::FunctionIndex;
use wasm_encoder::{
    CodeSection, ConstExpr, Function, FunctionSection, GlobalSection, GlobalType, Instruction,
    MemorySection, MemoryType, Module, TypeSection, ValType,
};
use wasmparser::{Parser, Payload, Validator};

fn decode(bytes: &[u8]) -> Vec<Vec<String>> {
    Validator::new().validate_all(bytes).unwrap();
    Parser::new(0)
        .parse_all(bytes)
        .filter_map(|payload| match payload.unwrap() {
            Payload::CodeSectionEntry(body) => Some(
                body.get_operators_reader()
                    .unwrap()
                    .into_iter()
                    .map(|op| format!("{:?}", op.unwrap()))
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}

#[test]
fn reusable_emission_uses_explicit_noncontiguous_function_and_global_indices() {
    use ValType::{I32, I64};
    let emitter = Emitter {
        classify: FunctionIndex::new(9),
        find: FunctionIndex::new(1),
        head: FunctionIndex::new(7),
        set_head: FunctionIndex::new(3),
        backing: FunctionIndex::new(5),
        last_identity: 4,
        registry: 2,
        heads: std::array::from_fn(|class| 10 + 2 * u32::try_from(class).unwrap()),
    };
    let mut types = TypeSection::new();
    for (params, results) in [
        (vec![I64], vec![I32]),
        (vec![I64], vec![I32, I64]),
        (vec![I32, I64], vec![]),
        (vec![I32], vec![I32]),
        (vec![I32, I32], vec![]),
        (vec![], vec![]),
    ] {
        types.ty().function(params, results);
    }
    let mut globals = GlobalSection::new();
    for index in 0..67 {
        let ty = if index == 4 { I64 } else { I32 };
        globals.global(
            GlobalType {
                val_type: ty,
                mutable: true,
                shared: false,
            },
            &if ty == I64 {
                ConstExpr::i64_const(0)
            } else {
                ConstExpr::i32_const(0)
            },
        );
    }
    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    let mut functions = FunctionSection::new();
    let mut code = CodeSection::new();
    for slot in 0..11 {
        let (ty, body) = match slot {
            0 => (2, emitter.release()),
            1 => (3, emitter.find(&[])),
            3 => (4, emitter.head(HeadAccess::Write)),
            7 => (3, emitter.head(HeadAccess::Read)),
            9 => (0, emitter.classify()),
            10 => (1, emitter.acquire(&[], &[])),
            _ => {
                let mut body = Function::new([]);
                body.instruction(&Instruction::Unreachable);
                body.instruction(&Instruction::End);
                (if slot == 5 { 3 } else { 5 }, body)
            }
        };
        functions.function(ty);
        code.function(&body);
    }
    let mut module = Module::new();
    module
        .section(&types)
        .section(&functions)
        .section(&memory)
        .section(&globals)
        .section(&code);
    let decoded = decode(&module.finish());
    let calls = |slot: usize| {
        decoded[slot]
            .iter()
            .filter(|op| op.starts_with("Call "))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        calls(10),
        [
            "Call { function_index: 9 }",
            "Call { function_index: 7 }",
            "Call { function_index: 5 }",
            "Call { function_index: 3 }",
        ]
    );
    assert_eq!(
        calls(0),
        [
            "Call { function_index: 1 }",
            "Call { function_index: 7 }",
            "Call { function_index: 3 }",
        ]
    );
    for (slot, operation) in [(7, "GlobalGet"), (3, "GlobalSet")] {
        let actual = decoded[slot]
            .iter()
            .filter(|op| op.starts_with(operation))
            .cloned()
            .collect::<Vec<_>>();
        let expected = (10..=66)
            .step_by(2)
            .map(|index| format!("{operation} {{ global_index: {index} }}"))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
    let globals = |slot: usize| {
        decoded[slot]
            .iter()
            .filter(|op| op.starts_with("Global"))
            .map(String::as_str)
            .collect::<Vec<_>>()
    };
    assert_eq!(globals(1), ["GlobalGet { global_index: 2 }"]);
    assert_eq!(
        globals(10),
        [
            "GlobalGet { global_index: 4 }",
            "GlobalGet { global_index: 4 }",
            "GlobalGet { global_index: 2 }",
            "GlobalSet { global_index: 2 }",
            "GlobalSet { global_index: 4 }",
        ]
    );
}
