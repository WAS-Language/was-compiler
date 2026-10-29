//! Private opt-in array construction, authority checks, relocation and append.
use std::ops::ControlFlow;

use crate::{
    ast::ArrayElementType,
    typed_ast::{BuiltinFunction, FunctionRef, TypedExpr, TypedExprKind, TypedProgram},
};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, Function, FunctionSection, GlobalSection, GlobalType,
    Instruction, ValType,
};

use super::{
    CodegenError, abi,
    allocator_core::{self, HeadAccess, emit, mem, trap_if},
    array,
    indices::{FunctionIndex, TypeIndex},
    layout::MemoryLayout,
    memory::Helper,
    walker::{self, Visitor},
};

use abi::{
    AllocatorHelper, RESIZABLE_CAPACITY as CAPACITY, RESIZABLE_HANDLE as HANDLE,
    RESIZABLE_IDENTITY as IDENTITY, RESIZABLE_KIND as KIND, RESIZABLE_LENGTH as LENGTH,
    RESIZABLE_NEXT as NEXT, RESIZABLE_OBJECT_BYTES as OBJECT_BYTES, RESIZABLE_PAYLOAD as PAYLOAD,
    RESIZABLE_RECORD as RECORD, RESIZABLE_RESERVED as RESERVED, Signature,
};

const LAST_IDENTITY_GLOBAL: u32 = 1;
const ALLOCATOR_REGISTRY_GLOBAL: u32 = LAST_IDENTITY_GLOBAL + 1;
const FIRST_HEAD_GLOBAL: u32 = ALLOCATOR_REGISTRY_GLOBAL + 1;
const ARRAY_REGISTRY_GLOBAL: u32 = FIRST_HEAD_GLOBAL + abi::ALLOCATOR_CLASS_COUNT;

pub(super) struct Plan {
    first_function: FunctionIndex,
    types: Vec<TypeIndex>,
    core: allocator_core::Emitter,
    array_registry: u32,
    elements: Vec<ArrayElementType>,
    constructors: Vec<ArrayElementType>,
    reserves: Vec<ArrayElementType>,
    pushes: Vec<ArrayElementType>,
    pops: Vec<ArrayElementType>,
    removes: Vec<ArrayElementType>,
}

#[derive(Clone, Copy)]
enum Take {
    Pop,
    Remove,
}

enum AuthorityHelper {
    Lookup,
    Validate,
}

impl AuthorityHelper {
    fn index(self) -> usize {
        match self {
            Self::Lookup => 0,
            Self::Validate => 1,
        }
    }
}

fn ordinal(element: ArrayElementType) -> usize {
    match element {
        ArrayElementType::Number => 0,
        ArrayElementType::Bool => 1,
        ArrayElementType::String => 2,
    }
}

impl Plan {
    pub(super) fn new(
        program: &TypedProgram,
        first_function: u32,
        backing: Option<FunctionIndex>,
        mut intern: impl FnMut(Signature) -> Result<TypeIndex, CodegenError>,
    ) -> Result<Option<Self>, CodegenError> {
        #[derive(Default)]
        struct Discovery {
            elements: [bool; 3],
            constructors: [bool; 3],
            reserves: [bool; 3],
            pushes: [bool; 3],
            pops: [bool; 3],
            removes: [bool; 3],
        }
        impl Visitor<'_> for Discovery {
            type Break = std::convert::Infallible;
            fn enter_expression(&mut self, expr: &TypedExpr) -> ControlFlow<Self::Break> {
                if let Some(element) = expr.array_element_type() {
                    self.elements[ordinal(element)] = true;
                }
                match &expr.kind {
                    TypedExprKind::Pop { receiver } => {
                        let element = ordinal(array::element(receiver));
                        self.pops[element] = true;
                        self.elements[element] = true;
                    }
                    TypedExprKind::Remove { receiver, .. } => {
                        let element = ordinal(array::element(receiver));
                        self.removes[element] = true;
                        self.elements[element] = true;
                    }
                    _ => {}
                }
                if let TypedExprKind::Push { receiver, .. } = &expr.kind {
                    self.pushes[ordinal(array::element(receiver))] = true;
                }
                if let TypedExprKind::Reserve { receiver, .. }
                | TypedExprKind::Push { receiver, .. } = &expr.kind
                {
                    let element = array::element(receiver);
                    self.reserves[ordinal(element)] = true;
                    self.elements[ordinal(element)] = true;
                }
                if let TypedExprKind::Call {
                    func: FunctionRef::Builtin(BuiltinFunction::Resizable),
                    args,
                    ..
                } = &expr.kind
                {
                    let element = args
                        .first()
                        .map_or(ArrayElementType::Number, array::element);
                    self.constructors[ordinal(element)] = true;
                    self.elements[ordinal(element)] = true;
                }
                ControlFlow::Continue(())
            }
        }
        let mut discovery = Discovery::default();
        for function in &program.functions {
            let _ = walker::walk_function(function, &mut discovery);
        }
        let select = |selected: [bool; 3]| -> Vec<_> {
            [
                ArrayElementType::Number,
                ArrayElementType::Bool,
                ArrayElementType::String,
            ]
            .into_iter()
            .filter(|e| selected[ordinal(*e)])
            .collect()
        };
        let constructors = select(discovery.constructors);
        let reserves = select(discovery.reserves);
        let pushes = select(discovery.pushes);
        let pops = select(discovery.pops);
        let removes = select(discovery.removes);
        if constructors.is_empty() && reserves.is_empty() && pops.is_empty() && removes.is_empty() {
            return Ok(None);
        }
        let elements = select(discovery.elements);
        let count = super::checked_u32(
            AllocatorHelper::ALL.len()
                + constructors.len()
                + 2 * elements.len()
                + reserves.len()
                + pushes.len()
                + pops.len()
                + removes.len(),
            "M4 helper count",
        )?;
        first_function
            .checked_add(count)
            .ok_or(CodegenError::LimitExceeded {
                what: "M4 planned index",
            })?;
        let types = AllocatorHelper::ALL
            .into_iter()
            .map(AllocatorHelper::signature)
            .chain(
                constructors
                    .iter()
                    .map(|_| abi::resizable_handle_signature()),
            )
            .chain(elements.iter().flat_map(|_| {
                [
                    abi::resizable_handle_signature(),
                    abi::resizable_handle_signature(),
                ]
            }))
            .chain(reserves.iter().map(|_| abi::resizable_reserve_signature()))
            .chain(pushes.iter().copied().map(abi::resizable_push_signature))
            .chain(pops.iter().copied().map(abi::resizable_pop_signature))
            .chain(removes.iter().copied().map(abi::resizable_remove_signature))
            .map(&mut intern)
            .collect::<Result<Vec<_>, _>>()?;
        let first_function = FunctionIndex::new(first_function);
        let core = allocator_core::Emitter {
            classify: first_function.offset(AllocatorHelper::Classify.index()),
            find: first_function.offset(AllocatorHelper::Find.index()),
            head: first_function.offset(AllocatorHelper::Head.index()),
            set_head: first_function.offset(AllocatorHelper::SetHead.index()),
            backing: backing.expect("M4 discovery also requires ordinary memory"),
            last_identity: LAST_IDENTITY_GLOBAL,
            registry: ALLOCATOR_REGISTRY_GLOBAL,
            heads: std::array::from_fn(|i| {
                FIRST_HEAD_GLOBAL + u32::try_from(i).expect("bounded class")
            }),
        };
        Ok(Some(Self {
            first_function,
            types,
            core,
            array_registry: ARRAY_REGISTRY_GLOBAL,
            elements,
            constructors,
            reserves,
            pushes,
            pops,
            removes,
        }))
    }

    pub(super) fn constructor(&self, element: ArrayElementType) -> FunctionIndex {
        let slot = self
            .constructors
            .iter()
            .position(|e| *e == element)
            .expect("constructor discovered structurally");
        self.first_function
            .offset(u32::try_from(AllocatorHelper::ALL.len() + slot).expect("bounded helpers"))
    }

    /// Returns zero for fixed storage; registered objects are fully validated.
    pub(super) fn lookup(&self, element: ArrayElementType) -> FunctionIndex {
        self.authority_helper(element, AuthorityHelper::Lookup)
    }

    /// Returns the authority record, trapping if the exact handle is not registered.
    pub(super) fn validate(&self, element: ArrayElementType) -> FunctionIndex {
        self.authority_helper(element, AuthorityHelper::Validate)
    }

    pub(super) fn reserve(&self, element: ArrayElementType) -> FunctionIndex {
        let slot = self
            .reserves
            .iter()
            .position(|e| *e == element)
            .expect("reserve discovered structurally");
        self.first_function.offset(
            u32::try_from(
                AllocatorHelper::ALL.len()
                    + self.constructors.len()
                    + 2 * self.elements.len()
                    + slot,
            )
            .expect("bounded helpers"),
        )
    }

    pub(super) fn push(&self, element: ArrayElementType) -> FunctionIndex {
        let slot = self
            .pushes
            .iter()
            .position(|e| *e == element)
            .expect("push discovered structurally");
        self.first_function.offset(
            u32::try_from(
                AllocatorHelper::ALL.len()
                    + self.constructors.len()
                    + 2 * self.elements.len()
                    + self.reserves.len()
                    + slot,
            )
            .expect("bounded helpers"),
        )
    }

    pub(super) fn pop(&self, element: ArrayElementType) -> FunctionIndex {
        self.take_helper(element, Take::Pop)
    }

    pub(super) fn remove(&self, element: ArrayElementType) -> FunctionIndex {
        self.take_helper(element, Take::Remove)
    }

    fn take_helper(&self, element: ArrayElementType, operation: Take) -> FunctionIndex {
        let (elements, preceding) = match operation {
            Take::Pop => (&self.pops, 0),
            Take::Remove => (&self.removes, self.pops.len()),
        };
        let slot = elements
            .iter()
            .position(|e| *e == element)
            .expect("pop/remove discovered structurally");
        self.first_function.offset(
            u32::try_from(
                AllocatorHelper::ALL.len()
                    + self.constructors.len()
                    + 2 * self.elements.len()
                    + self.reserves.len()
                    + self.pushes.len()
                    + preceding
                    + slot,
            )
            .expect("bounded helpers"),
        )
    }

    fn authority_helper(
        &self,
        element: ArrayElementType,
        operation: AuthorityHelper,
    ) -> FunctionIndex {
        let slot = self
            .elements
            .iter()
            .position(|e| *e == element)
            .expect("array type discovered structurally");
        self.first_function.offset(
            u32::try_from(
                AllocatorHelper::ALL.len() + self.constructors.len() + 2 * slot + operation.index(),
            )
            .expect("bounded helpers"),
        )
    }

    fn core_helper(&self, helper: AllocatorHelper) -> FunctionIndex {
        self.first_function.offset(helper.index())
    }

    pub(super) fn append(
        &self,
        functions: &mut FunctionSection,
        globals: &mut GlobalSection,
        code: &mut CodeSection,
        memory: &MemoryLayout,
    ) {
        use ValType::{I32, I64};
        let core = &self.core;
        // Identity, allocator registry, class heads, then array registry.
        for ty in std::iter::once(I64)
            .chain(std::iter::once(I32))
            .chain(std::iter::repeat_n(
                I32,
                abi::ALLOCATOR_CLASS_COUNT as usize,
            ))
            .chain(std::iter::once(I32))
        {
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
        for type_index in &self.types {
            functions.function(type_index.raw());
        }
        for body in [
            core.classify(),
            core.acquire(&[], &[]),
            core.find(&[]),
            core.head(HeadAccess::Read),
            core.head(HeadAccess::Write),
            core.validate(),
            core.release(),
        ] {
            code.function(&body);
        }
        for &element in &self.constructors {
            code.function(&self.emit_constructor(element, memory));
        }
        for &element in &self.elements {
            code.function(&self.emit_lookup(element, memory));
            code.function(&self.emit_validate(element));
        }
        for &element in &self.reserves {
            code.function(&self.emit_reserve(element));
        }
        for &element in &self.pushes {
            code.function(&self.emit_push(element, memory));
        }
        for &element in &self.pops {
            code.function(&self.emit_take(element, Take::Pop, memory));
        }
        for &element in &self.removes {
            code.function(&self.emit_take(element, Take::Remove, memory));
        }
    }

    fn emit_lookup(&self, element: ArrayElementType, memory: &MemoryLayout) -> Function {
        use Instruction::*;
        // handle0, record1. Never derive an authority record from a supplied handle.
        let mut f = Function::new([(1, ValType::I32)]);
        emit(
            &mut f,
            &[
                GlobalGet(self.array_registry),
                LocalSet(1),
                Block(BlockType::Empty),
                Loop(BlockType::Empty),
                LocalGet(1),
                I32Eqz,
                BrIf(1),
                LocalGet(1),
                I32Load(mem(HANDLE, 2)),
                LocalGet(0),
                I32Eq,
                If(BlockType::Empty),
                LocalGet(1),
                I32Load(mem(KIND, 2)),
                I32Const(i32::try_from(ordinal(element)).expect("element kind")),
                I32Ne,
                LocalGet(1),
                I32Load(mem(RESERVED, 2)),
                I32Or,
            ],
        );
        trap_if(&mut f);
        emit(
            &mut f,
            &[
                LocalGet(0),
                Call(
                    memory
                        .first_function
                        .offset(memory.plan.helper_index(array::validator(element)))
                        .raw(),
                ),
                Drop,
                LocalGet(1),
                I32Load(mem(LENGTH, 2)),
                LocalGet(0),
                I32Load(mem(
                    abi::DESCRIPTOR_LENGTH_OFFSET,
                    abi::DESCRIPTOR_FIELD_ALIGN_LOG2,
                )),
                I32Ne,
                LocalGet(1),
                I32Load(mem(PAYLOAD, 2)),
                LocalGet(0),
                I32Load(mem(
                    abi::DESCRIPTOR_DATA_OFFSET,
                    abi::DESCRIPTOR_FIELD_ALIGN_LOG2,
                )),
                I32Ne,
                I32Or,
                LocalGet(1),
                I64Load(mem(CAPACITY, 3)),
                I64Const(i64::from(abi::MIN_ALLOCATION_BYTES)),
                I64LtU,
                I32Or,
                LocalGet(1),
                I64Load(mem(CAPACITY, 3)),
                I64Const(abi::ALLOCATOR_MAX_REQUEST),
                I64GtU,
                I32Or,
                LocalGet(1),
                I32Load(mem(LENGTH, 2)),
                I64ExtendI32U,
                I64Const(i64::from(array::width(element))),
                I64Mul,
                LocalGet(1),
                I64Load(mem(CAPACITY, 3)),
                I64GtU,
                I32Or,
            ],
        );
        trap_if(&mut f);
        emit(
            &mut f,
            &[
                LocalGet(1),
                I32Load(mem(PAYLOAD, 2)),
                LocalGet(1),
                I64Load(mem(IDENTITY, 3)),
                LocalGet(1),
                I64Load(mem(CAPACITY, 3)),
                Call(self.core_helper(AllocatorHelper::Validate).raw()),
                LocalGet(1),
                Return,
                End,
                LocalGet(1),
                I32Load(mem(NEXT, 2)),
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

    fn emit_validate(&self, element: ArrayElementType) -> Function {
        use Instruction::*;
        let mut f = Function::new([(1, ValType::I32)]);
        emit(
            &mut f,
            &[
                LocalGet(0),
                Call(self.lookup(element).raw()),
                LocalTee(1),
                I32Eqz,
            ],
        );
        trap_if(&mut f);
        emit(&mut f, &[LocalGet(1), End]);
        f
    }

    fn emit_reserve(&self, element: ArrayElementType) -> Function {
        use Instruction::*;
        // handle0:i32, additional1:f64; record2, old3, new4, byte5:i32;
        // live6, capacity7, old_identity8, new_identity9:i64.
        let mut f = Function::new([(4, ValType::I32), (4, ValType::I64)]);
        emit(
            &mut f,
            &[
                // Argument effects have completed; capture the current published state anew.
                LocalGet(0),
                Call(self.validate(element).raw()),
                LocalSet(2),
                LocalGet(1),
                F64Const(0.0.into()),
                F64Ge,
                LocalGet(1),
                F64Const(
                    f64::from(
                        u32::try_from(abi::ALLOCATOR_MAX_REQUEST)
                            .expect("memory32 request ceiling"),
                    )
                    .into(),
                ),
                F64Le,
                I32And,
                I32Eqz,
                LocalGet(1),
                LocalGet(1),
                F64Trunc,
                F64Ne,
                I32Or,
            ],
        );
        trap_if(&mut f);
        emit(
            &mut f,
            &[
                // Both addends are bounded before conversion; sizing stays widened.
                LocalGet(2),
                I32Load(mem(LENGTH, 2)),
                I64ExtendI32U,
                I64Const(i64::from(array::width(element))),
                I64Mul,
                LocalSet(6),
                LocalGet(2),
                I32Load(mem(LENGTH, 2)),
                I64ExtendI32U,
                LocalGet(1),
                I64TruncF64U,
                I64Add,
                I64Const(i64::from(array::width(element))),
                I64Mul,
                LocalTee(7),
                I64Const(abi::ALLOCATOR_MAX_REQUEST),
                I64GtU,
            ],
        );
        trap_if(&mut f);
        emit(
            &mut f,
            &[
                LocalGet(7),
                LocalGet(2),
                I64Load(mem(CAPACITY, 3)),
                I64LeU,
                If(BlockType::Empty),
                Return,
                End,
                LocalGet(7),
                Call(self.core_helper(AllocatorHelper::Classify).raw()),
                I64ExtendI32U,
                I64Const(i64::from(abi::MIN_ALLOCATION_BYTES.trailing_zeros())),
                I64Add,
                LocalSet(7),
                I64Const(1),
                LocalGet(7),
                I64Shl,
                LocalSet(7),
                LocalGet(2),
                I32Load(mem(PAYLOAD, 2)),
                LocalSet(3),
                LocalGet(2),
                I64Load(mem(IDENTITY, 3)),
                LocalSet(8),
                LocalGet(7),
                Call(self.core_helper(AllocatorHelper::Acquire).raw()),
                LocalSet(9),
                LocalSet(4),
                LocalGet(4),
                LocalGet(9),
                LocalGet(7),
                Call(self.core_helper(AllocatorHelper::Validate).raw()),
                // Copy only live bytes. Spare capacity and unconsumed scalar values are not scanned.
                Block(BlockType::Empty),
                Loop(BlockType::Empty),
                LocalGet(5),
                I64ExtendI32U,
                LocalGet(6),
                I64GeU,
                BrIf(1),
                LocalGet(4),
                LocalGet(5),
                I32Add,
                LocalGet(3),
                LocalGet(5),
                I32Add,
                I32Load8U(mem(0, 0)),
                I32Store8(mem(0, 0)),
                LocalGet(5),
                I32Const(1),
                I32Add,
                LocalSet(5),
                Br(0),
                End,
                End,
                // Preparation is complete. Publication and release have no callbacks.
                LocalGet(0),
                LocalGet(4),
                I32Store(mem(
                    abi::DESCRIPTOR_DATA_OFFSET,
                    abi::DESCRIPTOR_FIELD_ALIGN_LOG2,
                )),
                LocalGet(2),
                LocalGet(4),
                I32Store(mem(PAYLOAD, 2)),
                LocalGet(2),
                LocalGet(7),
                I64Store(mem(CAPACITY, 3)),
                LocalGet(2),
                LocalGet(9),
                I64Store(mem(IDENTITY, 3)),
                LocalGet(3),
                LocalGet(8),
                Call(self.core_helper(AllocatorHelper::Release).raw()),
                End,
            ],
        );
        f
    }

    fn emit_push(&self, element: ArrayElementType, memory: &MemoryLayout) -> Function {
        use Instruction::*;
        // handle0:i32, item1:scalar; record2, length3:i32.
        let mut f = Function::new([(2, ValType::I32)]);
        emit(
            &mut f,
            &[LocalGet(0), Call(self.validate(element).raw()), LocalSet(2)],
        );
        if let Some(consumer) = array::consumer(element) {
            emit(
                &mut f,
                &[
                    LocalGet(1),
                    Call(
                        memory
                            .first_function
                            .offset(memory.plan.helper_index(consumer))
                            .raw(),
                    ),
                    Drop,
                ],
            );
        }
        emit(
            &mut f,
            &[
                // Reserve owns all sizing, acquisition, raw copy and release logic.
                LocalGet(0),
                F64Const(1.0.into()),
                Call(self.reserve(element).raw()),
                // No callbacks or expected failures remain: reserve proved this slot
                // fits the current owned extent. Initialize it before either length.
                LocalGet(2),
                I32Load(mem(LENGTH, 2)),
                LocalSet(3),
                LocalGet(2),
                I32Load(mem(PAYLOAD, 2)),
                LocalGet(3),
                I32Const(i32::try_from(array::width(element)).expect("element width")),
                I32Mul,
                I32Add,
                LocalGet(1),
                array::store(element, 0),
                LocalGet(0),
                LocalGet(3),
                I32Const(1),
                I32Add,
                I32Store(mem(
                    abi::DESCRIPTOR_LENGTH_OFFSET,
                    abi::DESCRIPTOR_FIELD_ALIGN_LOG2,
                )),
                LocalGet(2),
                LocalGet(3),
                I32Const(1),
                I32Add,
                I32Store(mem(LENGTH, 2)),
                End,
            ],
        );
        f
    }

    fn emit_take(
        &self,
        element: ArrayElementType,
        operation: Take,
        memory: &MemoryLayout,
    ) -> Function {
        use Instruction::*;
        let record = match operation {
            Take::Pop => 1,
            Take::Remove => 2,
        };
        let length = record + 1;
        let address = record + 2;
        let cursor = record + 3;
        let end = record + 4;
        let value = record + 5;
        let scalar = abi::value_type("<pop/remove>", "result", element.value_type())
            .expect("scalar element");
        let mut f = Function::new([(5, ValType::I32), (1, scalar)]);
        let helper = |helper| {
            memory
                .first_function
                .offset(memory.plan.helper_index(helper))
                .raw()
        };
        emit(
            &mut f,
            &[
                LocalGet(0),
                Call(self.validate(element).raw()),
                LocalSet(record),
            ],
        );
        match operation {
            Take::Pop => {
                emit(&mut f, &[LocalGet(record), I32Load(mem(LENGTH, 2)), I32Eqz]);
                trap_if(&mut f);
            }
            Take::Remove => {
                // Shared indexing owns finite/integral/nonnegative/bounds checks,
                // including -0, against the post-argument descriptor.
                emit(
                    &mut f,
                    &[
                        LocalGet(0),
                        LocalGet(1),
                        Call(helper(array::address(element))),
                        LocalSet(address),
                    ],
                );
            }
        }
        emit(
            &mut f,
            &[
                LocalGet(record),
                I32Load(mem(LENGTH, 2)),
                I32Const(1),
                I32Sub,
                LocalSet(length),
            ],
        );
        if matches!(operation, Take::Pop) {
            emit(
                &mut f,
                &[
                    LocalGet(record),
                    I32Load(mem(PAYLOAD, 2)),
                    LocalGet(length),
                    I32Const(i32::try_from(array::width(element)).expect("element width")),
                    I32Mul,
                    I32Add,
                    LocalSet(address),
                ],
            );
        }
        emit(&mut f, &[LocalGet(address), array::load(element, 0)]);
        if let Some(consumer) = array::consumer(element) {
            emit(&mut f, &[Call(helper(consumer))]);
        }
        emit(&mut f, &[LocalSet(value)]);
        if matches!(operation, Take::Remove) {
            // Forward byte copying is overlap-safe for a left shift. Only the
            // returned slot is consumed; unreturned bits/handles are copied raw.
            emit(
                &mut f,
                &[
                    LocalGet(address),
                    LocalSet(cursor),
                    LocalGet(record),
                    I32Load(mem(PAYLOAD, 2)),
                    LocalGet(length),
                    I32Const(i32::try_from(array::width(element)).expect("element width")),
                    I32Mul,
                    I32Add,
                    LocalSet(end),
                    Block(BlockType::Empty),
                    Loop(BlockType::Empty),
                    LocalGet(cursor),
                    LocalGet(end),
                    I32GeU,
                    BrIf(1),
                    LocalGet(cursor),
                    LocalGet(cursor),
                    I32Load8U(mem(u64::from(array::width(element)), 0)),
                    I32Store8(mem(0, 0)),
                    LocalGet(cursor),
                    I32Const(1),
                    I32Add,
                    LocalSet(cursor),
                    Br(0),
                    End,
                    End,
                ],
            );
        }
        // All checks precede the first mutation. Neither operation changes the
        // payload, capacity or acquisition, and no callback runs during publication.
        emit(
            &mut f,
            &[
                LocalGet(0),
                LocalGet(length),
                I32Store(mem(
                    abi::DESCRIPTOR_LENGTH_OFFSET,
                    abi::DESCRIPTOR_FIELD_ALIGN_LOG2,
                )),
                LocalGet(record),
                LocalGet(length),
                I32Store(mem(LENGTH, 2)),
                LocalGet(value),
                End,
            ],
        );
        f
    }

    fn emit_constructor(&self, element: ArrayElementType, memory: &MemoryLayout) -> Function {
        use Instruction::*;
        // source0, length1, payload2, object3, record4, index5:i32;
        // capacity6, identity7:i64. No callbacks occur during validation/copy/publication.
        let mut f = Function::new([(5, ValType::I32), (2, ValType::I64)]);
        let helper = |h| {
            memory
                .first_function
                .offset(memory.plan.helper_index(h))
                .raw()
        };
        emit(
            &mut f,
            &[
                LocalGet(0),
                Call(helper(array::validator(element))),
                Drop,
                LocalGet(0),
                I32Load(mem(
                    abi::DESCRIPTOR_LENGTH_OFFSET,
                    abi::DESCRIPTOR_FIELD_ALIGN_LOG2,
                )),
                LocalSet(1),
                I64Const(i64::from(array::width(element))),
                LocalGet(1),
                I64ExtendI32U,
                I64Mul,
                Call(
                    self.first_function
                        .offset(AllocatorHelper::Classify.index())
                        .raw(),
                ),
                I64ExtendI32U,
                I64Const(i64::from(abi::MIN_ALLOCATION_BYTES.trailing_zeros())),
                I64Add,
                LocalSet(6),
            ],
        );
        // Request the entire selected capacity, not merely the logical extent.
        emit(
            &mut f,
            &[
                I64Const(1),
                LocalGet(6),
                I64Shl,
                LocalTee(6),
                Call(
                    self.first_function
                        .offset(AllocatorHelper::Acquire.index())
                        .raw(),
                ),
                LocalSet(7),
                LocalSet(2),
                LocalGet(2),
                LocalGet(7),
                LocalGet(6),
                Call(
                    self.first_function
                        .offset(AllocatorHelper::Validate.index())
                        .raw(),
                ),
                I32Const(OBJECT_BYTES),
                Call(helper(Helper::Alloc)),
                LocalTee(3),
                I32Const(RECORD),
                I32Add,
                LocalSet(4),
                Block(BlockType::Empty),
                Loop(BlockType::Empty),
                LocalGet(5),
                LocalGet(1),
                I32GeU,
                BrIf(1),
                LocalGet(2),
                LocalGet(5),
                I32Const(i32::try_from(array::width(element)).expect("element width")),
                I32Mul,
                I32Add,
                LocalGet(0),
                I32Load(mem(
                    abi::DESCRIPTOR_DATA_OFFSET,
                    abi::DESCRIPTOR_FIELD_ALIGN_LOG2,
                )),
                LocalGet(5),
                I32Const(i32::try_from(array::width(element)).expect("element width")),
                I32Mul,
                I32Add,
                array::load(element, 0),
            ],
        );
        if let Some(consumer) = array::consumer(element) {
            emit(&mut f, &[Call(helper(consumer))]);
        }
        emit(
            &mut f,
            &[
                array::store(element, 0),
                LocalGet(5),
                I32Const(1),
                I32Add,
                LocalSet(5),
                Br(0),
                End,
                End,
                LocalGet(3),
                LocalGet(2),
                I32Store(mem(
                    abi::DESCRIPTOR_DATA_OFFSET,
                    abi::DESCRIPTOR_FIELD_ALIGN_LOG2,
                )),
                LocalGet(3),
                LocalGet(1),
                I32Store(mem(
                    abi::DESCRIPTOR_LENGTH_OFFSET,
                    abi::DESCRIPTOR_FIELD_ALIGN_LOG2,
                )),
                LocalGet(4),
                GlobalGet(self.array_registry),
                I32Store(mem(NEXT, 2)),
                LocalGet(4),
                LocalGet(3),
                I32Store(mem(HANDLE, 2)),
                LocalGet(4),
                I32Const(i32::try_from(ordinal(element)).expect("element kind")),
                I32Store(mem(KIND, 2)),
                LocalGet(4),
                LocalGet(1),
                I32Store(mem(LENGTH, 2)),
                LocalGet(4),
                LocalGet(6),
                I64Store(mem(CAPACITY, 3)),
                LocalGet(4),
                LocalGet(2),
                I32Store(mem(PAYLOAD, 2)),
                LocalGet(4),
                I32Const(0),
                I32Store(mem(RESERVED, 2)),
                LocalGet(4),
                LocalGet(7),
                I64Store(mem(IDENTITY, 3)),
                LocalGet(4),
                GlobalSet(self.array_registry),
                LocalGet(3),
                End,
            ],
        );
        f
    }
}

#[cfg(test)]
#[path = "resizable_failure_tests.rs"]
mod failure_tests;

#[cfg(test)]
mod tests {
    use crate::Compiler;
    use wasmparser::{Operator, Parser, Payload, Validator};

    #[test]
    fn push_only_selects_reserve_and_preserves_existing_helper_bodies() {
        for ty in ["number", "bool", "string"] {
            let compile = |method: &str, argument: &str| {
                Compiler::new()
                    .compile_source(&format!(
                        "fn main(a: {ty}[], item: {ty}) {{ a.{method}({argument}); }}"
                    ))
                    .unwrap()
            };
            let reserve = compile("reserve", "1");
            let push = compile("push", "item");
            Validator::new().validate_all(&push).unwrap();
            let bodies = |bytes: &[u8]| {
                Parser::new(0)
                    .parse_all(bytes)
                    .filter_map(|payload| match payload.unwrap() {
                        Payload::CodeSectionEntry(body) => Some(body.as_bytes().to_vec()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            let before = bodies(&reserve);
            let after = bodies(&push);
            assert_eq!(after.len(), before.len() + 1);
            // The changed user body is followed by byte-identical existing helpers;
            // push adds only its own helper after the selected reserve helper.
            assert_eq!(&after[1..before.len()], &before[1..]);
            let push_body = Parser::new(0)
                .parse_all(&push)
                .filter_map(|payload| match payload.unwrap() {
                    Payload::CodeSectionEntry(body) => Some(body),
                    _ => None,
                })
                .last()
                .unwrap();
            let ops = push_body
                .get_operators_reader()
                .unwrap()
                .into_iter()
                .map(Result::unwrap)
                .collect::<Vec<_>>();
            let calls = ops
                .iter()
                .filter_map(|op| match op {
                    Operator::Call { function_index } => Some(*function_index),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(calls.len(), if ty == "number" { 2 } else { 3 });
            assert_eq!(
                *calls.last().unwrap(),
                u32::try_from(before.len() - 1).unwrap()
            );
            let reserve_call = ops
                .iter()
                .rposition(|op| matches!(op, Operator::Call { .. }))
                .unwrap();
            let stores = ops
                .iter()
                .enumerate()
                .filter_map(|(index, op)| match op {
                    Operator::F64Store { memarg }
                    | Operator::I32Store8 { memarg }
                    | Operator::I32Store { memarg } => Some((index, memarg.offset)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(stores.len(), 3);
            assert!(reserve_call < stores[0].0);
            // Independent expected offsets: slot, public length, private length.
            assert_eq!(
                stores.iter().map(|(_, offset)| *offset).collect::<Vec<_>>(),
                [0, 4, 12]
            );
            assert!(!ops[reserve_call + 1..].iter().any(|op| matches!(
                op,
                Operator::Unreachable | Operator::MemoryGrow { .. } | Operator::Call { .. }
            )));
        }
    }
}
