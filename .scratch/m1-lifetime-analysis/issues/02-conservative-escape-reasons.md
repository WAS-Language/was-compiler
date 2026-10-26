## What to build

Extend M1's private classifier with deterministic, testable explanations for identified escape paths and cases outside its proof, keeping the positive set unchanged. An alias alone is not proof of escape; a retaining host boundary is a potential publication path, not evidence the host actually retained a handle.

Authoritative scope: `docs/was_compiler_memory_and_optimization_roadmap.md` §5, confirmed M1 interview decisions. This is ticket 2 of the approved four-ticket plan; M2/storage changes are excluded.

## Acceptance criteria

- [ ] Cover direct handle returns, alias-mediated returns, alias creation without return, and handle reassignment. Distinguish an identified outward propagation path from a merely unproven alias case without building general alias analysis.
- [ ] Cover captures (including nested/forwarded), unused callable declarations, internal/lexical/normalized calls, named/default calls, pipelines/immediate stages and recursion. All remain outside the positive set.
- [ ] A host call blocks positive proof even without handle arguments/results, even before allocation or after apparent last use. Exported memory and reentry prevent equating no handle argument with no observation.
- [ ] Cover branches, loops, short-circuiting, block expressions, unsupported handle operations and excluded origins such as string arrays/ranges/capture storage. Unsupported/unreachable/skipped syntax cannot make candidates positive.
- [ ] Collect relevant reasons deterministically: define source-ordered multiple reasons or a documented deterministic precedence. Do not lose an identifiable return/host path solely because an earlier unsupported form caused an early exit.
- [ ] Owner boundaries are preserved; helper declarations block the parent proof, and helper origins/bindings are not conflated with creator bindings. No interprocedural summaries, liveness or expanded positive proof.
- [ ] Initial positive tests remain green; no production consumer, ABI/API/AST, numeric-policy, allocator or emission changes.

## Implementation handoff

- **Main files:** private classifier/test paths delivered by ticket 1 (proposed `crates/compiler/src/lifetime.rs` and private test children). Read-only typed anchors in `crates/compiler/src/typed_ast.rs`: `BindingOwner`, `TypedClosure`, `TypedNormalizedCall`, `TypedCallBinding`, `FunctionRef` and exhaustive expression/statement variants. `crates/compiler/src/codegen/walker.rs` (`Visitor`, `walk_function`, `walk_expression`) is structural traversal evidence, not a mandate to move analysis into codegen or merge planner policies.
- **Read first:** ticket 1's result interface and tests; `AGENTS.md`; `GLOSSARY.md`; roadmap §5 confirmed decisions; `docs/memory-abi.md` revision 1; `docs/local-callable-interface.md` capture/ownership contracts; ADRs 0004, 0006 and 0007 in `docs/adr/`. Retained raw handles have instance lifetime; descriptor validity is not management authority.
- **Test anchors:** `crates/compiler/src/codegen/memory_tests/runtime.rs`: `node_arrays_sum_len_mutation_alias_reassignment_and_fresh_literals`, `node_host_memory_roundtrips_reentrant_allocator_growth_and_reference_mutation`; `crates/compiler/tests/closures_runtime.rs`: `recursive_creators_keep_original_cells_and_nested_live_dependencies_per_invocation`, `reentrant_live_contexts_and_retained_handles_survive_inner_traps_and_growth`, `declarations_do_not_execute_bodies_and_nested_dependencies_remain_lexical`; `crates/compiler/tests/typed_arrays_runtime.rs`: `raw_host_retained_typed_arrays_survive_recursive_reentry_traps_and_rhs_growth`. Extract small source scenarios into private classification tests; do not make classification coverage require Node.
- **Validation:** `cargo test --offline -p was-compiler --lib lifetime`; `cargo test --offline`; `cargo clippy --offline --all-targets -- -D warnings`; `git --no-pager diff --check`. Expected: reasons deterministic on repeated analysis, adversarial cases conservative, positive scope unchanged. Runtime execution/noninterference gets dedicated evidence in ticket 3.
- **Direction and open questions:** reason enum names and ordered-report representation are internal choices; document ordering and test it. Check all relevant children, including contracts/normalized bindings and immediate helpers; do not confuse structural traversal with executed paths. Conservative unknown results are valid; no silent positive default. Duplicate/sparse IDs in forged trees are not permission to trust annotations.
- **Suggested model:** `openai-subscribed/gpt-6.1-sol`.
- **Thinking effort:** high, provisional; supported effort levels were not exposed by the model catalog.
- **Rationale:** owner-qualified captures and normalized calls create subtle false-positive risks despite conservative scope. No verified price comparison is asserted.
- **Escalation trigger:** conflicting reason semantics or a proposed need for general alias/call-result inference requires replanning, not scope expansion; a counterexample classified positive must be isolated before proceeding. Missing tooling is a prerequisite issue.
