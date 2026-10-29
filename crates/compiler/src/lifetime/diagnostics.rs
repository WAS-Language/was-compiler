//! Structural explanations, independent of the closed positive proof.

use super::{EscapePath, Reason, UnprovenReason};
use crate::{
    ast::{BinaryOp, Type},
    typed_ast::{
        BindingOwner, FunctionRef, TypedBlock, TypedClosure, TypedClosureBody, TypedExpr,
        TypedExprKind, TypedFunction, TypedStatementKind, TypedValueBlock, VariableId,
    },
};
use std::collections::HashSet;

type Key = (BindingOwner, VariableId);

pub(super) fn collect(function: &TypedFunction, literal: &TypedExpr) -> Vec<Reason> {
    let owner = BindingOwner::Function(function.id);
    let mut scan = Scan {
        root: owner,
        owner,
        literal,
        phase: Phase::Inventory,
        declarations: HashSet::new(),
        writes: HashSet::new(),
        copies: HashSet::new(),
        reasons: Vec::new(),
        inconsistent: false,
    };
    // Inventory all owners first: even skipped/later writes disqualify a copy
    // chain as diagnostic evidence. No flow joins or alias fixpoint are computed.
    scan.function(function);
    scan.phase = Phase::Explanation;
    scan.function(function);
    if scan.inconsistent {
        scan.reasons
            .retain(|reason| !matches!(reason, Reason::Escape(_)));
        scan.reason(UnprovenReason::InconsistentTree);
    }
    scan.reasons
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Inventory,
    Explanation,
}

struct Scan<'tree> {
    root: BindingOwner,
    owner: BindingOwner,
    literal: &'tree TypedExpr,
    phase: Phase,
    declarations: HashSet<Key>,
    writes: HashSet<Key>,
    copies: HashSet<Key>,
    reasons: Vec<Reason>,
    inconsistent: bool,
}

impl Scan<'_> {
    fn reason(&mut self, reason: UnprovenReason) {
        if self.phase == Phase::Explanation {
            self.reasons.push(Reason::Unproven(reason));
        }
    }

    fn escape(&mut self, path: EscapePath) {
        if self.phase == Phase::Explanation {
            self.reasons.push(Reason::Escape(path));
        }
    }

    fn declare(&mut self, id: VariableId) {
        if self.phase == Phase::Inventory && !self.declarations.insert((self.owner, id)) {
            self.inconsistent = true;
        }
    }

    fn key(&self, expr: &TypedExpr) -> Option<Key> {
        match expr.kind {
            TypedExprKind::Identifier { id, .. } => Some((self.owner, id)),
            TypedExprKind::Captured { owner, id, .. } => Some((owner, id)),
            _ => None,
        }
    }

    fn is_copy(&self, expr: &TypedExpr) -> bool {
        self.key(expr).is_some_and(|key| self.copies.contains(&key))
    }

    fn bind(&mut self, id: VariableId, value: &TypedExpr) {
        self.declare(id);
        let key = (self.owner, id);
        if self.phase == Phase::Explanation
            && !self.writes.contains(&key)
            && (std::ptr::eq(value, self.literal) || self.is_copy(value))
        {
            self.copies.insert(key);
        }
    }

    fn function(&mut self, function: &TypedFunction) {
        for param in &function.params {
            self.declare(param.id);
            self.ty(param.ty);
        }
        for contract in &function.contracts {
            self.expr(&contract.value);
        }
        self.block(&function.body);
    }

    fn ty(&mut self, ty: Type) {
        if !matches!(
            ty,
            Type::Number
                | Type::Bool
                | Type::Void
                | Type::Array(
                    crate::ast::ArrayElementType::Number | crate::ast::ArrayElementType::Bool
                )
        ) {
            self.reason(UnprovenReason::UnsupportedType);
        }
    }

    fn closure(&mut self, closure: &TypedClosure) {
        self.reason(UnprovenReason::CallableDeclaration);
        if closure.owner != self.owner {
            self.inconsistent = true;
        }
        if !closure.captures.is_empty() || !closure.dependencies.is_empty() {
            self.reason(UnprovenReason::CaptureStorage);
        }
        let parent = self.owner;
        self.owner = BindingOwner::Closure(closure.id);
        let copies = self.copies.clone();
        for param in &closure.params {
            self.declare(param.id);
            self.ty(param.ty);
        }
        match &closure.body {
            TypedClosureBody::Expression(expr) => self.expr(expr),
            TypedClosureBody::Block(block) => self.value_block(block),
        }
        self.copies = copies;
        self.owner = parent;
    }

    fn block(&mut self, block: &TypedBlock) {
        let copies = self.copies.clone();
        self.statements(block);
        self.copies = copies;
    }

    fn statements(&mut self, block: &TypedBlock) {
        for statement in block {
            match &statement.kind {
                TypedStatementKind::Let { id, ty, value, .. } => {
                    self.ty(*ty);
                    self.expr(value);
                    if matches!(
                        value.kind,
                        TypedExprKind::Identifier { .. } | TypedExprKind::Captured { .. }
                    ) && matches!(ty, Type::Array(_) | Type::String | Type::Range)
                    {
                        self.reason(UnprovenReason::AliasCreation);
                    }
                    self.bind(*id, value);
                }
                TypedStatementKind::Assignment { target, value }
                | TypedStatementKind::CompoundAssignment { target, value, .. } => {
                    if let Some(key) = self.key(target) {
                        if self.phase == Phase::Inventory {
                            self.writes.insert(key);
                        }
                        if matches!(
                            target.ty.value_type(),
                            Some(Type::Array(_) | Type::String | Type::Range)
                        ) {
                            self.reason(UnprovenReason::HandleReassignment);
                        } else {
                            self.reason(UnprovenReason::OutsideSubset);
                        }
                    }
                    self.expr(target);
                    self.expr(value);
                }
                TypedStatementKind::Return(value) => {
                    if let Some(value) = value {
                        // Helper returns do not by themselves leave the allocating
                        // invocation. Explicit captured host arguments below can.
                        let publication =
                            (self.owner == self.root).then_some(EscapePath::ReturnedHandle);
                        self.expr_with_publication(value, publication);
                    }
                }
                TypedStatementKind::Expression(expr) => self.expr(expr),
                TypedStatementKind::Closure(closure) => self.closure(closure),
                TypedStatementKind::If { cond, then, else_ } => {
                    self.reason(UnprovenReason::ControlFlow);
                    self.expr(cond);
                    self.block(then);
                    if let Some(block) = else_ {
                        self.block(block);
                    }
                }
                TypedStatementKind::Match { scrutinee, arms } => {
                    self.reason(UnprovenReason::ControlFlow);
                    self.expr(scrutinee);
                    for arm in arms {
                        self.expr(&arm.body);
                    }
                }
                TypedStatementKind::While { cond, body, .. } => {
                    self.reason(UnprovenReason::ControlFlow);
                    self.expr(cond);
                    self.block(body);
                }
                TypedStatementKind::For {
                    variable,
                    iterable,
                    body,
                    ..
                } => {
                    self.reason(UnprovenReason::ControlFlow);
                    self.expr(iterable);
                    self.declare(*variable);
                    self.block(body);
                }
                TypedStatementKind::Loop { body, .. } => {
                    self.reason(UnprovenReason::ControlFlow);
                    self.block(body);
                }
                TypedStatementKind::BreakValue { value, .. } => {
                    self.reason(UnprovenReason::ControlFlow);
                    self.expr(value);
                }
                TypedStatementKind::Break { .. } | TypedStatementKind::Continue { .. } => {
                    self.reason(UnprovenReason::ControlFlow);
                }
            }
        }
    }

    fn value_block(&mut self, block: &TypedValueBlock) {
        self.published_block(block, None);
    }

    fn published_block(&mut self, block: &TypedValueBlock, publication: Option<EscapePath>) {
        let copies = self.copies.clone();
        self.statements(&block.statements);
        if let Some(tail) = &block.tail {
            self.expr_with_publication(tail, publication);
        }
        self.copies = copies;
    }

    fn expr(&mut self, expr: &TypedExpr) {
        self.expr_with_publication(expr, None);
    }

    fn expr_with_publication(&mut self, expr: &TypedExpr, publication: Option<EscapePath>) {
        if let Some(path) = publication
            && self.is_copy(expr)
        {
            self.escape(path);
        }
        match &expr.kind {
            TypedExprKind::Number(_) | TypedExprKind::Bool(_) => {}
            TypedExprKind::String(_) => self.reason(UnprovenReason::UnsupportedType),
            TypedExprKind::Identifier { .. } => {}
            TypedExprKind::Captured { .. } => self.reason(UnprovenReason::CaptureStorage),
            TypedExprKind::Unary { expr, .. } => self.expr(expr),
            TypedExprKind::Binary { left, op, right } => {
                if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::In) {
                    self.reason(UnprovenReason::ControlFlow);
                }
                if matches!(
                    left.ty.value_type(),
                    Some(Type::Array(_) | Type::String | Type::Range)
                ) {
                    self.reason(UnprovenReason::HandleUse);
                }
                self.expr(left);
                self.expr(right);
            }
            TypedExprKind::Array(elements, element) => {
                self.ty(Type::Array(*element));
                for element in elements {
                    self.expr(element);
                }
            }
            TypedExprKind::Reserve {
                receiver,
                additional,
            } => {
                self.reason(UnprovenReason::HandleUse);
                self.expr(receiver);
                self.expr(additional);
            }
            TypedExprKind::Push { receiver, item } => {
                self.reason(UnprovenReason::HandleUse);
                self.expr(receiver);
                self.expr(item);
            }
            TypedExprKind::Index { expr, index } => {
                self.expr(expr);
                self.expr(index);
            }
            TypedExprKind::Range {
                start, end, step, ..
            } => {
                self.reason(UnprovenReason::UnsupportedType);
                self.expr(start);
                self.expr(end);
                if let Some(step) = step {
                    self.expr(step);
                }
            }
            TypedExprKind::Call { func, args, .. } => {
                match func {
                    FunctionRef::Host(_) => {
                        self.reason(UnprovenReason::HostObservation);
                    }
                    FunctionRef::User(_) => self.reason(UnprovenReason::CallBoundary),
                    FunctionRef::Builtin(_) => {}
                }
                let publication = matches!(func, FunctionRef::Host(_))
                    .then_some(EscapePath::PotentialHostPublication);
                for arg in args {
                    self.expr_with_publication(arg, publication);
                }
            }
            TypedExprKind::LexicalCall { args, .. } => {
                self.reason(UnprovenReason::CallBoundary);
                for arg in args {
                    self.expr(arg);
                }
            }
            TypedExprKind::NormalizedCall(call) => {
                self.reason(UnprovenReason::CallBoundary);
                let copies = self.copies.clone();
                for binding in &call.bindings {
                    self.ty(binding.ty);
                    self.expr(&binding.value);
                    self.bind(binding.id, &binding.value);
                }
                if let Some(closure) = &call.immediate_closure {
                    self.closure(closure);
                }
                self.expr(&call.call);
                self.copies = copies;
            }
            TypedExprKind::Block(block) => {
                self.reason(UnprovenReason::ControlFlow);
                self.published_block(block, publication);
            }
            TypedExprKind::If { cond, then, else_ } => {
                self.reason(UnprovenReason::ControlFlow);
                self.expr(cond);
                self.published_block(then, publication);
                if let Some(block) = else_ {
                    self.published_block(block, publication);
                }
            }
            TypedExprKind::Match { scrutinee, arms } => {
                self.reason(UnprovenReason::ControlFlow);
                self.expr(scrutinee);
                for arm in arms {
                    self.expr_with_publication(&arm.body, publication);
                }
            }
            TypedExprKind::Loop { body, .. } => {
                self.reason(UnprovenReason::ControlFlow);
                self.block(body);
            }
        }
    }
}
