# M4 resizable arrays — contract-to-test map (#72)

## Status and audit boundary

This is a **source and historical-evidence audit**, not a validation record or an
M4 acceptance decision. No tests, builds, browser rebuilds or measurements were run
for this static audit. The separate [#72 release-readiness record](m4-release-readiness.md)
reports sequential final-source gates, measurement reproduction and the release
conclusion; its execution results must not be confused with source inventories.
Existing test names below identify available evidence mechanisms, **not passing
results from this audit**. No implementation, test or compatibility fixture was
edited.

Audited starting revision: `52a28557cb41652581ed247e045ca27a6a5ed75f` (#71).
The initial `git status --short` was empty. The contracts are
[roadmap §8](was_compiler_memory_and_optimization_roadmap.md#8-milestone-m4--resizable-typed-arrays),
[ADR 0009](adr/0009-opt-in-resizable-array-storage.md),
[resizable arrays](resizable-arrays.md),
[revision-1 memory ABI](memory-abi.md) and [glossary](../GLOSSARY.md).
The [M3 acceptance map](m3-allocation-infrastructure.md) supplies inherited
allocator/noninterference boundaries, **not M4 acceptance**.

### How to read the map

Names are exact Rust test functions, not invented scenario labels. File aliases
are relative links; integration names are `<target>::<test>`. Private unit names
are `codegen::resizable::{tests,failure_tests,measurement_tests}::<test>` even
though the latter two source filenames are `resizable_*_tests.rs`.

| Alias | Actual file / source inventory at audited revision |
| --- | --- |
| F | [Frontend][F]: 46 tests, none marked ignored |
| L | [Checked and independently decoded lowering][L]: 33 tests, none marked ignored |
| R | [Emitted-Wasm runtime][R]: 38 tests, all ignored by default |
| P | [Private failure transport][P]: 5 tests, 4 ignored by default |
| M | [Private measurement transport][M]: 5 tests, 2 ignored recording modes |
| H | [Production helper-planning module][H]: 1 inline unit test |
| C | [CLI adapter][C]: 4 tests, 1 ignored Node case |
| B | [Browser adapter][B]: 2 tests, 1 ignored rebuilt-package case |

These are source inventories, not execution counts. R's shared `run` helper
compiles twice at each of None/Basic/Standard/Aggressive, requires determinism,
requires identity **among enabled presets**, validates Wasm and starts a bounded
Node child. It does not require baseline/enabled byte identity. P and M compile
through the private checked backend with baseline numeric policy; they do not
independently execute all presets. Broader preset behavior belongs to F/L/R/C/B.

[F]: ../crates/compiler/tests/resizable_arrays_frontend.rs
[L]: ../crates/compiler/tests/resizable_arrays_lowering.rs
[R]: ../crates/compiler/tests/resizable_arrays_runtime.rs
[P]: ../crates/compiler/src/codegen/resizable_failure_tests.rs
[M]: ../crates/compiler/src/codegen/resizable_measurement_tests.rs
[H]: ../crates/compiler/src/codegen/resizable.rs
[C]: ../crates/cli/tests/resizable_arrays_adapters.rs
[B]: ../crates/browser/tests/resizable_arrays_adapters.rs

## 1. Frontend, contextual inference and diagnostic contracts

All entries in this table are in [F]. Runtime resize authority is deliberately
not a frontend type/provenance proof: an ordinary array receiver typechecks and
its attempted management operation traps at runtime.

| Contract | Exact test functions |
| --- | --- |
| Constructor preserves all three array types; expected destination context passes through nested constructors, but not backward from siblings or concrete results | `constructor_preserves_each_source_array_type`; `destination_array_context_reaches_the_source_through_nested_constructors`; `destination_context_does_not_refine_siblings_or_concrete_function_results` |
| Constructor placeholder source/result equality stays exact and region-local; callers cannot select an ambiguous element kind; conflict ordering, reachable control/nested-region evidence and immediate stages | `placeholder_constructor_relates_source_and_result_exactly_within_its_region`; `placeholder_constructor_stays_ambiguous_without_region_local_element_evidence`; `placeholder_scalar_result_conflicts_before_unrelated_ambiguity`; `placeholder_constructor_reachable_evidence_composes_with_control_and_nested_regions`; `placeholder_constructor_preserves_all_exact_array_kinds_including_immediate_stages` |
| `resizable` global/host-language-name migration, unrestricted external module/field names, ordinary identifier resolution and lexical shadowing | `constructor_name_is_reserved_for_hosts_but_not_external_module_or_field_names`; `constructor_uses_ordinary_identifier_resolution_and_lexical_shadowing` |
| Constructor positional arity before child errors; pure default/contract rejection, including skipped operands; UTF-8 statement spans and preset parity | `constructor_checks_call_structure_before_argument_expressions`; `defaults_and_contracts_reject_constructor_calls_including_skipped_operands`; `constructor_diagnostics_keep_utf8_statement_spans_and_preset_parity` |
| Divergent sources preserve Never and contextual literal kind; bool/string iteration metadata and independent forged metadata/call-arity checking | `divergent_sources_preserve_never_and_contextual_literal_storage`; `divergent_constructor_iteration_retains_bool_and_string_element_metadata`; `divergent_constructor_iteration_still_checks_forged_metadata_and_call_arity` |
| Reserve accepts each array type including fixed storage, yields Void; contextual method names, identifier-only receivers, no generalized postfix syntax | `reserve_accepts_each_array_kind_and_fixed_storage_with_a_void_result`; `reserve_keeps_identifier_receivers_without_general_postfix_syntax`; `reserve_is_contextual_and_does_not_use_callable_name_resolution` |
| Reserve arity/named-argument checks before children; receiver then numeric argument; UTF-8/preset diagnostic parity | `reserve_rejects_arity_and_named_arguments_semantically_before_children`; `reserve_checks_receiver_and_numeric_argument_without_proving_runtime_authority`; `reserve_diagnostics_preserve_utf8_spans_and_optimization_parity` |
| Reserve Never and unreachable/skipped checks, pure-context exclusion, original nested captures, dependency discovery and region-local placeholder argument evidence | `reserve_preserves_never_flow_and_checks_unreachable_and_skipped_children`; `reserve_rejects_defaults_and_contracts_even_in_skipped_operands`; `reserve_receiver_uses_original_captures_in_nested_helpers`; `reserve_arguments_participate_in_named_result_dependencies_and_callable_discovery`; `reserve_placeholder_argument_infers_number_with_region_local_array_evidence` |
| Push raw receiver/argument/UTF-8 method span; exact receiver-derived item type, Void result and no implicit authority proof | `push_raw_syntax_retains_receiver_argument_and_utf8_method_span`; `push_accepts_each_exact_array_element_with_a_void_result`; `push_checks_receiver_then_exact_scalar_without_proving_runtime_authority` |
| Push contextual identifier-only surface; no chaining, callable-method resolution, method pipeline stages or new pipeline discard permission; structure before receiver/item | `push_requires_identifier_receivers_and_does_not_add_chaining_or_method_pipeline_stages`; `push_is_contextual_without_callable_resolution_or_new_pipeline_discard_permissions`; `push_checks_call_structure_before_receiver_and_item` |
| Push exact region-local ArrayElement relations, delayed evidence, no caller/unreachable refinement or region leakage | `push_placeholder_types_follow_the_exact_region_local_array_element_relationship`; `push_placeholder_evidence_does_not_come_from_callers_or_unreachable_items`; `push_placeholder_relationship_propagates_delayed_evidence_and_preserves_region_boundaries` |
| Push Never/skipped/unreachable checking, pure-context exclusion, result dependencies/captures and UTF-8/preset diagnostic parity | `push_preserves_never_and_checks_every_unreachable_or_skipped_child`; `push_is_forbidden_in_pure_defaults_and_contracts_including_skipped_operands`; `push_item_participates_in_named_result_dependencies_and_nested_capture_discovery`; `push_diagnostics_preserve_utf8_spans_and_optimization_parity` |
| Pop/remove return exact scalar; zero/one positional arity, receiver/index order, contextual identifier-only syntax, pure-context exclusion and raw UTF-8 spans | `pop_and_remove_return_each_exact_receiver_element_type`; `pop_remove_check_structure_receiver_and_index_in_order`; `pop_remove_keep_identifier_only_contextual_method_syntax_and_purity`; `pop_remove_raw_nodes_preserve_utf8_spans_and_positional_arguments` |
| Pop/remove exact region-local receiver/result/index evidence, no caller/nested-region refinement; Never/skipped/capture/result-dependency composition | `pop_remove_placeholder_results_and_indices_use_exact_region_local_evidence`; `pop_remove_preserve_never_skipped_checks_captures_and_result_dependencies` |

Parser rejection is exercised by invalid receiver/chaining/pipeline source strings
in these tests and adapter diagnostics, not by a separate M4 parser target.
`pop_remove_keep_identifier_only_contextual_method_syntax_and_purity` has some
broad `is_err()` syntax assertions; not every rejection pins an exact parser error
variant/message. Preserve that distinction from the exact-error cases.

## 2. Checked lowering, planning and publication order

All entries are in [L] unless H is specified. L's `operators` helper runs
`wasmparser::Validator` before decoding. The independent assertions include
literal helper indices, signatures, offsets, widths, call lists and instruction
ordering; module validity alone is not the semantic proof.

| Contract | Exact test functions and assertion boundary |
| --- | --- |
| Mutable public typed constructor nodes: arity, operands and result annotations checked even with no exports; independently reject pure defaults/skipped contracts | `forged_constructor_arity_operands_and_annotations_are_checked_even_unexported`; `forged_constructor_is_rejected_independently_in_skipped_contract_and_default` |
| Structural constructor discovery through Never, skipped/unreachable code, nested/immediate helpers and nesting | `never_skipped_nested_and_immediate_callable_constructors_are_planned` |
| Once-only source; ordinary literal allocation unchanged inside M4; private multivalue acquisition is not a source ABI signature | `source_runs_once_and_ordinary_literals_keep_fixed_storage_in_an_m4_module` |
| Interned signatures and correct planned function/type indices; private globals and only ordinary user/memory/`__alloc` exports | `m4_signatures_are_interned_and_each_planned_function_uses_the_right_type`; `numeric_constructor_is_planned_valid_and_private` |
| Full-capacity acquisition, allocator-owned extent validation, typed loads/consumers and 48-byte descriptor/authority preparation before registry publication | `every_typed_copy_checks_consumed_values_and_publishes_after_preparation`; this is per-type decoding, not just a test derived from production offsets |
| Forged Reserve shape/operand/result annotations and original captured binding metadata, unexported/skipped/unreachable cases and independent pure-context checks | `forged_reserve_checks_operand_shapes_types_and_result_even_when_unexported`; `forged_reserve_capture_annotations_are_checked_against_original_bindings`; `forged_reserve_is_checked_in_skipped_and_unreachable_children`; `forged_reserve_is_rejected_in_pure_defaults_and_skipped_contracts` |
| Genuine Never Reserve and original captures; receiver saved/validated before once-only argument and shared reserve call | `reserve_accepts_genuinely_never_additional_and_original_captured_receivers`; `reserve_captures_and_validates_receiver_before_argument_then_calls_reserve_once` |
| Exact registered handle/kind, public/private length/payload/capacity and allocator-owned live identity/requested extent/class validation; no mutations in authority lookup | `authority_lookup_checks_exact_registered_handle_and_current_metadata_without_mutation`; `current_acquisition_preflight_requires_exact_live_registered_payload_identity_and_extent` |
| Current acquisition checked before both capacity-sufficient return and replacement; replacement validated before copy | `reserve_preflights_current_acquisition_before_noop_or_replacement_and_validates_new_before_copy` |
| Width-scaled live-byte raw copy, no unconsumed-value scan, replacement public/private publication before old payload release; no callbacks in copy/publication window | `reserve_relocation_raw_copies_before_publication_and_releases_only_after` |
| Owner-local scratch order; capture handle/index before RHS; saved old numeric value; movable post-RHS validation/address versus fixed saved address; correct typed value scratch and unchanged legacy no-M4 scratch | `pending_write_scratch_precedes_index_and_rhs_reserve_scratch`; `m4_write_value_scratch_matches_each_element_type_but_no_m4_keeps_legacy_scratch` |
| Independent old non-M4 emitter bytes | `no_m4_source_compilation_preserves_the_independent_emitter_golden` (baseline fixture, not a universal historical comparison) |
| Forged Push receiver/item/results, original captures, unexported/skipped/unreachable and pure default/skipped-contract checking | `forged_push_checks_receiver_item_and_result_even_when_unexported`; `forged_push_captures_check_annotations_and_original_binding_metadata`; `forged_push_is_checked_in_skipped_and_unreachable_children`; `forged_push_is_forbidden_in_defaults_and_skipped_contracts` |
| Genuine Never/captured Push and enabled-preset identity | `push_accepts_genuine_never_and_original_captured_receivers_with_preset_parity` |
| Save/validate receiver, evaluate item once, revalidate/consume new item, use shared reserve(1), typed slot store before public/private lengths; no expected failures/callbacks after reserve success | `push_saves_validates_and_evaluates_item_once_before_the_typed_helper` |
| Push-only discovery activates shared reserve and retains existing helper bodies | H: `push_only_selects_reserve_and_preserves_existing_helper_bodies` |
| Forged Pop/Remove shapes/types/results and original captures; independent pure/default/skipped/unreachable rejection | `forged_pop_remove_check_shapes_types_results_and_original_captures`; `pop_remove_remain_forbidden_in_defaults_contracts_skipped_and_unreachable_nodes` |
| Pop authority/empty/consumed-value guards before shared public/private length writes; no growth/release/capacity mutation | `pop_lowering_validates_authority_and_consumed_value_before_length_publication` |
| Remove receiver capture before once-only index, current validation/address, returned-value consumption before raw overlap-safe byte shift and shared lengths; no later calls/traps | `remove_lowering_captures_once_revalidates_and_shifts_raw_bytes_before_shared_lengths` |
| Genuine Never Remove index; separate locals, string, remainder and temporary discovery through index children | `remove_preserves_genuine_never_and_plans_every_index_child` |
| Take-only structural discovery without constructor/reserve/push; scalar signatures; deterministic append-only take helpers preserve earlier helper bodies | `pop_remove_only_helpers_use_scalar_signatures_and_are_discovered_when_skipped`; `pop_remove_append_helpers_without_changing_push_reserve_or_other_existing_bodies` |

## 3. Observable runtime semantics and host lifetime rights

All names below are in [R], all ignored by default, using its four-preset helper.
Assertions use independent expected values/event sequences, including
`Number.isNaN`, `Object.is` and raw special-value bits where applicable.

| Contract | Exact test functions |
| --- | --- |
| Once-only construction, distinct empty/nonempty handles and mutable slots, fresh copy of resizable input, immutable string sharing, number special values | `copies_all_types_once_with_distinct_handles_and_mutable_slots` |
| Complete outer-storage and every logical bool/string consumption; source survives failed construction; empty copies do not consume spare storage | `construction_consumes_every_logical_element_and_preserves_source_after_traps` |
| Constructor captures, recursion/reentry, actual memory growth and later traps retain completed effects and earlier arrays | `captured_recursive_and_reentrant_construction_survives_growth_and_later_traps` |
| Reserve all types preserves aliases, logical length and values | `reserve_preserves_aliases_length_and_values_for_all_types` |
| Fixed receiver traps before argument even for zero; finite/integral/nonnegative numeric guards after argument effects, accepting -0 | `reserve_rejects_fixed_receivers_before_effects_and_checks_numeric_domain_after_effects` |
| Exact runtime authority both before/after effects; nested reserve, rebinding and host reentry use captured object and current state | `reserve_checks_exact_runtime_authority_before_and_after_argument_effects`; `reserve_captures_receiver_through_nested_reserve_rebinding_and_reentry` |
| Reserve raw copy does not eagerly consume existing invalid bool/string slots | `reserve_copies_unconsumed_bool_and_string_slots_without_scanning` |
| Ordinary/all-five-compound stores preserve captured receiver and pre-RHS old numeric value across relocation and actual displaced-buffer reuse | `ordinary_stores_follow_relocated_payload_not_rebound_receiver_or_reused_buffer`; `all_numeric_compounds_keep_pre_rhs_value_across_relocation_and_deterministic_reuse` |
| Distinct invocation scratch under host reentry/caught inner trap; RHS trap leaves completed reserve/reuse but performs no final store | `reentrant_pending_writes_and_caught_inner_traps_use_distinct_invocation_scratch`; `failed_rhs_retains_completed_reserve_and_reused_payload_without_final_store` |
| Recursive suspended stores, all five compounds, aliases, captured rebinding, once-only indices, reentry, caught inner trap, real reuse and later calls | `recursive_suspended_stores_and_all_compounds_survive_reuse_reentry_and_caught_traps` |
| Post-RHS descriptor mismatch rejects final store in all types/all numeric compounds; recovery only after explicitly restoring deliberately damaged descriptor | `pending_stores_reject_post_rhs_authority_mismatch_and_recover_after_descriptor_restoration` |
| Fixed-storage writes in an M4 module retain original saved place through rebinding and memory growth | `fixed_storage_pending_writes_remain_bound_to_the_original_place_in_m4_modules` |
| Width-scaled 2^31-byte ceiling/adjacent requests, 2^32 and safe-integer boundaries; real capped memory-growth failure preserves post-argument array state and later use | `bounded_allocation_failure_preserves_post_argument_published_arrays` |
| Push item once, append uses post-item alias contents/length; captured receiver survives rebinding, nested push and host reentry | `push_traces_once_only_item_and_post_item_length_through_alias`; `push_captured_receiver_survives_rebinding_nested_push_and_host_reentry` |
| Push spare/growth paths for all kinds and number special values; initial authority before item effects and post-item consistency checks | `push_all_types_spare_growth_and_numeric_special_values`; `push_validates_authority_before_effects_and_rechecks_post_item_state` |
| Push consumes only new bool/string item; old invalid slots remain lazy through growth | `push_copies_old_bool_and_string_slots_lazily_but_consumes_new_items` |
| Push pending ordinary/numeric compounds and bool/string writes use current payload after actual reuse; reentrant/trapped RHS scratch is invocation-local | `push_pending_stores_and_all_compounds_follow_current_payload_after_reuse`; `push_pending_bool_and_string_stores_survive_growth_and_reused_payloads`; `push_reentrant_pending_writes_and_trapped_rhs_keep_invocation_state_separate` |
| Push iteration retains initial count, reads current elements for all kinds; bounded growth failure retains completed item effects | `push_iteration_uses_initial_length_and_current_elements_for_all_types`; `push_bounded_allocation_failure_preserves_post_item_state_for_all_types` |
| Pop/remove all types preserve handle, payload, capacity and identity; no shrinking; removal order and scalar result | `pop_remove_all_types_preserve_identity_capacity_and_payload` |
| Remove authority before once-only argument, numeric domain after effects (-0 accepted); captured receiver and post-argument current length after nested effects | `remove_checks_authority_before_once_only_argument_and_numeric_domain_after_effects`; `remove_uses_captured_receiver_and_post_argument_length_after_nested_effects` |
| Pop/remove returned bool/string consumed before any mutation; unreturned shifted slots stay lazy; consumption failure preserves prior argument effects | `pop_remove_validate_only_returned_bool_or_string_before_any_mutation`; `remove_validates_post_argument_consumed_value_without_rolling_back_argument_effects` |
| Numeric pop/remove and raw shift preserve special-value bits | `pop_remove_numeric_values_and_raw_shift_preserve_special_value_bits` |
| Post-shrink/shift pending stores use captured current index or trap; all numeric compounds use saved old values; all three types | `pop_remove_pending_numeric_stores_and_all_compounds_use_saved_old_value_and_current_index`; `pop_remove_pending_bool_and_string_stores_shift_current_index_or_trap_after_shrink` |
| Recursive/captured/reentrant pending writes survive caught traps; iteration keeps initial count, checks later yields and observes shifts instead of early termination | `pop_remove_recursive_captured_and_reentrant_pending_writes_survive_caught_inner_traps`; `pop_remove_iteration_keeps_initial_count_checks_later_yields_and_observes_shifts` |
| Node encoding creates fixed copy-isolated storage rejected by all resize methods before effects; decoding copies current logical contents, not spare slots | `node_copy_encoding_stays_fixed_and_decoding_snapshots_current_resizable_contents` |
| Stable movable handles through reentry/caught traps/later calls; same-buffer relocation requires pointer/length reacquisition independently of view refresh; actual growth retains fixed offsets and earlier arrays | `raw_hosts_reacquire_movable_payloads_across_reentry_traps_and_actual_memory_growth` |

## 4. Private deterministic failure and authority evidence (#70)

[P] is a `cfg(test)` child, unavailable to normal integration dependencies and
adapters. Fixtures first use `ValidatedProgram` and ordinary assembly, then rewrite
selected sections. Identity is seeded once at instantiation, never reset. An
optional fixed absolute bump-cursor ceiling inserts the existing allocator-core
before-backing guard into the planned acquire body; it applies only to fresh
acquisition, not reuse or ordinary allocation. Global exports are unit-fixture
observations, not a public management API.

| Contract | Exact P test and observation boundary |
| --- | --- |
| Default transport changes only exports; configured transport limits differences to globals/exports/code; deterministic ordinary compilation has no controls | `test_transport_changes_only_requested_fixture_sections_and_never_ordinary_compilation` |
| Failed bool/string construction publishes no authority entry, retains successful unpublished payload/object allocations, leaves published arrays usable and does not sweep abandoned active payloads into reuse | `failed_constructor_keeps_unpublished_acquisition_without_sweeping_published_arrays` |
| Copied/fabricated descriptor+record is not exact registry membership; reject inconsistent public/private kind, payload, length, capacity, identity and active acquisition before no-op/growth/push/pop/remove mutations | `exact_authority_preflight_rejects_fabrication_and_inconsistent_records_before_mutation` |
| Deterministic fresh backing failure preserves completed argument removal, old payload, remaining contents, allocator globals/free availability; later same-class reuse and smaller fresh growth succeed | `backing_failure_preserves_post_argument_state_free_reuse_and_later_fresh_acquisition` |
| Last u64 identity reached without wrap; exhausted acquisition traps before draining free class or acquiring fresh storage; later nonallocating reserve/write/push/pop remain usable | `exhausted_identity_preserves_free_payloads_and_allows_later_nonallocating_mutations` |

The latter two cover number/bool/string. Constructor invalid-element retention
covers bool/string because numbers have no analogous invalid scalar encoding.
The full-memory/global `unchangedTrap` snapshots establish fixture-specific
failure-state preservation, not invocation rollback. Deliberate metadata damage is
outside supported host behavior: recovery tests restore it before later use. It
is neither malicious-host isolation nor automatic repair. Synthetic ceiling
failure is supplemented by R's patched-memory-maximum **real** `memory.grow`
failure; neither demonstrates successful multi-GiB allocation.

## 5. CLI, browser and Node adapter contracts (#69)

| Boundary | Exact test functions and scope |
| --- | --- |
| C: binary/WAT parity with core per preset; enabled-preset identity; selected versus no user exports, required memory/`__alloc` and no management exports | `cli_resizable_bytes_wat_and_export_selection_match_core_at_each_preset` |
| C: malformed syntax, arity/receiver/item types, constructor migration and pure-context diagnostics; same rendered messages across presets; failed binary/WAT outputs remain unchanged | `cli_resizable_diagnostics_match_core_and_preserve_outputs_at_each_preset` |
| C: reserved host-language `resizable` registration diagnostics and unchanged output | `cli_resizable_host_language_name_rejection_matches_core_without_output_changes` |
| C: emitted modules execute all types, all methods, captures/nested effects and independent expected results; ordered external imports with `resizable` module/field remain legal | `cli_resizable_typed_results_captures_and_nested_effects_execute_at_each_preset` (ignored Node case) |
| B: native oracle checks selected/no-export surface, ordered imports, memory32 bounds, no start section, no management surface, enabled-preset identity and diagnostics | `browser_resizable_native_fixtures_validate_surface_and_preset_parity` (native only; does not load browser bindings) |
| B: actual browser `compile`/`compileWat` bytes/text/diagnostics versus native per preset, host registration, all kinds/methods/captures/effects, independent values including -0, fixed/empty traps, later calls and retained arrays | `rebuilt_browser_resizable_bytes_wat_diagnostics_and_execution_match_native` (ignored; requires **fresh current-source** `pkg/`) |

The [C] and [B] fixtures also contain a private range-returning function deliberately
excluded by selected/none exports. That is selected-export compatibility, not
permission to externally export range. Node marshaling/lifetime evidence is R's
last two rows, not a newly implicit compiler JSON interface. The ordinary
[25-group browser smoke](../tests/browser.cjs) is regression coverage; it is not a
substitute for the dedicated rebuilt-browser M4 case.

## 6. Measurements and observation isolation (#71)

[M] owns the following exact unit tests:

| Obligation | Test / actual evidence input |
| --- | --- |
| Observation-only exports do not alter any non-export section, ordinary bytes or deterministic compilation | `observations_are_private_and_ordinary_compilation_is_unchanged` |
| Exactly two i64 counters and four instructions per visited non-null FIND/LOOKUP record; remove increments to recover all original decoded bodies/locals/calls/memory operations; all three kinds and operation-only discovery | `counters_only_add_exact_increments_after_live_registry_guards` |
| Private export-name collisions affect only the requested observed/counter variant, not ordinary source compilation | `observation_export_collisions_are_private_to_the_requested_variant` |
| Logical length versus capacity, active/free payloads, allocator records, 48-byte retained objects, cursor/pages, cross-array exact-class reuse, stranding, geometric copy volume and search counts | `record_m4_accounting` (ignored exact recording mode) with [corpus](../tests/support/m4_resizable_corpus.was) and [Node harness](../tests/support/m4_resizable_measure.cjs) |
| Fresh uninstrumented-instance append, spare push/pop, ordered remove/push and oldest-of-32 no-op reserve timing for each kind | `record_m4_guest_timing` (ignored exact recording mode), same corpus/harness |

The [#71 evidence README](benchmarks/m4-resizable-arrays-2026-10-29/README.md),
[accounting JSON](benchmarks/m4-resizable-arrays-2026-10-29/accounting.json),
[timing JSON](benchmarks/m4-resizable-arrays-2026-10-29/timing.json),
[environment/source manifest](benchmarks/m4-resizable-arrays-2026-10-29/environment.json)
and [sequential recorder](../tests/support/record_m4_resizable.py) are the historical
exit artifacts. The recorder executes runtime/M3-compatibility/isolation
prerequisites before separate release-profile exact recording modes, refuses to
overwrite evidence, checks one executed recording test per mode and checks source
stability. A full ignored suite may invoke recorders; its timing is **not isolated
measurement evidence**, even though M4's mutex prevents its two modes racing each
other's module files.

Important interpretation limits:

- Accounting compares fresh ordinary/observed instances after every completed
  guest call with a JS sequence oracle, independent live values, initialized
  records/authority bytes and disjoint owned extents. Ordinary registry roots and
  cursor are inferred through paired observed addresses, not independently exported.
- Reconstructed free chains do not independently expose hidden class heads. No
  spare/free/stale removed payload bytes are decoded as consumer-owned values.
- `operationBoundaryPeak` excludes transient old-plus-new payload residency during
  growth. `inferredCopiedLiveBytes` is inferred from verified relocation/length,
  excludes constructor copies and is not a load/store counter.
- Referenced strings, ordinary literals/alignment, retained descriptors/authority,
  current payloads and failed-construction storage are not general reclamation.
  Successful accounting does not census failed unpublished constructions; P owns
  that separate retention evidence. Stable pages alone prove no storage plateau.
- Timing includes JS loops/crossings, parameter checks, search, item selection,
  allocation/copy/growth as applicable and return transport. It excludes setup,
  instantiation, validation, assertions, decoding and instrumentation. It is neither
  compiler timing nor isolated Wasm latency, significance or a speedup claim.
- Byte-class doubling gives bounded geometric copy-volume evidence, not O(1)
  total append-time proof: authority and physical registries have linear searches.
  Exact-class free bytes can be stranded; no splitting/coalescing/borrowing occurs.

## 7. Compatibility, presets, import snapshots, M2 and M3

These are named regression mechanisms that the parent gates must execute where
appropriate, not fresh outcomes here. M4-specific operation coverage does not
replace the older ABI/emitter/diagnostic or private-unit seams.

| Boundary | Exact existing tests (relative source links) |
| --- | --- |
| Historical baseline bytes, deterministic source/typed compilation and public descriptor layout | [abi.rs](../crates/compiler/tests/abi.rs): `memory_abi_preserves_baseline_bytes_and_determinism_at_every_level`, `static_descriptors_match_independent_little_endian_bytes`, `node_guest_and_explicit_host_marshaling_agree_on_descriptor_bytes` |
| Independent old emitter fixture, typed input unchanged, selection changes only exports, runtime stack/control/host behavior | [emitter.rs](../crates/compiler/tests/emitter.rs): `native_emitter_preserves_baseline_and_preset_determinism_without_mutating_typed_input`, `export_selection_preserves_non_export_sections_within_each_level`, `node_emitter_preserves_control_flow_temporaries_bool_normalization_and_host_effect_order` |
| Diagnostic fixtures and raw location semantics | [diagnostics.rs](../crates/compiler/tests/diagnostics.rs): `library_diagnostics_match_pre_refactor_cli_text_exactly`, `every_parse_variant_and_compile_wrapper_preserve_raw_span_offsets`, `every_semantic_variant_and_compile_wrapper_preserve_raw_span_offsets`, `every_located_codegen_variant_and_compile_wrapper_preserve_raw_span_offsets`, `every_unlocated_codegen_variant_remains_unlocated_through_rendering` |
| Analysis independent of optimization, error parity, owned host snapshots after registry changes | [compiler.rs](../crates/compiler/src/compiler.rs), `compiler::tests`: `optimization_levels_preserve_analysis_and_the_input_program`, `optimization_levels_preserve_errors`, `hosts_survive_analyzer_reuse_and_typed_output_owns_a_snapshot`, `compiling_existing_typed_program_uses_its_import_snapshot` |
| Selection preserves imports/types/bodies/call indices/data/helpers and snapshots; no pruning of invalid unexported bodies or forged host snapshots | [export_tests.rs](../crates/compiler/src/codegen/export_tests.rs), `codegen::export_tests`: `scalar_selection_preserves_all_imports_signatures_bodies_and_call_indices`, `memory_selection_preserves_runtime_exports_helpers_data_and_call_indices`, `import_only_memory_modules_keep_runtime_exports_with_no_user_exports`, `selection_never_skips_user_or_host_layout_validation`, `unexported_bodies_still_report_errors_at_every_optimization_level`, `export_and_level_changes_preserve_options_registration_and_typed_import_snapshots` |
| Existing numeric proof/diagnostic boundaries are not storage selection; enabled presets need identity, not universal None-versus-enabled identity | [integer_specialization.rs](../crates/compiler/tests/integer_specialization.rs): `proof_cache_preserves_forged_tree_errors_and_does_not_survive_compilation`, `comparison_and_negation_candidates_preserve_forged_and_skipped_errors`; [integer_locals.rs](../crates/compiler/tests/integer_locals.rs): `all_owner_local_writes_block_storage_even_when_skipped_or_unreachable`, `helpers_have_separate_write_sets_and_captured_cells_do_not_specialize`, `forged_duplicate_types_annotations_and_first_error_order_match_none`; F/L/R/C/B provide direct M4 parity evidence |
| M2 remains private, no storage-policy migration or ordinary-byte/preset interference | [region/tests.rs](../crates/compiler/src/codegen/region/tests.rs): `ordinary_bytes_presets_exports_and_diagnostics_remain_independent`, `excluded_forms_and_no_candidates_emit_no_region_machinery`; [region/measurement_tests.rs](../crates/compiler/src/codegen/region/measurement_tests.rs): `measured_presets_are_deterministic_and_enabled_bytes_match`, `instrumentation_is_explicit_and_adds_no_calls_or_storage_selection_changes` |
| M2 exclusive reuse, normal-return retirement, permanent trap quarantine and recursive/reentrant retained handles | [region/runtime_tests.rs](../crates/compiler/src/codegen/region/runtime_tests.rs): `reuse_reinitializes_distinct_aligned_literals_and_preserves_ordinary_handles`, `traps_quarantine_owned_blocks_and_keep_completed_initialization_and_writes`, `reentrant_recursive_callers_keep_retained_arrays_and_captures_isolated` |
| #64 explicit noncontiguous planned indices; M3 adapter remains private despite production-internal core promotion | [allocator/core_tests.rs](../crates/compiler/src/codegen/allocator/core_tests.rs): `reusable_emission_uses_explicit_noncontiguous_function_and_global_indices`; [allocator/tests.rs](../crates/compiler/src/codegen/allocator/tests.rs): `appended_management_preserves_existing_indices_bodies_and_ordinary_bytes`, `private_compilation_requires_planned_memory_and_preserves_checked_body_errors`, `management_export_collisions_reject_only_selected_user_exports` |
| Inherited allocation request/classes/widened sizing/identity/release ordering and real growth failure | [allocator/failure_order_tests.rs](../crates/compiler/src/codegen/allocator/failure_order_tests.rs): `decoded_classify_checks_signed_request_domain_before_any_narrowing_or_state_access`, `decoded_acquire_validates_request_and_identity_before_reading_or_removing_free_head`, `decoded_fresh_reservation_checks_widened_size_before_backing_and_commits_after_success`, `decoded_real_backing_checks_widened_end_and_growth_failure_before_cursor_commit`; [allocator/failure_tests.rs](../crates/compiler/src/codegen/allocator/failure_tests.rs): `real_backing_growth_failure_preserves_live_free_and_unpublished_storage`; [allocator/release_tests.rs](../crates/compiler/src/codegen/allocator/release_tests.rs): `decoded_release_guards_exact_provenance_identity_and_state_before_pool_writes`, `registry_positions_reject_wrong_double_and_aba_authority_in_same_instance` |
| M3 exact-class reuse/no coalescing, preserved ordinary/live storage through real growth | [allocator/runtime_tests.rs](../crates/compiler/src/codegen/allocator/runtime_tests.rs): `acquisition_reuses_only_exact_class_without_borrowing_larger_free_block`, `adjacent_free_blocks_are_not_coalesced_split_or_borrowed`, `real_memory_growth_preserves_initialized_live_blocks_identities_and_free_records`; [full M3 map](m3-allocation-infrastructure.md#final-contract-to-test-map) supplies the remaining inherited contracts |
| Older fixed typed arrays and shared capture retention survive recursion/reentry/traps/growth | [typed_arrays_runtime.rs](../crates/compiler/tests/typed_arrays_runtime.rs): `raw_host_retained_typed_arrays_survive_recursive_reentry_traps_and_rhs_growth`; [closures_runtime.rs](../crates/compiler/tests/closures_runtime.rs): `reentrant_live_contexts_and_retained_handles_survive_inner_traps_and_growth` |
| Existing typed-array CLI/browser regressions and Node copy accounting | [CLI typed_arrays_adapters.rs](../crates/cli/tests/typed_arrays_adapters.rs): `cli_typed_array_hosts_selected_exports_and_contextual_source_match_core`, `cli_typed_arrays_execute_with_raw_host_layouts`; [browser typed_arrays_adapters.rs](../crates/browser/tests/typed_arrays_adapters.rs): `rebuilt_browser_typed_array_hosts_bytes_diagnostics_and_execution_match_core`; [browser smoke](../tests/browser.cjs) retains accessor-snapshot and aggregate-copy-budget scenarios |
| Portable adapter-free production core and downstream API/assets | [workspace_contract.rs](../crates/compiler/tests/workspace_contract.rs): `core_production_dependency_graph_is_adapter_free`, `standalone_downstream_consumer_builds_and_uses_public_api`, `shared_compatibility_assets_resolve_outside_package_directory` |

M1 classification, M2 region activation, M3 configuration/observations and M4
storage are distinct. Historical M3 acceptance did not authorize a production
consumer; #65's explicit constructor scope did. M4 recycles only displaced owned
payloads after replacement publication. It does not retire stable objects,
reclaim ordinary arrays/captures/strings, activate M2, introduce a public allocator
option/export or alter numeric defaults.

### Fixture history checked without regeneration

`git diff 3f39ae7ba9121ec24bdc64199fd5c9c06eef72f2 HEAD --
tests/fixtures/abi tests/fixtures/emitter tests/fixtures/diagnostics` was empty at
the audited revision. This is repository-content history evidence since #63, not
execution of their tests. The latest commits touching those paths were
`35d3e83ae24bcac49c2427b99a7305dfdaccf49a` (emitter/ABI history) and
`2cec2fa63e0febccaabcea49bb0ad5a82fd67831` (diagnostics history).
Selected Git blob IDs from `git ls-tree -r HEAD` (Git objects, **not SHA-256 of
working-tree bytes**):

| Fixture | Git blob ID |
| --- | --- |
| [ABI memory.wasm](../tests/fixtures/abi/memory.wasm) | `4766c458f4aefb50b7e5680ec79282ddc426564f` |
| [Emitter control.wasm](../tests/fixtures/emitter/control.wasm) | `0a63d25240df4b58c677268ea78fe90b98204a77` |
| [Parse stderr](../tests/fixtures/diagnostics/parse.stderr) | `1e174aab49ca3529f1e6f1153592e7481577730b` |
| [Semantic stderr](../tests/fixtures/diagnostics/semantic.stderr) | `fc2878ffa64391dd9b8996cbebff19174039cc40` |
| [Codegen stderr](../tests/fixtures/diagnostics/codegen.stderr) | `8dc8e32ad35d08dfcc2b2dd103eff67a1db283e5` |
| [Unlocated stderr](../tests/fixtures/diagnostics/unlocated.stderr) | `8ae5c1de5e321036e4cb077a5d054e6fc5bb80a7` |

## 8. Predecessor exits and provenance available in Git

These committed exits were identified from local Git history and the linked
records, not by querying issue status or rerunning their commands. A commit proves
availability of the recorded slice, not independent verification of every reported
historical gate. Full-milestone acceptance cannot be inferred by adding subset
counts from different revisions.

| Issue | Committed exit revision | Durable reference and historical limitations |
| --- | --- | --- |
| #64 | `2069bae3fc17d60dcdf01e5892414aa34c8b36c8` | [Reusable core boundary/validation](m3-private-allocator.md#reusable-emission-boundary-64): 39 allocator / 23 M2 / 3 ABI tests reported, explicit-index test, representative pre/post byte identity. No M4 activation or fresh browser rebuild; core later moved to production-internal `allocator_core.rs` by #65. |
| #65 | `6d782f0139af04a5bb55fdc1e1e7c3929d70f5e6` | [Constructor validation/review](resizable-arrays.md#constructor-validation-and-review--2026-10-28): 16 frontend / 7 lowering / 3 four-preset runtimes reported; full compatibility gates recorded after an initial timeout; browser smoke was ordinary compatibility, not a dedicated constructor adapter scenario. |
| #66 | `2b612e1a8468bee52047be9d8e2758ebdba6ad44` | [Reserve final evidence](resizable-arrays.md#issue-66-validation): focused 27/17/14; broader final outcomes explicitly user-reported, initial combined timeout followed by a separate completed full run. Covers reserve/writes, not later methods. |
| #67 | `d4bd9b3b4ffbf016fe54778c06f6e73e17c451cf` | [Push validation/review](resizable-arrays.md#issue-67-validation): focused 40/23 and 24 runtimes, rebuilt-browser/full ignored workspace record (1,556 across 84 targets); existing browser smoke not dedicated push scenarios. |
| #68 | `2e473361ee831de584d21907d1e3c6c82c118f90` | [Pop/remove validation/review](resizable-arrays.md#issue-68-validation): focused 46/30/34 and default/build/browser-compatibility evidence; full ignored workspace timed out at 120 seconds. That timeout remains historical, not a pass. |
| #69 | `7d11c92c1fce2dfd31933011b042425d3b1cd6d1` | [Adapter/raw-host validation/review](resizable-arrays.md#issue-69-validation): dedicated CLI/browser and retained raw-host evidence, current-source package and full ignored workspace reported; not final M4 acceptance or measurements. |
| #70 | `64c2cd19c0a39983a160ddd0e7d310ee3365b9d9` | [Authority/failure evidence](resizable-arrays.md#issue-70-validation): 46/33 frontend/lowering, selected runtime and private library coverage reported; no fresh browser rebuild or full ignored workspace claim. |
| #71 | `52a28557cb41652581ed247e045ca27a6a5ed75f` | [Measurement exit](benchmarks/m4-resizable-arrays-2026-10-29/README.md): durable raw logs/JSON and executable-input manifest; production unchanged, no fresh browser rebuild/full ignored workspace; no integrated release acceptance. |

The separate design commit between #64 and #65 is
`794910d2f22f5862af3bc05b7bf09425181e21c8`. #71's measurement manifest records
**base** `64c2cd19c0a39983a160ddd0e7d310ee3365b9d9` plus dirty/untracked experiment
inputs, not a clean measurement run at the eventual #71 commit. Its recorded
`stableSourceHashesAfterRecording` is true. During this audit, direct SHA-256
comparison of all **97** manifest input paths against current working-tree bytes
found **zero mismatches**. That is static provenance verification, not measurement
reproduction; the manifest covers compiler source/manifest/lockfile and selected
harness inputs, not all adapter/test/browser-package files.

Recorded #71 hashes (copied from its JSON, generated Wasm artifacts not rebuilt
or rehashed here):

| Input/artifact | Recorded SHA-256 |
| --- | --- |
| Corpus (1,508 bytes) | `4db83c12cf22d5076f77b5accd6413ce3318e3ec344caa380d97bdee2c1454c7` |
| Node measurement harness | `522d0e524d9e454646c343707a14717839c901803b6d11e29ee5fa689eb87b8e` |
| Ordinary module (6,126 bytes) | `c89121663e51268435134faab16df589130bcd849846a8ae980736527e4bd912` |
| Observed module (6,305 bytes) | `767503dc4218b16cd371c9ada7e405e3c56c0d2b058a5ed48c72f04d696e3156` |

## 9. Explicit gaps, limits and parent handoff

1. **This map is not execution evidence.** The separate
   [release-readiness record](m4-release-readiness.md) and
   [manifest](m4-integrated-validation-2026-10-30.json) report completed final-source
   gates, counts, exit codes, tooling, source state and freshly rebuilt package hashes.
2. **Measurement execution and acceptance are distinct.** The
   [two sequential #72 recordings](benchmarks/m4-resizable-arrays-2026-10-29/integrated-72/README.md)
   preserve #71's samples, reproduce accounting and expose a material timing signal.
   Aggregate ignored-suite recorder output is not isolated timing; static input
   hash equality alone would not have satisfied this gate.
3. **Existing snapshot/export checks are compositional coverage.** Core import
   snapshot tests use ordinary fixtures; M4 adapter fixtures independently assert
   ordered imports and selected/none exports. There is no dedicated named M4 test
   that analyzes an M4 program, mutates the compiler registry, then recompiles that
   same typed program. Treat the combination as shared-path evidence, not an
   explicitly executed M4-specific snapshot scenario.
4. **Non-M4 byte compatibility is representative, not exhaustive.** Old golden
   fixtures and helper-body/index comparisons cover their exact inputs. They do
   not establish byte identity for every old program. Intentional global-name
   migration for `resizable` is not a regression to hide by fixture regeneration.
5. **M2/M3 noninterference is not a new joint storage policy.** Independent private
   suites and ordinary-byte/export checks protect their boundaries; there is no
   authorized M2 migration/shared pool/general reclamation or new mixed-manager
   public API to validate. Run their unit seams explicitly in the parent gates;
   adapter-only coverage cannot activate them.
6. **Bounded failures are not huge-allocation success or security evidence.**
   Patched memory maximum, fixed before-backing ceiling, seeded identity and
   deliberate metadata corruption have distinct scopes. Tests demonstrate
   same-instance preservation/recovery under those conditions, not malicious-host
   isolation, rollback, trap cleanup, sweeping or allocation success at the ceiling.
7. **Storage and performance limits remain material.** Current payloads, stable
   descriptors/authority, unreachable arrays, referenced strings and unpublished
   successful allocations remain retained. Exact-class fragmentation and linear
   searches remain; completed-call peaks exclude transient growth residency.
   No GC, heap/page return, generally bounded memory, performance improvement,
   statistical significance or release suitability follows from the measurements.
8. **Browser presence is not freshness.** The ignored browser test checks package
   files exist but does not itself prove their source provenance. Rebuild before
   dedicated browser/full ignored tests; default/native fixture success and the
   historical 25 smoke groups cannot stand in for that gate.

No contract conflict or demonstrated production defect was established by this
static audit. The listed limitations are evidence boundaries and final-gate
handoffs, not a declaration that every possible case is covered or that M4 is
accepted. Capacity shrinking, insertion/slices/views, nested arrays, public
capacity/authority queries, host ownership registration, M2 migration, general
free/GC/RC, new defaults and wider optimization remain outside the approved slice.
