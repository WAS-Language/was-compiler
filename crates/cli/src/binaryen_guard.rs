use std::fmt;

use wasmparser::BinaryReaderError;
use wasmparser::types::{EntityType, TypesRef};
use wasmparser::{
    FuncType, GlobalType, MemoryType, Parser, Payload, TableType, Validator, WasmFeatures,
};

#[derive(Debug, PartialEq, Eq)]
enum ExternalType {
    Func(FuncType),
    ExactFunc(FuncType),
    Table(TableType),
    Memory(MemoryType),
    Global(GlobalType),
    Tag(FuncType),
}

impl ExternalType {
    fn resolve(types: TypesRef<'_>, entity: EntityType) -> Self {
        match entity {
            EntityType::Func(id) => Self::Func(types[id].unwrap_func().clone()),
            EntityType::FuncExact(id) => Self::ExactFunc(types[id].unwrap_func().clone()),
            EntityType::Table(ty) => Self::Table(ty),
            EntityType::Memory(ty) => Self::Memory(ty),
            EntityType::Global(ty) => Self::Global(ty),
            EntityType::Tag(id) => Self::Tag(types[id].unwrap_func().clone()),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Interface<'a> {
    imports: Vec<(&'a str, &'a str, ExternalType)>,
    exports: Vec<(&'a str, ExternalType)>,
}

#[derive(Debug)]
pub(super) enum InspectError {
    InvalidWasm(BinaryReaderError),
    UnresolvedImport,
    UnresolvedExport,
    UnexpectedStart,
}

impl From<BinaryReaderError> for InspectError {
    fn from(error: BinaryReaderError) -> Self {
        Self::InvalidWasm(error)
    }
}

impl fmt::Display for InspectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWasm(error) => fmt::Display::fmt(error, f),
            Self::UnresolvedImport => f.write_str("unresolved import"),
            Self::UnresolvedExport => f.write_str("unresolved export"),
            Self::UnexpectedStart => f.write_str("unexpected start function"),
        }
    }
}

#[derive(Debug)]
pub(super) enum GuardError {
    InvalidInput(InspectError),
    InvalidOutput(InspectError),
    InterfaceChanged,
}

impl fmt::Display for GuardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(error) => write!(f, "Invalid Binaryen input: {error}"),
            Self::InvalidOutput(error) => write!(f, "Invalid Binaryen output: {error}"),
            Self::InterfaceChanged => {
                f.write_str("Binaryen output changed the ordered external interface")
            }
        }
    }
}

pub(super) fn check(input: &[u8], output: &[u8]) -> Result<(), GuardError> {
    let expected = inspect(input).map_err(GuardError::InvalidInput)?;
    let actual = inspect(output).map_err(GuardError::InvalidOutput)?;

    if actual != expected {
        return Err(GuardError::InterfaceChanged);
    }
    Ok(())
}

fn inspect(bytes: &[u8]) -> Result<Interface<'_>, InspectError> {
    let features = WasmFeatures::MVP | WasmFeatures::MULTI_VALUE;
    let types = Validator::new_with_features(features).validate_all(bytes)?;
    let types = types.as_ref();
    let mut interface = Interface::default();

    for payload in Parser::new(0).parse_all(bytes) {
        match payload? {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import?;
                    let entity = types
                        .entity_type_from_import(&import)
                        .ok_or(InspectError::UnresolvedImport)?;
                    interface.imports.push((
                        import.module,
                        import.name,
                        ExternalType::resolve(types, entity),
                    ));
                }
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export?;
                    let entity = types
                        .entity_type_from_export(&export)
                        .ok_or(InspectError::UnresolvedExport)?;
                    interface
                        .exports
                        .push((export.name, ExternalType::resolve(types, entity)));
                }
            }
            Payload::StartSection { .. } => return Err(InspectError::UnexpectedStart),
            _ => {}
        }
    }

    Ok(interface)
}
