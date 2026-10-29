//! Exact region-local constraints. Variables and relationships never cross the
//! concrete callable boundary; callers cannot refine a declaration's signature.
use super::*;
use crate::ArrayElementType;
use crate::ast::{MatchPatternKind, PipelineStageTarget};
use std::collections::HashSet;

pub(super) struct Parameter {
    pub(super) identity: usize,
    pub(super) span: Span,
    pub(super) ty: Type,
}

pub(super) fn has_owned(expression: &Expr) -> bool {
    first_owned_span(expression).is_some()
}

pub(super) fn owned_parameter_count(expression: &Expr) -> usize {
    let mut count = 0;
    visit_owned(expression, &mut |expr| {
        if matches!(expr, Expr::Placeholder { .. }) {
            count += 1;
        }
    });
    count
}

pub(super) fn first_owned_span(expression: &Expr) -> Option<Span> {
    let mut found = None;
    visit_owned(expression, &mut |expr| {
        if let Expr::Placeholder { span } = expr {
            found = found.or(Some(*span));
        }
    });
    found
}

// A bare placeholder initializer connects an ordinary local to this region.
// Compound placeholder initializers and explicit lambdas remain boundaries.
fn visit_owned(expression: &Expr, visit: &mut impl FnMut(&Expr)) {
    fn statements(block: &Block, visit: &mut impl FnMut(&Expr)) {
        for statement in block {
            match &statement.kind {
                StatementKind::Let { value, .. } => {
                    if matches!(value, Expr::Placeholder { .. }) {
                        visit_owned(value, visit);
                    }
                }
                StatementKind::Assignment { target, value }
                | StatementKind::CompoundAssignment { target, value, .. } => {
                    visit_owned(target, visit);
                    visit_owned(value, visit);
                }
                StatementKind::If { cond, then, else_ } => {
                    visit_owned(cond, visit);
                    statements(then, visit);
                    if let Some(block) = else_ {
                        statements(block, visit);
                    }
                }
                StatementKind::Match { scrutinee, arms } => {
                    visit_owned(scrutinee, visit);
                    for arm in arms {
                        visit_owned(&arm.body, visit);
                    }
                }
                StatementKind::While { cond, body } => {
                    visit_owned(cond, visit);
                    statements(body, visit);
                }
                StatementKind::For { iterable, body, .. } => {
                    visit_owned(iterable, visit);
                    statements(body, visit);
                }
                StatementKind::Loop { body } => statements(body, visit),
                StatementKind::Return(Some(expr))
                | StatementKind::Expression(expr)
                | StatementKind::BreakValue(expr) => visit_owned(expr, visit),
                StatementKind::Return(None) | StatementKind::Break | StatementKind::Continue => {}
            }
        }
    }
    fn block(block: &ValueBlock, visit: &mut impl FnMut(&Expr)) {
        statements(&block.statements, visit);
        if let Some(tail) = &block.tail {
            visit_owned(tail, visit);
        }
    }
    visit(expression);
    match expression {
        Expr::Number(_)
        | Expr::String(_)
        | Expr::Bool(_)
        | Expr::Identifier(_)
        | Expr::Placeholder { .. }
        | Expr::Lambda(_) => {}
        Expr::Unary { expr, .. } => visit_owned(expr, visit),
        Expr::Binary { left, right, .. }
        | Expr::Index {
            expr: left,
            index: right,
        } => {
            visit_owned(left, visit);
            visit_owned(right, visit);
        }
        Expr::Range {
            start, end, step, ..
        } => {
            visit_owned(start, visit);
            visit_owned(end, visit);
            if let Some(step) = step {
                visit_owned(step, visit);
            }
        }
        Expr::Pipeline { input, stages, .. } => {
            visit_owned(input, visit);
            for stage in stages {
                for arg in &stage.args {
                    visit_owned(&arg.value, visit);
                }
            }
        }
        Expr::Reserve { receiver, args, .. } | Expr::Push { receiver, args, .. } => {
            visit_owned(receiver, visit);
            for arg in args {
                visit_owned(&arg.value, visit);
            }
        }
        Expr::Call { args, .. } => {
            for arg in args {
                visit_owned(&arg.value, visit);
            }
        }
        Expr::Loop { body } => statements(body, visit),
        Expr::Block(value) => block(value, visit),
        Expr::If { cond, then, else_ } => {
            visit_owned(cond, visit);
            block(then, visit);
            if let Some(value) = else_ {
                block(value, visit);
            }
        }
        Expr::Match { scrutinee, arms } => {
            visit_owned(scrutinee, visit);
            for arm in arms {
                visit_owned(&arm.body, visit);
            }
        }
        Expr::Array(values) => {
            for value in values {
                visit_owned(value, visit);
            }
        }
    }
}

struct Candidate {
    identity: usize,
    span: Span,
    variable: Variable,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Variable(usize);

#[derive(Clone, Copy)]
enum Relation {
    Equal(Variable),
    SameArray(Variable),
    Element(Variable),
    Array(Variable),
    Yield(Variable),
    Iterable(Variable),
}

#[derive(Clone, Copy)]
enum Relationship {
    SameType,
    SameArray,
    ArrayElement,
}

#[derive(Default)]
struct Constraints {
    types: Vec<Option<Type>>,
    origins: Vec<Option<Site>>,
    edges: Vec<Vec<(Relation, Site)>>,
    conflict_site: Option<Site>,
}

#[derive(Clone, Copy)]
struct Site {
    order: usize,
    span: Span,
}

impl Site {
    fn later(self, other: Option<Self>) -> Self {
        other
            .filter(|other| other.order > self.order)
            .unwrap_or(self)
    }
}

impl Constraints {
    fn variable(&mut self) -> Variable {
        let variable = Variable(self.types.len());
        self.types.push(None);
        self.origins.push(None);
        self.edges.push(Vec::new());
        variable
    }

    fn concrete(&mut self, ty: Type) -> Variable {
        let variable = self.variable();
        self.types[variable.0] = Some(ty);
        variable
    }

    fn assign(&mut self, variable: Variable, ty: Type, site: Site) -> Result<(), (Type, Type)> {
        let mut pending = vec![(variable, ty, site)];
        let mut conflict = None;
        while let Some((variable, ty, site)) = pending.pop() {
            if let Some(expected) = self.types[variable.0] {
                if expected != ty {
                    let site = site.later(self.origins[variable.0]);
                    if conflict.is_none_or(|(_, previous): (_, Site)| site.order < previous.order) {
                        conflict = Some(((expected, ty), site));
                    }
                }
                continue;
            }
            self.types[variable.0] = Some(ty);
            self.origins[variable.0] = Some(site);
            for (relation, edge_site) in &self.edges[variable.0] {
                let site = site.later(Some(*edge_site));
                match Self::related(*relation, ty) {
                    Ok(Some((next, ty))) => pending.push((next, ty, site)),
                    Ok(None) => {}
                    Err(error) => {
                        if conflict.is_none_or(|(_, previous)| site.order < previous.order) {
                            conflict = Some((error, site));
                        }
                    }
                }
            }
        }
        if let Some((error, site)) = conflict {
            self.conflict_site = Some(site);
            Err(error)
        } else {
            Ok(())
        }
    }

    fn related(relation: Relation, ty: Type) -> Result<Option<(Variable, Type)>, (Type, Type)> {
        match relation {
            Relation::Equal(next) => Ok(Some((next, ty))),
            Relation::SameArray(next) => match ty {
                Type::Array(_) => Ok(Some((next, ty))),
                _ => Err((Type::Array(ArrayElementType::Number), ty)),
            },
            Relation::Element(next) => match ty {
                Type::Array(element) => Ok(Some((next, element.value_type()))),
                _ => Err((Type::Array(ArrayElementType::Number), ty)),
            },
            Relation::Array(next) | Relation::Iterable(next) => {
                if matches!(relation, Relation::Iterable(_)) && ty == Type::Number {
                    // Numeric iteration does not distinguish range from number[].
                    return Ok(None);
                }
                let element = match ty {
                    Type::Number => ArrayElementType::Number,
                    Type::Bool => ArrayElementType::Bool,
                    Type::String => ArrayElementType::String,
                    _ => return Err((Type::Number, ty)),
                };
                Ok(Some((next, Type::Array(element))))
            }
            Relation::Yield(next) => match ty {
                Type::Range => Ok(Some((next, Type::Number))),
                Type::Array(element) => Ok(Some((next, element.value_type()))),
                _ => Err((Type::Range, ty)),
            },
        }
    }

    fn connect(
        &mut self,
        left: Variable,
        right: Variable,
        relationship: Relationship,
        site: Site,
    ) -> Result<(), (Type, Type)> {
        let (forward, backward) = match relationship {
            Relationship::SameType => (Relation::Equal(right), Relation::Equal(left)),
            Relationship::SameArray => (Relation::SameArray(right), Relation::SameArray(left)),
            Relationship::ArrayElement => (Relation::Element(right), Relation::Array(left)),
        };
        if let (Some(left), Some(right)) = (self.types[left.0], self.types[right.0])
            && let Some((_, expected)) = Self::related(forward, left)?
            && expected != right
        {
            return Err((expected, right));
        }
        self.edges[left.0].push((forward, site));
        self.edges[right.0].push((backward, site));
        for (variable, relation) in [(left, forward), (right, backward)] {
            if let Some(ty) = self.types[variable.0]
                && let Some((next, ty)) = Self::related(relation, ty)?
            {
                self.assign(next, ty, site.later(self.origins[variable.0]))?;
            }
        }
        Ok(())
    }

    fn iteration(
        &mut self,
        iterable: Variable,
        element: Variable,
        site: Site,
    ) -> Result<(), (Type, Type)> {
        self.edges[iterable.0].push((Relation::Yield(element), site));
        self.edges[element.0].push((Relation::Iterable(iterable), site));
        for (source, relation) in [
            (iterable, Relation::Yield(element)),
            (element, Relation::Iterable(iterable)),
        ] {
            if let Some(ty) = self.types[source.0]
                && let Some((next, ty)) = Self::related(relation, ty)?
            {
                self.assign(next, ty, site.later(self.origins[source.0]))?;
            }
        }
        Ok(())
    }

    // Callable signatures and captured types supply evidence to their users,
    // never the reverse. In particular a caller cannot select a parameter type.
    fn forward(
        &mut self,
        source: Variable,
        destination: Variable,
        site: Site,
    ) -> Result<(), (Type, Type)> {
        self.edges[source.0].push((Relation::Equal(destination), site));
        if let Some(ty) = self.types[source.0] {
            self.assign(destination, ty, site.later(self.origins[source.0]))?;
        }
        Ok(())
    }
}

struct Region<'a> {
    analyzer: &'a SemanticAnalyzer,
    parameters: Vec<Candidate>,
    parameter_indices: HashMap<usize, usize>,
    conflicts: Vec<(usize, SemanticError)>,
    next_site: usize,
    scopes: Vec<HashMap<String, RegionBinding>>,
    constraints: Constraints,
    loops: Vec<Option<Variable>>,
    returned: Option<Variable>,
    capture_sources: HashMap<Variable, Variable>,
    used_captures: HashSet<Variable>,
    empty_arrays: Vec<Variable>,
    pending_callables: Vec<PendingCallable>,
}

#[derive(Clone)]
enum RegionBinding {
    Value(Option<Variable>),
    Callable(RegionCallable),
}

#[derive(Clone)]
struct RegionCallable {
    params: Vec<Variable>,
    result: Option<Variable>,
    origin: closures::CallableOrigin,
}

struct PendingCallable {
    prerequisites: Vec<Variable>,
    params: Vec<(Variable, Variable)>,
    result: Option<(Variable, Variable)>,
    empty_arrays: Vec<Variable>,
    span: Span,
    site: Site,
}

impl SemanticAnalyzer {
    pub(super) fn infer_placeholder_parameters(
        &self,
        expression: &Expr,
        span: Span,
    ) -> Result<Vec<Parameter>, SemanticError> {
        let mut region = Region {
            analyzer: self,
            parameters: Vec::new(),
            parameter_indices: HashMap::new(),
            conflicts: Vec::new(),
            next_site: 0,
            scopes: Vec::new(),
            constraints: Constraints::default(),
            loops: Vec::new(),
            returned: None,
            capture_sources: HashMap::new(),
            used_captures: HashSet::new(),
            empty_arrays: Vec::new(),
            pending_callables: Vec::new(),
        };
        let parameter_count = region.register_parameters(expression).len();
        let (tail, flow) = region.expression(expression, span, None, true)?;
        if flow.can_fall_through()
            && !matches!(expression, Expr::Block(block) if block.tail.is_none())
            && let Some(tail) = tail
        {
            region.return_constraint(tail, span);
        }
        region.resolve_callables();
        // Source traversal, not fallback-span start, orders constraint sites.
        // A later block tail can carry the whole block's earlier starting offset.
        if let Some((_, conflict)) = region.conflicts.into_iter().min_by_key(|(site, _)| *site) {
            return Err(conflict);
        }
        for parameter in &region.parameters {
            if region.constraints.types[parameter.variable.0].is_none() {
                return Err(SemanticError::CannotInferPlaceholderParameter {
                    span: parameter.span,
                });
            }
        }
        region
            .parameters
            .into_iter()
            .take(parameter_count)
            .map(|parameter| {
                Ok(Parameter {
                    identity: parameter.identity,
                    span: parameter.span,
                    ty: region.constraints.types[parameter.variable.0].ok_or(
                        SemanticError::CannotInferPlaceholderParameter {
                            span: parameter.span,
                        },
                    )?,
                })
            })
            .collect()
    }
}

impl Region<'_> {
    fn register_parameters(&mut self, expression: &Expr) -> Vec<Variable> {
        let mut params = Vec::new();
        visit_owned(expression, &mut |expr| {
            if let Expr::Placeholder { span } = expr {
                let identity = std::ptr::from_ref(expr).addr();
                self.parameter_indices
                    .insert(identity, self.parameters.len());
                let variable = self.constraints.variable();
                self.parameters.push(Candidate {
                    identity,
                    span: *span,
                    variable,
                });
                params.push(variable);
            }
        });
        params
    }

    fn resolve_callables(&mut self) {
        // Each signature is published once, after its concrete inputs are
        // available. This schedules a finite region-local constraint graph:
        // no speculative completion and no repeated AST/body inference.
        let mut pending = std::mem::take(&mut self.pending_callables);
        loop {
            let mut progress = false;
            let mut waiting = Vec::new();
            for callable in pending {
                if callable
                    .prerequisites
                    .iter()
                    .any(|v| self.constraints.types[v.0].is_none())
                {
                    waiting.push(callable);
                    continue;
                }
                for array in callable.empty_arrays {
                    if self.constraints.types[array.0].is_none() {
                        self.constrain(
                            array,
                            Type::Array(ArrayElementType::Number),
                            callable.site.order,
                            callable.span,
                        );
                    }
                }
                for (source, destination) in callable.params.into_iter().chain(callable.result) {
                    let result = self.constraints.forward(source, destination, callable.site);
                    self.record(result, callable.site.order, callable.site.span);
                }
                progress = true;
            }
            if !progress || waiting.is_empty() {
                break;
            }
            pending = waiting;
        }
    }

    fn return_constraint(&mut self, ty: Variable, span: Span) {
        if let Some(previous) = self.returned {
            let site = self.next_site;
            self.next_site += 1;
            self.relate(previous, ty, site, span);
        } else {
            self.returned = Some(ty);
        }
    }

    fn nested_callable(
        &mut self,
        expression: &Expr,
        span: Span,
    ) -> Result<RegionCallable, SemanticError> {
        let site = Site {
            order: self.next_site,
            span,
        };
        let scopes = std::mem::take(&mut self.scopes);
        let capture_sources = std::mem::take(&mut self.capture_sources);
        let used_captures = std::mem::take(&mut self.used_captures);
        let empty_arrays = std::mem::take(&mut self.empty_arrays);
        let mut nested_scopes = Vec::with_capacity(scopes.len() + 1);
        for scope in &scopes {
            let mut captured = HashMap::new();
            // Lookup-map iteration must not choose propagation or conflict order.
            let mut names = scope.keys().collect::<Vec<_>>();
            names.sort_unstable();
            for name in names {
                let binding = &scope[name];
                let binding = match binding {
                    RegionBinding::Callable(callable) => RegionBinding::Callable(callable.clone()),
                    RegionBinding::Value(Some(source)) => {
                        let destination = self.constraints.variable();
                        let result = self.constraints.forward(*source, destination, site);
                        self.record(result, self.next_site, span);
                        self.capture_sources.insert(
                            destination,
                            capture_sources.get(source).copied().unwrap_or(*source),
                        );
                        RegionBinding::Value(Some(destination))
                    }
                    RegionBinding::Value(None) => RegionBinding::Value(None),
                };
                captured.insert(name.clone(), binding);
            }
            nested_scopes.push(captured);
        }
        nested_scopes.push(HashMap::new());
        self.scopes = nested_scopes;
        let loops = std::mem::take(&mut self.loops);
        let returned = self.returned.take();
        let result = (|| {
            let mut params = Vec::new();
            if let Expr::Lambda(lambda) = expression {
                for param in &lambda.params {
                    self.analyzer
                        .check_variable_type(&param.name, param.ty, param.span)?;
                    if param.default.is_some() {
                        return Err(SemanticError::InvalidClosure {
                            reason: "closure parameters cannot have defaults",
                            span: param.span,
                        });
                    }
                    let variable = self.constraints.concrete(param.ty);
                    if self
                        .scopes
                        .last_mut()
                        .unwrap()
                        .insert(param.name.clone(), RegionBinding::Value(Some(variable)))
                        .is_some()
                    {
                        return Err(SemanticError::DuplicateVariable {
                            name: param.name.clone(),
                            span: param.span,
                        });
                    }
                    params.push(variable);
                }
            } else {
                params = self.register_parameters(expression);
            }
            let (ty, flow, no_tail) = match expression {
                Expr::Lambda(lambda) => match &lambda.body {
                    LambdaBody::Expression(body) => {
                        let (ty, flow) = self.expression(body, lambda.body_span, None, true)?;
                        (ty, flow, false)
                    }
                    LambdaBody::Block(body) => {
                        let (ty, flow) = self.block(body, None, true)?;
                        (ty, flow, body.tail.is_none())
                    }
                },
                Expr::Block(body) => {
                    let (ty, flow) = self.block(body, None, true)?;
                    (ty, flow, body.tail.is_none())
                }
                _ => {
                    let (ty, flow) = self.expression(expression, span, None, true)?;
                    (ty, flow, false)
                }
            };
            if flow.can_fall_through()
                && !no_tail
                && let Some(ty) = ty
            {
                self.return_constraint(ty, span);
            }
            let result = if no_tail && self.returned.is_none() {
                Some(self.constraints.concrete(Type::Void))
            } else {
                self.returned
            };
            let mut prerequisites = params.clone();
            prerequisites.extend(self.used_captures.iter().copied());
            let params = params
                .into_iter()
                .map(|source| (source, self.constraints.variable()))
                .collect::<Vec<_>>();
            let result = result.map(|source| (source, self.constraints.variable()));
            let signature = RegionCallable {
                params: params.iter().map(|(_, export)| *export).collect(),
                result: result.map(|(_, export)| export),
                origin: if matches!(expression, Expr::Lambda(_)) {
                    closures::CallableOrigin::Explicit
                } else {
                    closures::CallableOrigin::Placeholder
                },
            };
            self.pending_callables.push(PendingCallable {
                prerequisites,
                params,
                result,
                empty_arrays: std::mem::take(&mut self.empty_arrays),
                span,
                site,
            });
            Ok(signature)
        })();
        self.scopes = scopes;
        self.loops = loops;
        self.returned = returned;
        self.capture_sources = capture_sources;
        self.used_captures.extend(used_captures);
        self.empty_arrays = empty_arrays;
        result
    }

    fn constrain(&mut self, variable: Variable, ty: Type, site: usize, span: Span) {
        let result = self
            .constraints
            .assign(variable, ty, Site { order: site, span })
            .map_err(|(found, expected)| (expected, found));
        self.record(result, site, span);
    }

    fn supply(&mut self, source: Variable, destination: Variable, span: Span) {
        let site = self.next_site;
        self.next_site += 1;
        let result = self
            .constraints
            .forward(source, destination, Site { order: site, span });
        self.record(result, site, span);
    }

    fn apply_callable(
        &mut self,
        callable: &RegionCallable,
        args: &[CallArgument],
        span: Span,
        injection: Option<Variable>,
        reachable: bool,
    ) -> Result<(Option<Variable>, Flow), SemanticError> {
        if let Some(arg) = args.iter().find(|arg| arg.name.is_some()) {
            return Err(SemanticError::NamedArgumentsNotSupported {
                name: "<closure>".into(),
                span: arg.span,
            });
        }
        let supplied = args.len() + usize::from(injection.is_some());
        if supplied != callable.params.len() {
            return Err(SemanticError::ArgumentCountMismatch {
                name: "<closure>".into(),
                expected: callable.params.len(),
                found: supplied,
                span,
            });
        }
        let offset = usize::from(injection.is_some());
        if reachable && let Some(input) = injection {
            self.supply(callable.params[0], input, span);
        }
        let mut flow = Flow::FALLTHROUGH;
        for (arg, formal) in args.iter().zip(&callable.params[offset..]) {
            let active = reachable && flow.can_fall_through();
            let expected = self.constraints.types[formal.0];
            let (argument, next) = self.expression(&arg.value, arg.span, expected, active)?;
            if active
                && next.can_fall_through()
                && let Some(argument) = argument
            {
                self.supply(*formal, argument, arg.span);
            }
            flow = flow.then(next);
        }
        let result = callable.result.map(|signature| {
            let result = self.constraints.variable();
            if reachable && flow.can_fall_through() {
                self.supply(signature, result, span);
            }
            result
        });
        Ok((result, flow))
    }

    fn relate(&mut self, left: Variable, right: Variable, site: usize, span: Span) {
        let result = self.constraints.connect(
            left,
            right,
            Relationship::SameType,
            Site { order: site, span },
        );
        self.record(result, site, span);
    }

    fn record(&mut self, result: Result<(), (Type, Type)>, site: usize, span: Span) {
        if let Err((expected, found)) = result {
            let Site { order: site, span } = self
                .constraints
                .conflict_site
                .take()
                .unwrap_or(Site { order: site, span });
            self.conflicts.push((
                site,
                SemanticError::ConflictingPlaceholderConstraints {
                    expected,
                    found,
                    span,
                },
            ));
        }
    }

    fn expression(
        &mut self,
        expression: &Expr,
        span: Span,
        expected: Option<Type>,
        reachable: bool,
    ) -> Result<(Option<Variable>, Flow), SemanticError> {
        let site = self.next_site;
        self.next_site += 1;
        let mut flow = Flow::FALLTHROUGH;
        let ty = match expression {
            Expr::Placeholder { .. } => {
                let identity = std::ptr::from_ref(expression).addr();
                let Some(index) = self.parameter_indices.get(&identity) else {
                    let Expr::Placeholder { span } = expression else {
                        unreachable!()
                    };
                    return Err(SemanticError::InvalidPlaceholder { span: *span });
                };
                let variable = self.parameters[*index].variable;
                if reachable && let Some(expected) = expected {
                    self.constrain(variable, expected, site, span);
                }
                return Ok((Some(variable), flow));
            }
            Expr::Number(_) => Some(self.constraints.concrete(Type::Number)),
            Expr::Bool(_) => Some(self.constraints.concrete(Type::Bool)),
            Expr::String(_) => Some(self.constraints.concrete(Type::String)),
            Expr::Identifier(name) => {
                if let Some(binding) = self.scopes.iter().rev().find_map(|scope| scope.get(name)) {
                    match binding {
                        RegionBinding::Value(ty) => {
                            if let Some(source) = ty.and_then(|v| self.capture_sources.get(&v)) {
                                self.used_captures.insert(*source);
                            }
                            *ty
                        }
                        RegionBinding::Callable(callable) => {
                            return Err(
                                if callable.origin == closures::CallableOrigin::Placeholder {
                                    SemanticError::InvalidCallableValueUse { span }
                                } else {
                                    SemanticError::InvalidClosure {
                                        reason: "a local callable binding cannot be used as a value; invoke it directly or use a named function",
                                        span,
                                    }
                                },
                            );
                        }
                    }
                } else {
                    match self.analyzer.lookup_binding(name) {
                        Some(LexicalBinding::Value(binding)) => {
                            Some(self.constraints.concrete(binding.ty))
                        }
                        Some(LexicalBinding::Callable(callable))
                            if callable.origin == closures::CallableOrigin::Placeholder =>
                        {
                            return Err(SemanticError::InvalidCallableValueUse { span });
                        }
                        Some(LexicalBinding::Callable(_)) => {
                            return Err(SemanticError::InvalidClosure {
                                reason: "a local callable binding cannot be used as a value; invoke it directly or use a named function",
                                span,
                            });
                        }
                        None => {
                            return Err(SemanticError::UndefinedVariable {
                                name: name.clone(),
                                span,
                            });
                        }
                    }
                }
            }
            Expr::Unary { op, expr } => {
                let ty = match op {
                    UnaryOp::Not => Type::Bool,
                    UnaryOp::Negate => Type::Number,
                };
                let (_, next) = self.expression(expr, span, Some(ty), reachable)?;
                flow = next;
                Some(self.constraints.concrete(ty))
            }
            Expr::Binary { left, op, right } => {
                let (operand, result) = match op {
                    BinaryOp::Add
                    | BinaryOp::Subtract
                    | BinaryOp::Multiply
                    | BinaryOp::Divide
                    | BinaryOp::Remainder => (Some(Type::Number), Type::Number),
                    BinaryOp::LessThan
                    | BinaryOp::GreaterThan
                    | BinaryOp::LessThanOrEqual
                    | BinaryOp::GreaterThanOrEqual
                    | BinaryOp::In => (Some(Type::Number), Type::Bool),
                    BinaryOp::And | BinaryOp::Or => (Some(Type::Bool), Type::Bool),
                    BinaryOp::Equal | BinaryOp::NotEqual => (None, Type::Bool),
                };
                let (left_ty, left_flow) = self.expression(left, span, operand, reachable)?;
                let (right_ty, right_flow) = self.expression(
                    right,
                    span,
                    if *op == BinaryOp::In {
                        Some(Type::Range)
                    } else {
                        operand
                    },
                    reachable && left_flow.can_fall_through(),
                )?;
                if operand.is_none()
                    && reachable
                    && left_flow.can_fall_through()
                    && right_flow.can_fall_through()
                    && let (Some(left), Some(right)) = (left_ty, right_ty)
                {
                    self.relate(left, right, site, span);
                }
                flow = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    left_flow.then(Flow::FALLTHROUGH.join(right_flow.without_fallthrough()))
                } else {
                    left_flow.then(right_flow)
                };
                Some(self.constraints.concrete(result))
            }
            Expr::Reserve {
                receiver,
                args,
                span: method_span,
            } => {
                SemanticAnalyzer::check_single_method_argument("reserve", args, *method_span)?;
                let (receiver, receiver_flow) = self.expression(receiver, span, None, reachable)?;
                if reachable
                    && receiver_flow.can_fall_through()
                    && let Some(receiver) = receiver
                {
                    let array = self.constraints.variable();
                    let relationship = self.constraints.connect(
                        receiver,
                        array,
                        Relationship::SameArray,
                        Site { order: site, span },
                    );
                    self.record(relationship, site, span);
                }
                let (_, additional_flow) = self.expression(
                    &args[0].value,
                    span,
                    Some(Type::Number),
                    reachable && receiver_flow.can_fall_through(),
                )?;
                flow = receiver_flow.then(additional_flow);
                Some(self.constraints.concrete(Type::Void))
            }
            Expr::Push {
                receiver,
                args,
                span: method_span,
            } => {
                SemanticAnalyzer::check_single_method_argument("push", args, *method_span)?;
                let (receiver, receiver_flow) = self.expression(receiver, span, None, reachable)?;
                let element = self.constraints.variable();
                let active = reachable && receiver_flow.can_fall_through();
                if active && let Some(receiver) = receiver {
                    let relationship = self.constraints.connect(
                        receiver,
                        element,
                        Relationship::ArrayElement,
                        Site { order: site, span },
                    );
                    self.record(relationship, site, span);
                }
                let expected = self.constraints.types[element.0];
                let (item, item_flow) = self.expression(&args[0].value, span, expected, active)?;
                if active
                    && item_flow.can_fall_through()
                    && let Some(item) = item
                {
                    self.relate(element, item, site, span);
                }
                flow = receiver_flow.then(item_flow);
                Some(self.constraints.concrete(Type::Void))
            }
            Expr::Call {
                func,
                args,
                span: call_span,
            } => {
                if let Some(RegionBinding::Callable(callable)) = self
                    .scopes
                    .iter()
                    .rev()
                    .find_map(|scope| scope.get(func))
                    .cloned()
                {
                    let (result, next) =
                        self.apply_callable(&callable, args, *call_span, None, reachable)?;
                    if reachable
                        && next.can_fall_through()
                        && let (Some(result), Some(expected)) = (result, expected)
                    {
                        self.constrain(result, expected, site, span);
                    }
                    return Ok((result.filter(|_| next.can_fall_through()), next));
                }
                let lexical = match self.scopes.iter().rev().find_map(|scope| scope.get(func)) {
                    Some(RegionBinding::Value(_)) => {
                        return Err(SemanticError::NotCallable {
                            name: func.clone(),
                            span: *call_span,
                        });
                    }
                    Some(RegionBinding::Callable(_)) => {
                        unreachable!("region callable handled above")
                    }
                    None => self.analyzer.lookup_binding(func).cloned(),
                };
                let (params, result) = match lexical {
                    Some(LexicalBinding::Value(_)) => {
                        return Err(SemanticError::NotCallable {
                            name: func.clone(),
                            span: *call_span,
                        });
                    }
                    Some(LexicalBinding::Callable(callable)) => {
                        if let Some(arg) = args.iter().find(|a| a.name.is_some()) {
                            return Err(SemanticError::NamedArgumentsNotSupported {
                                name: "<closure>".into(),
                                span: arg.span,
                            });
                        }
                        if args.len() != callable.params.len() {
                            return Err(SemanticError::ArgumentCountMismatch {
                                name: "<closure>".into(),
                                expected: callable.params.len(),
                                found: args.len(),
                                span: *call_span,
                            });
                        }
                        (
                            callable
                                .params
                                .iter()
                                .copied()
                                .map(Some)
                                .collect::<Vec<_>>(),
                            callable.return_type,
                        )
                    }
                    None => {
                        let signature = self.analyzer.functions.get(func).ok_or_else(|| {
                            SemanticError::UndefinedFunction {
                                name: func.clone(),
                                span: *call_span,
                            }
                        })?;
                        if signature.reference == FunctionRef::Builtin(BuiltinFunction::Resizable) {
                            if let Some(arg) = args.iter().find(|arg| arg.name.is_some()) {
                                return Err(SemanticError::NamedArgumentsNotSupported {
                                    name: func.clone(),
                                    span: arg.span,
                                });
                            }
                            if args.len() != 1 {
                                return Err(SemanticError::ArgumentCountMismatch {
                                    name: func.clone(),
                                    expected: 1,
                                    found: args.len(),
                                    span: *call_span,
                                });
                            }
                            let array_context = expected.filter(|ty| matches!(ty, Type::Array(_)));
                            let (source, next) =
                                self.expression(&args[0].value, span, array_context, reachable)?;
                            let result = self.constraints.variable();
                            if reachable
                                && next.can_fall_through()
                                && let Some(source) = source
                            {
                                let relationship = self.constraints.connect(
                                    source,
                                    result,
                                    Relationship::SameArray,
                                    Site { order: site, span },
                                );
                                self.record(relationship, site, span);
                                if let Some(expected) = expected {
                                    self.constrain(result, expected, site, span);
                                }
                            }
                            return Ok((next.can_fall_through().then_some(result), next));
                        }
                        let params =
                            if signature.reference == FunctionRef::Builtin(BuiltinFunction::Len) {
                                if let Some(arg) = args.iter().find(|a| a.name.is_some()) {
                                    return Err(SemanticError::NamedArgumentsNotSupported {
                                        name: func.clone(),
                                        span: arg.span,
                                    });
                                }
                                if args.len() != 1 {
                                    return Err(SemanticError::ArgumentCountMismatch {
                                        name: func.clone(),
                                        expected: 1,
                                        found: args.len(),
                                        span: *call_span,
                                    });
                                }
                                // len is overloaded; its historical numeric-array
                                // signature is not evidence for an unknown argument.
                                vec![None; args.len()]
                            } else if matches!(signature.reference, FunctionRef::User(_)) {
                                self.analyzer
                                    .map_call_arguments(func, args, *call_span, false)?
                                    .into_iter()
                                    .map(|index| Some(signature.params[index]))
                                    .collect()
                            } else {
                                if let Some(arg) = args.iter().find(|a| a.name.is_some()) {
                                    return Err(SemanticError::NamedArgumentsNotSupported {
                                        name: func.clone(),
                                        span: arg.span,
                                    });
                                }
                                if args.len() != signature.params.len() {
                                    return Err(SemanticError::ArgumentCountMismatch {
                                        name: func.clone(),
                                        expected: signature.params.len(),
                                        found: args.len(),
                                        span: *call_span,
                                    });
                                }
                                signature.params.iter().copied().map(Some).collect()
                            };
                        (params, signature.return_type)
                    }
                };
                for (arg, expected) in args.iter().zip(params) {
                    let (_, next) = self.expression(
                        &arg.value,
                        arg.span,
                        expected,
                        reachable && flow.can_fall_through(),
                    )?;
                    flow = flow.then(next);
                }
                Some(self.constraints.concrete(result))
            }
            Expr::Block(block) => {
                let (ty, next) = self.block(block, expected, reachable)?;
                flow = next;
                ty
            }
            Expr::Range {
                start, end, step, ..
            } => {
                for child in [Some(start.as_ref()), Some(end.as_ref()), step.as_deref()]
                    .into_iter()
                    .flatten()
                {
                    let (_, next) = self.expression(
                        child,
                        span,
                        Some(Type::Number),
                        reachable && flow.can_fall_through(),
                    )?;
                    flow = flow.then(next);
                }
                Some(self.constraints.concrete(Type::Range))
            }
            Expr::Index { expr, index } => {
                let (array, before) = self.expression(expr, span, None, reachable)?;
                let (_, next) = self.expression(
                    index,
                    span,
                    Some(Type::Number),
                    reachable && before.can_fall_through(),
                )?;
                flow = before.then(next);
                let element = self.constraints.variable();
                if reachable
                    && flow.can_fall_through()
                    && let Some(array) = array
                {
                    let result = self.constraints.connect(
                        array,
                        element,
                        Relationship::ArrayElement,
                        Site { order: site, span },
                    );
                    self.record(result, site, span);
                }
                Some(element)
            }
            Expr::Array(values) => {
                // No numeric fallback supplies evidence about an unknown input.
                // Once parameters are concrete the ordinary checker applies its
                // existing literal context and empty-array fallback rules.
                let array = self.constraints.variable();
                if values.is_empty() {
                    self.empty_arrays.push(array);
                }
                if reachable && let Some(Type::Array(element)) = expected {
                    self.constrain(array, Type::Array(element), site, span);
                }
                for value in values {
                    let active = reachable && flow.can_fall_through();
                    let (element, next) = self.expression(value, span, None, active)?;
                    if active
                        && next.can_fall_through()
                        && let Some(element) = element
                    {
                        let result = self.constraints.connect(
                            array,
                            element,
                            Relationship::ArrayElement,
                            Site { order: site, span },
                        );
                        self.record(result, site, span);
                    }
                    flow = flow.then(next);
                }
                Some(array)
            }
            Expr::If { cond, then, else_ } => {
                let (_, condition_flow) =
                    self.expression(cond, span, Some(Type::Bool), reachable)?;
                let active = reachable && condition_flow.can_fall_through();
                let (then_type, then_flow) = self.block(then, expected, active)?;
                let (else_type, else_flow) = if let Some(block) = else_ {
                    self.block(block, expected, active)?
                } else {
                    (
                        Some(self.constraints.concrete(Type::Void)),
                        Flow::FALLTHROUGH,
                    )
                };
                if active
                    && then_flow.can_fall_through()
                    && else_flow.can_fall_through()
                    && let (Some(left), Some(right)) = (then_type, else_type)
                {
                    self.relate(left, right, site, span);
                }
                flow = condition_flow.then(then_flow.join(else_flow));
                if then_flow.can_fall_through() {
                    then_type
                } else {
                    else_type
                }
            }
            Expr::Match { scrutinee, arms } => {
                let (ty, next) = self.match_arms(
                    scrutinee,
                    arms,
                    span,
                    expected,
                    reachable,
                    ExprContext::Value,
                )?;
                flow = next;
                ty
            }
            Expr::Loop { body } => {
                let result = self.constraints.variable();
                self.loops.push(Some(result));
                let next = self.scoped_statements(body, reachable)?;
                self.loops.pop();
                flow = next.infinite_loop();
                Some(result)
            }
            Expr::Pipeline {
                input,
                input_span,
                stages,
            } => {
                let (mut input, before) = self.expression(input, *input_span, None, reachable)?;
                flow = before;
                for stage in stages {
                    let name = match &stage.target {
                        PipelineStageTarget::Named(name) => name,
                        PipelineStageTarget::Immediate {
                            expression,
                            expression_span,
                        } => {
                            if !stage.args.is_empty() {
                                return Err(SemanticError::InvalidPipeline {
                                    message: "immediate pipeline stages cannot have written arguments",
                                    span: stage.span,
                                });
                            }
                            let parameter_count = if let Expr::Lambda(lambda) = expression {
                                lambda.params.len()
                            } else {
                                let count = owned_parameter_count(expression);
                                if count == 0 {
                                    return Err(SemanticError::InvalidPipeline {
                                        message: "pipeline stage target is not callable",
                                        span: stage.span,
                                    });
                                }
                                count
                            };
                            if parameter_count != 1 {
                                return Err(SemanticError::InvalidPipeline {
                                    message: "immediate pipeline stage must have exactly one parameter",
                                    span: stage.span,
                                });
                            }
                            let callable = self.nested_callable(expression, *expression_span)?;
                            // Publish the independently solved signature toward the
                            // input, never input evidence toward the stage's region.
                            let injected = input.unwrap_or_else(|| self.constraints.variable());
                            let (result, next) = self.apply_callable(
                                &callable,
                                &stage.args,
                                stage.span,
                                Some(injected),
                                reachable && flow.can_fall_through(),
                            )?;
                            flow = flow.then(next);
                            input = result;
                            continue;
                        }
                    };
                    if let Some(RegionBinding::Callable(callable)) = self
                        .scopes
                        .iter()
                        .rev()
                        .find_map(|scope| scope.get(name))
                        .cloned()
                    {
                        // Even a noncompleting input occupies formal zero.
                        let injected = input.unwrap_or_else(|| self.constraints.variable());
                        let (result, next) = self.apply_callable(
                            &callable,
                            &stage.args,
                            stage.span,
                            Some(injected),
                            reachable && flow.can_fall_through(),
                        )?;
                        flow = flow.then(next);
                        input = result;
                        continue;
                    }
                    let lexical = match self.scopes.iter().rev().find_map(|scope| scope.get(name)) {
                        Some(RegionBinding::Callable(_)) => {
                            unreachable!("region callable handled above")
                        }
                        Some(RegionBinding::Value(_)) => {
                            return Err(SemanticError::NotCallable {
                                name: name.clone(),
                                span: stage.span,
                            });
                        }
                        None => self.analyzer.lookup_binding(name).cloned(),
                    };
                    let (first, written, result) = match lexical {
                        Some(LexicalBinding::Value(_)) => {
                            return Err(SemanticError::NotCallable {
                                name: name.clone(),
                                span: stage.span,
                            });
                        }
                        Some(LexicalBinding::Callable(callable)) => {
                            if let Some(arg) = stage.args.iter().find(|arg| arg.name.is_some()) {
                                return Err(SemanticError::NamedArgumentsNotSupported {
                                    name: "<closure>".into(),
                                    span: arg.span,
                                });
                            }
                            if callable.params.len() != stage.args.len() + 1 {
                                return Err(SemanticError::ArgumentCountMismatch {
                                    name: "<closure>".into(),
                                    expected: callable.params.len(),
                                    found: stage.args.len() + 1,
                                    span: stage.span,
                                });
                            }
                            (
                                callable.params[0],
                                callable.params[1..].to_vec(),
                                callable.return_type,
                            )
                        }
                        None => {
                            let signature = self.analyzer.functions.get(name).ok_or_else(|| {
                                SemanticError::UndefinedFunction {
                                    name: name.clone(),
                                    span: stage.span,
                                }
                            })?;
                            if !matches!(signature.reference, FunctionRef::User(_)) {
                                return Err(SemanticError::InvalidPipeline {
                                    message: "stages must name user-defined functions",
                                    span: stage.span,
                                });
                            }
                            if signature.params.is_empty() {
                                return Err(SemanticError::InvalidPipeline {
                                    message: "a stage requires an input parameter",
                                    span: stage.span,
                                });
                            }
                            let mapping = self.analyzer.map_call_arguments(
                                name,
                                &stage.args,
                                stage.span,
                                true,
                            )?;
                            (
                                signature.params[0],
                                mapping.into_iter().map(|i| signature.params[i]).collect(),
                                signature.return_type,
                            )
                        }
                    };
                    if reachable
                        && flow.can_fall_through()
                        && let Some(input) = input
                    {
                        self.constrain(input, first, site, stage.span);
                    }
                    for (arg, expected) in stage.args.iter().zip(written) {
                        let (_, next) = self.expression(
                            &arg.value,
                            arg.span,
                            Some(expected),
                            reachable && flow.can_fall_through(),
                        )?;
                        flow = flow.then(next);
                    }
                    input = Some(self.constraints.concrete(result));
                }
                input
            }
            Expr::Lambda(lambda) => {
                return Err(SemanticError::InvalidClosure {
                    reason: "a closure must initialize a local callable binding",
                    span: lambda.span,
                });
            }
        };
        if reachable
            && flow.can_fall_through()
            && let (Some(expected), Some(variable)) = (expected, ty)
        {
            self.constrain(variable, expected, site, span);
        }
        Ok((ty.filter(|_| flow.can_fall_through()), flow))
    }

    fn match_arms(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        span: Span,
        expected: Option<Type>,
        reachable: bool,
        context: ExprContext,
    ) -> Result<(Option<Variable>, Flow), SemanticError> {
        let domain = arms.iter().find_map(|arm| match &arm.pattern.kind {
            MatchPatternKind::Wildcard => None,
            MatchPatternKind::Bool(_) => Some(Type::Bool),
            MatchPatternKind::String(_) => Some(Type::String),
            MatchPatternKind::Number(_)
            | MatchPatternKind::Relational { .. }
            | MatchPatternKind::Interval { .. } => Some(Type::Number),
        });
        let (_, before) = self.expression(scrutinee, span, domain, reachable)?;
        let active = reachable && before.can_fall_through();
        let mut flow = Flow::NONE;
        let mut result = None;
        for arm in arms {
            self.scopes.push(HashMap::new());
            let (ty, next) = self.expression(&arm.body, arm.span, expected, active)?;
            self.scopes.pop();
            if context != ExprContext::Discard
                && active
                && next.can_fall_through()
                && let Some(ty) = ty
            {
                if let Some(previous) = result {
                    let site = self.next_site;
                    self.next_site += 1;
                    self.relate(previous, ty, site, arm.span);
                } else {
                    result = Some(ty);
                }
            }
            flow = flow.join(next);
        }
        Ok((result, before.then(flow)))
    }

    fn scoped_statements(
        &mut self,
        statements: &Block,
        reachable: bool,
    ) -> Result<Flow, SemanticError> {
        self.scopes.push(HashMap::new());
        let result = self.statements(statements, reachable);
        self.scopes.pop();
        result
    }

    fn block(
        &mut self,
        block: &ValueBlock,
        expected: Option<Type>,
        reachable: bool,
    ) -> Result<(Option<Variable>, Flow), SemanticError> {
        self.scopes.push(HashMap::new());
        let result = (|| {
            let flow = self.statements(&block.statements, reachable)?;
            if let Some(tail) = &block.tail {
                let (ty, tail_flow) = self.expression(
                    tail,
                    block.span,
                    expected,
                    reachable && flow.can_fall_through(),
                )?;
                Ok((ty, flow.then(tail_flow)))
            } else {
                Ok((Some(self.constraints.concrete(Type::Void)), flow))
            }
        })();
        self.scopes.pop();
        result
    }

    fn statements(&mut self, statements: &Block, reachable: bool) -> Result<Flow, SemanticError> {
        let mut flow = Flow::FALLTHROUGH;
        for statement in statements {
            let span = statement.span;
            let reachable = reachable && flow.can_fall_through();
            let next = match &statement.kind {
                StatementKind::Expression(expr) => self.expression(expr, span, None, reachable)?.1,
                StatementKind::Return(value) => {
                    let next = if let Some(value) = value {
                        let (ty, next) = self.expression(value, span, None, reachable)?;
                        if reachable
                            && next.can_fall_through()
                            && let Some(ty) = ty
                        {
                            self.return_constraint(ty, span);
                        }
                        next
                    } else {
                        Flow::FALLTHROUGH
                    };
                    next.then(Flow::RETURN)
                }
                StatementKind::BreakValue(value) => {
                    let (ty, next) = self.expression(value, span, None, reachable)?;
                    if reachable
                        && next.can_fall_through()
                        && let (Some(Some(result)), Some(ty)) = (self.loops.last(), ty)
                    {
                        let site = self.next_site;
                        self.next_site += 1;
                        self.relate(*result, ty, site, span);
                    }
                    next.then(Flow::BREAK)
                }
                StatementKind::Let {
                    name,
                    name_span,
                    value,
                    value_span,
                    ty,
                } => {
                    if (has_owned(value) && !matches!(value, Expr::Placeholder { .. }))
                        || matches!(value, Expr::Lambda(_))
                    {
                        if ty.is_some() {
                            return Err(SemanticError::InvalidClosure {
                                reason: "a local callable binding cannot have a value type annotation",
                                span: *name_span,
                            });
                        }
                        let callable = self.nested_callable(value, *value_span)?;
                        let scope = self.scopes.last_mut().expect("value block scope is active");
                        if scope
                            .insert(name.clone(), RegionBinding::Callable(callable))
                            .is_some()
                        {
                            return Err(SemanticError::DuplicateVariable {
                                name: name.clone(),
                                span,
                            });
                        }
                        flow = flow.then(Flow::FALLTHROUGH);
                        continue;
                    }
                    let (inferred, next) = self.expression(value, span, *ty, reachable)?;
                    let scope = self.scopes.last_mut().expect("value block scope is active");
                    if scope.contains_key(name) {
                        return Err(SemanticError::DuplicateVariable {
                            name: name.clone(),
                            span,
                        });
                    }
                    scope.insert(name.clone(), RegionBinding::Value(inferred));
                    next
                }
                StatementKind::Assignment { target, value }
                | StatementKind::CompoundAssignment { target, value, .. } => {
                    let numeric =
                        matches!(statement.kind, StatementKind::CompoundAssignment { .. });
                    let (target_type, target_flow) =
                        self.expression(target, span, numeric.then_some(Type::Number), reachable)?;
                    let (value_type, value_flow) = self.expression(
                        value,
                        span,
                        if numeric { Some(Type::Number) } else { None },
                        reachable && target_flow.can_fall_through(),
                    )?;
                    if reachable
                        && target_flow.can_fall_through()
                        && value_flow.can_fall_through()
                        && let (Some(target), Some(value)) = (target_type, value_type)
                    {
                        let site = self.next_site;
                        self.next_site += 1;
                        self.relate(target, value, site, span);
                    }
                    target_flow.then(value_flow)
                }
                StatementKind::Break => Flow::BREAK,
                StatementKind::Continue => Flow::CONTINUE,
                StatementKind::Loop { body } => {
                    self.loops.push(None);
                    let next = self.scoped_statements(body, reachable)?;
                    self.loops.pop();
                    next.infinite_loop()
                }
                StatementKind::If { cond, then, else_ } => {
                    let (_, before) = self.expression(cond, span, Some(Type::Bool), reachable)?;
                    let active = reachable && before.can_fall_through();
                    let then = self.scoped_statements(then, active)?;
                    let else_ = if let Some(block) = else_ {
                        self.scoped_statements(block, active)?
                    } else {
                        Flow::FALLTHROUGH
                    };
                    before.then(then.join(else_))
                }
                StatementKind::Match { scrutinee, arms } => {
                    self.match_arms(scrutinee, arms, span, None, reachable, ExprContext::Discard)?
                        .1
                }
                StatementKind::While { cond, body } => {
                    self.loops.push(None);
                    let (_, before) = self.expression(cond, span, Some(Type::Bool), reachable)?;
                    let next =
                        self.scoped_statements(body, reachable && before.can_fall_through())?;
                    self.loops.pop();
                    if before.can_fall_through() {
                        before.then(next).while_loop()
                    } else {
                        before.infinite_loop()
                    }
                }
                StatementKind::For {
                    name,
                    iterable,
                    body,
                } => {
                    let (iterable, before) = self.expression(iterable, span, None, reachable)?;
                    let element = self.constraints.variable();
                    if reachable
                        && before.can_fall_through()
                        && let Some(iterable) = iterable
                    {
                        let result = self.constraints.iteration(
                            iterable,
                            element,
                            Site {
                                order: self.next_site,
                                span,
                            },
                        );
                        self.record(result, self.next_site, span);
                    }
                    self.scopes.push(HashMap::from([(
                        name.clone(),
                        RegionBinding::Value(Some(element)),
                    )]));
                    self.loops.push(None);
                    let next =
                        self.scoped_statements(body, reachable && before.can_fall_through())?;
                    self.loops.pop();
                    self.scopes.pop();
                    before.then(next.while_loop())
                }
            };
            flow = flow.then(next);
        }
        Ok(flow)
    }
}
