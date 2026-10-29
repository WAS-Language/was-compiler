//! Private reusable allocation emission. Indices are planned by the caller.
use super::{abi, indices::FunctionIndex};
use wasm_encoder::{BlockType, Function, Instruction, MemArg, ValType};

pub(super) use abi::{
    ALLOCATOR_ACTIVE as ACTIVE, ALLOCATOR_CLASS as CLASS, ALLOCATOR_CLASS_COUNT as CLASS_COUNT,
    ALLOCATOR_IDENTITY as IDENTITY, ALLOCATOR_REQUESTED as REQUESTED,
};
use abi::{
    ALLOCATOR_FREE_NEXT as FREE_NEXT, ALLOCATOR_RECORD_BYTES as RECORD_BYTES,
    ALLOCATOR_REGISTRY_NEXT as REGISTRY_NEXT,
};

pub(super) enum HeadAccess {
    Read,
    Write,
}

pub(super) struct Emitter {
    pub classify: FunctionIndex,
    pub find: FunctionIndex,
    pub head: FunctionIndex,
    pub set_head: FunctionIndex,
    pub backing: FunctionIndex,
    pub last_identity: u32,
    pub registry: u32,
    pub heads: [u32; CLASS_COUNT as usize],
}

pub(super) fn mem(offset: u64, align: u32) -> MemArg {
    MemArg {
        offset,
        align,
        memory_index: 0,
    }
}

pub(super) fn emit(body: &mut Function, ops: &[Instruction<'_>]) {
    for op in ops {
        body.instruction(op);
    }
}

pub(super) fn trap_if(body: &mut Function) {
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
    pub(super) fn classify(&self) -> Function {
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
                I64Const(abi::ALLOCATOR_MAX_REQUEST),
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

    pub(super) fn head(&self, access: HeadAccess) -> Function {
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
                    &[
                        LocalGet(1),
                        GlobalSet(self.heads[usize::try_from(class).expect("bounded class index")]),
                        Return,
                    ],
                ),
                HeadAccess::Read => emit(
                    &mut f,
                    &[
                        GlobalGet(self.heads[usize::try_from(class).expect("bounded class index")]),
                        Return,
                    ],
                ),
            }
            emit(&mut f, &[End]);
        }
        emit(&mut f, &[Unreachable, End]);
        f
    }

    /// The adapter's counter operations run once per visited registry record.
    /// They must preserve the operand stack and locals and must not trap.
    pub(super) fn find(&self, before_compare: &[Instruction<'_>]) -> Function {
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
        emit(&mut f, before_compare);
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

    /// Adapter operations preserve the operand stack and locals. `before_backing`
    /// runs only for fresh storage, with the checked i64 reservation size in local 4;
    /// it may trap before any allocator mutation. `on_fresh` runs after registry
    /// publication and must be infallible; reuse executes neither sequence.
    pub(super) fn acquire(
        &self,
        before_backing: &[Instruction<'_>],
        on_fresh: &[Instruction<'_>],
    ) -> Function {
        use Instruction::*;
        // request0:i64; class1, record2:i32; capacity3, total4, identity5:i64.
        let mut f = Function::new([(2, ValType::I32), (3, ValType::I64)]);
        emit(
            &mut f,
            &[
                LocalGet(0),
                Call(self.classify.raw()),
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
                Call(self.head.raw()),
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
        emit(&mut f, before_backing);
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
                Call(self.set_head.raw()),
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
            ],
        );
        emit(&mut f, on_fresh);
        emit(
            &mut f,
            &[
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

    /// Checks exact live acquisition authority and its requested extent without mutation.
    pub(super) fn validate(&self) -> Function {
        use Instruction::*;
        // payload0:i32, identity1:i64, extent2:i64; record3:i32.
        let mut f = Function::new([(1, ValType::I32)]);
        emit(
            &mut f,
            &[LocalGet(0), Call(self.find.raw()), LocalTee(3), I32Eqz],
        );
        trap_if(&mut f);
        emit(
            &mut f,
            &[
                LocalGet(3),
                I64Load(mem(IDENTITY, 3)),
                LocalGet(1),
                I64Ne,
                LocalGet(3),
                I32Load(mem(ACTIVE, 2)),
                I32Const(1),
                I32Ne,
                I32Or,
                LocalGet(3),
                I64Load(mem(REQUESTED, 3)),
                LocalGet(2),
                I64Ne,
                I32Or,
                I64Const(i64::from(abi::MIN_ALLOCATION_BYTES)),
                LocalGet(3),
                I32Load(mem(CLASS, 2)),
                I64ExtendI32U,
                I64Shl,
                LocalGet(2),
                I64Ne,
                I32Or,
            ],
        );
        trap_if(&mut f);
        emit(&mut f, &[End]);
        f
    }

    pub(super) fn release(&self) -> Function {
        use Instruction::*;
        // payload0:i32, identity1:i64; record2,class3:i32.
        let mut f = Function::new([(2, ValType::I32)]);
        emit(
            &mut f,
            &[LocalGet(0), Call(self.find.raw()), LocalTee(2), I32Eqz],
        );
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
                Call(self.head.raw()),
                I32Store(mem(FREE_NEXT, 2)),
                LocalGet(2),
                I32Const(0),
                I32Store(mem(ACTIVE, 2)),
                LocalGet(3),
                LocalGet(2),
                Call(self.set_head.raw()),
                End,
            ],
        );
        f
    }
}
