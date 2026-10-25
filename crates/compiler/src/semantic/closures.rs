use super::*;

#[derive(Clone)]
pub(super) struct CallableBinding {
    pub(super) id: CallableId,
    pub(super) params: Vec<Type>,
    pub(super) return_type: Type,
    pub(super) origin: CallableOrigin,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CallableOrigin {
    Explicit,
    Placeholder,
}

pub(super) struct CaptureContext {
    callable: CallableId,
    captures: HashMap<(BindingOwner, VariableId), TypedCapture>,
}

#[derive(Clone, Copy)]
pub(super) enum BindingUse {
    Read,
    Write,
}

#[derive(Clone)]
pub(super) enum LexicalBinding {
    Value(VariableBinding),
    Callable(CallableBinding),
}

/// Both forms borrow their original body. Synthesized parameters have no lexical
/// spelling: placeholder reads resolve by raw-node identity, not a source name.
#[derive(Clone, Copy)]
pub(super) enum ClosureSource<'a> {
    Explicit(&'a Lambda),
    Placeholder {
        expression: &'a Expr,
        span: Span,
        params: &'a [placeholders::Parameter],
    },
}

#[derive(Clone, Copy)]
enum Body<'a> {
    Expression(&'a Expr),
    Block(&'a ValueBlock),
}

impl<'a> ClosureSource<'a> {
    fn identity(self) -> usize {
        match self {
            Self::Explicit(lambda) => std::ptr::from_ref(lambda).addr(),
            Self::Placeholder { expression, .. } => std::ptr::from_ref(expression).addr(),
        }
    }

    fn span(self) -> Span {
        match self {
            Self::Explicit(lambda) => lambda.span,
            Self::Placeholder { span, .. } => span,
        }
    }

    fn body_span(self) -> Span {
        match self {
            Self::Explicit(lambda) => lambda.body_span,
            Self::Placeholder { span, .. } => span,
        }
    }

    fn body(self) -> Body<'a> {
        match self {
            Self::Explicit(lambda) => match &lambda.body {
                LambdaBody::Expression(expr) => Body::Expression(expr),
                LambdaBody::Block(block) => Body::Block(block),
            },
            Self::Placeholder {
                expression: Expr::Block(block),
                ..
            } => Body::Block(block),
            Self::Placeholder { expression, .. } => Body::Expression(expression),
        }
    }
}

impl SemanticAnalyzer {
    pub(super) fn lookup_binding(&self, name: &str) -> Option<&LexicalBinding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    pub(super) fn check_value_binding(
        &mut self,
        name: &str,
        span: Span,
        usage: BindingUse,
    ) -> Result<(), SemanticError> {
        match self.lookup_binding(name) {
            Some(LexicalBinding::Callable(callable))
                if callable.origin == CallableOrigin::Placeholder =>
            {
                Err(SemanticError::InvalidCallableValueUse { span })
            }
            Some(LexicalBinding::Callable(_)) => Err(SemanticError::InvalidClosure {
                reason: match usage {
                    BindingUse::Write => "a local callable binding cannot be reassigned",
                    BindingUse::Read => {
                        "a local callable binding cannot be used as a value; invoke it directly or use a named function"
                    }
                },
                span,
            }),
            Some(LexicalBinding::Value(binding)) if binding.owner != self.owner => {
                let binding = *binding;

                let owner = binding.owner.expect("captured bindings have an owner");
                // Every intervening helper must forward the original cell, but
                // its owning helper accesses its own binding as an Identifier.
                for context in self.capture_contexts.iter_mut().rev() {
                    if owner == BindingOwner::Closure(context.callable) {
                        break;
                    }
                    context
                        .captures
                        .entry((owner, binding.id))
                        .or_insert(TypedCapture {
                            owner,
                            variable: binding.id,
                            ty: binding.ty,
                            mutability: match binding.mutability {
                                BindingMutability::Mutable => CaptureMutability::Mutable,
                                BindingMutability::ReadOnly => CaptureMutability::ReadOnly,
                            },
                        });
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub(super) fn binding_expression(&self, binding: VariableBinding, name: &str) -> TypedExprKind {
        match binding.owner {
            Some(owner) if Some(owner) != self.owner => TypedExprKind::Captured {
                owner,
                id: binding.id,
                name: name.into(),
            },
            _ => TypedExprKind::Identifier {
                id: binding.id,
                name: name.into(),
            },
        }
    }

    pub(super) fn declare_callable(
        &mut self,
        closure: &TypedClosure,
        span: Span,
        origin: CallableOrigin,
    ) -> Result<(), SemanticError> {
        let scope = self
            .scopes
            .last_mut()
            .expect("callables require a lexical scope");
        if scope.contains_key(&closure.name) {
            return Err(SemanticError::DuplicateVariable {
                name: closure.name.clone(),
                span,
            });
        }
        scope.insert(
            closure.name.clone(),
            LexicalBinding::Callable(CallableBinding {
                id: closure.id,
                params: closure.params.iter().map(|param| param.ty).collect(),
                return_type: closure.return_type,
                origin,
            }),
        );
        Ok(())
    }

    pub(super) fn analyze_lexical_call(
        &mut self,
        callable: &CallableBinding,
        args: &[CallArgument],
        span: Span,
        context: ReturnContext,
    ) -> Result<(TypedExpr, Flow), SemanticError> {
        if let Some(arg) = args.iter().find(|arg| arg.name.is_some()) {
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
                span,
            });
        }
        // Nested declarations preserve earlier external dependencies in every
        // intervening helper, but not dependencies on that helper's own children.
        self.register_callable_dependency(callable.id);
        let mut arguments = Vec::with_capacity(args.len());
        let mut flow = Flow::FALLTHROUGH;
        for (arg, expected) in args.iter().zip(&callable.params) {
            let (value, next) =
                self.analyze_expected(&arg.value, arg.span, context, Some(*expected))?;
            self.expect_type(*expected, value.ty, arg.span)?;
            flow = flow.then(next);
            arguments.push(value);
        }
        Ok((
            TypedExpr {
                kind: TypedExprKind::LexicalCall {
                    callable: callable.id,
                    args: arguments,
                    span,
                },
                ty: Self::completion_result(callable.return_type.into(), flow),
            },
            flow,
        ))
    }

    pub(super) fn register_callable_dependency(&mut self, callable: CallableId) {
        for (owner, dependencies) in &mut self.dependencies {
            if callable.0 < owner.0 && !dependencies.contains(&callable) {
                dependencies.push(callable);
            }
        }
    }

    pub(super) fn analyze_closure(
        &mut self,
        name: &str,
        name_span: Span,
        lambda: ClosureSource<'_>,
    ) -> Result<TypedClosure, SemanticError> {
        let identity = lambda.identity();
        let id = self.callable_ids[&identity];
        let owner = self.owner.expect("closure declarations have an owner");
        let next_variable = self.next_variable_id;
        let next_loop = self.next_loop_id;
        let loops = std::mem::take(&mut self.loops);
        let result_expected = self.result_expected.take();
        self.owner = Some(BindingOwner::Closure(id));
        self.dependencies.push((id, Vec::new()));
        self.capture_contexts.push(CaptureContext {
            callable: id,
            captures: HashMap::new(),
        });

        let result = (|| {
            // Named dependency scheduling supplies concrete signatures before
            // inference. Cache only the result: every visit still checks the body
            // in its current lexical context and reconstructs dependencies.
            let return_type = match self.closure_results.get(&identity).copied() {
                Some(ty) => ty,
                None => self.infer_closure_result(lambda)?,
            };
            let (params, body, _, _) =
                self.analyze_closure_body(lambda, ReturnContext::Known(return_type))?;
            self.closure_results.insert(identity, return_type);
            Ok((params, body, return_type))
        })();

        let (_, mut dependencies) = self
            .dependencies
            .pop()
            .expect("closure dependency context is active");
        dependencies.sort_unstable_by_key(|id| id.0);
        let mut captures = self
            .capture_contexts
            .pop()
            .expect("capture context is active")
            .captures
            .into_values()
            .collect::<Vec<_>>();
        // Semantic variable IDs follow declaration order (including parameters).
        // The backend independently derives order for mutable, possibly sparse IDs.
        captures.sort_unstable_by_key(|capture| {
            let owner = match capture.owner {
                BindingOwner::Function(id) => (0, id.0),
                BindingOwner::Closure(id) => (1, id.0),
            };
            (owner, capture.variable.0)
        });
        self.owner = Some(owner);
        self.next_variable_id = next_variable;
        self.next_loop_id = next_loop;
        self.loops = loops;
        self.result_expected = result_expected;
        let (params, body, return_type) = result?;
        Ok(TypedClosure {
            id,
            owner,
            name: name.into(),
            name_span,
            params,
            return_type,
            body,
            captures,
            dependencies,
            span: lambda.span(),
            body_span: lambda.body_span(),
        })
    }

    fn infer_closure_result(&mut self, lambda: ClosureSource<'_>) -> Result<Type, SemanticError> {
        let (_, _, result, flow) = self.analyze_closure_body(lambda, ReturnContext::Infer)?;
        let returned = flow.return_type().map_err(|(expected, found)| {
            SemanticError::IncompatibleInferredReturns {
                expected,
                found,
                span: lambda.body_span(),
            }
        })?;
        let has_value_return = match lambda {
            ClosureSource::Explicit(lambda) => inference::closure_has_value_return(lambda),
            ClosureSource::Placeholder { expression, .. } => {
                inference::expression_has_value_return(expression, lambda.body_span())
            }
        };
        let no_tail = matches!(lambda.body(), Body::Block(block) if block.tail.is_none());
        if no_tail && flow.can_fall_through() && returned.is_some_and(|ty| ty != Type::Void) {
            return Err(SemanticError::MissingReturn {
                name: "<closure>".into(),
                span: lambda.body_span(),
            });
        }
        let result = if let Some(returned) = returned {
            self.join_results(returned.into(), result, lambda.body_span())?
        } else {
            result
        };
        match result {
            ExprResult::Value(ty) => Ok(ty),
            ExprResult::Void if !has_value_return => Ok(Type::Void),
            ExprResult::Never if no_tail && !has_value_return => Ok(Type::Void),
            _ => Err(SemanticError::CannotInferClosureResult {
                span: lambda.body_span(),
            }),
        }
    }

    fn analyze_closure_body(
        &mut self,
        lambda: ClosureSource<'_>,
        context: ReturnContext,
    ) -> Result<(Vec<TypedParam>, TypedClosureBody, ExprResult, Flow), SemanticError> {
        self.next_variable_id = 0;
        self.next_loop_id = 0;
        self.scopes.push(HashMap::new());
        self.placeholder_bindings.push(HashMap::new());
        let result = (|| {
            let mut params = Vec::new();
            if let ClosureSource::Explicit(lambda) = lambda {
                params.reserve(lambda.params.len());
                for param in &lambda.params {
                    self.check_variable_type(&param.name, param.ty, param.span)?;
                    if param.default.is_some() {
                        return Err(SemanticError::InvalidClosure {
                            reason: "closure parameters cannot have defaults",
                            span: param.span,
                        });
                    }
                    let id = self.declare_variable(
                        &param.name,
                        param.ty,
                        BindingMutability::Mutable,
                        param.span,
                    )?;
                    params.push(TypedParam {
                        id,
                        name: param.name.clone(),
                        ty: param.ty,
                        span: param.span,
                    });
                }
            } else if let ClosureSource::Placeholder {
                params: anonymous, ..
            } = lambda
            {
                params.reserve(anonymous.len());
                for param in anonymous {
                    let id = VariableId(self.next_variable_id);
                    self.next_variable_id += 1;
                    self.placeholder_bindings
                        .last_mut()
                        .expect("body context is active")
                        .insert(
                            param.identity,
                            VariableBinding {
                                owner: self.owner,
                                id,
                                ty: param.ty,
                                mutability: BindingMutability::ReadOnly,
                            },
                        );
                    params.push(TypedParam {
                        id,
                        name: "<placeholder>".into(),
                        ty: param.ty,
                        span: param.span,
                    });
                }
            }
            let (body, ty, flow) = match lambda.body() {
                Body::Expression(expr) => {
                    let (expr, flow) = self.analyze_expr(expr, lambda.body_span(), context)?;
                    if let ReturnContext::Known(expected) = context {
                        self.expect_type(expected, expr.ty, lambda.body_span())?;
                    }
                    let ty = expr.ty;
                    (TypedClosureBody::Expression(Box::new(expr)), ty, flow)
                }
                Body::Block(block) => {
                    let (body, ty, flow) =
                        self.analyze_value_block(block, context, ExprContext::Value)?;
                    if let (ReturnContext::Known(expected), Some(tail)) = (context, &body.tail) {
                        self.expect_type(expected, tail.ty, lambda.body_span())?;
                    }
                    (TypedClosureBody::Block(body), ty, flow)
                }
            };
            Ok((params, body, ty, flow))
        })();
        self.placeholder_bindings.pop();
        self.scopes.pop();
        self.loops.clear();
        result
    }
}
