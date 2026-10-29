use crate::{
    ast::Type,
    host::{supports_external_result, supports_external_value},
};
use wasm_encoder::ValType;

use super::{CodegenError, checked_u32};

pub(super) const DESCRIPTOR_SIZE: u32 = 8;
pub(super) const DESCRIPTOR_DATA_OFFSET: u64 = 0;
pub(super) const DESCRIPTOR_LENGTH_OFFSET: u64 = 4;
pub(super) const DESCRIPTOR_ALIGNMENT: u32 = 8;
pub(super) const DESCRIPTOR_FIELD_ALIGN_LOG2: u32 = 2;
pub(super) const NUMBER_ELEMENT_SIZE: u32 = 8;
pub(super) const NUMBER_ELEMENT_ALIGNMENT: u32 = 8;
pub(super) const NUMBER_ELEMENT_ALIGN_LOG2: u32 = NUMBER_ELEMENT_ALIGNMENT.trailing_zeros();
pub(super) const STRING_ELEMENT_SIZE: u32 = 1;
pub(super) const BOOL_ELEMENT_SIZE: u32 = 1;
pub(super) const STRING_ARRAY_ELEMENT_SIZE: u32 = 4;
pub(super) const MIN_ADDRESS: u32 = 8;
pub(super) const ALLOCATION_ALIGNMENT: u32 = 8;
pub(super) const MIN_ALLOCATION_BYTES: u32 = 8;

pub(super) const RANGE_SIZE: u32 = 32;
pub(super) const RANGE_ALIGNMENT: u32 = 8;
pub(super) const RANGE_START_OFFSET: u64 = 0;
pub(super) const RANGE_END_OFFSET: u64 = 8;
pub(super) const RANGE_STEP_OFFSET: u64 = 16;
pub(super) const RANGE_INCLUSIVE_OFFSET: u64 = 24;
pub(super) const RANGE_RESERVED_OFFSET: u64 = 28;
pub(super) const RANGE_INTEGER_ALIGN_LOG2: u32 = RANGE_ALIGNMENT.trailing_zeros();
pub(super) const RANGE_FLAG_ALIGN_LOG2: u32 = 2;
pub(super) const RANGE_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

// Private allocator records precede, and are excluded from, usable payloads.
pub(super) const ALLOCATOR_RECORD_BYTES: i32 = 32;
pub(super) const ALLOCATOR_REGISTRY_NEXT: u64 = 0;
pub(super) const ALLOCATOR_FREE_NEXT: u64 = 4;
pub(super) const ALLOCATOR_CLASS: u64 = 8;
pub(super) const ALLOCATOR_ACTIVE: u64 = 12;
pub(super) const ALLOCATOR_IDENTITY: u64 = 16;
pub(super) const ALLOCATOR_REQUESTED: u64 = 24;
pub(super) const ALLOCATOR_CLASS_COUNT: u32 = 29;
pub(super) const ALLOCATOR_MAX_REQUEST: i64 = 1 << 31;

// The public descriptor and private authority record share one retained object.
// Registry links point to the record, not to a structurally supplied descriptor.
pub(super) const RESIZABLE_OBJECT_BYTES: i32 = 48;
pub(super) const RESIZABLE_RECORD: i32 = 8;
pub(super) const RESIZABLE_NEXT: u64 = 0;
pub(super) const RESIZABLE_HANDLE: u64 = 4;
pub(super) const RESIZABLE_KIND: u64 = 8;
pub(super) const RESIZABLE_LENGTH: u64 = 12;
pub(super) const RESIZABLE_CAPACITY: u64 = 16;
pub(super) const RESIZABLE_PAYLOAD: u64 = 24;
pub(super) const RESIZABLE_RESERVED: u64 = 28;
pub(super) const RESIZABLE_IDENTITY: u64 = 32;

#[derive(Clone, Copy)]
pub(super) enum AllocatorHelper {
    Classify,
    Acquire,
    Find,
    Head,
    SetHead,
    Validate,
    Release,
}

impl AllocatorHelper {
    pub const ALL: [Self; 7] = [
        Self::Classify,
        Self::Acquire,
        Self::Find,
        Self::Head,
        Self::SetHead,
        Self::Validate,
        Self::Release,
    ];

    pub fn index(self) -> u32 {
        match self {
            Self::Classify => 0,
            Self::Acquire => 1,
            Self::Find => 2,
            Self::Head => 3,
            Self::SetHead => 4,
            Self::Validate => 5,
            Self::Release => 6,
        }
    }

    pub fn signature(self) -> Signature {
        use ValType::{I32, I64};
        let (params, results) = match self {
            Self::Classify => (vec![I64], vec![I32]),
            Self::Acquire => (vec![I64], vec![I32, I64]),
            Self::Find | Self::Head => (vec![I32], vec![I32]),
            Self::SetHead => (vec![I32, I32], vec![]),
            Self::Validate => (vec![I32, I64, I64], vec![]),
            Self::Release => (vec![I32, I64], vec![]),
        };
        Signature { params, results }
    }
}

pub(super) fn resizable_handle_signature() -> Signature {
    Signature {
        params: vec![ValType::I32],
        results: vec![ValType::I32],
    }
}

pub(super) fn resizable_push_signature(element: crate::ast::ArrayElementType) -> Signature {
    Signature {
        params: vec![
            ValType::I32,
            value_type("<push>", "item", element.value_type()).expect("scalar element"),
        ],
        results: vec![],
    }
}

pub(super) fn resizable_reserve_signature() -> Signature {
    Signature {
        params: vec![ValType::I32, ValType::F64],
        results: vec![],
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Signature {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

pub(super) fn internal_signature(
    function: &str,
    params: impl ExactSizeIterator<Item = Type>,
    return_type: Type,
) -> Result<Signature, CodegenError> {
    checked_u32(params.len(), "function parameter count")?;

    let params = params
        .enumerate()
        .map(|(index, ty)| {
            if ty == Type::Void {
                return Err(CodegenError::VoidParameter {
                    function: function.to_owned(),
                    index,
                });
            }

            value_type(function, "parameter", ty)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let result = if return_type == Type::Void {
        None
    } else {
        Some(value_type(function, "return", return_type)?)
    };

    Ok(Signature {
        params,
        results: result.into_iter().collect(),
    })
}

pub(super) fn check_external_signature(
    function: &str,
    params: impl Iterator<Item = Type>,
    return_type: Type,
) -> Result<(), CodegenError> {
    for (index, ty) in params.enumerate() {
        if ty == Type::Void {
            return Err(CodegenError::VoidParameter {
                function: function.to_owned(),
                index,
            });
        }

        if !supports_external_value(ty) {
            return Err(CodegenError::UnsupportedAbiType {
                function: function.to_owned(),
                position: "parameter",
                ty,
            });
        }
    }

    if !supports_external_result(return_type) {
        return Err(CodegenError::UnsupportedAbiType {
            function: function.to_owned(),
            position: "return",
            ty: return_type,
        });
    }

    Ok(())
}

pub(super) fn value_type(
    function: &str,
    position: &'static str,
    ty: Type,
) -> Result<ValType, CodegenError> {
    match ty {
        Type::Number => Ok(ValType::F64),
        Type::Bool | Type::String | Type::Array(_) | Type::Range => Ok(ValType::I32),
        Type::Void => Err(CodegenError::UnsupportedAbiType {
            function: function.to_owned(),
            position,
            ty,
        }),
    }
}

// Private closure storage: uniform slots trade space for deterministic alignment.
pub(super) const CAPTURE_CELL_SIZE: u32 = 8;
pub(super) const ENV_SLOT_SIZE: u32 = 8;
pub(super) fn environment_size(count: usize) -> Result<u32, CodegenError> {
    checked_u32(count, "closure environment slots")?
        .checked_mul(ENV_SLOT_SIZE)
        .ok_or(CodegenError::LimitExceeded {
            what: "closure environment size",
        })
}
pub(super) fn environment_slot(index: usize) -> Result<wasm_encoder::MemArg, CodegenError> {
    Ok(wasm_encoder::MemArg {
        offset: u64::from(environment_size(index)?),
        align: 2,
        memory_index: 0,
    })
}
pub(super) fn cell_arg(ty: Type) -> wasm_encoder::MemArg {
    wasm_encoder::MemArg {
        offset: 0,
        align: if ty == Type::Number { 3 } else { 2 },
        memory_index: 0,
    }
}
