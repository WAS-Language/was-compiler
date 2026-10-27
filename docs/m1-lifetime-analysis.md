# M1 lifetime analysis — implemented contract and validation

**M1 implemented and integrated validation passed: 2026-10-26 (#50–#53).**
The confirmed contract is in
[roadmap §5](was_compiler_memory_and_optimization_roadmap.md#5-milestone-m1--conservative-escape-and-lifetime-analysis).
This milestone implements private, explicitly test-invoked classification only.
It changes no storage or production compilation behavior. #54 adds a separate
[private M2 storage consumer](m2-region-prototype.md); public activation, general
reclamation and managed-heap support remain unauthorized.

## Invocation and facts

`crates/compiler/src/lifetime.rs` is compiled only under `cfg(test)` and explicitly
invoked by its unit tests and the explicitly experimental M2 compilation seam.
Ordinary compilation and emission never call it. Facts borrow
the original function and literal, retaining the function-local binding identity
and static element type. Functions and direct body declarations determine report
order; hash maps are used only for binding lookup. No public AST fields, IDs,
exports, dependencies or storage-selection consumers are added.

The first slice reports **direct function-body let-bound array literals only**.
Nested origins, nonliteral initializers, capture cells and environments are not
inventoried. Their enclosing unsupported forms still block the invocation-wide
positive proof. This is intentionally not a complete allocation inventory.

Outcomes distinguish temporary candidates, identified outward handle paths, and
unproven results with typed reasons. Returns and host arguments are identified
structurally even in unreachable syntax; these are potential paths, not claims
that execution reaches them. A host call is an observation boundary, not evidence
that the host actually retained any particular allocation. Issue #51 adds the
explanation traversal below without widening #50's positive proof.

## Closed positive subset

Only single-binding `number[]` and `bool[]` literals qualify. The entire allocating
function must stay in the subset, including syntax after a return and parameter
contracts. Supported operations are scalar number/bool parameters and ordinary
locals, scalar literals, arithmetic/comparisons/unary operators, array element
reads, builtin `len`, checked element replacement, all five numeric element
compounds, and scalar/bare returns. Element initializers, indices and right-hand
sides are checked recursively. Empty arrays and potentially trapping operations
are allowed; candidate status does not imply trap freedom.

Aliases, handle reassignment, scalar writes, other handle uses/types, non-builtin
calls, normalized/lexical calls, captures/callable declarations, block expressions,
branches, match, loops and short-circuiting cannot earn a positive result. Scalar
result annotations never excuse unsupported operands. Sparse binding IDs work by
lookup; duplicate or missing bindings and inconsistent supported annotations are
unproven. This is not a replacement checked-tree API: existing backend validation
remains independent and may reject a forged tree regardless of classification.

Candidate status changes no allocation, bytes, runtime behavior, ABI or numeric
preset. It never authorizes heap rewind, early cleanup or reclamation after traps.
Existing [ownership and lifetime guarantees](memory-abi.md#ownership-and-lifetime)
remain unchanged. No performance or reclaimed-memory claim is made.

## Conservative explanations (#51)

The closed positive proof remains separate from the exhaustive structural
explanation traversal in `crates/compiler/src/lifetime/diagnostics.rs`. A failed
proof never stops explanation collection. Every fact retains a `reasons` vector:

- Structural preorder, contracts before body, statements and expression children
  in their stored order. Form-level reasons precede their children. Return/host
  publication evidence appears at the corresponding handle leaf. Repeated reasons
  are retained; this is syntax order, not a claim about executed effects.
- Normalized bindings are inspected in evaluation order, then an immediate helper
  declaration/body (when present), then the canonical call. Named/default calls,
  lexical stages, immediate stages and recursion remain call barriers. No callee
  summaries or call-result provenance are inferred.
- If absent from that vector, the first closed-proof failure is appended last.
  Duplicate owner-local declarations or malformed closure ownership suppress
  escape evidence and append an inconsistency reason. Hash iteration never
  determines report order.
- The summary `outcome` uses deterministic precedence: returned handle, potential
  host publication, then the original closed-proof result. The summary is not a
  replacement for the multiple-reason vector; a later return never hides an
  earlier host path, or vice versa.

Path evidence follows only direct literal bindings and immutable, direct let-copy
chains (including invocation-local normalized argument bindings). A preliminary
structural inventory excludes **every** written binding, even if the write is
later, skipped, unreachable or inside a capturing helper. Aliases without an
identified outward path receive `AliasCreation`, not an escape claim. Reassignment
receives `HandleReassignment`; unsupported or mutable propagation stays unproven.
This is bounded syntactic copy evidence, not general alias analysis, liveness,
branch refinement or an expanded candidate set.

Return/host publication context passes through block tails, if branches and match
arms, but not arbitrary operations, loop results or call results. Nested returns,
normalized bindings, array elements, indices and right-hand sides are still
visited. A scalar-only host call receives `HostObservation` even before an origin
or after apparent last use: the proof is invocation-wide, and exported memory
and reentry prevent a no-observation inference from absent handle arguments.

Keys are `(BindingOwner, VariableId)`, never numeric variable IDs alone. Callable
declarations block the creator proof even when unused; captures and forwarded
contexts receive `CaptureStorage`. Helper bodies are inspected but do not produce
creator origins, and helper returns alone are not returns from the allocating
invocation. Explicit original-owner captured arguments to hosts can identify
potential publication even within an uninvoked helper's syntax. This is not an
assertion that a declaration executes its body. String arrays, ranges, capture
cells/environments and unsupported handle operations remain outside the positive
set and the existing direct-body origin inventory remains unchanged.

No production consumer, public API/AST/ABI, numeric policy, allocation or emission
changes are introduced. #50 established the positive proof, #51 added conservative
explanations without widening it, and #52 added noninterference tests. Their
historical subset results are not the integrated evidence recorded below for #53.

## Contract-to-test map

All names below refer to actual tests, freshly passed in the integrated run.
Private files are under [`crates/compiler/src/`](../crates/compiler/src/).
The `lifetime` filter selects 42 tests: 39 native classification/lowering tests
and three ignored-by-default Node runtime tests. For a focused rerun use
`cargo test --offline -p was-compiler --lib lifetime -- --include-ignored`.
The #53 full native run executed all 42, not just the default 39.

| Contract | Tests and observation |
| --- | --- |
| Both positive types, empty arrays, scalar parameters/locals, nested elements/indices/RHSs, reads, `len`, replacement and scalar/bare returns | [`lifetime.rs`](../crates/compiler/src/lifetime.rs): `lifetime_direct_numeric_literal_is_a_candidate`, `lifetime_boolean_empty_and_nested_scalar_operations` |
| Every supported arithmetic/comparison operator and all five numeric element compounds | `lifetime.rs`: `lifetime_each_scalar_operator_and_numeric_element_compound` |
| Invocation-wide closed subset, including after return; scalar writes, string arrays/ranges and other handles excluded | `lifetime.rs`: `lifetime_excluded_forms_block_the_entire_invocation_even_after_return`, `lifetime_scalar_writes_are_not_implicitly_added_to_the_closed_set`, `lifetime_contracts_and_handle_parameters_cannot_bypass_the_subset` |
| Returned handles versus aliases/reassignment without proven publication; later evidence survives earlier unsupported forms | `lifetime.rs`: `lifetime_alias_return_is_an_identified_path_not_just_an_unknown_alias`, `lifetime_aliases_and_reassignments_are_not_automatic_escape_evidence`, `lifetime_reports_later_host_and_return_paths_after_unsupported_syntax`, `lifetime_returned_block_tail_preserves_an_identifiable_path` |
| Host observation before allocation/after apparent last use; potential publication, skipped/unreachable/nested children; contracts/defaults | `lifetime.rs`: `lifetime_host_barriers_are_invocation_wide_and_publication_is_only_potential`, `lifetime_nested_and_skipped_children_do_not_hide_host_paths`, `lifetime_contracts_and_normalized_defaults_are_structurally_inspected` |
| Internal/named/default/lexical calls, recursion, known/placeholder/immediate stages remain barriers | [`lifetime/conservative_tests.rs`](../crates/compiler/src/lifetime/conservative_tests.rs): `conservative_recursive_calls_do_not_supply_interprocedural_lifetime_proofs`, `conservative_named_and_default_calls_retain_call_boundary_reasons`, `conservative_handle_arguments_to_internal_calls_are_not_positive_proofs`, `conservative_lexical_calls_and_known_pipeline_stages_are_boundaries`, `conservative_placeholder_declarations_remain_barriers_even_when_unused`, `conservative_immediate_and_placeholder_stages_do_not_expand_positive_subset` |
| Owner isolation, nested/forwarded captures, unused helpers/context dependencies, sparse/duplicate IDs | `lifetime/conservative_tests.rs`: `conservative_nested_helpers_forward_original_capture_without_root_escape`, `conservative_unused_helper_return_of_capture_is_not_root_return`, `conservative_unused_lexical_dependency_keeps_capture_storage_barrier`, `conservative_helper_local_ids_do_not_alias_root_bindings`, `conservative_sparse_captured_binding_keeps_original_owner_identity`, `conservative_sparse_binding_ids_preserve_candidate_origin`, `conservative_duplicate_helper_ids_are_not_hidden_by_owner_boundary`, `conservative_duplicate_root_ids_after_callable_barrier_are_inconsistent` |
| Deterministic fact order, reasons, borrowed identity and unchanged input | `lifetime.rs`: `lifetime_fact_order_and_binding_identity_are_owner_local`, `lifetime_reports_later_host_and_return_paths_after_unsupported_syntax`; repeated analysis and tree equality in both [`lifetime/noninterference_tests.rs`](../crates/compiler/src/lifetime/noninterference_tests.rs) tests |
| Before/after byte identity separately within None/Basic/Standard/Aggressive, including excluded programs; valid Wasm plus independent decoded widths/alignment/helper counts/numeric lowering | `lifetime.rs`: `lifetime_analysis_preserves_bytes_and_tree_at_each_preset`; `lifetime/noninterference_tests.rs`: `lifetime_candidates_preserve_typed_widths_helpers_and_numeric_preset_lowering`, `lifetime_excluded_programs_preserve_bytes_facts_and_tree_at_each_preset`; existing [`typed_arrays_lowering.rs`](../crates/compiler/tests/typed_arrays_lowering.rs): `typed_widths_and_helper_families_are_static_deterministic_and_lazy` |
| Forged annotations do not bypass independent backend checks; before/after errors retain expected variants/messages/spans and first-error order at each preset | `lifetime.rs`: `lifetime_forged_annotations_remain_unproven_and_backend_checked`; all four [`lifetime/forged_tests.rs`](../crates/compiler/src/lifetime/forged_tests.rs) tests: `lifetime_forged_array_index_and_len_metadata_preserve_independent_errors`, `lifetime_forged_len_arity_precedes_hidden_malformed_arguments`, `lifetime_forged_short_circuit_operands_remain_backend_checked`, `lifetime_forged_unreachable_and_skipped_nested_operands_remain_backend_checked` |
| Potentially trapping supported accesses remain candidates; completed writes/allocations survive traps, aliases remain shared, reentry retains distinct live storage | All three [`lifetime/runtime_tests.rs`](../crates/compiler/src/lifetime/runtime_tests.rs) tests: `lifetime_candidates_can_trap_without_rolling_back_writes_or_allocations`, `lifetime_aliases_across_user_calls_remain_conservative_and_shared`, `lifetime_host_barrier_reentry_and_trap_preserve_distinct_live_arrays`; each executes before/after modules separately at every preset with independent expected effects and Wasm validation |
| Existing raw-host retention/reentry/trap and typed-array contracts preserved | All six [`typed_arrays_runtime.rs`](../crates/compiler/tests/typed_arrays_runtime.rs) tests, including `construction_captures_reentry_and_traps_keep_completed_effects` and `raw_host_retained_typed_arrays_survive_recursive_reentry_traps_and_rhs_growth` |
| CLI/core bytes, selected exports, diagnostics/output safety and raw-host execution; current-source browser/core byte/diagnostic/execution parity within presets | All four [`cli/tests/typed_arrays_adapters.rs`](../crates/cli/tests/typed_arrays_adapters.rs) tests, including `cli_typed_array_hosts_selected_exports_and_contextual_source_match_core`, `cli_typed_array_diagnostics_match_core_without_touching_output`, `cli_typed_arrays_execute_with_raw_host_layouts`; [`browser/tests/typed_arrays_adapters.rs`](../crates/browser/tests/typed_arrays_adapters.rs): `rebuilt_browser_typed_array_hosts_bytes_diagnostics_and_execution_match_core` |
| Browser smoke against rebuilt package | [`browser/tests/browser.rs`](../crates/browser/tests/browser.rs): `node_browser_compiler` invokes [`tests/browser.cjs`](../tests/browser.cjs) and requires its 25-test completion summary |

Adapter tests exercise existing public APIs; they neither expose nor invoke the
private classifier. Byte noninterference is proved at the private borrowed-tree
seam, not inferred from adapter parity or from Wasm validity alone. None-to-enabled
preset byte identity is not required; enabled-preset parity and experimental
numeric-policy status remain unchanged.

## Integrated validation — 2026-10-26 (#53)

Stable implementation revision: `4416e3d9684b0034e3230809b9ab3a760f245326`.
The working tree was clean at the start. Only documentation/evidence changed
after validation; compiler, tests, runtime, adapters, fixtures and dependencies
were unchanged throughout. Every command below ran from the repository root in
the listed order, with a 180-second bound per tool invocation (the three fast
commands after the initial test shared one bound). All exited 0 without retries
or timeouts. No prerequisite or unexplained failure remains pending.

| Command | Actual result |
| --- | --- |
| `cargo test --offline` | 1,184 passed, 208 ignored, none failed; 69 unit/integration targets plus one empty doctest target. Not runtime/browser completion evidence. |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed |
| `cargo test --offline -p was-compiler --test workspace_contract` | 3 passed, none failed/ignored |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed; fresh current-source package before browser tests |
| `cargo test --offline -p was-compiler-browser --tests -- --include-ignored` | 13 passed, none failed/ignored; 11 unit/integration targets including an empty library target. Includes the 25-group browser smoke and typed-array adapter parity. |
| `cargo test --offline -- --include-ignored` | 1,392 passed, none failed/ignored; 69 unit/integration targets plus one empty doctest target. Includes all 42 private lifetime tests, six typed-array runtime tests, two typed-array lowering tests and four CLI typed-array adapter tests. |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed |
| `git --no-pager diff --check` | Passed after the sequence; repeated after documentation edits |

This used the separate browser/default-member route, not the alternative
workspace-wide invocation: **1,405 tests passed across both full test runs**,
none failed or ignored. The 25 browser smoke groups are contained in one Rust
test and are not added again to that count. All ignored runtime cases were
explicitly included. Existing ignored-test policy is unchanged.

Tools: rustc `1.98.0 (88d9e12ae 2026-08-18)`, Cargo
`1.98.0 (797e8a9bc 2026-08-05)`, Node `v24.12.0`, wasm-pack `0.15.0`,
Windows native target `x86_64-pc-windows-msvc`; `wasm32-unknown-unknown` was installed.
`wasm-bindgen` was absent from PATH; the compatible `0.2.129` executable was
verified in `AppData/Local/.wasm-pack/wasm-bindgen-06a9b89f25ab66ba/`, matching
`Cargo.lock`. The no-install build succeeded using wasm-pack tooling resolution.
Its log says “Installing wasm-bindgen”; no separate installer or fetch command
was invoked. The only packaging notice concerned optional missing `description`,
`repository` and `license` fields. Dependencies and tooling were not upgraded.

The committed [validation transcript](m1-lifetime-validation-2026-10-26.txt)
retains exact command/result summaries, named M1/adapter assertions, source and
package hashes. Complete transient logs are `target/m1-53-*.log`; the committed
record remains available independently of generated output. Regenerate `pkg/`
before future browser evidence, even when those historical hashes match.

M1's classification and noninterference gates passed. This is not M2 storage
selection, allocator integration, reclamation, managed-heap support, a numeric
release/default change, or evidence of memory/page-count/performance improvement.
No new benchmark gate was required. Raw-host instance-lifetime retention rights,
ABI revision 1 and [M0's historical evidence](m0-memory-baseline.md) remain intact;
M0 success was not substituted for these fresh M1 results.
