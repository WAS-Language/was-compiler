use super::*;

impl SemanticAnalyzer {
    pub(super) fn analyze_pipeline(
        &mut self,
        input: &Expr,
        input_span: Span,
        stages: &[PipelineStage],
        return_type: ReturnContext,
        context: ExprContext,
    ) -> Result<(TypedExpr, Flow), SemanticError> {
        let (mut value, mut flow) = self.analyze_expr(input, input_span, return_type)?;
        let mut prefix_span = input_span;

        if stages.is_empty() {
            return Err(SemanticError::InvalidPipeline {
                message: "a pipeline requires at least one stage",
                span: input_span,
            });
        }

        for (index, stage) in stages.iter().enumerate() {
            let span = stage.span;
            let name = match &stage.target {
                PipelineStageTarget::Named(name) => name,
                PipelineStageTarget::Immediate {
                    expression,
                    expression_span,
                } => {
                    if !stage.args.is_empty() {
                        return Err(SemanticError::InvalidPipeline {
                            message: "immediate pipeline stages cannot have written arguments",
                            span,
                        });
                    }
                    let params;
                    let source = match expression {
                        Expr::Lambda(lambda) => {
                            if lambda.params.len() != 1 {
                                return Err(SemanticError::InvalidPipeline {
                                    message: "immediate pipeline stage must have exactly one parameter",
                                    span,
                                });
                            }
                            closures::ClosureSource::Explicit(lambda)
                        }
                        _ if placeholders::has_owned(expression) => {
                            if placeholders::owned_parameter_count(expression) != 1 {
                                return Err(SemanticError::InvalidPipeline {
                                    message: "immediate pipeline stage must have exactly one parameter",
                                    span,
                                });
                            }
                            params =
                                self.infer_placeholder_parameters(expression, *expression_span)?;
                            closures::ClosureSource::Placeholder {
                                expression,
                                span: *expression_span,
                                params: &params,
                            }
                        }
                        _ => {
                            return Err(SemanticError::InvalidPipeline {
                                message: "pipeline stage target is not callable",
                                span,
                            });
                        }
                    };
                    let closure = self.analyze_closure("<pipeline stage>", span, source)?;
                    let expected = closure.params[0].ty;
                    self.expect_type(expected, value.ty, prefix_span)?;
                    let result_type = closure.return_type;
                    let message = if result_type != Type::Void {
                        None
                    } else if index + 1 != stages.len() {
                        Some("a nonfinal stage cannot return void")
                    } else if context != ExprContext::Discard {
                        Some("a final void stage requires a discarded result")
                    } else {
                        None
                    };
                    if let Some(message) = message {
                        return Err(SemanticError::InvalidPipeline { message, span });
                    }
                    let id = VariableId(self.next_variable_id);
                    self.next_variable_id += 1;
                    let call = TypedExpr {
                        kind: TypedExprKind::LexicalCall {
                            callable: closure.id,
                            args: vec![TypedExpr {
                                kind: TypedExprKind::Identifier {
                                    id,
                                    name: String::new(),
                                },
                                ty: expected.into(),
                            }],
                            span,
                        },
                        ty: result_type.into(),
                    };
                    value = TypedExpr {
                        kind: TypedExprKind::NormalizedCall(TypedNormalizedCall {
                            immediate_closure: Some(closure),
                            bindings: vec![TypedCallBinding {
                                id,
                                ty: expected,
                                value,
                                origin: CallBindingOrigin::PipelineInput,
                                span: prefix_span,
                            }],
                            formal_to_binding: vec![id],
                            call: Box::new(call),
                        }),
                        ty: Self::completion_result(result_type.into(), flow),
                    };
                    prefix_span = Span::new(input_span.start(), span.end());
                    continue;
                }
            };
            if let Some(binding) = self.lookup_binding(name).cloned() {
                match binding {
                    LexicalBinding::Callable(callable) => {
                        if stage.args.iter().any(|arg| arg.name.is_some()) {
                            let arg = stage.args.iter().find(|arg| arg.name.is_some()).unwrap();
                            return Err(SemanticError::NamedArgumentsNotSupported {
                                name: "<closure>".into(),
                                span: arg.span,
                            });
                        }
                        let supplied = stage.args.len() + 1;
                        if supplied != callable.params.len() {
                            return Err(SemanticError::ArgumentCountMismatch {
                                name: "<closure>".into(),
                                expected: callable.params.len(),
                                found: supplied,
                                span,
                            });
                        }
                        self.register_callable_dependency(callable.id);
                        let mut bindings = Vec::with_capacity(callable.params.len());
                        let mut formal_to_binding = Vec::with_capacity(callable.params.len());
                        let mut stage_flow = flow;
                        let expected = callable.params[0];
                        self.expect_type(expected, value.ty, prefix_span)?;
                        let id = VariableId(self.next_variable_id);
                        self.next_variable_id += 1;
                        bindings.push(TypedCallBinding {
                            id,
                            ty: expected,
                            value,
                            origin: CallBindingOrigin::PipelineInput,
                            span: prefix_span,
                        });
                        formal_to_binding.push(id);
                        for (argument, (arg, expected)) in stage
                            .args
                            .iter()
                            .zip(callable.params.iter().skip(1))
                            .enumerate()
                        {
                            let (value, next) = self.analyze_expected(
                                &arg.value,
                                arg.span,
                                return_type,
                                Some(*expected),
                            )?;
                            self.expect_type(*expected, value.ty, arg.span)?;
                            stage_flow = stage_flow.then(next);
                            let id = VariableId(self.next_variable_id);
                            self.next_variable_id += 1;
                            bindings.push(TypedCallBinding {
                                id,
                                ty: *expected,
                                value,
                                origin: CallBindingOrigin::Explicit { argument },
                                span: arg.span,
                            });
                            formal_to_binding.push(id);
                        }
                        let args = formal_to_binding
                            .iter()
                            .zip(&callable.params)
                            .map(|(id, ty)| TypedExpr {
                                kind: TypedExprKind::Identifier {
                                    id: *id,
                                    name: String::new(),
                                },
                                ty: (*ty).into(),
                            })
                            .collect();
                        let result_type = callable.return_type;
                        let call = TypedExpr {
                            kind: TypedExprKind::LexicalCall {
                                callable: callable.id,
                                args,
                                span,
                            },
                            ty: result_type.into(),
                        };
                        let stage_value = TypedExpr {
                            kind: TypedExprKind::NormalizedCall(TypedNormalizedCall {
                                immediate_closure: None,
                                bindings,
                                formal_to_binding,
                                call: Box::new(call),
                            }),
                            ty: Self::completion_result(result_type.into(), stage_flow),
                        };
                        if result_type == Type::Void && index + 1 != stages.len() {
                            return Err(SemanticError::InvalidPipeline {
                                message: "a nonfinal stage cannot return void",
                                span,
                            });
                        }
                        if result_type == Type::Void && context != ExprContext::Discard {
                            return Err(SemanticError::InvalidPipeline {
                                message: "a final void stage requires a discarded result",
                                span,
                            });
                        }
                        value = stage_value;
                        flow = stage_flow;
                        prefix_span = Span::new(input_span.start(), span.end());
                        continue;
                    }
                    LexicalBinding::Value(_) => {
                        return Err(SemanticError::NotCallable {
                            name: name.clone(),
                            span,
                        });
                    }
                }
            }
            if self.lookup_variable(name).is_some() {
                return Err(SemanticError::NotCallable {
                    name: name.clone(),
                    span,
                });
            }
            let signature =
                self.functions
                    .get(name)
                    .ok_or_else(|| SemanticError::UndefinedFunction {
                        name: name.clone(),
                        span,
                    })?;
            if !matches!(signature.reference, FunctionRef::User(_)) {
                return Err(SemanticError::InvalidPipeline {
                    message: "stages must name user-defined functions",
                    span,
                });
            }
            if signature.params.is_empty() {
                return Err(SemanticError::InvalidPipeline {
                    message: "a stage requires an input parameter",
                    span,
                });
            }
            let result_type = signature.return_type;
            let (stage_value, stage_flow) = self.analyze_call_bindings(
                name,
                &stage.args,
                span,
                return_type,
                Some((value, prefix_span, flow)),
            )?;
            if result_type == Type::Void {
                let message = if index + 1 != stages.len() {
                    Some("a nonfinal stage cannot return void")
                } else if context != ExprContext::Discard {
                    Some("a final void stage requires a discarded result")
                } else {
                    None
                };
                if let Some(message) = message {
                    return Err(SemanticError::InvalidPipeline { message, span });
                }
            }
            value = stage_value;
            flow = stage_flow;
            prefix_span = Span::new(input_span.start(), span.end());
        }

        Ok((value, flow))
    }

    pub(super) fn analyze_bound_call(
        &mut self,
        name: &str,
        args: &[CallArgument],
        span: Span,
        return_type: ReturnContext,
    ) -> Result<(TypedExpr, Flow), SemanticError> {
        let signature = &self.functions[name];

        if !matches!(signature.reference, FunctionRef::User(_)) {
            return Err(SemanticError::NamedArgumentsNotSupported {
                name: name.into(),
                span: args
                    .iter()
                    .find(|arg| arg.name.is_some())
                    .expect("non-user bound calls require a named argument")
                    .span,
            });
        }

        self.analyze_call_bindings(name, args, span, return_type, None)
    }

    pub(super) fn map_call_arguments(
        &self,
        name: &str,
        args: &[CallArgument],
        span: Span,
        injected: bool,
    ) -> Result<Vec<usize>, SemanticError> {
        let signature = &self.functions[name];
        let mut written_to_formal = Vec::with_capacity(args.len());
        let mut supplied = vec![false; signature.params.len()];
        let mut named = false;
        let mut positional = 0;

        if injected {
            supplied[0] = true;
            positional = 1;
        }

        for arg in args {
            let formal = if let Some(argument) = &arg.name {
                named = true;
                signature
                    .param_names
                    .iter()
                    .position(|formal| formal == argument)
                    .ok_or_else(|| SemanticError::UnknownArgument {
                        name: name.into(),
                        argument: argument.clone(),
                        span: arg.span,
                    })?
            } else {
                if named {
                    return Err(SemanticError::PositionalAfterNamed {
                        name: name.into(),
                        span: arg.span,
                    });
                }

                if positional >= supplied.len() {
                    return Err(SemanticError::TooManyPositionalArguments {
                        name: name.into(),
                        span: arg.span,
                    });
                }

                let formal = positional;
                positional += 1;
                formal
            };

            if supplied[formal] {
                return Err(SemanticError::DuplicateArgument {
                    name: name.into(),
                    argument: signature.param_names[formal].clone(),
                    span: arg.span,
                });
            }

            supplied[formal] = true;
            written_to_formal.push(formal);
        }

        if let Some(formal) = supplied
            .iter()
            .enumerate()
            .position(|(formal, supplied)| !supplied && signature.defaults[formal].is_none())
        {
            return Err(SemanticError::MissingArgument {
                name: name.into(),
                argument: signature.param_names[formal].clone(),
                span,
            });
        }

        Ok(written_to_formal)
    }

    fn analyze_call_bindings(
        &mut self,
        name: &str,
        args: &[CallArgument],
        span: Span,
        return_type: ReturnContext,
        injection: Option<(TypedExpr, Span, Flow)>,
    ) -> Result<(TypedExpr, Flow), SemanticError> {
        let written_to_formal = self.map_call_arguments(name, args, span, injection.is_some())?;
        let signature = &self.functions[name];
        let reference = signature.reference;
        let result_type = signature.return_type;
        let formal_count = signature.params.len();
        let mut bindings = Vec::with_capacity(formal_count);
        let mut mapping = vec![None; formal_count];
        let mut flow = Flow::FALLTHROUGH;

        if let Some((value, prefix_span, input_flow)) = injection {
            let expected = signature.params[0];
            self.expect_type(expected, value.ty, prefix_span)?;
            let id = VariableId(self.next_variable_id);
            self.next_variable_id += 1;
            bindings.push(TypedCallBinding {
                id,
                ty: expected,
                value,
                origin: CallBindingOrigin::PipelineInput,
                span: prefix_span,
            });
            mapping[0] = Some(id);
            flow = input_flow;
        }

        for (argument, (arg, formal)) in args.iter().zip(written_to_formal).enumerate() {
            let expected = self.functions[name].params[formal];
            let id = VariableId(self.next_variable_id);
            self.next_variable_id += 1;
            let (value, value_flow) =
                self.analyze_expected(&arg.value, arg.span, return_type, Some(expected))?;

            self.expect_type(expected, value.ty, arg.span)?;
            flow = flow.then(value_flow);
            bindings.push(TypedCallBinding {
                id,
                ty: expected,
                value,
                origin: CallBindingOrigin::Explicit { argument },
                span: arg.span,
            });
            mapping[formal] = Some(id);
        }

        for formal in 0..formal_count {
            if mapping[formal].is_some() {
                continue;
            }

            let signature = &self.functions[name];
            let default = signature.defaults[formal]
                .as_ref()
                .expect("structural binding checked every required formal");
            let value = default.instantiate(&mapping);
            let id = VariableId(self.next_variable_id);
            self.next_variable_id += 1;
            bindings.push(TypedCallBinding {
                id,
                ty: signature.params[formal],
                value,
                origin: CallBindingOrigin::Default { formal },
                span: default.span,
            });
            mapping[formal] = Some(id);
        }

        let signature = &self.functions[name];
        let formal_to_binding: Vec<_> = mapping
            .into_iter()
            .map(|id| id.expect("every formal has an explicit or default binding"))
            .collect();
        let final_args = formal_to_binding
            .iter()
            .enumerate()
            .map(|(formal, &id)| TypedExpr {
                kind: TypedExprKind::Identifier {
                    id,
                    name: signature.param_names[formal].clone(),
                },
                ty: signature.params[formal].into(),
            })
            .collect();
        let call = TypedExpr {
            kind: TypedExprKind::Call {
                func: reference,
                name: name.into(),
                args: final_args,
            },
            ty: result_type.into(),
        };

        Ok((
            TypedExpr {
                kind: TypedExprKind::NormalizedCall(TypedNormalizedCall {
                    immediate_closure: None,
                    bindings,
                    formal_to_binding,
                    call: Box::new(call),
                }),
                ty: Self::completion_result(result_type.into(), flow),
            },
            flow,
        ))
    }
}
