## What to build

Demonstrate that explicitly running M1 analysis has no effect on compilation, errors or guest behavior, including trapping operations and forged typed input. Candidate status is not trap freedom and never authorizes cleanup.

Authoritative scope: `docs/was_compiler_memory_and_optimization_roadmap.md` §5 confirmed M1 decisions. Ticket 3 of the approved four-ticket plan; no reclaiming allocator or production integration.

## Acceptance criteria

- [ ] Supported accesses that may trap can remain temporary candidates; no trap-freedom requirement or false implication of guaranteed normal completion is added.
- [ ] For valid representative programs, compile a borrowed tree, explicitly analyze that same tree, then compile it again under each of None/Basic/Standard/Aggressive. Bytes are identical before/after within each preset; do not require None bytes to match enabled presets.
- [ ] Facts are deterministic and the analysis leaves the input tree unchanged. Code emission and production optimization still do not invoke or consume M1.
- [ ] Validate emitted modules with `wasmparser::Validator` and retain independent decoded instruction/layout assertions for relevant cases; validity alone is not semantic evidence.
- [ ] Analyze selected forged trees and then compile them, preserving the existing error variant/message/span behavior. Include hidden/skipped/unreachable malformed operands and incorrect array/index/len metadata as applicable. Do not promise recovery for every arbitrarily malformed public tree or introduce a second backend validator.
- [ ] Bounded guest execution checks show completed writes/effects survive traps and alias/reentry behavior is unchanged. Reuse established fixtures where appropriate; classification remains conservative for call/reentry cases.
- [ ] Runtime cases retain the ignored-by-default policy and are explicitly run for this ticket. No fixture regeneration, allocator cleanup, memory reuse or changed host contract.

## Implementation handoff

- **Main files:** private analysis tests delivered by tickets 1–2, with test-only access to `Compiler::analyze`/`Compiler::compile` in `crates/compiler/src/compiler.rs`; `crates/compiler/src/optimization.rs` is a nonrewriting baseline, not a new consumer. Add narrowly scoped tests; no public access hook just to reach integration tests. Private tests can use existing bounded subprocess support via `crate::test_process` declared in `crates/compiler/src/lib.rs`.
- **Read first:** blockers' classifier/interface/reason tests; `AGENTS.md`; `GLOSSARY.md`; roadmap §5; `docs/memory-abi.md` revision 1; `docs/m0-memory-baseline.md`; `docs/validation.md`; ADRs 0004/0006/0007 in `docs/adr/`. Preserve parameter checks, trap order and instance-lifetime published storage.
- **Test anchors:** `crates/compiler/src/codegen/memory_tests/validation.rs`: `forged_memory_roots_operands_and_nested_errors_preserve_statement_spans`, `len_checks_arity_and_rejects_strings_in_source_and_typed_ast`, `indexed_assignment_checks_identifier_base_target_index_and_rhs_types`, `memory_local_initializers_remain_checked_in_unreachable_nested_blocks`; `crates/compiler/src/codegen/memory_tests/runtime.rs`: `node_all_invalid_indices_trap_and_negative_zero_is_valid_for_reads_and_writes`, `node_memory_expression_order_live_prefixes_and_assignment_bounds_before_rhs`; `crates/compiler/tests/typed_arrays_runtime.rs`: `construction_captures_reentry_and_traps_keep_completed_effects`; `crates/compiler/tests/typed_arrays_lowering.rs`: `typed_widths_and_helper_families_are_static_deterministic_and_lazy`. `tests/support/process.rs` owns bounded subprocesses.
- **Validation:** `cargo test --offline -p was-compiler --lib lifetime`; `cargo test --offline -p was-compiler --lib lifetime -- --include-ignored`; `cargo test --offline -p was-compiler --test typed_arrays_lowering`; `cargo test --offline -p was-compiler --test typed_arrays_runtime -- --include-ignored`; `cargo test --offline`; `cargo clippy --offline --all-targets -- -D warnings`; `git --no-pager diff --check`. New private tests must remain under the lifetime filter. Node-dependent missing prerequisites leave runtime evidence pending, not passed.
- **Direction and open questions:** use independent expectations for effects and decoded instructions. Compare codegen errors before/after the same explicit analysis, not only successful bytes. Existing raw-host retention suites are baseline scenarios, not permission to widen the candidate set. Runtime tests must terminate; a subprocess timeout is not guest fuel.
- **Suggested model:** `openai-subscribed/gpt-6.1-sol`.
- **Thinking effort:** high, provisional; configurable levels were not verified in the catalog.
- **Rationale:** noninterference spans mutable public typed trees, independent validation, numeric presets and observable trap effects. No price comparison is claimed.
- **Escalation trigger:** any before/after byte/error/effect difference, positive forged-metadata counterexample or required public exposure of the classifier needs root-cause diagnosis/replanning; do not change fixtures or production semantics to force success.
