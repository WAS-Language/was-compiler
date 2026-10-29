//! Opt-in array construction; ordinary descriptors never acquire authority implicitly.
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
    allocator_core::{self, HeadAccess, emit, mem},
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
        struct Discovery([bool; 3]);
        impl Visitor<'_> for Discovery {
            type Break = std::convert::Infallible;
            fn enter_expression(&mut self, expr: &TypedExpr) -> ControlFlow<Self::Break> {
                if let TypedExprKind::Call {
                    func: FunctionRef::Builtin(BuiltinFunction::Resizable),
                    args,
                    ..
                } = &expr.kind
                {
                    let element = args
                        .first()
                        .map_or(ArrayElementType::Number, array::element);
                    self.0[ordinal(element)] = true;
                }
                ControlFlow::Continue(())
            }
        }
        let mut discovery = Discovery([false; 3]);
        for function in &program.functions {
            let _ = walker::walk_function(function, &mut discovery);
        }
        let elements: Vec<_> = [
            ArrayElementType::Number,
            ArrayElementType::Bool,
            ArrayElementType::String,
        ]
        .into_iter()
        .filter(|e| discovery.0[ordinal(*e)])
        .collect();
        if elements.is_empty() {
            return Ok(None);
        }
        let core_count = super::checked_u32(AllocatorHelper::ALL.len(), "M4 core helper count")?;
        let count = core_count + super::checked_u32(elements.len(), "M4 constructor count")?;
        first_function
            .checked_add(count)
            .ok_or(CodegenError::LimitExceeded {
                what: "M4 planned index",
            })?;
        let types = AllocatorHelper::ALL
            .into_iter()
            .map(AllocatorHelper::signature)
            .chain(
                elements
                    .iter()
                    .map(|_| abi::resizable_constructor_signature()),
            )
            .map(&mut intern)
            .collect::<Result<Vec<_>, _>>()?;
        let first_function = FunctionIndex::new(first_function);
        let core = allocator_core::Emitter {
            classify: first_function.offset(AllocatorHelper::Classify.index()),
            find: first_function.offset(AllocatorHelper::Find.index()),
            head: first_function.offset(AllocatorHelper::Head.index()),
            set_head: first_function.offset(AllocatorHelper::SetHead.index()),
            backing: backing.expect("constructor discovery also requires ordinary memory"),
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
        }))
    }

    pub(super) fn constructor(&self, element: ArrayElementType) -> FunctionIndex {
        let slot = self
            .elements
            .iter()
            .position(|e| *e == element)
            .expect("constructor discovered structurally");
        self.first_function
            .offset(u32::try_from(AllocatorHelper::ALL.len() + slot).expect("bounded helpers"))
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
        ] {
            code.function(&body);
        }
        for &element in &self.elements {
            code.function(&self.emit_constructor(element, memory));
        }
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
