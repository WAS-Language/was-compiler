//! Private M3 emitted-runtime experiment; never part of ordinary compilation.
use crate::{exports::ExportOptions, typed_ast::TypedProgram};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, ExportKind, ExportSection, Function, FunctionSection,
    GlobalSection, GlobalType, Instruction, MemArg, TypeSection, ValType,
};

use super::{
    CodegenError, NumericPolicy, abi,
    indices::{FunctionIndex, TypeIndex},
    memory,
    validation::ValidatedProgram,
};

// One reusable record per physical block, separate from the usable payload.
const RECORD_BYTES: i32 = 32;
const REGISTRY_NEXT: u64 = 0;
const FREE_NEXT: u64 = 4;
const CLASS: u64 = 8;
const ACTIVE: u64 = 12;
const IDENTITY: u64 = 16;
const REQUESTED: u64 = 24;
const CLASS_COUNT: u32 = 29;
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

enum HeadAccess {
    Read,
    Write,
}

struct Emitter {
    first_function: FunctionIndex,
    backing: FunctionIndex,
    last_identity: u32,
    registry: u32,
    population: u32,
    heads: u32,
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
        first_function,
        backing: memory.first_function.offset(memory::Helper::Alloc.index()),
        last_identity: first_global,
        registry: first_global + 1,
        population: first_global + 2,
        heads: first_global + 3,
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
        emitter.classify(),
        emitter.acquire(),
        emitter.release(),
        emitter.observe(),
        emitter.find(),
        emitter.head(HeadAccess::Read),
        emitter.head(HeadAccess::Write),
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

fn mem(offset: u64, align: u32) -> MemArg {
    MemArg {
        offset,
        align,
        memory_index: 0,
    }
}

fn emit(body: &mut Function, ops: &[Instruction<'_>]) {
    for op in ops {
        body.instruction(op);
    }
}

fn trap_if(body: &mut Function) {
    emit(
        body,
        &[
            Instruction::If(BlockType::Empty),
            Instruction::Unreachable,
            Instruction::End,
        ],
    );
}

impl Emitter {
    fn call(&self, slot: u32) -> Instruction<'static> {
        Instruction::Call(self.first_function.offset(slot).raw())
    }

    fn classify(&self) -> Function {
        use Instruction::*;
        // request:i64, class:i32, capacity:i64. No truncation before validation.
        let mut f = Function::new([(1, ValType::I32), (1, ValType::I64)]);
        emit(
            &mut f,
            &[
                LocalGet(0),
                I64Const(0),
                I64LtS,
                LocalGet(0),
                I64Const(1 << 31),
                I64GtU,
                I32Or,
            ],
        );
        trap_if(&mut f);
        emit(
            &mut f,
            &[
                I64Const(i64::from(abi::MIN_ALLOCATION_BYTES)),
                LocalSet(2),
                Block(BlockType::Empty),
                Loop(BlockType::Empty),
                LocalGet(2),
                LocalGet(0),
                I64GeU,
                BrIf(1),
                LocalGet(2),
                I64Const(1),
                I64Shl,
                LocalSet(2),
                LocalGet(1),
                I32Const(1),
                I32Add,
                LocalSet(1),
                Br(0),
                End,
                End,
                LocalGet(1),
                End,
            ],
        );
        f
    }

    fn head(&self, access: HeadAccess) -> Function {
        use Instruction::*;
        let mut f = Function::new([]);
        // Wasm globals have static indices; a small bounded dispatch keeps the
        // class table outside usable linear memory and needs no startup allocator.
        for class in 0..CLASS_COUNT {
            emit(
                &mut f,
                &[
                    LocalGet(0),
                    I32Const(class as i32),
                    I32Eq,
                    If(BlockType::Empty),
                ],
            );
            match access {
                HeadAccess::Write => emit(
                    &mut f,
                    &[LocalGet(1), GlobalSet(self.heads + class), Return],
                ),
                HeadAccess::Read => emit(&mut f, &[GlobalGet(self.heads + class), Return]),
            }
            emit(&mut f, &[End]);
        }
        emit(&mut f, &[Unreachable, End]);
        f
    }

    fn find(&self) -> Function {
        use Instruction::*;
        let mut f = Function::new([(1, ValType::I32)]);
        emit(
            &mut f,
            &[
                GlobalGet(self.registry),
                LocalSet(1),
                Block(BlockType::Empty),
                Loop(BlockType::Empty),
                LocalGet(1),
                I32Eqz,
                BrIf(1),
            ],
        );
        // Count attempted exact-payload comparisons, including searches that later trap.
        if let Some(index) = self.comparisons {
            emit(
                &mut f,
                &[GlobalGet(index), I64Const(1), I64Add, GlobalSet(index)],
            );
        }
        emit(
            &mut f,
            &[
                LocalGet(1),
                I32Const(RECORD_BYTES),
                I32Add,
                LocalGet(0),
                I32Eq,
                If(BlockType::Empty),
                LocalGet(1),
                Return,
                End,
                LocalGet(1),
                I32Load(mem(REGISTRY_NEXT, 2)),
                LocalSet(1),
                Br(0),
                End,
                End,
                I32Const(0),
                End,
            ],
        );
        f
    }

    fn acquire(&self) -> Function {
        use Instruction::*;
        // request0:i64; class1, record2:i32; capacity3, total4, identity5:i64.
        let mut f = Function::new([(2, ValType::I32), (3, ValType::I64)]);
        emit(
            &mut f,
            &[
                LocalGet(0),
                self.call(CLASSIFY),
                LocalSet(1),
                GlobalGet(self.last_identity),
                I64Const(-1),
                I64Eq,
            ],
        );
        trap_if(&mut f);
        emit(
            &mut f,
            &[
                GlobalGet(self.last_identity),
                I64Const(1),
                I64Add,
                LocalSet(5),
                I64Const(i64::from(abi::MIN_ALLOCATION_BYTES)),
                LocalGet(1),
                I64ExtendI32U,
                I64Shl,
                LocalSet(3),
                LocalGet(1),
                self.call(HEAD),
                LocalTee(2),
                I32Eqz,
                If(BlockType::Empty),
                LocalGet(3),
                I64Const(i64::from(RECORD_BYTES)),
                I64Add,
                LocalSet(4),
                LocalGet(4),
                I64Const(i64::from(u32::MAX)),
                I64GtU,
            ],
        );
        trap_if(&mut f);
        if let Some(limit) = self.config.backing_limit {
            emit(
                &mut f,
                &[
                    GlobalGet(0),
                    I64ExtendI32U,
                    LocalGet(4),
                    I64Add,
                    I64Const(limit as i64),
                    I64GtU,
                ],
            );
            trap_if(&mut f);
        }
        // The only fallible backing reservation happens before any publication,
        // identity advance or free-list mutation. Record and payload are atomic.
        emit(
            &mut f,
            &[
                LocalGet(4),
                I32WrapI64,
                Call(self.backing.raw()),
                LocalSet(2),
                LocalGet(2),
                GlobalGet(self.registry),
                I32Store(mem(REGISTRY_NEXT, 2)),
                LocalGet(2),
                LocalGet(1),
                I32Store(mem(CLASS, 2)),
                Else,
                LocalGet(1),
                LocalGet(2),
                I32Load(mem(FREE_NEXT, 2)),
                self.call(SET_HEAD),
                End,
                LocalGet(2),
                I32Const(0),
                I32Store(mem(FREE_NEXT, 2)),
                LocalGet(2),
                I32Const(1),
                I32Store(mem(ACTIVE, 2)),
                LocalGet(2),
                LocalGet(5),
                I64Store(mem(IDENTITY, 3)),
                LocalGet(2),
                LocalGet(0),
                I64Store(mem(REQUESTED, 3)),
            ],
        );
        // Publish only fully initialized records. total4 is zero-initialized
        // and set only on the fresh branch; reuse never adds a registry entry.
        emit(
            &mut f,
            &[
                LocalGet(4),
                I64Eqz,
                I32Eqz,
                If(BlockType::Empty),
                LocalGet(2),
                GlobalSet(self.registry),
                GlobalGet(self.population),
                I64Const(1),
                I64Add,
                GlobalSet(self.population),
                End,
                LocalGet(5),
                GlobalSet(self.last_identity),
                LocalGet(2),
                I32Const(RECORD_BYTES),
                I32Add,
                LocalGet(5),
                End,
            ],
        );
        f
    }

    fn release(&self) -> Function {
        use Instruction::*;
        // payload0:i32, identity1:i64; record2,class3:i32.
        let mut f = Function::new([(2, ValType::I32)]);
        emit(&mut f, &[LocalGet(0), self.call(FIND), LocalTee(2), I32Eqz]);
        trap_if(&mut f);
        emit(
            &mut f,
            &[
                LocalGet(2),
                I64Load(mem(IDENTITY, 3)),
                LocalGet(1),
                I64Ne,
                LocalGet(2),
                I32Load(mem(ACTIVE, 2)),
                I32Const(1),
                I32Ne,
                I32Or,
            ],
        );
        trap_if(&mut f);
        emit(
            &mut f,
            &[
                LocalGet(2),
                I32Load(mem(CLASS, 2)),
                LocalSet(3),
                LocalGet(2),
                LocalGet(3),
                self.call(HEAD),
                I32Store(mem(FREE_NEXT, 2)),
                LocalGet(2),
                I32Const(0),
                I32Store(mem(ACTIVE, 2)),
                LocalGet(3),
                LocalGet(2),
                self.call(SET_HEAD),
                End,
            ],
        );
        f
    }

    fn observe(&self) -> Function {
        use Instruction::*;
        let mut f = Function::new([(1, ValType::I32)]);
        for (selector, ops) in [
            (0, vec![GlobalGet(0), I64ExtendI32U]),
            (1, vec![MemorySize(0), I64ExtendI32U]),
            (2, vec![GlobalGet(self.population)]),
            (7, vec![LocalGet(1), self.call(HEAD), I64ExtendI32U]),
            (8, vec![GlobalGet(self.last_identity)]),
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
                self.call(FIND),
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

mod failure_order_tests;
mod failure_tests;
mod measurement_tests;
mod release_tests;
mod runtime_tests;
mod tests;
