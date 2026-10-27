//! Explicitly test-invoked storage experiment; ordinary compilation never selects it.
use crate::{
    exports::ExportOptions,
    typed_ast::{TypedExpr, TypedExprKind, TypedProgram},
};
use wasm_encoder::{BlockType, Instruction};

use super::{function::BodyEmitter, indices::LocalIndex, memory::Helper};

use super::{CodegenError, NumericPolicy};

mod instrumentation;
use instrumentation::{Counters, Event};
pub(super) use instrumentation::{append as append_instrumentation, compile_instrumented};

pub(super) fn compile(
    program: &TypedProgram,
    exports: &ExportOptions,
    policy: NumericPolicy,
) -> Result<Vec<u8>, CodegenError> {
    let validated = super::validation::ValidatedProgram::with_regions(program, exports, policy)?;
    Ok(super::assemble(&validated))
}

#[derive(Debug)]
pub(super) enum Selection<'a> {
    Ineligible,
    CapacityExceeded,
    Region(Plan<'a>),
}

#[derive(Debug)]
pub(super) struct Plan<'a> {
    pub capacity: u32,
    pub cache: u32,
    literals: Vec<(&'a TypedExpr, u32)>,
    counters: Option<Counters>,
}

// No in-memory management header: cache globals and invocation locals own blocks.
fn footprint(length: usize, width: u32) -> Option<u32> {
    let bytes = u64::try_from(length).ok()?.checked_mul(u64::from(width))?;
    let alignment_mask = u64::from(super::abi::ALLOCATION_ALIGNMENT - 1);
    let aligned = bytes
        .checked_add(u64::from(super::abi::DESCRIPTOR_SIZE))?
        .checked_add(alignment_mask)?
        & !alignment_mask;
    (aligned <= super::memory::MAX_BYTES)
        .then(|| u32::try_from(aligned).ok())
        .flatten()
}

fn add_capacity(capacity: u32, size: u32) -> Option<u32> {
    capacity
        .checked_add(size)
        .filter(|total| u64::from(*total) <= super::memory::MAX_BYTES)
}

pub(super) fn select(program: &TypedProgram) -> Vec<Selection<'_>> {
    let mut selections: Vec<_> = program
        .functions
        .iter()
        .map(|_| Selection::Ineligible)
        .collect();
    for (function, literal) in crate::lifetime::temporary_literals(program) {
        let TypedExprKind::Array(elements, element) = &literal.kind else {
            unreachable!("M1 candidate is an array literal")
        };
        selections[function.id.0].reserve(
            literal,
            footprint(elements.len(), super::array::width(*element)),
        );
    }
    // Global zero remains the ordinary heap cursor. Only selected functions get slots.
    let mut cache = 1;
    for selection in &mut selections {
        if let Selection::Region(plan) = selection {
            plan.cache = cache;
            cache += 1;
        }
    }
    selections
}

impl<'a> Selection<'a> {
    fn reserve(&mut self, literal: &'a TypedExpr, size: Option<u32>) {
        if matches!(self, Self::CapacityExceeded) {
            return;
        }
        if matches!(self, Self::Ineligible) {
            *self = Self::Region(Plan {
                capacity: 0,
                cache: 0,
                literals: Vec::new(),
                counters: None,
            });
        }
        let Self::Region(plan) = self else {
            unreachable!()
        };
        let Some(total) = size.and_then(|size| add_capacity(plan.capacity, size)) else {
            // Discard all slices, not just the overflowing literal: fallback is invocation-wide.
            *self = Self::CapacityExceeded;
            return;
        };
        plan.literals.push((literal, plan.capacity));
        plan.capacity = total;
    }
}

#[derive(Clone, Copy)]
pub(super) struct Invocation<'a> {
    pub plan: &'a Plan<'a>,
    pub base: LocalIndex,
}

impl Invocation<'_> {
    pub fn allocate(
        self,
        literal: &TypedExpr,
        body: &mut BodyEmitter<'_>,
    ) -> Result<(), CodegenError> {
        let offset = self
            .plan
            .literals
            .iter()
            .find_map(|(candidate, offset)| std::ptr::eq(*candidate, literal).then_some(*offset))
            .expect("selected literal belongs to borrowed region plan");
        body.local_get(self.base);
        body.instruction(&Instruction::I32Eqz);
        body.begin_if(BlockType::Empty)?;
        body.instruction(&Instruction::GlobalGet(self.plan.cache));
        body.local_set(self.base);
        // Clear ownership before any potentially trapping provisioning or initialization.
        body.instruction(&Instruction::I32Const(0));
        body.instruction(&Instruction::GlobalSet(self.plan.cache));
        body.local_get(self.base);
        body.instruction(&Instruction::I32Eqz);
        body.begin_if(BlockType::Empty)?;
        body.instruction(&Instruction::I32Const(self.plan.capacity as i32));
        body.call(body.helper(Helper::Alloc));
        body.local_set(self.base);
        if let Some(counters) = self.plan.counters {
            // Failed provisioning never reaches this event.
            counters.record(Event::Provision, body);
            body.otherwise();
            counters.record(Event::Reuse, body);
        }
        body.end();
        body.end();
        if let Some(counters) = self.plan.counters {
            counters.record(Event::Literal, body);
        }
        body.local_get(self.base);
        body.instruction(&Instruction::I32Const(offset as i32));
        body.instruction(&Instruction::I32Add);
        Ok(())
    }

    pub fn retire(self, body: &mut BodyEmitter<'_>) -> Result<(), CodegenError> {
        // This empty-result region preserves an already computed scalar return below it.
        body.local_get(self.base);
        body.begin_if(BlockType::Empty)?;
        body.local_get(self.base);
        body.instruction(&Instruction::GlobalSet(self.plan.cache));
        if let Some(counters) = self.plan.counters {
            counters.record(Event::Retirement, body);
        }
        body.end();
        Ok(())
    }
}

mod measurement_tests;
mod runtime_tests;
mod tests;

#[test]
fn selected_function_has_private_cache_and_checked_array_stores() {
    use wasmparser::{Operator, Parser, Payload, Validator};

    let program = crate::Compiler::new()
        .analyze("fn main(n: number) -> number { let a = [n, 2]; a[0] += 3; return a[0]; }")
        .unwrap();
    let bytes = compile(&program, &ExportOptions::All, NumericPolicy::Baseline).unwrap();
    Validator::new().validate_all(&bytes).unwrap();
    let mut globals = 0;
    let mut first = true;
    let mut takes_cache = false;
    let mut returns_cache = false;
    let mut stores_element = false;
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            Payload::GlobalSection(section) => globals = section.count(),
            Payload::CodeSectionEntry(body) if first => {
                first = false;
                for op in body.get_operators_reader().unwrap() {
                    match op.unwrap() {
                        Operator::GlobalGet { global_index: 1 } => takes_cache = true,
                        Operator::GlobalSet { global_index: 1 } => returns_cache = true,
                        Operator::F64Store { memarg } if memarg.offset == 8 => {
                            stores_element = true
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    assert_eq!(
        globals, 2,
        "heap cursor plus one private reusable-block slot"
    );
    assert!(takes_cache && returns_cache && stores_element);
}
