# M6 contract-to-test map — #87

This is an inventory of current tests and recorded evidence, not an execution log or
release decision. See [M6 release readiness](m6-release-readiness.md) for the integrated
commands, counts, gate outcomes, and acceptance status. The browser entries below
are test assertions, not execution results.

## Scope and implementation boundary

[ADR 0011](adr/0011-bounded-m6-range-scalar-replacement.md), the [confirmed M6
contracts](was_compiler_memory_and_optimization_roadmap.md#m6-interview-decisions-2026-11-16-confirmed),
and the [private range ABI](memory-abi.md#private-range-record) govern this map.
Issue #82 implements the narrow [exception to ADR 0001](adr/0001-private-range-representation.md#later-exception-direct-range-loops-82):
only a `For` iterable rooted in a typed `Range` can be scalar-replaced. Grouping
parentheses, effectful or divergent operands, and placement inside a local callable
do not disqualify it. Other range values keep their allocated representation.

[`OptimizationLevel::range_policy`](../crates/compiler/src/optimization.rs) selects
this independently of integer specialization; [`Policy::scalar_replaces`](../crates/compiler/src/codegen/range.rs)
is shared by [scratch planning](../crates/compiler/src/codegen/temporaries.rs) and
[loop lowering](../crates/compiler/src/codegen/control_flow.rs). Explicit `None`
and direct codegen remain baseline. No new syntax, public flag, descriptor,
host release obligation, or heap reset is part of M6. Closure specialization, string
fusion, temporary-array reuse, and compact layouts remain deferred.

## Source, forged input, and decoded Wasm

| Contract | Current assertions |
| --- | --- |
| Source eligibility, types, and preset-neutral analysis | [`for_frontend.rs`](../crates/compiler/tests/for_frontend.rs): `grouping_preserves_direct_range_root_and_presets_do_not_change_analysis` compares the typed grouped range and block wrapper at all presets. `range_for_exposes_a_numeric_iteration_binding` pins the binding and iterable types; `for_accepts_range_values_from_existing_expression_forms` accepts identifier, call, block, conditional, and value-loop iterables as well as direct ranges. Acceptance alone does not prove a lowering choice. |
| Source checks survive abrupt completion and skipping | [`for_frontend.rs`](../crates/compiler/tests/for_frontend.rs): `never_range_start_does_not_hide_later_operands_or_readonly_capture_writes` checks end-before-step errors and a read-only capture write, with spans; `bodies_and_later_iterable_children_are_checked_despite_divergence_or_skipping` covers hidden children. `iterable_transfers_target_enclosing_loops_not_the_new_for` and `iterable_resolves_outer_names_before_introducing_the_iteration_binding` pin target and scope. [`range_frontend.rs`](../crates/compiler/tests/range_frontend.rs): `malformed_ranges_and_chains_are_parser_errors_with_locations` and `invalid_numeric_domains_are_not_new_static_errors` separate syntax errors from runtime numeric traps. |
| Direct path and module infrastructure | [`for_lowering.rs`](../crates/compiler/tests/for_lowering.rs): `enabled_presets_remove_only_the_direct_loop_record` validates and decodes the grouped direct case: explicit `None` vs. direct-codegen byte equality, all enabled bytes equal, no calls or record loads in this simple direct body, and start/end/step saved before ordered domain/integrality guards and the step nonzero guard. It checks three signed conversions, integer advancement, unchanged block-wrapper and helper bodies at the same indices, and identical non-code sections. This is not a ban on calls or loads in effectful operands or bodies. |
| Exact induction, cached record path, and structured branches | [`for_lowering.rs`](../crates/compiler/tests/for_lowering.rs): `iteration_binding_and_scratch_have_exact_types_and_order`, `iterable_finishes_and_record_is_cached_before_for_regions_and_binding`, and `nested_expression_transfers_decode_latch_and_exit_depths_not_fixed_depths` pin baseline scratch, validation/cache order, signed comparisons, and latch/exit depths. [`for_loops.rs`](../crates/compiler/tests/for_loops.rs): `decoded_for_uses_integer_induction_empty_regions_and_a_continue_latch` independently decodes the baseline empty regions, `I64Add`, yield conversion, and branches. `iterable_transfers_use_the_enclosing_loop_never_the_new_for` in `for_lowering.rs` decodes enclosing depths at **all** presets for transfers in direct start/end/step and indirect iterables, and rejects forged transfers to the new loop. |
| Forged annotations, operands, bindings, and targets | [`for_lowering.rs`](../crates/compiler/tests/for_lowering.rs): `range_annotations_do_not_disguise_numbers_or_forged_never_results`, `genuine_never_iterable_preserves_completion_but_still_checks_the_body`, and `direct_range_checks_later_operands_after_abrupt_completion_before_checking_body` check annotations, later operands, and error precedence. `iteration_binding_annotations_are_checked_against_numeric_storage`, `ordinary_and_all_compound_writes_reject_exact_readonly_id_and_span_everywhere`, and `both_transfers_reject_unknown_inactive_outer_and_cross_function_targets` cover binding and transfer checks. The rejection helper compares exact errors and spans through direct codegen and every preset with `All`, `None`, and selected exports, and checks that the typed program is not mutated. |
| Unexported/local callable checks | [`for_loops.rs`](../crates/compiler/tests/for_loops.rs): `hidden_unreachable_for_bodies_still_reject_forged_targets_and_value_breaks`. [`closures_lowering.rs`](../crates/compiler/tests/closures_lowering.rs): `checked_lifecycle_orders_layout_all_user_bodies_and_helpers_in_preorder` and `iteration_captures_reject_forged_writes_even_after_return_or_in_skipped_operands` cover helper lifecycle without a range loop and capture writes in an array loop, respectively. They cover the shared checks indirectly, not forged direct-range lowering inside a local callable. |
| Existing private range and host boundaries | [`ranges.rs`](../crates/compiler/tests/ranges.rs): `decoded_record_stores_and_mvp_regions_use_exact_integers_and_i32_handles`, `forged_range_children_in_skipped_arms_and_after_abrupt_inputs_are_not_pruned`, and `only_selected_range_signatures_are_rejected_but_every_host_snapshot_is_checked`. [`range_tests.rs`](../crates/compiler/src/codegen/range_tests.rs): `internal_range_parameters_are_validated_as_records_not_descriptors` and `range_helpers_follow_remainder_even_when_only_hidden_construction_needs_them` pin the allocated path and discovery order. [`for_loops.rs`](../crates/compiler/tests/for_loops.rs): `owned_import_snapshots_and_export_selection_preserve_bodies_and_indices` compares non-export sections for `All`/`Only`/`None` within each preset and pins import order and `memory`/`__alloc` indices. |

The block wrapper has an independent decoded negative control; binding and call
fallbacks have decoded constructor checks and allocation observations below. The
conditional/block/value-loop alias fixture exercises reuse of an already-allocated
handle, not separate constructor counts for every wrapper. The typed-root rule also
excludes range-returning pipelines; this map identifies no dedicated M6 pipeline-as-
iterable decoded or allocation fixture. Whole-module equality across presets is only
asserted for controls without either an eligible loop or unrelated integer rewrites;
it is not an O0–O3 byte-identity promise for arbitrary programs.

## Bounded core execution (#83 and existing range regressions)

The named [`for_loops.rs`](../crates/compiler/tests/for_loops.rs) Node tests are
ignored by default. They run `None`, `Basic`, `Standard`, and `Aggressive` against
independent expectations (not just against each other), require enabled byte identity,
and compare source/typed compilation and analysis for the executed fixtures.

| Contract | Current assertions |
| --- | --- |
| Full-domain progression rules | `node_exact_progressions_match_bounded_bigint_reference_and_membership` uses a bounded BigInt reference, literal edge expectations, and exhaustive *small-domain* cases. It covers both step signs, default step, inclusive/exclusive ends, equal or reversed bounds, canonical yielded `+0`, safe-integer extremes, and normal exhaustion after advancement beyond the safe domain. It cross-checks membership; it is not exhaustive over the entire safe-integer domain. |
| Once-only effects and validation before yield | `node_iterable_and_endpoints_evaluate_once_left_to_right_before_validation` checks literal traces, no yields after invalid inputs even for an empty direction, and no effects for the skipped case. It tries each field with nonfinite, fractional, subnormal, and out-of-domain values and both zero signs for the step. The decoded test above pins the order of the guards themselves; the runtime trap type does not distinguish which guard failed. |
| Transfers, enclosing context, and scope | `node_iterable_transfers_use_enclosing_targets_and_return_before_body` checks literal traces for outer break/continue, value break, return, nested direct operands, and a `while` condition; later effects and the body are suppressed on abrupt paths. `node_body_transfers_advance_through_nested_regions_and_restore_shadow_scopes` covers the body latch, break, return, nested loops, and mutable inner shadowing. |
| Captures, reentry, and published lifetimes | `node_direct_loop_captures_and_published_arrays_survive_recursion_reentry_and_traps` exercises live transitive callable dependencies, recursive and host-reentrant calls, a caught bounds trap after publication, growth while the outer loop is suspended, and subsequent calls. It checks literal traces, completed writes, shared identity and distinct invocation handles **within** each instance. [`closures_runtime.rs`](../crates/compiler/tests/closures_runtime.rs): `captured_range_iterations_keep_fresh_monotonic_eight_byte_cells` checks distinct retained cells and their environments across calls; `captured_iterations_allocate_only_successful_yields_even_when_declarations_are_skipped` covers yield-dependent promotion. |
| Allocated range compatibility | `node_consumed_records_validate_before_yield_and_fields_are_cached_before_body` checks corrupt fields before even empty iteration and confirms no reread after caching. [`ranges.rs`](../crates/compiler/tests/ranges.rs): `node_records_pin_bytes_safe_limits_signed_zero_default_steps_and_fresh_construction`, `node_invalid_discarded_empty_and_constant_ranges_trap_after_all_operand_effects`, and `node_internal_calls_blocks_if_loops_and_copies_share_one_immutable_record` cover ordinary allocated records. [`range_membership.rs`](../crates/compiler/tests/range_membership.rs): `node_membership_validates_corrupted_records_before_invalid_candidates` checks record validation before the membership guard. Private-record corruption is a test probe, not supported host marshaling or a guarantee against malicious host writes. |

Subprocess deadlines and host effect caps bound these fixtures; they are not guest
fuel. Growth in the lifetime fixture checks live values across a change of memory
buffer, not equal page counts or offsets across presets. ADR 0011 permits different
later addresses, growth, and allocation-exhaustion outcomes; exhaustion parity is
neither required nor tested here. Ordinary invalid-input traps remain protected.

## Core, CLI, and browser parity (#85)

[`m6_adapter_cases.rs`](../tests/support/m6_adapter_cases.rs) supplies `core_matrix`
and `diagnostics` to both adapters. The five fixtures cover selected and no user
exports, a binding/call fallback alongside direct loops, an indirect-only control,
a range-free control, and a direct-only module. The shared core reference validates
all modules, checks default against explicit `Basic` and explicit `None` against
direct codegen, enabled byte identity, unchanged bodies with no user exports,
and retained helper bodies/indices
even after the direct-only module and a local helper stop calling the range helpers.

| Target | Test and comparison |
| --- | --- |
| [CLI](../crates/cli/tests/m6_range_adapters.rs) | `cli_bytes_and_wat_match_core_in_each_preset` compares actual CLI binary and WAT output to core bytes and their printed WAT, including unset CLI default. `cli_range_diagnostics_match_core_in_each_preset` pins rendered parse/semantic errors (UTF-8/CRLF and an unreachable invalid step type), private range export/host-parameter/host-result errors, and CLI guidance. Ignored `cli_range_modules_execute_with_expected_values_effects_and_traps` runs actual CLI Wasm through the shared oracle. |
| [Browser](../crates/browser/tests/m6_range_adapters.rs) | Ignored `rebuilt_browser_matches_core_and_executes_range_modules_at_every_preset` loads the compiler from `pkg/` with the [browser Node harness](../tests/support/m6_adapter_browser.cjs). It checks default binary output, binary and WAT per preset, both compilation APIs' exact rendered errors and host-registration guidance, then runs actual browser-produced guest bytes through the same oracle. Requires a fresh package; this is Node execution of the web-target package, not browser UI testing. **Not run for #87.** |

The [shared runtime oracle](../tests/support/m6_adapter_runtime.cjs) checks literal
results/traces, local captures, abrupt return, empty and nonempty progressions,
safe-integer edges, traps and recovery. It checks exact export names/kinds, keyword
host module/field names and the requirement to supply unused imports. The no-user-
exports case exercises raw `memory`/`__alloc`, not an invocation of a hidden function.
See [#85's record](m6-range-adapter-compatibility.md) for its historical validation;
these adapter tests do not attribute allocation deltas or measure timing.

## Deterministic record elimination (#84)

- [`range_allocation_tests.rs`](../crates/compiler/src/codegen/range_allocation_tests.rs),
  `ordinary_lowering_and_private_observation_are_deterministic`: validates both
  ordinary and observed Wasm. It identifies the constructor by decoded `i64` stores
  at offsets `0`, `8`, and `16`, pins its single `__alloc(32)` call and checks call
  sites in ordinary/direct, local helper, abrupt, binding, and call fixtures. It
  compares non-constructor bodies, checks repeatability, and verifies the observation
  exports are absent from ordinary bytes. A constructor call site in a baseline
  abrupt fixture does not establish a runtime allocation.
- The ignored `attributable_range_record_elimination` runs ordinary/observed pairs
  with [the WAS fixture](../tests/support/m6_range_allocations.was) and [Node
  harness](../tests/support/m6_range_allocations.cjs). A test-only rewrite of a copy
  counts successful constructor reservations and exposes the existing bump cursor.
  At checkpoints, return, or caught trap, subtracting `32 × record delta` from the
  cursor delta separates records from operand arrays, iteration cells, closure
  environments, and body arrays. For completed eligible loops (including zero
  yields), the fixture checks **one 32-byte record under `None`, zero when enabled**.
  Binding and call controls retain one under every preset; abrupt operands or
  invalid completed operands allocate no record on either path. Ordinary and
  observed runs use independent results and traces, including repeat calls and
  recovery on the same instance.
- [`for_loops.rs`](../crates/compiler/tests/for_loops.rs),
  `node_direct_loops_remove_records_but_keep_operand_body_and_capture_allocations`,
  independently brackets ordinary modules with `__alloc(8)` to check expected total
  bump bytes for nonempty/empty, recursive, reentrant, and trapped calls. This
  cross-check does not itself identify allocations in arbitrary modules.

See [#84's allocation record](m6-range-allocation-evidence.md) for the repeated
trace and module digests. These are successful reservations in bounded fixtures,
not production telemetry, physical memory, page-growth or exhaustion measurements,
proof of allocation-free loops, or evidence of reclamation or timing benefit.

## Ordinary measurements and release gates (#86–#87)

[`m6_benchmarks.rs`](../crates/compiler/tests/m6_benchmarks.rs),
`measurement_modules_keep_the_allocated_baseline_and_preset_parity`, exercises
[corpus preparation](../crates/compiler/benches/support/m6_corpus.rs): Wasm validation,
source/typed byte parity, `None` vs. direct codegen, and enabled byte identity for
direct, binding, and call workloads. This test does **not** run or validate timings.
The [native recorder](../crates/compiler/benches/m6_ranges.rs) measures source-to-Wasm
and pre-analyzed typed-to-Wasm separately. The [guest recorder](../tests/support/m6_range_timing.cjs)
times ordinary modules in fresh instances per sample after warmup, checks sums
independently, and measures repeated 4- and 1,024-trip calls with both fallback
controls. It includes JS/export crossings and guest work, not startup or warmup.
Size is raw Wasm bytes; none of these measurements uses the private observer.

The [#86 evidence](benchmarks/m6-ranges-86/README.md) links environment/provenance,
raw samples and module hashes for two sequential sessions, and the
[all-sample summaries](benchmarks/m6-ranges-86/results.md). It records matching
module bytes across sessions and an independent pre-#82 CLI baseline for **these
three inputs** at `None`. The direct module grew 89 bytes; both controls were
unchanged. Short-loop medians improved, long-loop medians were slightly slower,
native medians increased even for unchanged-output controls, and guest timing had
unexplained slow samples. These synthetic full-preset comparisons neither isolate
the pass's compile cost nor establish a general speedup. No timing threshold or
size reduction substitutes for deterministic record elimination.

The [validation workflow](validation.md#finite-direct-commands) and ADR 0011 require
final-source native tests and strict Clippy, portable core and workspace-boundary
checks, rebuilt browser and strict browser Clippy, and full ignored-workspace
execution. See the [sibling release record](m6-release-readiness.md) for results and
explicit disposition of the M6 timing/size signals and pending M4/M5 performance
signals. Neither the inventory nor historical slice evidence closes those gates,
accepts M4/M5 implicitly, or changes the default preset.
