# M5 contract-to-test map — #81

## Evidence status and scope

This is a **source-audited inventory**, dated 2026-10-30, for the implemented
`string_concat`, `string_byte_len`, `string_slice_bytes`, `string_replace_all`,
and `string_join` builtins. Test names below are actual Rust test functions;
statements about their assertions describe inspected source, **not newly observed
passes**. No tests, builds, browser rebuilds or benchmarks were run for this map.
The [final-source manifest](m5-integrated-validation-2026-10-30.json),
[fresh measurement archives](benchmarks/m5-integrated-81/README.md) and
[release decision](m5-release-readiness.md) establish execution, provenance and
acceptance separately. The full workspace run executed all 99 named tests;
integrated acceptance remains pending explicit timing-signal disposition.
Do not substitute these inventory counts for actual per-target execution counts.

Normative/implementation references:

- [String operation contracts](string-operations.md), including #76's first-four
  adapter gate and #77's join gate. Neither historical slice alone accepts final M5.
- The separately owned, local/unpublished
  `docs/adr/0010-immutable-string-operations-and-fragment-construction.md`,
  [memory roadmap §9](was_compiler_memory_and_optimization_roadmap.md#9-milestone-m5--string-operations-and-efficient-construction),
  [glossary](../GLOSSARY.md) and [revision-1 ABI](memory-abi.md).
  ADR/design confirmation is not implementation or release evidence. The glossary's
  “proposed M5 operations” wording is historical, not a reason to disregard the
  implemented contracts/tests; this map does not edit that separately owned document.
- [#78 private observations](m5-string-observations.md) is the historical concat
  foundation. [#79 construction](benchmarks/m5-fragment-construction-2026-10-30/README.md)
  extends it to join/array observations; its additions supersede #78's historical
  statements that join-store and transient array-capacity observations are unavailable.
- [#80 ordinary measurements](benchmarks/m5-strings-2026-10-30/README.md) separates
  ordinary guest runtime from native compilation. Successful recording is not
  performance approval. Its material unexplained identical-code variability remains
  an explicit input to #81, not a changed-code regression proved by this map.
- [M4 release decision](m4-release-readiness.md) remains pending performance-signal
  disposition. M5 evidence does not resolve M4 acceptance.

### Counted inventory

Counts come from static `#[test]` / `#[ignore]` declarations in these ten files.
A loop over cases/presets is still one Rust test. Categories do not imply one test
per contract cell. “Default” means not marked ignored, not necessarily selected by
root default-member Cargo commands (notably the browser package).

| Alias / source | Total | Default | Ignored |
| --- | ---: | ---: | ---: |
| F: [compiler frontend](../crates/compiler/tests/string_operations_frontend.rs) | 16 | 16 | 0 |
| L: [compiler checked/decoded lowering](../crates/compiler/tests/string_operations_lowering.rs) | 24 | 24 | 0 |
| R: [compiler runtime](../crates/compiler/tests/string_operations_runtime.rs) | 27 | 0 | 27 |
| C: [CLI adapters](../crates/cli/tests/string_operations_adapters.rs) | 9 | 5 | 4 |
| B: [browser adapters](../crates/browser/tests/string_operations_adapters.rs) | 5 | 1 | 4 |
| O: [private string observations](../crates/compiler/src/codegen/string_observation_tests.rs) | 6 | 5 | 1 |
| A: [private array construction observations](../crates/compiler/src/codegen/resizable_construction_observations.rs) | 3 | 2 | 1 |
| K: [private construction corpus](../crates/compiler/src/codegen/string_construction_tests.rs) | 2 | 1 | 1 |
| T: [ordinary runtime measurement correctness](../crates/compiler/tests/m5_measurements.rs) | 2 | 1 | 1 |
| N: [native benchmark corpus correctness](../crates/compiler/tests/m5_benchmarks.rs) | 5 | 4 | 1 |
| **Total** | **99** | **59** | **40** |

The dedicated five-layer language/adapter inventory is **81 = 16 + 24 + 27 + 9 + 5**
(46 default, 35 ignored); private observations/construction add **11** (8/3), and
ordinary measurement correctness adds **7** (5/2). These totals exclude generic
ABI/M4/workspace tests, Python recorder tests, browser smoke groups, benchmark cases
and JS subcases. They are not a claim of 99 independent behavioral scenarios.

## Implementation anchors checked

| Boundary | Actual implementation and contract |
| --- | --- |
| Resolution/types | `crates/compiler/src/semantic.rs` registers exact signatures and builtin identities. Calls retain ordinary typed `Call` nodes; join supplies `string[]` context. `semantic/pure_expression.rs` permits only positional builtin `len` calls in pure expressions and checks both logical children. `host.rs` reserves all five language names, not external module/field strings. |
| Checked calls | `crates/compiler/src/codegen/call.rs::emit` checks arity/result, checks and emits every argument in order with exact operand types (Never compatibility), then calls the planned helper. No semantic-only trust or skipped-child exemption is introduced. |
| Structural planning | `crates/compiler/src/codegen/memory.rs` discovers builtin identity even without concrete string/array annotations and appends required helpers in byte-length/concat/slice/replace/join order after existing memory groups. Join explicitly requests string-array support. |
| Byte length / concat | `emit_planned` routes byte length to `emit_typed_len(ValidateString)`: unsigned descriptor length to f64, no result allocation. `emit_string_concat` validates left/right before empty reuse; otherwise checks widened descriptor-inclusive sizing and copies both inputs into one ordinary bump allocation. Both empty returns right. |
| Slice | `emit_string_slice_bytes` validates source, checks both ordered finite/integral/nonnegative/in-range endpoints and start <= end before conversion, probes only interior continuation-byte markers, reuses full ranges, otherwise allocates/copies even a proper empty slice. No clamping/full UTF-8 scan. |
| Replacement | `emit_string_replace_all` validates source/needle/replacement, rejects empty needle, reuses source for content-identical replacement or no matches. Two original-source search passes use constant locals; changed output gets one final ordinary allocation. Insertions are not rescanned; no linear-time search claim. |
| Join | `emit_string_join` validates separator, current captured outer storage, then entries in increasing index order before singleton reuse or allocation. Widened sizing includes all separator occurrences. Two byte-copy loops write final payload; complete static call list is validators plus one allocation, with no callback/concat/snapshot collection. Fixed arrays need no resize authority. |
| Private seam | `codegen/mod.rs` gates O/K with `cfg(test)`; `codegen/resizable.rs` gates A likewise. O compiles through `ValidatedProgram`/assembly, then rewrites the explicitly requested observed variant. Production/adapters/integration dependencies cannot select these modules. Counters/registry observations are not public compiler options or allocator controls. |

These anchors explain how assertions relate to behavior; implementation inspection
alone is not an independent runtime oracle or proof for every malformed public AST.

## Frontend contracts — all 16 F tests

| Contract | Named tests and assertion scope |
| --- | --- |
| Byte-length/concat signatures, inference, captures and placeholder stages | `exact_signatures_compose_with_inference_captures_and_placeholders`; `calls_check_structure_before_expressions_and_require_exact_types` checks structure before children, positional-only calls and representative wrong types. |
| Slice signature / exact region-local contexts | `slice_signature_supplies_exact_region_local_contexts`; `string_operations_do_not_expand_syntax_pipelines_or_purity` includes wrong source/start/end types, Never with a bad later child, unreachable and short-circuit-skipped invalid slice calls. |
| Replacement signature and typed identity/order | `replace_all_signature_supplies_exact_region_local_contexts`; `replace_all_retains_builtin_identity_and_ordered_arguments`; `replace_all_requires_three_exact_positional_strings_before_checking_children`. Wrong types are tested at each position; arity/named-argument errors precede undefined children. |
| Replacement Never/skipped/pure behavior | `replace_all_propagates_never_but_checks_all_children_and_skipped_calls`; `replace_all_rejects_direct_stages_and_pure_defaults_and_contracts`. Includes each divergent position, a later undefined child, conflicting placeholder constraints, post-return invalid code, skipped RHS and both pure logical guards. |
| Join contextual literals, placeholders and identity | `join_contextually_types_empty_parts_as_strings`; `join_signature_supplies_exact_contexts_and_placeholder_constraints`; `join_retains_builtin_identity_and_ordered_arguments`. |
| Join exact types, structure and flow | `join_requires_two_exact_positional_arguments_before_checking_children`; `join_propagates_never_but_checks_all_children_and_skipped_calls`. Rejects number/bool arrays, wrong scalars/range/Void, named arguments, malformed arity, later invalid/undefined children after Never, post-return and skipped invalid calls. |
| Join purity/pipelines | `join_rejects_direct_builtin_stages_defaults_and_contracts` rejects direct builtin stages, defaults and logically skipped contracts. Body calls inside lexical/immediate callable stages remain distinct from direct builtin stages. |
| All-five names/migration | `builtin_names_are_reserved_globally_but_lexically_shadowable` checks global rejection, exact host `ReservedName`, arbitrary external module/field names, parameter shadow `NotCallable`, and lexical callable pipeline shadowing for every name. |
| No syntax/whitelist expansion | `string_operations_do_not_expand_syntax_pipelines_or_purity` rejects string `+`, `len(string)`, direct byte-length/concat/slice stages, pure uses, unreachable concat and skipped byte length; accepts divergent concat with a valid later child. Together with the dedicated replacement/join tests, all five stay outside pure/default/contract and direct builtin-stage support. |

## Forged-tree, planning and independent decoding — all 24 L tests

Every module decoded by L's `operations` helper first passes
`wasmparser::Validator`. The instruction assertions below are additional evidence,
not an inference that valid Wasm implies correct semantics.

| Contract | Actual named tests |
| --- | --- |
| Concat validators before reuse; widened header-inclusive bound, ordinary allocation, descriptor fields and two byte-copy sites | `concat_validates_both_before_reuse_and_allocates_one_widened_result` |
| Byte-length validator, offset/alignment, unsigned f64 conversion, absence of allocation in helper | `byte_length_uses_string_validation_unsigned_length_and_no_allocation` |
| Slice validation before numeric conversion/reuse/allocation; two guarded endpoint loads and one copy loop, widened bound | `slice_helper_validates_before_numeric_checks_reuse_and_widened_copy_allocation` |
| Replacement validation order, empty check, content reuse, two-pass search/copy, constant locals, final allocation | `replace_validates_three_strings_before_checks_and_content_reuse_then_allocates_once` |
| Join dependencies, validation before singleton reuse/allocation, widened separator product, only final descriptor/byte stores and no indirect calls | `join_plans_string_array_support_and_validates_before_one_final_allocation` |
| Join index-ordered sizing/validation and direct final-payload copying with bounded locals | `join_decoded_passes_validate_in_index_order_then_copy_directly_with_bounded_locals` |
| Byte-length/concat forged arity/results/operands/kind, even export-none | `checked_calls_reject_arity_types_categories_and_forged_children_even_when_unexported` |
| Slice forged arity/results, every operand type/category/kind, later invalid child after divergent source | `slice_checked_input_rejects_forged_arity_types_categories_and_later_children` |
| Replacement forged arity/results/operands/kind, each later invalid child after earlier Never, export-none | `replace_checked_input_rejects_forged_arity_types_categories_and_all_children` |
| Join forged arity/results/types/kind, post-return and `false &&` wrappers for bad operands, bad separator after Never parts, export-none | `join_checked_input_rejects_forged_arity_types_results_and_unreachable_children` |
| Byte-length/concat Never-only discovery and later bad concat child | `never_arguments_discover_helpers_without_concrete_string_types_and_check_later_children` |
| Slice each divergent position plus callable/unreachable discovery and ordered source/start/end calls across presets | `slice_structural_discovery_and_argument_order_compose_with_never_helpers_and_presets` |
| Replacement each/all divergent positions and nested capture/unreachable discovery | `replace_structural_discovery_handles_each_never_operand_and_all_never_annotations`; `replace_discovery_visits_nested_capturing_callables_and_unreachable_calls` |
| Join each/all divergent positions without array annotations; nested capture and unreachable all-Never helper calls | `join_structural_discovery_handles_never_without_any_array_annotation`; `join_discovery_visits_nested_captures_and_all_never_unreachable_helper_calls` |
| Byte-length/concat capture helpers and unreachable expressions | `discovery_visits_capture_helpers_and_unreachable_expressions` |
| Independent call-order/live-prefix/preset checks | `nested_arguments_are_once_in_order_with_a_live_numeric_prefix_at_all_presets`; `replace_nested_arguments_are_once_in_order_with_a_live_prefix_at_all_presets`; `join_arguments_are_once_in_order_and_helpers_append_deterministically_at_all_presets` (the last nests all five operations) |
| Required-only deterministic helper ordering | `replace_helper_is_appended_after_slice_and_other_required_m5_helpers_only`, plus join's all-preset test above |
| All-five forged pure contracts/default binding rejection | `m5_calls_remain_forbidden_in_forged_contracts_and_defaults`; normalized default binding is forged, not a new default-expression AST. |
| Explicit skipped forged pure join | `join_is_forbidden_even_in_runtime_skipped_forged_pure_contracts` |
| Non-M5 compatibility | `absent_m5_preserves_independent_golden_and_legacy_string_array_helper_order`: exact existing `tests/fixtures/emitter/control.wasm` bytes, then string-array-only helper count/calls and Compiler/direct-codegen parity. No golden regeneration. |

## Runtime semantics — all 27 R tests

All R tests are ignored by default. The shared `run` compiles deterministically,
validates Wasm, executes each script at **all four presets**, and checks enabled
preset byte identity. Each Node child has a 30-second direct-child bound through
`tests/support/process.rs`; it is not guest fuel or process-tree cancellation.
UTF-8/content expectations use explicit strings, Buffer bytes/scalar boundaries and
independent JS results, not the production search/copy algorithm.

### Byte length and concat (6)

| Named test | Assertions |
| --- | --- |
| `byte_length_counts_utf8_bytes_including_empty_nul_and_unsigned_payload_bytes` | Empty, ASCII, embedded NUL, multibyte/combining text and larger inputs return byte counts, preserving inputs; not scalar/grapheme counts. |
| `concat_reuses_validated_empty_operands_and_copies_nonempty_utf8_without_mutating_aliases` | Empty reuse, fresh nonempty output, exact payload bytes; interned literals, aliases and array-held strings remain unchanged after binding/slot replacement. |
| `argument_effects_run_once_in_order_and_argument_traps_stop_later_evaluation` | Concat and byte-length effects, reuse still evaluating operands, left/right argument traps, retained earlier output, invalid host return stopping later evaluation. |
| `builtin_validation_after_argument_effects_checks_both_operands_before_empty_reuse` | Post-entry pointer/length corruption in both concat operands and byte-length source; all argument effects precede builtin checks; entry validation still precedes body effects. |
| `retained_concat_results_survive_growth_reentry_and_caught_traps_with_suspended_arguments` | Suspended outer argument, reentrant calls, actual growth, host-retained results and later caught traps/allocations do not invalidate prior strings. |
| `nested_operations_preserve_live_stack_prefixes_captures_and_recursive_invocation_state` | Nested byte length/concat under live numeric prefixes, captures, arrays and recursion. |

### Strict byte slicing (6)

| Named test | Assertions |
| --- | --- |
| `slice_bytes_checks_numeric_edges_and_every_utf8_endpoint_including_empty_interiors` | Every start/end pair of representative UTF-8 strings; rejects continuation boundaries even for empty slices; finite integral strict bounds, reversed/out-of-range, NaN/infinities, fractions/subnormals/extremes; accepts -0. |
| `slice_bytes_accepts_memory_end_endpoints_and_does_not_scan_utf8_payloads` | Exact memory-end payload/empty endpoints without out-of-bounds probes; deliberately malformed interior UTF-8 copied structurally rather than scanned. This does not license malformed host UTF-8. |
| `slice_bytes_reuses_full_ranges_and_copies_proper_ranges_without_changing_aliases` | Full identity, nonempty proper disjoint payload, immutable aliases/interned literals/array entries and nested live-prefix operations. |
| `slice_bytes_evaluates_arguments_once_before_checks_but_preserves_argument_trap_boundaries` | Source/start/end effects before numeric/boundary checks; each argument trap stops later effects; prior retained slices survive; host-return validation boundary unchanged. |
| `slice_bytes_revalidates_post_entry_descriptors_after_effects_even_before_full_or_empty_reuse` | Source pointer/length corruption at every argument-effect position, full/proper/empty cases, entry checks and subsequent recovery. |
| `retained_slices_survive_growth_reentry_and_caught_traps_with_suspended_arguments` | Proper-slice allocation itself forces growth, reentrant/suspended inputs, retained outputs across traps and later allocations. |

### Literal replacement (6)

| Named test | Assertions |
| --- | --- |
| `replace_all_matches_literal_nonoverlapping_utf8_and_copies_only_changed_nonempty_results` | Overlap (`aaa`/`aa` => `ba`), deletion/empty result, expansion without rescanning, case/normalization distinctions, NUL, literal template characters, no-match/content-identical reuse with distinct input handles, disjoint changed nonempty payload, aliases/interned arrays. Fresh changed-empty identity is not asserted here. |
| `replace_all_runs_arguments_once_before_empty_needle_checks_and_retains_effects_after_traps` | All three operands including replacement execute before empty-needle trap, empty source too; each argument trap/invalid host return boundary; published earlier effects persist. |
| `replace_all_revalidates_even_unused_operands_after_all_argument_effects` | Every operand descriptor corrupted after entry at every argument position, including no-match/identical/empty-needle paths; all effects precede checks and entry checks remain earlier. |
| `replace_all_results_survive_growth_reentry_and_traps_with_suspended_arguments` | Replacement allocation itself grows memory; nested reentry, suspended source, retained outputs and subsequent valid calls. |
| `replace_all_preserves_live_stack_captures_recursion_and_bounded_repetitive_searches` | Captures, arrays, recursion/live prefix; bounded repetitive near-matches, periodic overlap, dense expansion/deletion and multibyte cases. Correctness/deadline evidence, not linear search or timing evidence. |
| `replace_all_checked_expansion_traps_before_allocation_without_large_memory` | Repeated small inputs generate logical 2^32 / descriptor-inclusive overflow; buffer/cursor unchanged, inputs and retained result survive, later valid calls work. |

### Fragment join (9)

| Named test | Assertions |
| --- | --- |
| `join_captures_parts_once_and_observes_separator_mutation_resize_and_rebinding` | Once-only argument effects; reserve/slot write/push and variable rebinding affect the captured array, not a redirected binding. |
| `join_handles_fixed_resizable_empty_utf8_and_nul_parts_with_validated_reuse_and_fresh_copies` | Fixed/resizable empty/singleton/multi-entry cases, empty occurrences/separators, multibyte/NUL, larger counts; singleton identity, positive multi-entry fresh/disjoint copied output, immutable retained entries after slot replacement. |
| `join_inspects_post_separator_fixed_writes_shrink_and_empty_to_nonempty_growth` | Current captured storage/length after fixed writes, remove/pop/clear, reserve/push; surviving singleton reuse and retained removed entries. |
| `join_argument_traps_stop_later_effects_and_completed_mutations_and_results_survive` | Parts trap skips separator; separator trap preserves mutations and published earlier join; host-return validation, empty/nonempty separator effects. |
| `join_validates_unused_separator_outer_storage_and_every_entry_before_result_allocation` | Post-entry separator/outer pointer/length corruption and each invalid entry, including zero/singleton/empty entries; no result allocation before failure, entry checks remain earlier. Exact inter-validator order additionally comes from L decoding. |
| `join_accepts_raw_nonowning_shared_storage_and_memory_end_empty_payloads_without_utf8_scans` | Noncontiguous/nonowning raw revision-1 storage without authority, shared payloads, memory-end empty data, singleton handle reuse, structurally copied invalid UTF-8 demonstrates no scan. |
| `join_preserves_nested_capture_forwarding_live_stack_prefixes_and_recursive_state` | Nested captures/forwarding, separator-time mutation, arrays/nested joins, live numeric prefix and recursion. |
| `join_results_and_captured_movable_parts_survive_growth_reentry_and_caught_traps` | Separator reentry actually relocates captured parts; join allocation itself grows memory; suspended handles, original fixed source, old/new retained results survive traps/later allocations. |
| `join_checked_fragment_and_separator_expansion_traps_before_allocation_without_large_memory` | Separate fragment-sum/separator-product and descriptor-inclusive overflows; no cursor advance/growth, retained result and later valid join preserved. |

## CLI and current-source browser boundaries — all 14 C/B tests

C tests use the actual CLI binary. B's one default test is **native fixture
validation only**. Four ignored B tests load `pkg/was_compiler_bg.wasm` and its JS
bindings: parent execution must freshly rebuild `pkg/` first. File existence alone
is not provenance. C/B helpers exercise all four presets and selected/no exports;
byte/WAT identity is **within each preset** against core, with separate enabled
preset identity, not a requirement that None equals enabled bytes.

| Named tests | Contract/assertion scope |
| --- | --- |
| C `cli_string_join_tracer_matches_core_with_an_explicit_export` | Minimal explicit-export join byte parity and valid Wasm. |
| C `cli_string_bytes_wat_imports_and_selected_or_no_exports_match_core_at_each_preset` | All-five source, exact core/CLI binary and printed WAT, deterministic bytes, imports and selected/no-export surface. Required memory/allocator exports remain. |
| C `cli_string_diagnostics_and_unchanged_artifacts_match_core_at_each_preset` | All-five reservations, shadow `NotCallable`, arity/named/type/purity/pipeline/skipped/unreachable errors, malformed join; error identity/span/rendering parity and existing output preserved for both formats. |
| C `cli_string_errors_preserve_raw_utf8_spans_and_structure_before_child_order` | All-five arity/named/first-undefined order with multibyte comment, CRLF and tabs; explicit raw byte-span assertions plus rendered parity. |
| C `cli_string_reserved_host_names_fail_before_source_loading_without_changing_output` | All-five registration failures before missing source is loaded, exact messages/exit code, unchanged destination. |
| C `cli_string_copy_marshaling_utf8_empty_nul_aliases_and_edge_results_execute_at_each_preset` (ignored) | First four builtins: bytes, copy/reuse, strict slice -0/empty, literal replacement, array encoding/decoding mutation isolation and lexical shadow. |
| C `cli_string_ordered_effects_traps_growth_and_retained_handles_execute_at_each_preset` (ignored) | Composed ordered effects, traps, retained results, numeric/boundary errors, empty needle, actual growth and subsequent valid operations. |
| C `cli_string_join_fixed_resizable_utf8_copies_and_captures_execute_at_each_preset` (ignored) | Join fixed/resizable/capture/stage/lexical-shadow cases, UTF-8/NUL/empty entries, singleton reuse and copy isolation from mutable slots/JS arrays. |
| C `cli_string_join_validation_order_reentry_growth_and_retention_execute_at_each_preset` (ignored) | Post-entry separator/outer/entry corruption; effect order, caught trap/reentry, actual guest join growth and old results retained. |
| B `browser_string_native_fixtures_validate_surface_diagnostics_and_preset_parity` | Generates/validates native reference bytes/WAT, exact imports/exports and memory shape, no start section, invalid-source/registration fixtures. Does **not** execute browser compilation. |
| B `rebuilt_browser_string_values_strict_slicing_and_replacement_match_native` (ignored) | Browser/core bytes/WAT/diagnostics/registration via shared harness, then first-four UTF-8/NUL/copy/reuse, all boundary pairs of a mixed string, numeric errors, literal replacement, alias/interned strings, host-copy isolation and lexical/immediate stages. Also asserts fresh changed-empty replacement and proper-empty slice handles versus source. |
| B `rebuilt_browser_string_order_validation_retention_growth_and_reentry_match_native` (ignored) | First-four once-only effects and argument traps; post-entry descriptor corruption before reuse, entry validation, caught-trap publication, actual guest concat growth/detached views, reentry and retained strings/arrays. |
| B `rebuilt_browser_string_join_fixed_resizable_utf8_copies_and_captures_match_native` (ignored) | Browser join values, fixed/resizable captures, lexical/stage calls, singleton reuse, JS copy isolation, prior output unchanged after slot mutation. |
| B `rebuilt_browser_string_join_validation_order_reentry_growth_and_retention_match_native` (ignored) | Browser join descriptor/entry corruption after entry, argument order, real guest growth/reentry, retained output and early-trap stopping later effects. |

B's shared `run_browser` also checks arbitrary external names (including Unicode/NUL
field text), all registered imports required even if unused, no extra management
exports, browser compile/compileWat errors against native diagnostics, and exact
all-five reserved-host errors. Its Node bound is 60 seconds. Runtime adapters use
`runtime/abi.cjs` copies; they do not make Node encoding resize-authorized.

## Private observations and construction — all 11 O/A/K tests

| Named tests | What is actually observed/checked |
| --- | --- |
| O `removing_only_expected_counter_sites_restores_every_body_and_local` | Independent decoding removes only expected instrumentation, restores bodies/locals and checks original globals/sections/ordered export tuples; import-free and imported/M4 fixtures distinguish function from body indices. |
| O `join_observation_isolation_and_direct_final_payload_copying` | Same isolation for join alone and imported M4/concat composition; 12 i32 + one i64 auxiliary locals, validators then one static allocation, both byte stores target final payload, no intermediate concat/collection. |
| O `collisions_are_rejected_only_by_the_private_variant`; `join_collisions_are_rejected_only_by_the_private_variant` | Every string observation export/cursor collision fails only private rewriting; ordinary compilation remains identical/valid. |
| O `observations_are_private_and_deterministic` | Ordinary Compiler bytes equal unobserved fixture assembly; deterministic variants and expected private export presence/absence. |
| O `paired_concat_events_effects_growth_and_caught_trap_retention` (ignored) | `tests/support/m5_string_observations.cjs`: paired whole memories/handles, exact successful allocation/request/reservation and concat payload/descriptor/store deltas, empty reuse, argument effects, retained trap outputs, real growth, mixed string-array accounting, counter wrap and real capped-memory concat allocation failure. |
| A `removing_array_observations_restores_layered_bodies_locals_and_sections` | Strips only array instrumentation from the string-observed layer, restoring original body locals/sections/globals/export tuples; does not conflate the two layers. |
| A `array_export_collisions_do_not_affect_ordinary_compilation` | Private array counter/registry export collision errors without changing ordinary compilation. |
| A `paired_array_capacity_reuse_transient_peaks_copy_stores_and_failures` (ignored) | Fresh/reused acquisition, actual reserve relocation/no-op, constructor handle stores, capacities/release, old-plus-new acquisition-time peak, invalid request, capped backing-allocation failure, partial successful stores/acquisition retained after later trap, modular counter arithmetic; paired whole memory. |
| K `construction_corpus_is_valid_deterministic_and_observations_are_private` | Valid deterministic ordinary/observed modules, ordinary Compiler byte parity, exactly 15 private global exports only on observed variant. |
| K `paired_fragment_construction_is_linear` (ignored) | `tests/support/m5_construction.cjs`: paired whole-memory/content/accounting checks and deterministic accounting output; 120-second Node bound. O/A Node bounds are 60 seconds. |

### #79 acceptance cells inside K's JS harness

These are subcases/assertions of one Rust test, **not additional Rust tests**:

- **103 construction rows**: initial two-fragment case plus six patterns at 17
  sizes `0, 1, 2, 3, 4, 5, 8, 9, 16, 17, 32, 33, 64, 65, 129, 257, 1025`.
  Includes both sides of multiple growth boundaries, duplicates/empty occurrences,
  ASCII/multibyte fragments and empty/multibyte separators.
- **34 controlled concat baselines**: two nonempty fragment byte widths at those
  sizes, language optimization None with ordinary copy-based lowering and initial
  empty-operand reuse retained. Cumulative copied bytes independently expected as
  `f * (n(n + 1)/2 - 1)` for n >= 1, zero at n = 0; reservations summed per event.
  This is not a requirement that every future optimized concat remain quadratic.
- `construction` uses occurrence count n (including empty entries) and actual output
  UTF-8 bytes B (including separator occurrences), separates handle copying from
  string byte stores, and checks geometric capacities, active/free registry records,
  metadata/descriptors/padding and exact physical extents partitioning cursor growth.
  For n >= 2, storage <= `256(n + 1) + B + 8` and copying <= `8n + B`;
  singleton/zero cases have separate exact checks. This plus decoded lowering and
  growth policy supports the O(n + B) storage/copy argument, not a finite-samples-only
  proof of asymptotics or an O(n + B) runtime claim.
- Reports preexisting input byte-range union once and zero fragment-production cost
  for static-input rows. `productionExample` separately measures a produced six-byte
  concat fragment before accumulation and join; it does not hide production in overhead.
- Every row measures a separately retained second join with its own allocation/copy
  delta while keeping the first valid. Fixed-array cases (five shapes × three
  separators), separator mutation/relocation/rebinding and ordered effects augment it.
- Empty/singleton invalid separator/outer/entry cases assert no counter delta on
  failure. `lifetime` retains inputs/parts/two results over actual growth and a
  caught invalid reserve. This private case is **not** host reentry; R/C/B own that.
- `array_peak_active_bytes` is observed inside acquisition before old-buffer release;
  completed-call active capacity, retained physical capacity and monotonic bump
  space are separate. Category peaks are not summed into an invented simultaneous
  total. Engine RSS/general liveness remain unavailable.

String payload/descriptor attribution is a partition of successful physical bump
requests, **not extra storage to add again**. Success counters omit failed attempts;
completed stores/allocations survive later traps. Current exact allocation counts
are regression expectations for this implementation, not lasting mandatory language
identity/allocation rules. Counters wrap nontrapping i64; seeded wrap tests are not
measurement totals. Bounded accounting uses unsigned BigInt and separately stated
bounds below 2^64. No constant-retained-memory or general reclamation claim follows.

## Ordinary measurement correctness — all 7 T/N tests

| Named tests | Scope, not a timing result |
| --- | --- |
| T `runtime_corpus_is_ordinary_deterministic_and_preset_compatible` | Ordinary source/analyze+compile parity, valid/deterministic modules at all presets, enabled byte identity, exact ordinary export names without observations. |
| T `runtime_cases_have_independent_oracles_and_no_observation_exports` (ignored) | Runs `tests/support/m5_string_timing.cjs` in **check**, not timing, mode; asserts 123 checked cases; 120-second Node bound. |
| N `source_size_families_are_deterministic_valid_and_grow` | 21 generated source cases, deterministic growing source/module sizes, analyze/compile/source-compile parity and Validator. |
| N `raw_sample_schedule_and_records_are_complete_and_alternate_order` | 42 boundary/workload pairs, reversed alternate order, 3 repetitions × 5 samples = 630 records, iteration/warmup counts and exact sample formatting. |
| N `exported_corpus_matches_ordinary_compilation_and_never_overwrites` | 21 source/module pairs match ordinary compilation; refuses overwrite and preserves sentinel. |
| N `analysis_is_preset_invariant_and_enabled_presets_have_identical_bytes` | Typed analysis identity at all four presets, enabled bytes equal, valid source/typed compilation parity for all native-corpus cases. |
| N `ordinary_modules_execute_against_independent_string_results_at_all_presets` (ignored) | 84 workload/preset modules against independent UTF-8 content and byte-length results; correctness only, 30-second Node bound. |

#80 times fresh ordinary instances separately from private #79 accounting, and
native source compilation separately from compilation of an already-owned typed
program. Its workload includes repetitive replacement search. Consult its README
and retained raw samples/provenance for sampling, exclusions and variability;
these tests do not reproduce those measurements. Recorder Python tests and Divan
cases are outside the 99-test inventory.

## Gaps, limits and parent-owned closure

1. **Execution gap (all 99 inventory entries).** This audit executed none. Historical
   #73–#80 results are linked, not relabeled final-source passes. Parent must attach
   the stable-source manifest with actual nonzero per-target counts, exact argv,
   bounds, exit codes/durations and current-source browser rebuild provenance,
   including full workspace ignored tests, architecture/portability/Clippy gates.
   A native B fixture pass or old `pkg/` does not close browser execution.
2. **Measurement/release gap.** Parent must separately reproduce #79 accounting and
   #80 ordinary runtime/native measurements sequentially without competing work,
   retain raw samples/environment/executable-input hashes and stable-source checks,
   and explicitly disposition material variability. Neither 99 inventoried tests,
   historical successful recorders nor fresh aggregate-suite execution is isolated
   timing or integrated acceptance. M4's pending decision remains unchanged.
3. **Allocation-failure asymmetry.** Actual capped-memory exhaustion is directly
   covered for concat (O) and array backing growth (A). Slice/replace/join have
   decoded checked sizing/ordinary allocator calls, real-growth/lifetime tests,
   and replace/join pre-allocation overflow runtimes, but this inventory contains
   no dedicated capped-memory allocator-exhaustion execution for each of those
   three string helpers. Overflow rejection is not allocator exhaustion. No
   near-4-GiB real allocation is claimed; widened/unsigned decoded checks and small
   repeated-input overflow fixtures cover representability without machine exhaustion.
4. **Forged skipped/unreachable coverage is not Cartesian.** Join explicitly wraps
   malformed operand trees after return and under `false &&`, and forges a skipped
   pure contract. All five have forged type/category/child checks, pure-context
   rejection and structural discovery evidence; frontend/adapter skipped cases are
   broader. The other four do not each have the same full malformed-operand ×
   skipped/post-return × category matrix. Valid unreachable discovery and a bad
   later child after Never are different assertions; do not label them equivalent.
5. **Private observation coverage is intentionally narrow.** Operation-specific
   payload/copy counters instrument concat/join, not slice/replace/byte length.
   Generic bump totals do not supply those missing categories. Array transient
   active capacity is observed, but no simultaneous all-category active peak,
   failed-attempt counter, engine RSS, general liveness or production telemetry
   exists here. This is enough to target fragment construction, not arbitrary
   allocation attribution or general memory reclamation.
6. **Reuse/empty identity evidence has different strengths.** Core R asserts
   content for changed-empty replacement and proper-empty slice without universally
   requiring fresh identity; decoded lowering and ignored rebuilt-browser tests
   add that current-policy evidence. Concat both-empty right identity is decoded
   and explicitly exercised in B with distinct empty handles. Join's zero/all-empty
   allocation policy is checked by K accounting/decoded lowering, while singleton
   reuse and positive multi-entry disjointness have direct R assertions. The
   representation-independent allowance for shared empty results must not become
   a permanent fresh-allocation requirement.
7. **Compatibility scope is bounded.** Exact absent-M5 golden bytes cover the
   existing control fixture; string-array-only checks cover helper count/order and
   direct-codegen parity, not archived pre-M5 bytes for every possible non-M5
   program/preset. Production `cfg(test)` exclusion plus ordinary/observed stripping,
   ordinary exact surfaces and adapter byte parity establish isolation at inspected
   seams, not an exhaustive proof over all programs/options. Parent's broader
   unchanged golden/workspace/M4 compatibility gates remain necessary.
8. **Trust/non-goals are not bugs to “cover” by changing semantics.** No full UTF-8
   validation, malicious-host isolation, rollback, mutable string/builder type,
   string `+`/`len` overload, fusion, production controls, M4 recycling of published
   strings, scalar/grapheme indexing, constant retained memory, guaranteed speedup
   or linear-time replacement is asserted. Tests deliberately corrupt host storage
   only to isolate structural checks. Node marshaling remains copy-based and fixed
   on encoding; movable array payload/length must be reacquired separately from
   refreshing views after memory growth.

The mapped sources cover both immutable-operation and fragment-construction
contracts at their stated seams. The limitations above are explicit coverage or
evidence boundaries, not a fabricated pass, an inferred implementation defect,
or an authorization to expand scope. **Final M5 acceptance is not decided here.**
