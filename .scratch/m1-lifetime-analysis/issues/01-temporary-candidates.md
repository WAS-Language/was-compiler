## What to build

Implement the first useful, private M1 lifetime analysis: explicitly invoked tests identify directly let-bound `number[]` and `bool[]` literals as temporary candidates in the confirmed straight-line subset. Candidate status changes no allocation, emitted bytes or runtime behavior and never authorizes reclamation.

Source of truth: `docs/was_compiler_memory_and_optimization_roadmap.md`, §5, **M1 interview decisions (2026-10-26; confirmed)**. The user approved this four-ticket breakdown after confirming the contract. Creating these tickets is not an instruction to start implementation. M2 remains unauthorized.

## Acceptance criteria

- [ ] Private borrowed metadata distinguishes allocation origin, temporary candidate, identified escape path and unproven outcomes with typed reasons; no public AST fields/IDs, ownership annotations or API exports.
- [ ] At least one positive case for each of `number[]` and `bool[]`; cover empty literals, scalar parameters/locals, scalar arithmetic/comparisons/unary operators, indexing, builtin `len`, element replacement, all numeric element compounds, scalar/bare returns, and supported nested element/index/RHS expressions.
- [ ] Positive origins are directly bound to one ordinary let, with no handle copies or reassignment. The entire allocating function body must satisfy the closed subset. No last-use, fragment, interprocedural or general alias proof.
- [ ] Block expressions, branches, loops, short-circuiting, callable declarations, calls except builtin `len`, other handle uses and unknown forms conservatively block positive proof, including unreachable syntax. A generic scalar-result expression is not sufficient evidence that its operands are supported.
- [ ] The interface can support later conservative reason coverage without ever accepting unsupported forms in this ticket. Do not claim actual host retention merely from a possible-retention boundary.
- [ ] No production invocation or emission consumer; no preset change, allocator change, AST rewrite/clone, public checked-tree contract or additional IR/dependency. Existing backend validation remains independent.
- [ ] Focused tests and native tests/Clippy pass offline, with no blanket dead-code/lint suppression to hide unfinished boundaries.

## Implementation handoff

- **Main files:** proposed new `crates/compiler/src/lifetime.rs` and private unit-test children own the classifier and borrowed facts; `crates/compiler/src/lib.rs` declares the private module. `crates/compiler/src/typed_ast.rs`: `TypedProgram`, `TypedFunction`, `TypedStatementKind`, `TypedExprKind`, `VariableId`, `FunctionRef::Builtin(BuiltinFunction::Len)` are read-only input contracts, not fields to extend. `codegen/representation.rs` is a private-facts precedent only; do not merge lifetime and numeric policies.
- **Read first:** `AGENTS.md`; `docs/agents/domain.md`; `GLOSSARY.md` (Temporary candidate); roadmap §5 confirmed decisions override the illustrative StorageClass and broader future prose; `docs/memory-abi.md` revision 1; `docs/m0-memory-baseline.md`; ADRs `0004-local-callable-ownership-and-storage.md`, `0006-typed-array-layout-and-lazy-element-validation.md`, `0007-proven-integer-specialization.md` under `docs/adr/`.
- **Test anchors:** `crates/compiler/tests/typed_arrays.rs`: `numeric_public_migration_preserves_compilation`, `both_element_types_preserve_exact_joins_whitelist_and_numeric_only_compounds`; `crates/compiler/src/codegen/tests/compound_assignment.rs`: `indexed_compounds_decode_once_only_address_and_old_load_before_rhs_for_all_operators`. Add private tests with `lifetime` in their module/filter so no public access is needed. These anchors describe current behavior, not existing lifetime assertions.
- **Validation:** `cargo test --offline -p was-compiler --lib lifetime`; `cargo test --offline`; `cargo clippy --offline --all-targets -- -D warnings`. Format only actual edited Rust paths with `rustfmt --edition 2024 --config skip_children=true`; finish `git --no-pager diff --check`. Expected: both positive types identified, excluded forms never positive, existing behavior unchanged.
- **Direction and open questions:** no product-scope questions remain. Internal result/key layout is implementation discretion, provided borrowed-tree lifetime, deterministic source order and owner isolation hold. Lookup addresses/IDs must not determine emitted or report order. Contracts and nested/helper syntax must not become unchecked ways around the closed proof; inspect their typed structure conservatively. Avoid wide prefactoring; scaffolding and first useful proof land together. Arrange truly test-only entry/use appropriately without adding a runtime consumer merely to silence dead code.
- **Suggested model:** `openai-subscribed/gpt-6.1-sol`.
- **Thinking effort:** high, provisional; the catalog confirms the model but not configurable effort levels.
- **Rationale:** establishing the bounded proof requires cross-node correctness reasoning; narrow scope avoids a general escape-analysis framework. No current-price comparison is asserted.
- **Escalation trigger:** stop and replan if positive proof requires public AST changes, production wiring, interprocedural reasoning, new host obligations, or widening the confirmed operation set. Missing offline prerequisites are prerequisites, not a reason to switch models.

## Blocked by

None (can start immediately).
