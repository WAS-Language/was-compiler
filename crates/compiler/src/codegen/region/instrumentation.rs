//! Counters exist only in explicitly instrumented private experimental modules.
use super::*;
use wasm_encoder::{ConstExpr, ExportKind, ExportSection, GlobalSection, GlobalType, ValType};

#[derive(Clone, Copy, Debug)]
pub(super) struct Counters(u32);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Event {
    Provision,
    Reuse,
    Literal,
    Retirement,
}

impl Event {
    const ALL: [Self; 4] = [
        Self::Provision,
        Self::Reuse,
        Self::Literal,
        Self::Retirement,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Provision => "provisions",
            Self::Reuse => "reuses",
            Self::Literal => "literals",
            Self::Retirement => "retirements",
        }
    }

    fn index(self) -> u32 {
        let index = Self::ALL
            .iter()
            .position(|event| *event == self)
            .expect("counter event is in the exported schema");
        u32::try_from(index).expect("counter schema fits u32")
    }
}

impl Counters {
    pub fn record(self, event: Event, body: &mut BodyEmitter<'_>) {
        let index = self.0 + event.index();
        body.instruction(&Instruction::GlobalGet(index));
        body.instruction(&Instruction::I64Const(1));
        body.instruction(&Instruction::I64Add);
        body.instruction(&Instruction::GlobalSet(index));
    }
}

pub(in crate::codegen) fn compile_instrumented(
    program: &TypedProgram,
    exports: &ExportOptions,
    policy: NumericPolicy,
) -> Result<Vec<u8>, CodegenError> {
    let validated = super::super::validation::ValidatedProgram::with_region_selection(
        program,
        exports,
        policy,
        |program| {
            let mut selections = select(program);
            let cache_count = u32::try_from(
                selections
                    .iter()
                    .filter(|s| matches!(s, Selection::Region(_)))
                    .count(),
            )
            .expect("selected cache globals fit u32");
            let mut next = cache_count
                .checked_add(1)
                .expect("heap and cache globals fit u32");
            for selection in &mut selections {
                if let Selection::Region(plan) = selection {
                    plan.counters = Some(Counters(next));
                    next = next
                        .checked_add(
                            u32::try_from(Event::ALL.len()).expect("counter schema fits u32"),
                        )
                        .expect("measurement globals fit u32");
                }
            }
            selections
        },
    )?;
    Ok(super::super::assemble(&validated))
}

pub(in crate::codegen) fn append(
    program: &TypedProgram,
    selections: &[Selection<'_>],
    globals: &mut GlobalSection,
    exports: &mut ExportSection,
) {
    for (function, selection) in program.functions.iter().zip(selections) {
        if let Selection::Region(Plan {
            counters: Some(counters),
            ..
        }) = selection
        {
            for event in Event::ALL {
                globals.global(
                    GlobalType {
                        val_type: ValType::I64,
                        mutable: true,
                        shared: false,
                    },
                    &ConstExpr::i64_const(0),
                );
                exports.export(
                    &format!("__m2_{}_{}", function.name, event.name()),
                    ExportKind::Global,
                    counters.0 + event.index(),
                );
            }
        }
    }
}
