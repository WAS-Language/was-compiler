//! Private M3 emitted-runtime experiment; never part of ordinary compilation.
use crate::{exports::ExportOptions, typed_ast::TypedProgram};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, ExportKind, ExportSection, Function, FunctionSection,
    GlobalSection, GlobalType, Instruction, TypeSection, ValType,
};

use super::{
    CodegenError, NumericPolicy, abi,
    indices::{FunctionIndex, TypeIndex},
    memory,
    validation::ValidatedProgram,
};

use super::allocator_core as core;
use core::{ACTIVE, CLASS, CLASS_COUNT, HeadAccess, IDENTITY, REQUESTED, emit, mem, trap_if};
const HELPER_COUNT: u32 = 7;
const EXPORTS: [&str; 4] = ["__m3_class", "__m3_acquire", "__m3_release", "__m3_observe"];

#[derive(Clone, Copy, Default)]
pub(super) struct Config {
    // Seed only at instantiation: skipped identities can never be issued later.
    last_identity: u64,
    backing_limit: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Checked(#[from] CodegenError),
    #[error("private allocator workload requires ordinarily planned memory")]
    MissingMemory,
}

pub(super) struct AppendConfig {
    config: Config,
    comparisons: bool,
}

fn compile(
    program: &TypedProgram,
    exports: &ExportOptions,
    config: Config,
) -> Result<Vec<u8>, Error> {
    compile_variant(
        program,
        exports,
        AppendConfig {
            config,
            comparisons: false,
        },
    )
}

fn compile_instrumented(
    program: &TypedProgram,
    exports: &ExportOptions,
    config: Config,
) -> Result<Vec<u8>, Error> {
    compile_variant(
        program,
        exports,
        AppendConfig {
            config,
            comparisons: true,
        },
    )
}

fn compile_variant(
    program: &TypedProgram,
    exports: &ExportOptions,
    options: AppendConfig,
) -> Result<Vec<u8>, Error> {
    let validated = ValidatedProgram::with_policy(program, exports, NumericPolicy::Baseline)?;
    let layout = validated.layout();
    let memory = layout.memory.as_ref().ok_or(Error::MissingMemory)?;

    // Preflight all append-index arithmetic before infallible section assembly.
    let count = [
        program.host_functions.len(),
        layout.users.len(),
        memory.helpers.len(),
        usize::from(layout.remainder.is_some()),
        layout.ranges.len(),
        layout.callables.len(),
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .ok_or(CodegenError::LimitExceeded {
        what: "private allocator functions",
    })?;
    super::checked_u32(count, "private allocator functions")?
        .checked_add(HELPER_COUNT)
        .ok_or(CodegenError::LimitExceeded {
            what: "private allocator functions",
        })?;
    super::checked_u32(layout.signatures.len(), "private allocator types")?
        .checked_add(HELPER_COUNT)
        .ok_or(CodegenError::LimitExceeded {
            what: "private allocator types",
        })?;

    for (user, plan) in program.functions.iter().zip(&layout.users) {
        if plan.exported
            && (EXPORTS.contains(&user.name.as_str())
                || (options.comparisons && user.name == "__m3_registry_comparisons"))
        {
            return Err(CodegenError::DuplicateExportName {
                name: user.name.clone(),
            }
            .into());
        }
    }

    Ok(super::assemble_inner(&validated, Some(options)))
}

pub(super) struct Sections<'a> {
    pub types: &'a mut TypeSection,
    pub functions: &'a mut FunctionSection,
    pub globals: &'a mut GlobalSection,
    pub exports: &'a mut ExportSection,
    pub code: &'a mut CodeSection,
}

// Relative helper slots; all existing function/type/global indices stay intact.
const CLASSIFY: u32 = 0;
const FIND: u32 = 4;
const HEAD: u32 = 5;
const SET_HEAD: u32 = 6;

struct Emitter {
    core: core::Emitter,
    population: u32,
    config: Config,
    comparisons: Option<u32>,
}

pub(super) fn append(
    validated: &ValidatedProgram<'_>,
    options: AppendConfig,
    sections: Sections<'_>,
) {
    let config = options.config;
    let memory = validated
        .layout()
        .memory
        .as_ref()
        .expect("allocator preflight requires memory");
    let first_function = FunctionIndex::new(
        u32::try_from(validated.program().host_functions.len()).expect("checked import count")
            + sections.functions.len(),
    );
    let first_global = sections.globals.len();
    let emitter = Emitter {
        core: core::Emitter {
            classify: first_function.offset(CLASSIFY),
            find: first_function.offset(FIND),
            head: first_function.offset(HEAD),
            set_head: first_function.offset(SET_HEAD),
            backing: memory.first_function.offset(memory::Helper::Alloc.index()),
            last_identity: first_global,
            registry: first_global + 1,
            heads: std::array::from_fn(|class| {
                first_global + 3 + u32::try_from(class).expect("bounded class index")
            }),
        },
        population: first_global + 2,
        config,
        comparisons: options
            .comparisons
            .then_some(first_global + 3 + CLASS_COUNT),
    };

    for (ty, init) in [
        // Wasm i64 carries the identity's unsigned bit pattern.
        (ValType::I64, config.last_identity as i64),
        (ValType::I32, 0),
        (ValType::I64, 0),
    ] {
        sections.globals.global(
            GlobalType {
                val_type: ty,
                mutable: true,
                shared: false,
            },
            &if ty == ValType::I64 {
                ConstExpr::i64_const(init)
            } else {
                ConstExpr::i32_const(0)
            },
        );
    }
    for _ in 0..CLASS_COUNT {
        sections.globals.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i32_const(0),
        );
    }

    if let Some(index) = emitter.comparisons {
        sections.globals.global(
            GlobalType {
                val_type: ValType::I64,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i64_const(0),
        );
        sections
            .exports
            .export("__m3_registry_comparisons", ExportKind::Global, index);
    }

    use ValType::{I32, I64};
    for (params, results) in [
        (vec![I64], vec![I32]),
        (vec![I64], vec![I32, I64]),
        (vec![I32, I64], vec![]),
        (vec![I32, I32], vec![I64]),
        (vec![I32], vec![I32]),
        (vec![I32], vec![I32]),
        (vec![I32, I32], vec![]),
    ] {
        let type_index = TypeIndex::new(sections.types.len());
        sections.types.ty().function(params, results);
        sections.functions.function(type_index.raw());
    }
    let bodies = [
        emitter.core.classify(),
        emitter.acquire(),
        emitter.core.release(),
        emitter.observe(),
        emitter.find(),
        emitter.core.head(HeadAccess::Read),
        emitter.core.head(HeadAccess::Write),
    ];
    for (slot, body) in bodies.iter().enumerate() {
        let slot = u32::try_from(slot).expect("fixed helper count");
        sections.code.function(body);
        if let Some(name) = EXPORTS.get(slot as usize) {
            sections
                .exports
                .export(name, ExportKind::Func, first_function.offset(slot).raw());
        }
    }
}

impl Emitter {
    fn find(&self) -> Function {
        use Instruction::*;
        // Count attempted exact-payload comparisons, including searches that trap.
        let counter = self
            .comparisons
            .map(|index| [GlobalGet(index), I64Const(1), I64Add, GlobalSet(index)]);
        self.core
            .find(counter.as_ref().map_or(&[], |ops| ops.as_slice()))
    }

    fn acquire(&self) -> Function {
        use Instruction::*;
        // This private ceiling applies only to fresh reservations, before backing.
        let guard = self.config.backing_limit.map(|limit| {
            [
                GlobalGet(0),
                I64ExtendI32U,
                LocalGet(4),
                I64Add,
                I64Const(limit as i64),
                I64GtU,
                If(BlockType::Empty),
                Unreachable,
                End,
            ]
        });
        self.core.acquire(
            guard.as_ref().map_or(&[], |ops| ops.as_slice()),
            &[
                GlobalGet(self.population),
                I64Const(1),
                I64Add,
                GlobalSet(self.population),
            ],
        )
    }

    fn observe(&self) -> Function {
        use Instruction::*;
        let mut f = Function::new([(1, ValType::I32)]);
        for (selector, ops) in [
            (0, vec![GlobalGet(0), I64ExtendI32U]),
            (1, vec![MemorySize(0), I64ExtendI32U]),
            (2, vec![GlobalGet(self.population)]),
            (
                7,
                vec![LocalGet(1), Call(self.core.head.raw()), I64ExtendI32U],
            ),
            (8, vec![GlobalGet(self.core.last_identity)]),
        ] {
            emit(
                &mut f,
                &[LocalGet(0), I32Const(selector), I32Eq, If(BlockType::Empty)],
            );
            emit(&mut f, &ops);
            emit(&mut f, &[Return, End]);
        }
        // Reject invalid selectors without any lookup or mutation.
        emit(
            &mut f,
            &[
                LocalGet(0),
                I32Const(3),
                I32LtU,
                LocalGet(0),
                I32Const(6),
                I32GtU,
                I32Or,
            ],
        );
        trap_if(&mut f);
        emit(
            &mut f,
            &[
                LocalGet(1),
                Call(self.core.find.raw()),
                LocalTee(2),
                I32Eqz,
                If(BlockType::Empty),
                I64Const(0),
                Return,
                End,
            ],
        );
        for (selector, ops) in [
            (
                3,
                vec![
                    I32Const(2),
                    LocalGet(2),
                    I32Load(mem(ACTIVE, 2)),
                    I32Sub,
                    I64ExtendI32U,
                ],
            ),
            (4, vec![LocalGet(2), I64Load(mem(IDENTITY, 3))]),
            (
                5,
                vec![
                    I64Const(i64::from(abi::MIN_ALLOCATION_BYTES)),
                    LocalGet(2),
                    I32Load(mem(CLASS, 2)),
                    I64ExtendI32U,
                    I64Shl,
                ],
            ),
            (6, vec![LocalGet(2), I64Load(mem(REQUESTED, 3))]),
        ] {
            emit(
                &mut f,
                &[LocalGet(0), I32Const(selector), I32Eq, If(BlockType::Empty)],
            );
            emit(&mut f, &ops);
            emit(&mut f, &[Return, End]);
        }
        emit(&mut f, &[Unreachable, End]);
        f
    }
}

mod core_tests;
mod failure_order_tests;
mod failure_tests;
mod measurement_tests;
mod release_tests;
mod runtime_tests;
mod tests;
