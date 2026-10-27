# M2 temporary regions — integrated contract and validation (#57)

**Private prototype implemented; integrated gates passed on 2026-10-27.** This
record reconciles #54's implementation, #55's adversarial safety tests and #56's
measurements against the [confirmed M2 contract](was_compiler_memory_and_optimization_roadmap.md#m2-interview-decisions-2026-10-26-confirmed).
It closes the private milestone's validation work, not public activation,
production storage selection, general reclamation or a release/default change.

## Agreed scope and implemented boundary

The [prototype contract](m2-region-prototype.md) describes actual storage behavior;
[M1's closed positive subset](m1-lifetime-analysis.md#closed-positive-subset)
remains the only eligibility proof. Only direct function-body let-bound number/bool
array literals in the invocation-wide closed subset qualify. Unsupported syntax,
including unreachable syntax, prevents selection. Classification alone changes no
storage. Aliases, calls, branches, loops, captures and string arrays remain out.

`crates/compiler/src/lib.rs` gates `lifetime` with `cfg(test)`;
`crates/compiler/src/codegen/mod.rs` likewise gates `region`, including its
instrumentation and consumers. Region fields and emission hooks are test-gated.
The private `region::compile` and `ValidatedProgram::with_region_selection` use
existing layout, checked source-order body lowering, cached bodies and assembly.
Layout validation precedes selection. No second validator, public AST field,
option, feature, dependency or adapter activation exists. Production native and
Wasm builds succeeded without the machinery; this exclusion also follows from
the source gates, not merely from successful compilation.

Every selected function has one private i32 cache global and each invocation one
i32 base local; there is no linear-memory management header. All candidates,
including unreachable suffixes, contribute individually aligned fixed slices:
number length N uses `8 + 8*N`, bool uses `align8(8 + N)`, and empty arrays use 8.
Checked unrepresentable totals discard the entire plan and select baseline
storage with private `Selection::CapacityExceeded`, not a new compilation error.

The first executed literal takes and clears its function's cache, or provisions
the complete exclusive block through the checked ordinary allocator. Each literal
initializes its descriptor before once-only left-to-right element evaluation.
Normal scalar/bare return or void fallthrough retires an acquired block only after
result computation. No literal execution means no acquisition or retirement.
There is no global heap rewind, intra-invocation reuse, relocation, region growth
or cross-function pooling.

Traps bypass retirement, quarantining the entire acquired block for instance life
and preserving completed writes. Failed provisioning traps without per-literal
retry, without changing the allocator cursor or existing storage. Front-loaded
reservation can grow memory or fail before an earlier initializer's access trap;
these are accepted resource/failure-order differences, not baseline parity.
Ordinary allocations and legitimately published handles retain revision-1 rights.
Observing retired experimental temporary addresses through raw memory does not
create instance-lifetime retention rights. Host descriptor and view-refresh
obligations remain unchanged.

## Final contract-to-test map

All paths are repository-relative. Prefixes below name the actual final files;
case names are exact. These are existing tests, freshly executed for #57; no code,
fixture or test expectation was changed for this validation task.

- **R:** `crates/compiler/src/codegen/region/tests.rs`
- **N:** `crates/compiler/src/codegen/region/runtime_tests.rs`
- **M:** `crates/compiler/src/codegen/region/measurement_tests.rs`

| Obligation | Final evidence |
| --- | --- |
| Exactly M1's closed proof; excluded/unreachable forms and no-candidate programs get no machinery | R: `excluded_forms_and_no_candidates_emit_no_region_machinery`; `crates/compiler/src/lifetime.rs`: `lifetime_excluded_forms_block_the_entire_invocation_even_after_return`, `lifetime_contracts_and_handle_parameters_cannot_bypass_the_subset` |
| Both widths, empty arrays, bool alignment boundaries, distinct slices and unreachable capacity | R: `checked_capacity_includes_aligned_empty_and_unreachable_literals`; N: `reuse_reinitializes_distinct_aligned_literals_and_preserves_ordinary_handles` |
| Deterministic whole-function overflow fallback, no partial region, ordinary byte identity and malformed-child rejection | R: `unrepresentable_inventory_discards_all_slices_and_keeps_a_deterministic_reason`, `capacity_overflow_lowers_the_whole_function_with_baseline_storage`; synthetic footprints reach the real selection transition and checked assembly, rather than allocating a multi-GiB AST |
| Lazy acquisition, no executed literal, cached block survives a pre-acquisition trap | N: `acquisition_is_lazy_after_scalar_contract_and_absent_for_unreachable_literals`; M: `record_m2_accounting` |
| Cache take/clear, provisioning, descriptor-before-elements and normal retirement order; no heap rewind | R: `checked_capacity_includes_aligned_empty_and_unreachable_literals`; `crates/compiler/src/codegen/region.rs`: `selected_function_has_private_cache_and_checked_array_stores`; validator plus independent decoded instructions/globals/stores |
| Complete initialization on reuse, changing inputs, reads/replacement, all five numeric compounds, scalar/bare/early returns and void fallthrough | N: `reuse_reinitializes_distinct_aligned_literals_and_preserves_ordinary_handles`; poisoned idle bool storage exposes stale descriptor/element reuse; early return reserves but does not execute its suffix |
| Whole-block lifetime quarantine after initializer/access traps, completed writes, repeated traps and fresh recovery | N: `traps_quarantine_owned_blocks_and_keep_completed_initialization_and_writes`; twelve distinct read-trap blocks plus initializer/bool blocks survive later calls and growth; M: `record_m2_accounting` |
| Separate function caches, successful plateau, unreachable function acquires nothing | M: `record_m2_accounting`, `record_m2_capacity`; `tests/support/m2_region_measure.cjs` asserts independent events/results and per-function capacity |
| Ordinary retained number/bool/string arrays, suspended recursive/reentrant callers and live captures stay isolated | N: `reentrant_recursive_callers_keep_retained_arrays_and_captures_isolated`, `reuse_reinitializes_distinct_aligned_literals_and_preserves_ordinary_handles`; callers remain excluded and eligible callees remain call-free |
| Representable provisioning failure: no retry, cursor corruption, retained-storage damage or false quarantine | N: `failed_front_loaded_reservation_preserves_cursor_and_ordinary_storage`; M: `record_m2_failed_provisioning`; one-page caps distinguish failed acquisition from acquired-then-trapped blocks |
| Accepted earlier reservation/failure/growth versus baseline access traps; unreachable initializer does not run | N: `front_loaded_reservation_precedes_earlier_initializer_trap`, `capped_later_literal_reservation_changes_failure_and_growth_before_access_traps`; independent cursor/descriptor/page/sentinel observations |
| Independent forged annotations/IDs, skipped/unreachable/hidden children, layout-first and source-order diagnostics | R: `malformed_unreachable_candidate_and_invalid_function_id_preserve_error_order`, `forged_binding_ids_and_skipped_children_keep_ordinary_diagnostics`, `mixed_selected_and_excluded_hidden_bodies_keep_source_order_errors`, `sparse_binding_ids_still_select_and_lower_checked_storage`, `ordinary_bytes_presets_exports_and_diagnostics_remain_independent` |
| Deterministic modules, unchanged typed input/ordinary bytes and diagnostics within each preset, enabled-preset byte identity | R: `ordinary_bytes_presets_exports_and_diagnostics_remain_independent`; N's `modules` helper checks every scenario at None/Basic/Standard/Aggressive; M: `measured_presets_are_deterministic_and_enabled_bytes_match` |
| Instrumentation only when explicitly selected; no calls, changed classification or production inspection exports | M: `instrumentation_is_explicit_and_adds_no_calls_or_storage_selection_changes`; source `cfg(test)` gates and independent import/call/global/export decoding |

Ordinary compatibility remains a separate boundary:

- `crates/compiler/tests/abi.rs` retains
  `memory_abi_preserves_baseline_bytes_and_determinism_at_every_level` and
  `static_descriptors_match_independent_little_endian_bytes`.
- `crates/compiler/tests/typed_arrays_runtime.rs` retains
  `raw_host_retained_typed_arrays_survive_recursive_reentry_traps_and_rhs_growth`;
  `crates/compiler/tests/closures_runtime.rs` retains
  `reentrant_live_contexts_and_retained_handles_survive_inner_traps_and_growth`.
- `crates/compiler/src/lifetime/noninterference_tests.rs` preserves ordinary
  compiler/direct-codegen behavior; the full run also covers ABI, emitter and
  diagnostic fixtures without regeneration.
- `crates/cli/tests/typed_arrays_adapters.rs`, `crates/browser/tests/typed_arrays_adapters.rs`
  and `tests/browser.cjs` establish ordinary CLI/library/current-source browser
  byte, diagnostic and execution compatibility. They do **not** activate M2.

Runtime tests validate Wasm and execute finite guest workloads through the bounded
subprocess helper. Seven N tests and four M recording tests remain ignored by
default; explicit inclusion selected them. Native assertions and runtime effects
are independent evidence beyond Wasm validity. Deadlines are not guest fuel or
process-tree cancellation.

## Measurement reconciliation

The [#56 evidence and definitions](benchmarks/m2-temporary-regions-2026-10-27/README.md)
remain historical measurements, not fresh timing claims from the correctness run.
The audit verified all twelve input-manifest files against current byte lengths
and SHA-256 hashes, all sixteen recorded JSON source/harness identities, and the
48 available module artifacts against recorded sizes/hashes before validation.
Rust source CRLF normalization yields the preserved 345-byte workload exactly.
Fresh correctness tests subsequently regenerated transient `target/` artifacts;
the preserved #56 JSON/timing samples were not replaced by those runs.

| Observation | Reconciled result and limit |
| --- | --- |
| 256 successful interleaved calls per function | Numeric capacity 24 and aligned multi-array capacity 48 bytes: one provision and 255 reuses each; combined successful backing plateaus at **72 bytes** after warm-up. Unreachable `lazy` reserves a 16-byte plan but acquires nothing. |
| Eight later numeric traps and recovery | **8 quarantined blocks / 192 bytes**; the aligned function keeps its separate 48-byte cache. Recovery provisions a new numeric block, then reuses it; combined capacity is 192 quarantined + 72 cached = 264 bytes. Repeated traps can exhaust memory. |
| Separate uninstrumented marker instances | **18,432 baseline / 72 prototype aligned bump bytes**. `after - before - 8` accounts for the first marker's minimum reservation; both marker calls allocate eight bytes and can grow memory. This is consumption, not live/reclaimed bytes. |
| Separate fresh unmarked instances, 4,096 calls per function | **1 → 5 baseline / 1 → 1 prototype pages**. Pages are capacity, not occupancy or peak live memory. |
| One-page failed provision | Only 16 bytes remain for a 24-byte block: trap, unchanged bytes/cursor, zero events and zero quarantine. Failure-mode markers/pages are diagnostics, not the unmarked capacity experiment. |
| Uninstrumented module cost | None: 1,350 → 1,534 bytes; enabled: 1,314 → 1,498. **184-byte prototype regression**; instrumentation adds another **477 bytes**, separately reported. No in-memory header; three cache globals and one base local per selected body. Physical engine overhead is unavailable. |
| Timing | Uninstrumented/unmarked, three alternating-order repetitions, ten samples each, 512 guest calls/sample and independent checksum 68,864. JS loop/call/guest work is included; compilation, instantiation, validation, warmup, accounting and reporting are excluded. Recorded medians favor this prototype but noise, short samples and tiering limits prevent a general speedup conclusion. |

Provision/reuse/literal-entry/retirement counters have the exact meanings in #56.
Quarantine and cached capacity are derived only at synchronous quiescent boundaries,
with no suspended invocation. Reused-block bytes count the same capacity across
reuse events, not unique reclaimed storage. A literal entry is counted after
acquisition, before initialization; failed provisioning counts neither acquisition
nor quarantine. General baseline allocation counts, peak live bytes and RSS remain
unavailable. The ordinary 19-workload M0 corpus was not made M1-positive.

## Fresh integrated validation — 2026-10-27

Source revision: `9bbfc79f3cc4e0778c10c6f7cf2ce67bba6b6843` (final #56).
Before validation the working tree contained only a pre-existing roadmap edit and
`examples/sum.was` deletion; both were preserved. Compiler, adapters, shared tests,
runtime, manifests, lockfile and fixtures remained at that revision throughout.
Only documentation/evidence packaging changed after the stable run.

[Auditable command evidence](m2-temporary-regions-validation-2026-10-27.json)
records exact commands, exit codes, durations, 180-second per-command limits,
per-target summaries, full-log hashes, source status, tool versions and rebuilt
package hashes. Complete transient logs are `target/m2-57-*.log`. Commands below
ran sequentially in the documented fast/full order; no retries or timeouts.

| Command | Exit | Actual result |
| --- | ---: | --- |
| `cargo test --offline -p was-compiler --lib codegen::region -- --include-ignored` | 0 | 23 passed, zero failed/ignored; seven adversarial Node and four measurement recording cases included |
| `cargo test --offline -p was-compiler --lib lifetime -- --include-ignored` | 0 | 42 passed, zero failed/ignored |
| `cargo test --offline -p was-compiler --lib codegen::memory_tests -- --include-ignored` | 0 | 21 passed, zero failed/ignored |
| `cargo test --offline -p was-compiler --test typed_arrays_lowering` | 0 | 2 passed |
| `cargo test --offline -p was-compiler --test typed_arrays_runtime raw_host_retained_typed_arrays_survive_recursive_reentry_traps_and_rhs_growth -- --ignored --exact` | 0 | 1 passed |
| `cargo test --offline -p was-compiler --test closures_runtime reentrant_live_contexts_and_retained_handles_survive_inner_traps_and_growth -- --ignored --exact` | 0 | 1 passed |
| `cargo test --offline` | 0 | **1,196 passed, 219 intentionally ignored**, zero failed; 69 unit/integration targets plus one empty doctest target |
| `cargo clippy --offline --all-targets -- -D warnings` | 0 | Passed |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | 0 | Passed; production portable core |
| `cargo test --offline -p was-compiler --test workspace_contract` | 0 | 3 passed; production/downstream dependency boundary |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | 0 | Fresh current-source browser package rebuilt before full tests |
| `cargo test --offline --workspace -- --include-ignored` | 0 | **1,428 passed, zero failed/ignored**; 80 unit/integration targets plus one empty doctest target, all packages/native runtimes/browser selected |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | 0 | Passed |
| `node tests/browser.cjs pkg` | 0 | Additional explicit smoke run after the chain: 25 groups passed |
| `git --no-pager diff --check` | 0 | Final documentation/evidence whitespace check passed |

Environment: Windows, rustc/Cargo 1.98.0, Node 24.12.0, wasm-pack 0.15.0,
installed `wasm32-unknown-unknown`, cached Rust dependencies and compatible
wasm-pack-resolved wasm-bindgen tooling. `wasm-bindgen` was not on PATH. The
successful no-install build printed “Installing wasm-bindgen” as its tooling
resolution message; no separate installer or fetch command was invoked. The
only packaging notice concerned optional description/repository/license metadata.
No prerequisite or validation gate remains pending for this private milestone.

## Decision and deferred work

The confirmed M2 obligations reconcile with the implementation, actual tests and
measurements. Fresh integrated correctness/noninterference gates pass. Revision-1
retention, fixtures, preset defaults and raw-host obligations remain unchanged.
No substantive code fix was required, so no new red/green test seam was introduced.

Two-axis review used task-start revision
`9bbfc79f3cc4e0778c10c6f7cf2ce67bba6b6843` against the scoped uncommitted changes,
including the new record and JSON. Pre-existing roadmap sections and the deleted
example were excluded. Standards reported zero hard violations or judgement-call
findings. Spec identified one missing durable entry for the final whitespace
command; that evidence gap was corrected and rechecked. No allocator, contract,
noninterference or measurement finding remained. No issue or parent was modified,
and no changes were staged or committed.

Public activation needs a separate decision. Wider M1 proofs, tighter reachable
sizing, cross-function pooling/eviction, trap recovery/reclamation, managed heaps,
ordinary/capture reclamation, public profiling and release/default changes remain
deferred. Successful-call plateau is not generalized bounded memory; the recorded
size overhead and timing limits are retained rather than treated as release approval.
