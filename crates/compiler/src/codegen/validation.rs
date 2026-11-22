use crate::{exports::ExportOptions, typed_ast::TypedProgram};

use super::{CodegenError, closures, function, layout::Layout};

pub(super) struct ValidatedProgram<'a> {
    program: &'a TypedProgram,
    layout: Layout<'a>,
    bodies: Vec<Vec<u8>>,
    callable_bodies: Vec<Vec<u8>>,
}

impl<'a> ValidatedProgram<'a> {
    #[cfg(test)]
    pub fn new(program: &'a TypedProgram, exports: &ExportOptions) -> Result<Self, CodegenError> {
        Self::with_policy(program, exports, super::NumericPolicy::Baseline)
    }

    #[cfg(test)]
    pub fn with_policy(
        program: &'a TypedProgram,
        exports: &ExportOptions,
        policy: super::NumericPolicy,
    ) -> Result<Self, CodegenError> {
        Self::with_policies(
            program,
            exports,
            policy,
            super::RangePolicy::Allocated,
            super::FoldingPolicy::Disabled,
        )
    }

    pub(super) fn with_policies(
        program: &'a TypedProgram,
        exports: &ExportOptions,
        numeric: super::NumericPolicy,
        ranges: super::RangePolicy,
        folding: super::FoldingPolicy,
    ) -> Result<Self, CodegenError> {
        let mut layout = match exports {
            ExportOptions::All => Layout::new(program),
            _ => Layout::with_exports(program, exports),
        }?;
        layout.range_policy = ranges;
        layout.folding_policy = folding;
        Self::lower(program, layout, numeric)
    }

    #[cfg(test)]
    pub(super) fn with_regions(
        program: &'a TypedProgram,
        exports: &ExportOptions,
        policy: super::NumericPolicy,
    ) -> Result<Self, CodegenError> {
        Self::with_region_selection(program, exports, policy, super::region::select)
    }

    #[cfg(test)]
    pub(super) fn with_region_selection(
        program: &'a TypedProgram,
        exports: &ExportOptions,
        policy: super::NumericPolicy,
        select: impl FnOnce(&'a TypedProgram) -> Vec<super::region::Selection<'a>>,
    ) -> Result<Self, CodegenError> {
        let mut layout = Layout::with_exports(program, exports)?;
        layout.regions = select(program);
        Self::lower(program, layout, policy)
    }

    fn lower(
        program: &'a TypedProgram,
        layout: Layout<'a>,
        policy: super::NumericPolicy,
    ) -> Result<Self, CodegenError> {
        let mut bodies = Vec::with_capacity(program.functions.len());

        for function in &program.functions {
            bodies.push(function::emit(function, &layout, policy)?);
        }

        let mut callable_bodies = Vec::with_capacity(layout.callables.len());
        for plan in &layout.callables {
            callable_bodies.push(closures::emit_closure(plan.declaration, &layout, policy)?);
        }

        Ok(Self {
            program,
            layout,
            bodies,
            callable_bodies,
        })
    }

    pub fn program(&self) -> &'a TypedProgram {
        self.program
    }

    pub fn layout(&self) -> &Layout<'a> {
        &self.layout
    }

    pub fn callable_bodies(&self) -> &[Vec<u8>] {
        &self.callable_bodies
    }

    pub fn bodies(&self) -> &[Vec<u8>] {
        &self.bodies
    }
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
