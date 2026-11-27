//! Private M10 #100 proof for direct fixed numeric literals and local handle copies.
//! Input must come from source analysis. This is not a checked-tree validator or
//! a storage policy; temporary candidates may still trap.

use std::collections::HashMap;

use crate::{
    ArrayElementType, Type,
    ast::{BinaryOp, UnaryOp},
    typed_ast::{
        BuiltinFunction, FunctionRef, TypedExpr, TypedExprKind, TypedFunction, TypedProgram,
        TypedStatementKind, VariableId,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Object(VariableId);

#[derive(Debug, PartialEq)]
struct Facts<'tree> {
    function: &'tree TypedFunction,
    aliases: Vec<(VariableId, Object)>,
    // Ok proves every reported origin temporary; aliases are structural, not executed facts.
    proof: Proof,
    // Ordered possible effects of the supported prefix, not an execution trace.
    effects: Vec<(Object, Effect)>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Blocker {
    Unsupported,
    HandleReassignment,
}

fn analyze(program: &TypedProgram) -> Vec<Facts<'_>> {
    program
        .functions
        .iter()
        .map(|function| {
            let mut analysis = Analysis::default();
            let proof = analysis.check_body(function);
            Facts {
                function,
                aliases: analysis.aliases,
                proof,
                effects: analysis.effects,
            }
        })
        .collect()
}

const NUMBERS: Type = Type::Array(ArrayElementType::Number);

type Proof<T = ()> = Result<T, Blocker>;

#[derive(Default)]
struct Analysis {
    objects: HashMap<VariableId, Object>,
    aliases: Vec<(VariableId, Object)>,
    effects: Vec<(Object, Effect)>,
    after_return: bool,
}

impl Analysis {
    fn check_body(&mut self, function: &TypedFunction) -> Proof {
        if !function.contracts.is_empty()
            || function.params.iter().any(|p| !is_scalar(p.ty))
            || !(is_scalar(function.return_type) || function.return_type == Type::Void)
        {
            return Err(Blocker::Unsupported);
        }

        // Proof eligibility is structural, even after a return.
        for statement in &function.body {
            self.check_statement(&statement.kind)?;
            self.after_return |= matches!(statement.kind, TypedStatementKind::Return(_));
        }
        Ok(())
    }

    fn check_statement(&mut self, statement: &TypedStatementKind) -> Proof {
        match statement {
            TypedStatementKind::Let { id, ty, value, .. } if *ty == NUMBERS => {
                let object =
                    if let TypedExprKind::Array(items, ArrayElementType::Number) = &value.kind {
                        let object = Object(*id);
                        self.record(object, Effect::MayTrap);
                        self.record(object, Effect::Allocate);
                        self.record(object, Effect::WriteDescriptor);
                        for item in items {
                            self.check_scalar(item)?;
                            self.record(object, Effect::WriteElement);
                        }
                        object
                    } else {
                        self.resolve_object(value)?
                    };
                self.objects.insert(*id, object);
                self.aliases.push((*id, object));
                Ok(())
            }
            TypedStatementKind::Let { value, .. }
            | TypedStatementKind::Expression(value)
            | TypedStatementKind::Return(Some(value)) => self.check_scalar(value),
            TypedStatementKind::Return(None) => Ok(()),
            TypedStatementKind::Assignment { target, .. }
                if matches!(target.kind, TypedExprKind::Identifier { .. })
                    && target.ty == NUMBERS =>
            {
                Err(Blocker::HandleReassignment)
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
            _ => Err(Blocker::Unsupported),
        }
    }

    fn check_store(&mut self, object: Object, value: &TypedExpr) -> Proof {
        self.check_scalar(value)?;
        self.record(object, Effect::WriteElement);
        Ok(())
    }

    fn record(&mut self, object: Object, effect: Effect) {
        if !self.after_return {
            self.effects.push((object, effect));
        }
    }

    fn resolve_object(&self, expr: &TypedExpr) -> Proof<Object> {
        if let TypedExprKind::Identifier { id, .. } = expr.kind
            && let Some(&object) = self.objects.get(&id)
        {
            return Ok(object);
        }
        Err(Blocker::Unsupported)
    }

    fn check_index_access(&mut self, place: &TypedExpr) -> Proof<Object> {
        let TypedExprKind::Index { expr, index } = &place.kind else {
            return Err(Blocker::Unsupported);
        };
        let object = self.resolve_object(expr)?;
        self.check_scalar(index)?;
        self.record(object, Effect::MayTrap);
        self.record(object, Effect::ReadIndexMetadata);
        Ok(object)
    }

    fn check_scalar(&mut self, expr: &TypedExpr) -> Proof {
        if !expr.ty.value_type().is_some_and(is_scalar) {
            return Err(Blocker::Unsupported);
        }

        match &expr.kind {
            TypedExprKind::Number(_)
            | TypedExprKind::Bool(_)
            | TypedExprKind::Identifier { .. } => Ok(()),
            TypedExprKind::Unary {
                op: UnaryOp::Not | UnaryOp::Negate,
                expr,
            } => self.check_scalar(expr),
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
            } => {
                self.check_scalar(left)?;
                self.check_scalar(right)
            }
            TypedExprKind::Index { .. } => {
                let object = self.check_index_access(expr)?;
                self.record(object, Effect::ReadElement);
                Ok(())
            }
            TypedExprKind::Call {
                func: FunctionRef::Builtin(BuiltinFunction::Len),
                args,
                ..
            } => {
                let [array] = args.as_slice() else {
                    return Err(Blocker::Unsupported);
                };
                let object = self.resolve_object(array)?;
                self.record(object, Effect::MayTrap);
                self.record(object, Effect::ReadLengthMetadata);
                Ok(())
            }
            _ => Err(Blocker::Unsupported),
        }
    }
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

    assert!(std::ptr::eq(facts.function, function));
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
            .map(|effect| (Object(a), effect))
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
        .map(|effect| (object, effect))
    );

    use wasmparser::{Operator, Parser, Payload, Validator};

    let bytes = crate::codegen::compile(&program).unwrap();
    Validator::new().validate_all(&bytes).unwrap();
    let main = Parser::new(0)
        .parse_all(&bytes)
        .find_map(|payload| match payload.unwrap() {
            Payload::CodeSectionEntry(body) => Some(body),
            _ => None,
        })
        .unwrap();
    let accesses: Vec<_> = main
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

#[test]
fn handle_rebinding_blocks_proof_even_after_return() {
    use crate::Compiler;

    for body in ["b = a; return b[0];", "return b[0]; b = a;"] {
        let source = format!("fn main() -> number {{ let a = [1]; let b = a; {body} }}");
        let program = Compiler::new().analyze(&source).unwrap();
        assert_eq!(analyze(&program)[0].proof, Err(Blocker::HandleReassignment));
    }
}

#[test]
fn handles_without_supported_local_provenance_cannot_earn_a_proof() {
    use crate::Compiler;

    for source in [
        "fn main() -> number[] { let a = [1]; let b = a; return b; }",
        "fn main(a: number[]) -> number { let b = a; return b[0]; }",
        "fn main() -> number { let a = [1]; let b = resizable(a); return b[0]; }",
        "fn main() -> number { let a = [1]; let f = () => a[0]; return f(); }",
        "fn main() -> number { let a = [1]; return main(); }",
    ] {
        let program = Compiler::new().analyze(source).unwrap();
        assert_eq!(
            analyze(&program)[0].proof,
            Err(Blocker::Unsupported),
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
    assert_eq!(reports[0].proof, Err(Blocker::Unsupported));
    assert_eq!(reports[0].effects, analyze(&baseline)[0].effects);
}

const LEVELS: [crate::OptimizationLevel; 4] = [
    crate::OptimizationLevel::None,
    crate::OptimizationLevel::Basic,
    crate::OptimizationLevel::Standard,
    crate::OptimizationLevel::Aggressive,
];

#[test]
fn private_analysis_is_deterministic_and_preserves_ordinary_bytes() {
    let mut compiler = crate::Compiler::new();
    let program = compiler.analyze(PROBE).unwrap();
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

#[test]
#[ignore = "requires Node.js"]
fn ordinary_wasm_reads_alias_visible_writes() {
    use std::{process::Command, time::Duration};

    let script = r#"
        const assert = require('node:assert/strict');
        const bytes = require('node:fs').readFileSync(0);
        const guest = new WebAssembly.Instance(new WebAssembly.Module(bytes));
        assert.equal(guest.exports.main(), 16);
    "#;
    let mut compiler = crate::Compiler::new();
    let program = compiler.analyze(PROBE).unwrap();
    for level in LEVELS {
        compiler.set_optimization_level(level);
        let bytes = compiler.compile(&program).unwrap();
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        let output = crate::test_process::output(
            Command::new("node").args(["-e", script]),
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
