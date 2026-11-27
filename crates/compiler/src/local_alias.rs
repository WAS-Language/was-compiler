//! Private M10 proof for direct fixed numeric/boolean literals and local handle copies.
//! Input must come from source analysis. This is not a checked-tree validator or
//! a storage policy; temporary candidates may still trap. Reports describe one
//! callable at a time, with no callee traversal. A blocker can stop the proof
//! without implying publication, and effects cover only the modeled prefix.

use std::collections::{HashMap, HashSet};

use crate::{
    ArrayElementType, Type,
    ast::{BinaryOp, UnaryOp},
    typed_ast::{
        BindingOwner, BuiltinFunction, FunctionRef, TypedClosure, TypedClosureBody, TypedExpr,
        TypedExprKind, TypedFunction, TypedNormalizedCall, TypedProgram, TypedStatement,
        TypedStatementKind, VariableId,
    },
};

// The binding of a supported fresh fixed literal, never an arbitrary handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Object(VariableId);

#[derive(Debug, PartialEq)]
struct Facts<'tree> {
    callable: Callable<'tree>,
    aliases: Vec<(VariableId, Object)>,
    reachable_alias_count: usize,
    // Ok proves every reported origin temporary; aliases are structural, not executed facts.
    proof: Proof,
    // Ordered possible effects of the observed prefix, not an execution trace.
    effects: Vec<(Option<Object>, Effect)>,
    // Locals still holding their initializer result if the observed prefix completes.
    unchanged_scalars: Vec<VariableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectIdentity {
    Same,
    Distinct,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadStorage {
    SameAllocation, // May have an empty payload, so this does not assert byte overlap.
    Disjoint,
    Unknown,
}

impl Facts<'_> {
    // Relations hold when both bindings have completed in the same invocation.
    fn relations(&self, left: VariableId, right: VariableId) -> (ObjectIdentity, PayloadStorage) {
        match (self.reachable_origin(left), self.reachable_origin(right)) {
            (Some(a), Some(b)) if a == b => (ObjectIdentity::Same, PayloadStorage::SameAllocation),
            // Disjointness follows from separate fixed allocations, not distinct handles.
            (Some(_), Some(_)) => (ObjectIdentity::Distinct, PayloadStorage::Disjoint),
            _ => (ObjectIdentity::Unknown, PayloadStorage::Unknown),
        }
    }

    fn reachable_origin(&self, binding: VariableId) -> Option<Object> {
        self.proof.ok()?;
        self.aliases[..self.reachable_alias_count]
            .iter()
            .find_map(|&(id, origin)| (id == binding).then_some(origin))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    Allocate,
    MayTrap,
    WriteDescriptor,
    ReadLengthMetadata,
    ReadIndexMetadata,
    ReadElement,
    WriteElement,
    UnknownCall(CallKind),
    Publish(Publication),
    ReadCapture(BindingOwner, VariableId),
    WriteCapture(BindingOwner, VariableId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallKind {
    // Exported-memory observation, previously retained handles, and reentry are
    // possible even when the current arguments do not publish any handle.
    Host,
    User,
    Lexical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Publication {
    Return,
    Argument(CallKind),
    CaptureStore(BindingOwner, VariableId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reason {
    Publication(Object, Publication),
    UnknownCall(CallKind),
    CaptureAccess,
    ClosureDeclaration,
    Unsupported,
    HandleReassignment,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Callable<'tree> {
    Function(&'tree TypedFunction),
    Closure(&'tree TypedClosure),
}

fn analyze(program: &TypedProgram) -> Vec<Facts<'_>> {
    program
        .functions
        .iter()
        .map(|function| analyze_callable(Callable::Function(function)))
        .collect()
}

fn analyze_callable(callable: Callable<'_>) -> Facts<'_> {
    let (params, return_type) = match callable {
        Callable::Function(function) => (&function.params, function.return_type),
        Callable::Closure(closure) => (&closure.params, closure.return_type),
    };
    let mut analysis = Analysis::default();
    if params.iter().any(|p| !is_scalar(p.ty)) {
        analysis.block(Reason::Unsupported);
    }
    let structural = match callable {
        Callable::Function(function) if function.contracts.is_empty() => {
            analysis.check_statements(&function.body)
        }
        Callable::Function(_) => Err(Reason::Unsupported),
        Callable::Closure(closure) => match &closure.body {
            TypedClosureBody::Expression(expr) => analysis.check_return(expr),
            TypedClosureBody::Block(block) => {
                analysis.check_statements(&block.statements).and_then(|()| {
                    block
                        .tail
                        .as_ref()
                        .map_or(Ok(()), |tail| analysis.check_return(tail))
                })
            }
        },
    };
    let proof = analysis.first_reason.map_or_else(
        || {
            structural.and_then(|()| {
                if is_scalar(return_type) || return_type == Type::Void {
                    Ok(())
                } else {
                    Err(Reason::Unsupported)
                }
            })
        },
        Err,
    );
    Facts {
        callable,
        aliases: analysis.aliases,
        reachable_alias_count: analysis.reachable_alias_count,
        proof,
        effects: analysis.effects,
        unchanged_scalars: analysis.unchanged_scalars,
    }
}

type Proof<T = ()> = Result<T, Reason>;

#[derive(Default)]
struct Analysis {
    objects: HashMap<VariableId, Object>,
    aliases: Vec<(VariableId, Object)>,
    effects: Vec<(Option<Object>, Effect)>,
    reachable_alias_count: usize,
    after_return: bool,

    first_reason: Option<Reason>,
    captured: HashSet<VariableId>,
    unchanged_scalars: Vec<VariableId>,
}

impl Analysis {
    fn block(&mut self, reason: Reason) {
        self.first_reason.get_or_insert(reason);
    }

    fn check_statements(&mut self, body: &[TypedStatement]) -> Proof {
        // Proof eligibility is structural, even after a return. Only modeled
        // straight-line operations allow observation to continue past a blocker.
        for statement in body {
            self.check_statement(&statement.kind)?;
            self.after_return |= matches!(statement.kind, TypedStatementKind::Return(_));
        }
        Ok(())
    }

    fn check_statement(&mut self, statement: &TypedStatementKind) -> Proof {
        match statement {
            TypedStatementKind::Let { id, ty, value, .. } if is_supported_array_type(*ty) => {
                let object = if let TypedExprKind::Array(items, _) = &value.kind {
                    let object = Object(*id);
                    self.record(object, Effect::MayTrap);
                    self.record(object, Effect::Allocate);
                    self.record(object, Effect::WriteDescriptor);
                    for item in items {
                        self.check_store(Some(object), item)?;
                    }
                    object
                } else if let Some(object) = self.local_object(value) {
                    object
                } else {
                    return self.check_value(value);
                };
                self.objects.insert(*id, object);
                self.aliases.push((*id, object));
                if !self.after_return {
                    self.reachable_alias_count = self.aliases.len();
                }
                Ok(())
            }
            TypedStatementKind::Let { id, ty, value, .. } => {
                self.check_value(value)?;
                if is_scalar(*ty) && !self.after_return && !self.captured.contains(id) {
                    self.unchanged_scalars.push(*id);
                }
                Ok(())
            }
            TypedStatementKind::Expression(value) => self.check_value(value),
            TypedStatementKind::Return(Some(value)) => self.check_return(value),
            TypedStatementKind::Closure(closure) => {
                self.note_closure(closure);
                Ok(())
            }
            TypedStatementKind::Return(None) => Ok(()),
            TypedStatementKind::Assignment { target, .. }
                if matches!(target.kind, TypedExprKind::Identifier { .. })
                    && target.ty.value_type().is_some_and(is_supported_array_type) =>
            {
                Err(Reason::HandleReassignment)
            }
            TypedStatementKind::Assignment {
                target:
                    TypedExpr {
                        kind: TypedExprKind::Captured { owner, id, .. },
                        ..
                    },
                value,
            }
            | TypedStatementKind::CompoundAssignment {
                target:
                    TypedExpr {
                        kind: TypedExprKind::Captured { owner, id, .. },
                        ..
                    },
                value,
                ..
            } => {
                if matches!(statement, TypedStatementKind::CompoundAssignment { .. }) {
                    self.block(Reason::CaptureAccess);
                    self.record(None, Effect::ReadCapture(*owner, *id));
                }
                self.check_argument(value)?;
                self.record(None, Effect::WriteCapture(*owner, *id));
                if let Some(object) = self.local_object(value) {
                    self.publish(object, Publication::CaptureStore(*owner, *id));
                }
                self.block(Reason::CaptureAccess);
                Ok(())
            }
            TypedStatementKind::Assignment { target, value } => {
                let object = self.check_index_access(target)?;
                self.check_store(object, value)
            }
            TypedStatementKind::CompoundAssignment {
                target,
                value,
                op:
                    BinaryOp::Add
                    | BinaryOp::Subtract
                    | BinaryOp::Multiply
                    | BinaryOp::Divide
                    | BinaryOp::Remainder,
            } => {
                let object = self.check_index_access(target)?;
                self.record(object, Effect::ReadElement);
                self.check_store(object, value)
            }
            _ => Err(Reason::Unsupported),
        }
    }

    fn check_return(&mut self, value: &TypedExpr) -> Proof {
        if let Some(object) = self.local_object(value) {
            self.publish(object, Publication::Return);
            Ok(())
        } else {
            self.check_value(value)
        }
    }

    fn check_store(&mut self, object: Option<Object>, value: &TypedExpr) -> Proof {
        self.check_value(value)?;
        if object.is_none() {
            // A captured array can be resizable. A pending write may need to
            // revalidate the saved index against current storage after the RHS.
            self.record(None, Effect::MayTrap);
        }
        // Checked boolean stores consume the scalar before writing the byte.
        if value.ty == Type::Bool {
            self.record(object, Effect::MayTrap);
        }
        self.record(object, Effect::WriteElement);
        Ok(())
    }

    fn record(&mut self, object: impl Into<Option<Object>>, effect: Effect) {
        if !self.after_return {
            self.effects.push((object.into(), effect));
        }
    }

    fn local_object(&self, expr: &TypedExpr) -> Option<Object> {
        if let TypedExprKind::Identifier { id, .. } = expr.kind {
            return self.objects.get(&id).copied();
        }
        None
    }

    fn publish(&mut self, object: Object, path: Publication) {
        if !self.after_return {
            self.block(Reason::Publication(object, path));
            self.record(object, Effect::Publish(path));
        }
    }

    fn check_call(&mut self, kind: CallKind, args: &[TypedExpr]) -> Proof {
        let mut handles = Vec::new();
        for arg in args {
            self.check_argument(arg)?;
            if let Some(object) = self.local_object(arg) {
                handles.push(object);
            }
        }
        for object in handles {
            self.publish(object, Publication::Argument(kind));
        }
        self.block(Reason::UnknownCall(kind));
        self.record(None, Effect::UnknownCall(kind));
        self.record(None, Effect::MayTrap);
        if !self.after_return {
            self.objects.retain(|id, _| !self.captured.contains(id));
        }
        Ok(())
    }

    fn note_closure(&mut self, closure: &TypedClosure) {
        self.block(Reason::ClosureDeclaration);
        for capture in &closure.captures {
            if capture.owner == closure.owner {
                self.captured.insert(capture.variable);
                self.unchanged_scalars.retain(|id| *id != capture.variable);
            }
        }
    }

    fn check_normalized_call(&mut self, call: &TypedNormalizedCall) -> Proof {
        // The synthetic bindings are evaluated in source/default order, not
        // canonical formal order. Keep each handle captured at that point.
        for binding in &call.bindings {
            self.check_argument(&binding.value)?;
            if let Some(object) = self.local_object(&binding.value) {
                self.objects.insert(binding.id, object);
            }
        }
        if let Some(closure) = &call.immediate_closure {
            self.note_closure(closure);
        }
        let proof = self.check_value(&call.call);
        for binding in &call.bindings {
            self.objects.remove(&binding.id);
        }
        proof
    }

    fn check_argument(&mut self, arg: &TypedExpr) -> Proof {
        match arg.kind {
            TypedExprKind::Identifier { .. } => Ok(()),
            _ => self.check_value(arg),
        }
    }

    fn check_index_access(&mut self, place: &TypedExpr) -> Proof<Option<Object>> {
        let TypedExprKind::Index { expr, index } = &place.kind else {
            return Err(Reason::Unsupported);
        };
        let object = self.check_array_base(expr)?;
        self.check_value(index)?;
        self.record(object, Effect::MayTrap);
        self.record(object, Effect::ReadIndexMetadata);
        Ok(object)
    }

    fn check_array_base(&mut self, expr: &TypedExpr) -> Proof<Option<Object>> {
        if let TypedExprKind::Captured { .. } = expr.kind {
            self.check_value(expr)?;
            return Ok(None);
        }
        self.local_object(expr).map(Some).ok_or(Reason::Unsupported)
    }

    fn check_value(&mut self, expr: &TypedExpr) -> Proof {
        match &expr.kind {
            TypedExprKind::Call {
                func: FunctionRef::Host(_),
                args,
                ..
            } => {
                return self.check_call(CallKind::Host, args);
            }
            TypedExprKind::Call {
                func: FunctionRef::User(_),
                args,
                ..
            } => {
                return self.check_call(CallKind::User, args);
            }
            TypedExprKind::LexicalCall { args, .. } => {
                return self.check_call(CallKind::Lexical, args);
            }
            TypedExprKind::NormalizedCall(call) => return self.check_normalized_call(call),
            TypedExprKind::Captured { owner, id, .. } => {
                self.block(Reason::CaptureAccess);
                self.record(None, Effect::ReadCapture(*owner, *id));
                return Ok(());
            }
            _ => {}
        }
        if !expr.ty.value_type().is_some_and(is_scalar) {
            return Err(Reason::Unsupported);
        }

        match &expr.kind {
            TypedExprKind::Number(_)
            | TypedExprKind::Bool(_)
            | TypedExprKind::Identifier { .. } => Ok(()),
            TypedExprKind::Unary {
                op: UnaryOp::Not | UnaryOp::Negate,
                expr,
            } => self.check_value(expr),
            TypedExprKind::Binary {
                left,
                op:
                    BinaryOp::Add
                    | BinaryOp::Subtract
                    | BinaryOp::Multiply
                    | BinaryOp::Divide
                    | BinaryOp::Remainder
                    | BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::LessThan
                    | BinaryOp::GreaterThan
                    | BinaryOp::LessThanOrEqual
                    | BinaryOp::GreaterThanOrEqual,
                right,
            } if left.ty.value_type().is_some_and(is_scalar)
                && right.ty.value_type().is_some_and(is_scalar) =>
            {
                self.check_value(left)?;
                self.check_value(right)
            }
            TypedExprKind::Index { .. } => {
                let object = self.check_index_access(expr)?;
                self.record(object, Effect::ReadElement);
                if expr.ty == Type::Bool {
                    self.record(object, Effect::MayTrap);
                }
                Ok(())
            }
            TypedExprKind::Call {
                func: FunctionRef::Builtin(BuiltinFunction::Len),
                args,
                ..
            } => {
                let [array] = args.as_slice() else {
                    return Err(Reason::Unsupported);
                };
                let object = self.check_array_base(array)?;
                self.record(object, Effect::MayTrap);
                self.record(object, Effect::ReadLengthMetadata);
                Ok(())
            }
            _ => Err(Reason::Unsupported),
        }
    }
}

// This is type eligibility only; fixed storage must come from literal provenance.
fn is_supported_array_type(ty: Type) -> bool {
    matches!(
        ty,
        Type::Array(ArrayElementType::Number | ArrayElementType::Bool)
    )
}

fn is_scalar(ty: Type) -> bool {
    matches!(ty, Type::Number | Type::Bool)
}

const PROBE: &str = include_str!("../../../tests/fixtures/m10-local-alias.was");

#[test]
fn opposite_names_share_one_temporary_candidate() {
    use crate::{Compiler, typed_ast::TypedStatementKind};

    let program = Compiler::new().analyze(PROBE).unwrap();
    let facts = analyze(&program);
    let [facts] = facts.as_slice() else {
        panic!("expected one function's facts, got {facts:?}");
    };
    let function = &program.functions[0];
    let TypedStatementKind::Let { id: a, .. } = function.body[0].kind else {
        panic!("expected literal binding");
    };
    let TypedStatementKind::Let { id: b, .. } = function.body[1].kind else {
        panic!("expected alias binding");
    };

    assert!(matches!(facts.callable, Callable::Function(found) if std::ptr::eq(found, function)));
    assert_eq!(facts.proof, Ok(()));
    assert_eq!(facts.aliases, [(a, Object(a)), (b, Object(a))]);
    let accesses: Vec<_> = facts
        .effects
        .iter()
        .copied()
        .filter(|(_, effect)| matches!(effect, Effect::ReadElement | Effect::WriteElement))
        .collect();
    assert_eq!(
        accesses,
        [Effect::WriteElement; 4]
            .into_iter()
            .chain([Effect::ReadElement; 2])
            .map(|effect| (Some(Object(a)), effect))
            .collect::<Vec<_>>()
    );
}

#[test]
fn checked_compound_reads_old_element_before_rhs_and_may_write_after() {
    use crate::Compiler;
    use Effect::*;

    let program = Compiler::new()
        .analyze(
            "fn main(i: number) -> number { let a = [1]; let b = a; b[i] += len(a); return a[i]; }",
        )
        .unwrap();
    let reports = analyze(&program);
    let facts = &reports[0];
    assert_eq!(facts.proof, Ok(()));
    let object = facts.aliases[0].1;
    assert_eq!(
        facts.effects,
        [
            MayTrap,
            Allocate,
            WriteDescriptor,
            WriteElement,
            MayTrap,
            ReadIndexMetadata,
            ReadElement,
            MayTrap,
            ReadLengthMetadata,
            WriteElement,
            MayTrap,
            ReadIndexMetadata,
            ReadElement,
        ]
        .map(|effect| (Some(object), effect))
    );

    use wasmparser::Operator;

    let bytes = crate::codegen::compile(&program).unwrap();
    let accesses: Vec<_> = validated_main_body(&bytes)
        .get_operators_reader()
        .unwrap()
        .into_iter()
        .map(Result::unwrap)
        .filter_map(|op| match op {
            Operator::F64Load { .. } => Some("load"),
            Operator::F64Store { .. } => Some("store"),
            Operator::Call { function_index: 5 } => Some("len"),
            _ => None,
        })
        .collect();
    assert_eq!(accesses, ["store", "load", "len", "store", "load"]);
}

// The probe modules define main first, before their runtime helpers.
fn validated_main_body(bytes: &[u8]) -> wasmparser::FunctionBody<'_> {
    use wasmparser::{Parser, Payload, Validator};

    Validator::new().validate_all(bytes).unwrap();
    Parser::new(0)
        .parse_all(bytes)
        .find_map(|payload| match payload.unwrap() {
            Payload::CodeSectionEntry(body) => Some(body),
            _ => None,
        })
        .unwrap()
}

const BOOL_CHAIN: &str = "fn main(i: number) -> bool {
    let a = [false]; let b = a; let c = b;
    c[i] = true; return b[len(a) - 1];
}";

#[test]
fn boolean_copy_chain_attributes_checked_accesses_to_the_literal() {
    use Effect::*;
    use wasmparser::Operator;

    let program = crate::Compiler::new().analyze(BOOL_CHAIN).unwrap();
    let reports = analyze(&program);
    let facts = &reports[0];
    assert_eq!(facts.proof, Ok(()));
    assert_eq!(facts.aliases.len(), 3);
    let object = facts.aliases[0].1;
    assert!(facts.aliases.iter().all(|&(_, alias)| alias == object));
    assert_eq!(
        facts.effects,
        [
            MayTrap,
            Allocate,
            WriteDescriptor,
            MayTrap,
            WriteElement,
            MayTrap,
            ReadIndexMetadata,
            MayTrap,
            WriteElement,
            MayTrap,
            ReadLengthMetadata,
            MayTrap,
            ReadIndexMetadata,
            ReadElement,
            MayTrap,
        ]
        .map(|effect| (Some(object), effect))
    );

    let bytes = crate::codegen::compile(&program).unwrap();
    let accesses: Vec<_> = validated_main_body(&bytes)
        .get_operators_reader()
        .unwrap()
        .into_iter()
        .map(Result::unwrap)
        .filter_map(|op| match op {
            Operator::I32Store8 { memarg } => Some(("store", memarg.offset)),
            Operator::I32Load8U { memarg } => Some(("load", memarg.offset)),
            _ => None,
        })
        .collect();
    assert_eq!(accesses, [("store", 8), ("store", 0), ("load", 0)]);
}

#[test]
fn fresh_literals_are_distinct_and_disjoint_but_copies_share_storage() {
    use ObjectIdentity::*;
    use PayloadStorage::{Disjoint, SameAllocation};

    for (literal, result) in [("[0]", "number"), ("[false]", "bool")] {
        let program = crate::Compiler::new()
            .analyze(&format!(
                "fn main() -> {result} {{ let a = {literal}; let b = a; let c = b; let d = {literal}; return c[0]; let dead = {literal}; }}"
            ))
            .unwrap();
        let reports = analyze(&program);
        let facts = &reports[0];
        assert_eq!(facts.proof, Ok(()));

        let [a, b, c, d, dead] = program.functions[0]
            .body
            .iter()
            .filter_map(|stmt| match stmt.kind {
                TypedStatementKind::Let { id, .. } => Some(id),
                _ => None,
            })
            .collect::<Vec<_>>()[..]
        else {
            panic!("expected five bindings");
        };
        assert_eq!(facts.relations(a, c), (Same, SameAllocation));
        assert_eq!(facts.relations(b, c), (Same, SameAllocation));
        assert_eq!(facts.relations(c, d), (Distinct, Disjoint));
        assert_eq!(facts.relations(d, a), (Distinct, Disjoint));
        assert_eq!(facts.relations(a, dead), (Unknown, PayloadStorage::Unknown));
    }
}

const RAW_OVERLAP: &str =
    "fn main(a: bool[], b: bool[]) -> bool { let copy = a; copy[1] = true; return b[0]; }";

#[test]
fn distinct_raw_parameters_have_unknown_object_and_payload_relations() {
    let program = crate::Compiler::new().analyze(RAW_OVERLAP).unwrap();
    let reports = analyze(&program);
    let facts = &reports[0];
    let [a, b] = program.functions[0].params.as_slice() else {
        panic!("expected two array parameters");
    };
    assert_eq!(facts.proof, Err(Reason::Unsupported));
    assert_eq!(
        facts.relations(a.id, b.id),
        (ObjectIdentity::Unknown, PayloadStorage::Unknown)
    );
}

#[test]
fn handle_rebinding_blocks_proof_even_after_return() {
    use crate::Compiler;

    for (literal, result) in [("[1]", "number"), ("[true]", "bool")] {
        for body in ["b = a; return b[0];", "return b[0]; b = a;"] {
            let source =
                format!("fn main() -> {result} {{ let a = {literal}; let b = a; {body} }}");
            let program = Compiler::new().analyze(&source).unwrap();
            let reports = analyze(&program);
            let facts = &reports[0];
            assert_eq!(facts.proof, Err(Reason::HandleReassignment));
            let [(a, _), (b, _)] = facts.aliases.as_slice() else {
                panic!("expected two recorded aliases");
            };
            assert_eq!(
                facts.relations(*a, *b),
                (ObjectIdentity::Unknown, PayloadStorage::Unknown)
            );
        }
    }
}

#[test]
fn handles_without_supported_local_provenance_cannot_earn_a_proof() {
    use crate::Compiler;

    for source in [
        "fn main(a: number[]) -> number { let b = a; return b[0]; }",
        "fn main() -> number { let a = [1]; let b = resizable(a); return b[0]; }",
        "fn main() -> bool { let a = [false]; let b = resizable(a); return b[0]; }",
        "fn main() -> number { let a = [\"text\"]; let b = a; return len(b); }",
    ] {
        let program = Compiler::new().analyze(source).unwrap();
        assert_eq!(
            analyze(&program)[0].proof,
            Err(Reason::Unsupported),
            "{source}"
        );
    }
}

#[test]
fn dead_suffix_can_block_proof_but_adds_no_possible_effects() {
    use crate::Compiler;

    let prefix = "let a = [1]; let b = a; return b[0];";
    let suffix = "a[0] = 9; loop {}";
    let compiler = Compiler::new();
    let baseline = compiler
        .analyze(&format!("fn main() -> number {{ {prefix} }}"))
        .unwrap();
    let program = compiler
        .analyze(&format!("fn main() -> number {{ {prefix} {suffix} }}"))
        .unwrap();
    let reports = analyze(&program);
    assert_eq!(reports[0].proof, Err(Reason::Unsupported));
    assert_eq!(reports[0].effects, analyze(&baseline)[0].effects);
}

fn callback_compiler() -> crate::Compiler {
    let mut compiler = crate::Compiler::new();
    compiler
        .register_host_function("observe", "env", "observe", vec![], Type::Void)
        .unwrap();
    compiler
        .register_host_function(
            "retain",
            "env",
            "retain",
            vec![Type::Array(ArrayElementType::Number)],
            Type::Void,
        )
        .unwrap();
    compiler
}

#[test]
fn publication_and_handle_free_host_observation_have_distinct_reasons() {
    let compiler = callback_compiler();
    for (result, suffix, path) in [
        ("number[]", "return b;", Some(Publication::Return)),
        (
            "number",
            "retain(b); return b[0];",
            Some(Publication::Argument(CallKind::Host)),
        ),
        ("number", "observe(); return b[0];", None),
    ] {
        let program = compiler
            .analyze(&format!(
                "fn main() -> {result} {{ let a = [1]; let b = a; {suffix} }}"
            ))
            .unwrap();
        let reports = analyze(&program);
        let facts = &reports[0];
        let object = facts.aliases[0].1;
        let expected = path.map_or(Reason::UnknownCall(CallKind::Host), |path| {
            Reason::Publication(object, path)
        });
        assert_eq!(facts.proof, Err(expected));
        let publications: Vec<_> = facts
            .effects
            .iter()
            .filter_map(|&(target, effect)| {
                if let Effect::Publish(path) = effect {
                    Some((target, path))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            publications,
            path.map(|path| (Some(object), path))
                .into_iter()
                .collect::<Vec<_>>()
        );
        assert_eq!(reports, analyze(&program));
    }
}

#[test]
fn known_calls_use_conservative_effects_after_current_body_arguments() {
    use CallKind::*;
    use Effect::*;

    let program = callback_compiler()
        .analyze(
            r#"
        fn main(n: number) -> number {
            let a = [1];
            return main(a[n]);
        }
        fn lexical() -> number {
            let a = [1]; let b = a;
            let f = (x: number) => b[x];
            return f(len(b));
        }
        fn named() -> number {
            let a = [1]; let b = a;
            return sink(size: len(b), array: b);
        }
        fn sink(array: number[], size: number) -> number { return size; }
    "#,
        )
        .unwrap();
    let reports = analyze(&program);
    let recursive = &reports[0];
    let object = Some(recursive.aliases[0].1);
    assert_eq!(recursive.proof, Err(Reason::UnknownCall(User)));
    assert!(recursive.effects.ends_with(&[
        (object, ReadIndexMetadata),
        (object, ReadElement),
        (None, UnknownCall(User)),
        (None, MayTrap),
    ]));
    let lexical = &reports[1];
    let object = Some(lexical.aliases[0].1);
    assert_eq!(lexical.proof, Err(Reason::ClosureDeclaration));
    assert!(lexical.effects.ends_with(&[
        (object, ReadLengthMetadata),
        (None, UnknownCall(Lexical)),
        (None, MayTrap),
    ]));
    let named = &reports[2];
    let object = named.aliases[0].1;
    let path = Publication::Argument(User);
    assert_eq!(named.proof, Err(Reason::Publication(object, path)));
    assert!(named.effects.ends_with(&[
        (Some(object), ReadLengthMetadata),
        (Some(object), Publish(path)),
        (None, UnknownCall(User)),
        (None, MayTrap),
    ]));
}

#[test]
fn capture_accesses_belong_to_the_borrowed_body_not_the_caller() {
    let program = callback_compiler()
        .analyze(
            r#"
        fn main() -> number {
            let count = 2; let stable = 17; let a = [3];
            let f = () => { count += a[0]; a[0] = count; count };
            observe();
            return stable;
        }
    "#,
        )
        .unwrap();
    let function = &program.functions[0];
    let TypedStatementKind::Let { id: stable, .. } = function.body[1].kind else {
        panic!("expected stable local")
    };
    let TypedStatementKind::Closure(closure) = &function.body[3].kind else {
        panic!("expected closure")
    };
    let reports = analyze(&program);
    let outer = &reports[0];
    assert_eq!(outer.proof, Err(Reason::ClosureDeclaration));
    assert_eq!(outer.unchanged_scalars, [stable]);
    assert!(outer.effects.ends_with(&[
        (None, Effect::UnknownCall(CallKind::Host)),
        (None, Effect::MayTrap)
    ]));
    assert!(
        !outer.effects.iter().any(|(_, effect)| matches!(
            effect,
            Effect::ReadCapture(..) | Effect::WriteCapture(..)
        ))
    );

    let inner = analyze_callable(Callable::Closure(closure));
    let [count, array] = closure.captures.as_slice() else {
        panic!("expected two captures")
    };
    let read = |capture: &crate::typed_ast::TypedCapture| {
        (None, Effect::ReadCapture(capture.owner, capture.variable))
    };
    assert_eq!(inner.proof, Err(Reason::CaptureAccess));
    assert_eq!(
        inner.effects,
        [
            read(count),
            read(array),
            (None, Effect::MayTrap),
            (None, Effect::ReadIndexMetadata),
            (None, Effect::ReadElement),
            (None, Effect::WriteCapture(count.owner, count.variable)),
            read(array),
            (None, Effect::MayTrap),
            (None, Effect::ReadIndexMetadata),
            read(count),
            (None, Effect::MayTrap),
            (None, Effect::WriteElement),
            read(count),
        ]
    );
}

#[test]
fn storing_an_alias_in_an_enclosing_capture_is_publication() {
    let program = callback_compiler()
        .analyze(
            r#"
        fn main() -> number {
            let outer = [0];
            let publish = () => { let a = [1]; let b = a; outer = b; };
            publish();
            return outer[0];
        }
    "#,
        )
        .unwrap();
    let TypedStatementKind::Closure(closure) = &program.functions[0].body[1].kind else {
        panic!("expected closure");
    };
    let facts = analyze_callable(Callable::Closure(closure));
    let object = facts.aliases[0].1;
    let capture = &closure.captures[0];
    let path = Publication::CaptureStore(capture.owner, capture.variable);
    assert_eq!(facts.proof, Err(Reason::Publication(object, path)));
    assert!(facts.effects.ends_with(&[
        (None, Effect::WriteCapture(capture.owner, capture.variable)),
        (Some(object), Effect::Publish(path)),
    ]));
}

#[test]
fn unreachable_calls_and_control_flow_block_without_observed_effects() {
    let compiler = callback_compiler();
    let prefix = "let a = [1]; let b = a; return b[0];";
    let baseline = compiler
        .analyze(&format!("fn main() -> number {{ {prefix} }}"))
        .unwrap();
    let baseline = analyze(&baseline);
    for (suffix, reason) in [
        ("retain(b);", Reason::UnknownCall(CallKind::Host)),
        ("observe();", Reason::UnknownCall(CallKind::Host)),
        ("if true { retain(b); }", Reason::Unsupported),
        ("while true { observe(); }", Reason::Unsupported),
    ] {
        let program = compiler
            .analyze(&format!("fn main() -> number {{ {prefix} {suffix} }}"))
            .unwrap();
        let reports = analyze(&program);
        assert_eq!(reports[0].proof, Err(reason));
        assert_eq!(reports[0].effects, baseline[0].effects);
    }
}

const LEVELS: [crate::OptimizationLevel; 4] = [
    crate::OptimizationLevel::None,
    crate::OptimizationLevel::Basic,
    crate::OptimizationLevel::Standard,
    crate::OptimizationLevel::Aggressive,
];

#[test]
fn private_analysis_is_deterministic_and_preserves_ordinary_bytes() {
    let mut compiler = callback_compiler();
    for source in [
        PROBE,
        BOOL_CHAIN,
        RAW_OVERLAP,
        "fn main() -> number[] { let a = [1]; let b = a; retain(b); return b; }",
        "fn main() -> number { let a = [1]; let f = () => a[0]; observe(); return f(); }",
    ] {
        let program = compiler.analyze(source).unwrap();
        let before = LEVELS.map(|level| {
            compiler.set_optimization_level(level);
            compiler.compile(&program).unwrap()
        });
        assert_eq!(analyze(&program), analyze(&program));

        for (level, bytes) in LEVELS.into_iter().zip(before) {
            compiler.set_optimization_level(level);
            assert_eq!(compiler.compile(&program).unwrap(), bytes, "{level:?}");
        }
    }
}

#[test]
#[ignore = "requires Node.js"]
fn ordinary_wasm_reads_alias_visible_writes() {
    run_wasm(PROBE, "assert.equal(e.main(), 16);");
    run_wasm(BOOL_CHAIN, "assert.equal(e.main(0), 1);");
}

#[test]
#[ignore = "requires Node.js"]
fn distinct_revision_one_raw_descriptors_can_share_payload_bytes() {
    run_wasm(
        RAW_OVERLAP,
        r#"
        const payload = e.__alloc(8);
        const a = e.__alloc(8), b = e.__alloc(8), disjoint = e.__alloc(8);
        const view = new DataView(e.memory.buffer);
        function descriptor(handle, data) {
            view.setUint32(handle, data, true);
            view.setUint32(handle + 4, 2, true);
        }
        assert.notEqual(a, b);
        descriptor(a, payload);
        descriptor(b, payload + 1); // Only a[1] and b[0] share a byte.
        descriptor(disjoint, payload + 2);
        assert.equal(e.main(a, disjoint), 0);
        assert.equal(view.getUint8(payload + 1), 1);
        view.setUint8(payload + 1, 0);
        assert.equal(e.main(a, b), 1);
        assert.deepEqual([...new Uint8Array(e.memory.buffer, payload, 3)], [0, 1, 0]);
    "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn published_writes_survive_traps_and_a_handle_free_callback_can_resume_outer_call() {
    run_wasm_with(
        callback_compiler(),
        r#"
        fn fail(value: number) -> void {
            let a = [0]; let b = a;
            retain(b);
            b[0] = value;
            a[1] = 99;
        }
        fn outer() -> number {
            let a = [1]; let b = a;
            observe();
            b[0] += 5;
            return a[0];
        }
        fn read(a: number[]) -> number { return a[0]; }
        "#,
        r#"
        const retained = [];
        let observations = 0;
        const e = new WebAssembly.Instance(moduleBytes, { env: {
            retain(handle) { retained.push(handle); },
            observe(...args) {
                assert.deepEqual(args, []);
                assert.equal(++observations, 1);
                assert.equal(e.read(retained[0]), 7);
                assert.throws(() => e.fail(9), WebAssembly.RuntimeError);
                assert.equal(retained.length, 2);
                assert.equal(e.read(retained[1]), 9);
            },
        }}).exports;
        assert.throws(() => e.fail(7), WebAssembly.RuntimeError);
        assert.equal(retained.length, 1);
        assert.equal(e.read(retained[0]), 7);
        assert.equal(e.outer(), 6);
        assert.equal(observations, 1);
        assert.notEqual(retained[0], retained[1]);
        assert.deepEqual(retained.map(handle => e.read(handle)), [7, 9]);
        "#,
    );
}

fn run_wasm(source: &str, script: &str) {
    run_wasm_with(
        crate::Compiler::new(),
        source,
        &format!("const e = new WebAssembly.Instance(moduleBytes).exports;\n{script}"),
    );
}

fn run_wasm_with(mut compiler: crate::Compiler, source: &str, script: &str) {
    use std::{process::Command, time::Duration};

    let bootstrap = r#"
        const assert = require('node:assert/strict');
        const bytes = require('node:fs').readFileSync(0);
        const moduleBytes = new WebAssembly.Module(bytes);
    "#;
    let script = format!("{bootstrap}{script}");
    let program = compiler.analyze(source).unwrap();
    for level in LEVELS {
        compiler.set_optimization_level(level);
        let bytes = compiler.compile(&program).unwrap();
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        let output = crate::test_process::output(
            Command::new("node").arg("-e").arg(&script),
            Some(&bytes),
            Duration::from_secs(15),
        );
        assert!(
            output.status.success(),
            "{level:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
