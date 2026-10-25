//! Exact region-local constraints. Variables and relationships never cross the
//! concrete callable boundary; callers cannot refine a declaration's signature.
use super::*;
use crate::ArrayElementType;

pub(super) struct Parameter {
    pub(super) identity: usize,
    pub(super) span: Span,
    pub(super) ty: Type,
}

pub(super) fn has_owned(expression: &Expr) -> bool {
    first_owned_span(expression).is_some()
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

#[derive(Clone, Copy)]
struct Variable(usize);

#[derive(Clone, Copy)]
enum Relation {
    Equal(Variable),
    Element(Variable),
    Array(Variable),
}

#[derive(Clone, Copy)]
enum Relationship {
    SameType,
    ArrayElement,
}

#[derive(Default)]
struct Constraints {
    types: Vec<Option<Type>>,
    edges: Vec<Vec<Relation>>,
}

impl Constraints {
    fn variable(&mut self) -> Variable {
        let variable = Variable(self.types.len());
        self.types.push(None);
        self.edges.push(Vec::new());
        variable
    }

    fn concrete(&mut self, ty: Type) -> Variable {
        let variable = self.variable();
        self.types[variable.0] = Some(ty);
        variable
    }

    fn assign(&mut self, variable: Variable, ty: Type) -> Result<(), (Type, Type)> {
        let mut pending = vec![(variable, ty)];
        while let Some((variable, ty)) = pending.pop() {
            if let Some(expected) = self.types[variable.0] {
                if expected != ty {
                    return Err((expected, ty));
                }
                continue;
            }
            self.types[variable.0] = Some(ty);
            for relation in &self.edges[variable.0] {
                let (next, ty) = Self::related(*relation, ty)?;
                pending.push((next, ty));
            }
        }
        Ok(())
    }

    fn related(relation: Relation, ty: Type) -> Result<(Variable, Type), (Type, Type)> {
        match relation {
            Relation::Equal(next) => Ok((next, ty)),
            Relation::Element(next) => match ty {
                Type::Array(element) => Ok((next, element.value_type())),
                _ => Err((Type::Array(ArrayElementType::Number), ty)),
            },
            Relation::Array(next) => {
                let element = match ty {
                    Type::Number => ArrayElementType::Number,
                    Type::Bool => ArrayElementType::Bool,
                    Type::String => ArrayElementType::String,
                    _ => return Err((Type::Number, ty)),
                };
                Ok((next, Type::Array(element)))
            }
        }
    }

    fn connect(
        &mut self,
        left: Variable,
        right: Variable,
        relationship: Relationship,
    ) -> Result<(), (Type, Type)> {
        let (forward, backward) = match relationship {
            Relationship::SameType => (Relation::Equal(right), Relation::Equal(left)),
            Relationship::ArrayElement => (Relation::Element(right), Relation::Array(left)),
        };
        if let (Some(left), Some(right)) = (self.types[left.0], self.types[right.0]) {
            let (_, expected) = Self::related(forward, left)?;
            if expected != right {
                return Err((expected, right));
            }
        }
        self.edges[left.0].push(forward);
        self.edges[right.0].push(backward);
        for (variable, relation) in [(left, forward), (right, backward)] {
            if let Some(ty) = self.types[variable.0] {
                let (next, ty) = Self::related(relation, ty)?;
                self.assign(next, ty)?;
            }
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
    scopes: Vec<HashMap<String, Option<Variable>>>,
    constraints: Constraints,
    deferred: bool,
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
            deferred: false,
        };
        visit_owned(expression, &mut |expr| {
            if let Expr::Placeholder { span } = expr {
                let identity = std::ptr::from_ref(expr).addr();
                region
                    .parameter_indices
                    .insert(identity, region.parameters.len());
                let variable = region.constraints.variable();
                region.parameters.push(Candidate {
                    identity,
                    span: *span,
                    variable,
                });
            }
        });
        region.expression(expression, span, None, true)?;
        // Source traversal, not fallback-span start, orders constraint sites.
        // A later block tail can carry the whole block's earlier starting offset.
        if let Some((_, conflict)) = region.conflicts.into_iter().min_by_key(|(site, _)| *site) {
            return Err(conflict);
        }
        if region.deferred {
            return Err(SemanticError::CannotInferPlaceholderParameter {
                span: region.parameters[0].span,
            });
        }
        region
            .parameters
            .into_iter()
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
    fn constrain(&mut self, variable: Variable, ty: Type, site: usize, span: Span) {
        let result = self
            .constraints
            .assign(variable, ty)
            .map_err(|(found, expected)| (expected, found));
        self.record(result, site, span);
    }

    fn relate(&mut self, left: Variable, right: Variable, site: usize, span: Span) {
        let result = self
            .constraints
            .connect(left, right, Relationship::SameType);
        self.record(result, site, span);
    }

    fn record(&mut self, result: Result<(), (Type, Type)>, site: usize, span: Span) {
        if let Err((expected, found)) = result {
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
                let variable = self.parameters[self.parameter_indices[&identity]].variable;
                if reachable && let Some(expected) = expected {
                    self.constrain(variable, expected, site, span);
                }
                return Ok((Some(variable), flow));
            }
            Expr::Number(_) => Some(self.constraints.concrete(Type::Number)),
            Expr::Bool(_) => Some(self.constraints.concrete(Type::Bool)),
            Expr::String(_) => Some(self.constraints.concrete(Type::String)),
            Expr::Identifier(name) => {
                if let Some(ty) = self.scopes.iter().rev().find_map(|scope| scope.get(name)) {
                    *ty
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
            Expr::Call {
                func,
                args,
                span: call_span,
            } => {
                if self
                    .scopes
                    .iter()
                    .rev()
                    .any(|scope| scope.contains_key(func))
                {
                    return Err(SemanticError::NotCallable {
                        name: func.clone(),
                        span: *call_span,
                    });
                }
                let lexical = self.analyzer.lookup_binding(func);
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
                    let result =
                        self.constraints
                            .connect(array, element, Relationship::ArrayElement);
                    self.record(result, site, span);
                }
                Some(element)
            }
            Expr::Array(values) => {
                // No numeric fallback supplies evidence about an unknown input.
                // Once parameters are concrete the ordinary checker applies its
                // existing literal context and empty-array fallback rules.
                let array = self.constraints.variable();
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
                        let result =
                            self.constraints
                                .connect(array, element, Relationship::ArrayElement);
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
            // These compositions belong to #47. Discover their parameters
            // structurally but do not use partial evidence to choose a signature.
            Expr::Match { .. } | Expr::Loop { .. } | Expr::Pipeline { .. } => {
                self.deferred = true;
                None
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
                        self.expression(value, span, None, reachable)?.1
                    } else {
                        Flow::FALLTHROUGH
                    };
                    next.then(Flow::RETURN)
                }
                StatementKind::BreakValue(value) => self
                    .expression(value, span, None, reachable)?
                    .1
                    .then(Flow::BREAK),
                StatementKind::Let {
                    name, value, ty, ..
                } => {
                    if (has_owned(value) && !matches!(value, Expr::Placeholder { .. }))
                        || matches!(value, Expr::Lambda(_))
                    {
                        // Nested callable composition is deliberately deferred.
                        // Do not treat its body as evidence for this region.
                        return Err(SemanticError::CannotInferPlaceholderParameter {
                            span: self.parameters[0].span,
                        });
                    }
                    let (inferred, next) = self.expression(value, span, *ty, reachable)?;
                    let scope = self.scopes.last_mut().expect("value block scope is active");
                    if scope.contains_key(name) {
                        return Err(SemanticError::DuplicateVariable {
                            name: name.clone(),
                            span,
                        });
                    }
                    scope.insert(name.clone(), inferred);
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
                StatementKind::Loop { body } => self.statements(body, reachable)?.infinite_loop(),
                _ => {
                    return Err(SemanticError::CannotInferPlaceholderParameter {
                        span: self.parameters[0].span,
                    });
                }
            };
            flow = flow.then(next);
        }
        Ok(flow)
    }
}
