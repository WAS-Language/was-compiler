# M3 allocation infrastructure — integrated validation (#63)

The approved private M3 prototype is implemented and characterized. On 2026-10-28,
final-source correctness and noninterference gates passed, including the private
library-unit seam, M2 regressions, portable builds and freshly rebuilt ordinary
browser compilation. This is **private milestone acceptance**, not a release,
default change, production consumer approval or a complete memory manager.

## Scope and implementation boundary

The confirmed requirements remain in [roadmap §7](was_compiler_memory_and_optimization_roadmap.md#7-milestone-m3--allocation-infrastructure-and-reclamation-policy).
The [frozen private interface](m3-private-allocator.md) specifies transport, record
layout, observations and failure ordering. This document is the integrated
contract-to-test/evidence map; the earlier records keep their historical scope:
[#58/#59/#61](m3-private-allocator.md), [#60](m3-invalid-release-evidence.md) and
[#62](benchmarks/m3-allocation-infrastructure-2026-10-28/README.md).

- `crates/compiler/src/codegen/allocator.rs` and its children are private under
  `cfg(test)`. Explicit `compile`/`compile_instrumented` use the existing
  `ValidatedProgram::with_policy`, baseline numeric policy, checked body cache and
  assembly. Fixtures must already require ordinary memory. There is no second
  typed-tree validator, public option, Cargo feature or adapter activation.
- `codegen::assemble_inner` appends private types, functions, globals and exports
  after existing entries, including callable helpers. Ordinary `assemble` supplies
  no allocator configuration; production excludes the module and append branch.
  `memory::Helper::ALL`, source single-result signatures and revision-1 descriptors
  are unchanged. The private multivalue result is not a source ABI change.
- M3 uses the ordinary checked allocator for fresh backing. M2 remains independent:
  no migration, shared pool, changed per-function cache, normal-return retirement
  or permanent trap quarantine. [Integrated M2 evidence](m2-temporary-regions.md)
  is historical; fresh M2 checks here establish noninterference, not M3 consumption.
- A registry proves management membership, not that arbitrary references are dead.
  Ordinary arrays, strings, captures and compliant host-retained descriptors keep
  [revision-1 retention rights](memory-abi.md#contract-revision-1--2026-10-26).

## Final contract-to-test map

All case names below are exact, existing tests, freshly executed on the same stable
sources. Paths are repository-relative. Within
`crates/compiler/src/codegen/allocator/`, prefixes are:

- **D:** `tests.rs` — source/checked-input and independent decoded contracts.
- **R:** `runtime_tests.rs` — bounded emitted-Wasm Node execution.
- **V:** `release_tests.rs` — decoded and adversarial invalid-release workloads.
- **O:** `failure_order_tests.rs` — independent decoded failure-order assertions.
- **F:** `failure_tests.rs` — bounded same-instance acquisition/trap scenarios.
- **M:** `measurement_tests.rs` — decoded instrumentation and Node recorders.

Fully qualified names are `codegen::allocator::<file-stem>::<case>`. Fixtures pass
`wasmparser::Validator`; decoded instructions/layout and independently expected
runtime contents provide evidence beyond module validity.

| Approved obligation | Named evidence and observation boundary |
| --- | --- |
| Frozen signatures, private activation and existing checked errors | D: `private_compilation_emits_valid_management_exports`, `private_compilation_requires_planned_memory_and_preserves_checked_body_errors`; scalar-only fixtures reject missing memory, forged function-ID errors retain variant/message/span |
| Append-only imports/functions/types/globals/exports; deterministic ordinary bytes and no management machinery | D: `appended_management_preserves_existing_indices_bodies_and_ordinary_bytes`; combined host import, arrays, static strings, remainder, ranges/membership and captured callable fixture compares existing raw bodies, indices, data and memory limits |
| Selected-export collision handling, no public name restriction | D: `management_export_collisions_reject_only_selected_user_exports`; M: `instrumentation_export_collision_is_private_to_the_requested_variant` |
| Signed i64 request guard before narrowing/state access; all 29 classes, 8 through 2^31 | D: `class_helper_checks_wide_domain_before_narrowing_and_globals_are_private`, `decoded_rounding_covers_every_capacity_and_adjacent_request`; O: `decoded_classify_checks_signed_request_domain_before_any_narrowing_or_state_access`; R: `zero_requests_are_distinct_and_classification_rejects_wide_invalid_requests` covers classification without multi-GiB allocations |
| Eight-byte alignment, widened record-plus-payload arithmetic and unsigned addresses | D: `decoded_reservation_math_preserves_large_capacities_and_unsigned_addresses`; F: `metadata_plus_payload_address_limit_traps_without_large_memory_or_partial_commit`; synthetic address/capacity cases do not claim successful huge allocations |
| Distinct simultaneous zero-byte blocks, minimum class, explicit release | R: `simultaneous_zero_requests_remain_active_until_explicit_release`; six live identities/addresses, only released blocks reusable, no zero-extent payload reads |
| Distinct head per class, exact-class reuse only, no borrowing/splitting/coalescing | D: `decoded_free_list_dispatch_has_one_distinct_global_per_class`; R: `acquisition_reuses_only_exact_class_without_borrowing_larger_free_block`, `adjacent_free_blocks_are_not_coalesced_split_or_borrowed` |
| One reusable 32-byte record per physical block; single combined backing reservation; initialize before publication | D: `management_uses_one_backing_call_and_registry_authority_before_header_access`; O: `decoded_fresh_reservation_checks_widened_size_before_backing_and_commits_after_success`; M: `record_m3_accounting` checks population/record plateau, not just pages |
| Multiple live/free blocks, renewed identity, disjoint storage and preserved contents | R: `live_blocks_reuse_with_new_identity_and_preserve_other_storage`, `mixed_classes_preserve_live_extents_and_metadata_across_unordered_reuse`; independent initialized requested extents and record bytes, membership-based recovery without assuming a pool order |
| Reused payload is not promised zeroed; requested extent is not capacity | The preceding R workloads initialize requested bytes before consumption, preserve other live payloads/metadata and never treat spare capacity as consumer-owned; M: `record_m3_accounting` separates requested/capacity/waste |
| Real memory growth, view refresh and live/free/ordinary storage preservation | R: `real_memory_growth_preserves_initialized_live_blocks_identities_and_free_records`; two actual growth operations with refreshed views |
| Exact payload registry match before metadata access, then current identity/active state; class derived from record | V: `decoded_release_guards_exact_provenance_identity_and_state_before_pool_writes`; D: `management_uses_one_backing_call_and_registry_authority_before_header_access`; null termination and read-only lookup/head helpers independently decoded |
| Unknown/interior/high-bit/static/ordinary/host-created addresses and forged adjacent headers confer no authority | V: `unmanaged_descriptors_boundaries_and_forged_interior_headers_have_no_authority`; R: `unknown_interior_and_wrong_identity_release_preserve_live_and_free_state`; valid separate host descriptor/payload storage stays usable |
| Double release, wrong identity, ABA, registry head/middle/tail in active/released states | V: `registry_positions_reject_wrong_double_and_aba_authority_in_same_instance`; R: `double_release_and_stale_identity_trap_without_corrupting_same_instance` |
| Invalid releases preserve state and free availability in the same instance | Both V Node cases snapshot cursor/pages/population/identity/all class heads, block observations and record bytes before recovery; independently verify live contents, then drain known free blocks simultaneously, detecting cycles/duplicates/overlap |
| Instance-wide monotonic u64 identity; exhaustion before pool removal or backing reservation | O: `decoded_acquire_validates_request_and_identity_before_reading_or_removing_free_head`; R: `identity_exhaustion_preserves_live_and_free_blocks_without_wrap`; F: `exhausted_identity_never_consumes_free_or_fresh_storage_even_after_releases`; fixed instance seed, final identity through fresh and reuse paths, never reset |
| Request rejection preserves live blocks and every populated free class | F: `rejected_request_domain_preserves_all_free_classes_and_live_contents`; signed extremes/oversized requests, repeated same-instance recovery and independent contents |
| Failed reservation preserves cursor, population, identities and free capacity; no partial record publication | O: `decoded_real_backing_checks_widened_end_and_growth_failure_before_cursor_commit`; F: `real_backing_growth_failure_preserves_live_free_and_unpublished_storage`, `bounded_memory_preserves_a_multi_page_static_minimum`; real capped `memory.grow` failure and unpublished-storage sentinel, preserving actual static minimum |
| Configured fresh-reservation ceiling does not prevent reuse | R: `backing_limit_failure_preserves_free_availability_and_live_contents`; this supplemental private ceiling is distinct from the real backing-failure workloads |
| Completed releases/writes/host effects survive later traps; active blocks never swept | F: `completed_release_survives_later_traps_while_active_blocks_are_never_swept`; host reentry releases B, ordinary guest indexing traps with A active, repeated calls retain distinct A blocks while B remains reusable |
| Nonmutating observations without cursor-probe allocations; bounded storage and metadata under fixed demand | M: `record_m3_accounting`; R/V/F snapshot the frozen observations. Invalid class indices and unknown-address selectors execute; invalid selector rejection itself is source-audited in `Emitter::observe`, not separately claimed as a runtime test |
| Fragmentation, retained free capacity and linear registry search characterized honestly | M: `record_m3_accounting`, `record_m3_registry_search`; exact workload/formulas/raw data linked below |
| Instrumentation explicitly selected, behavior unchanged; noninstrumented timing | M: `instrumentation_is_explicit_and_preserves_calls_and_memory_operations`, `instrumentation_export_collision_is_private_to_the_requested_variant`, `record_m3_guest_timing`; exactly one extra global and four-op FIND increment, unchanged calls/load/store/growth, no instrumented timing |

The final allocator filter selects **38 tests: 16 native source/decoded checks and
22 ignored-by-default Node cases**, including three measurement recorders. All 38
ran, none failed or remained ignored. Aggregate test execution can invoke timing
recorders concurrently; those timings are **not** the isolated measurement evidence.
The separate sequential release-profile recording below is authoritative for costs.

### Ordinary compilation, presets and M2

The M3 append fixture proves representative baseline index/body noninterference,
not an exhaustive historical byte comparison. Broader named regressions ran too:

| Boundary | Existing evidence |
| --- | --- |
| Historical bytes, determinism, public descriptor layout at every preset | `crates/compiler/tests/abi.rs`: `memory_abi_preserves_baseline_bytes_and_determinism_at_every_level`, `static_descriptors_match_independent_little_endian_bytes`; unchanged ABI/emitter/diagnostic fixtures in default/full suites |
| Ordinary byte/diagnostic behavior and enabled-preset identity, not blanket None-versus-enabled identity | `crates/compiler/src/codegen/region/tests.rs`: `ordinary_bytes_presets_exports_and_diagnostics_remain_independent`; `region/measurement_tests.rs`: `measured_presets_are_deterministic_and_enabled_bytes_match`; ordinary lifetime noninterference and integer-specialization suites |
| M2 normal-return reuse, exclusive caches, trap quarantine and reentry | `region/runtime_tests.rs`: `reuse_reinitializes_distinct_aligned_literals_and_preserves_ordinary_handles`, `traps_quarantine_owned_blocks_and_keep_completed_initialization_and_writes`, `reentrant_recursive_callers_keep_retained_arrays_and_captures_isolated`; all 23 region tests executed |
| Raw-host retained typed arrays, captures, recursion/reentry/traps and later growth | `crates/compiler/tests/typed_arrays_runtime.rs`: `raw_host_retained_typed_arrays_survive_recursive_reentry_traps_and_rhs_growth`; `crates/compiler/tests/closures_runtime.rs`: `reentrant_live_contexts_and_retained_handles_survive_inner_traps_and_growth`; each explicitly selected one passing test |
| Package/dependency isolation and downstream API | `crates/compiler/tests/workspace_contract.rs`; all three tests passed explicitly and in broader suites |
| CLI/library/rebuilt-browser byte and diagnostic parity within each level | Package-owned adapter suites in the full workspace run and all 25 `tests/browser.cjs` groups; browser evidence exercises ordinary compilation only, not private M3 |

## Reproduced measurements and limits

[#62's formulas, workload and boundaries](benchmarks/m3-allocation-infrastructure-2026-10-28/README.md)
remain authoritative; its samples were not overwritten. The
[fresh #63 recording](benchmarks/m3-allocation-infrastructure-2026-10-28/integrated-63/README.md)
preserves raw accounting/search/timing JSON, command logs and provenance.
All executable hashes in #62's manifest and all four fixture/module artifact
hashes match this clean final implementation revision exactly. Accounting and
search `results` reproduce identically; timing varies as expected.

- Fixed requests `[0,9,17,33,65]`, five classes and four peak blocks per class
  plateau at **20 records / 640 record bytes / 992 payload-capacity bytes /
  1,632 aligned backing bytes**. No fresh backing or metadata is consumed over
  256 post-warmup cycles. Live contents are checked throughout; stable pages alone
  are not the proof. New classes, higher simultaneous demand or retained active
  blocks after traps can grow storage without bound.
- **128 free class-16 payload bytes** cannot satisfy request 17/class 32. That
  acquisition needs **64 new backing bytes**, with **15 active rounding-waste
  bytes** reported separately. Free storage and records are retained, not returned
  to the engine. No general fragmentation resistance is claimed.
- At 512 physical records, head/middle/tail/absent lookup performs
  **1/257/512/512 comparisons**. These are attempted non-null record comparisons,
  including those completed before a later release trap, not successful-release
  counts. Observation selectors using FIND also increment the opt-in counter;
  release deltas are isolated before those observations.
- Ordinary/private/instrumented modules are **730/2,186/2,226 bytes**. The
  **40-byte instrumentation cost** is separate from private allocator machinery;
  logical global widths are not engine/RSS measurements.
- All **150 noninstrumented timing samples** are retained. Deep searches cost
  more in this run; head timings and differences from #62 expose noise/tiering.
  Samples include JS iteration, two JS/Wasm calls per release/reacquire pair and
  multivalue transport, not isolated Wasm instruction latency. Setup, validation,
  warmup, initialization, observations, instrumentation and reporting are outside
  timing. No speedup, arbitrary timing threshold or production suitability follows.

Trusted test code may inspect management metadata. Two older smoke/profiling
snapshots also inspect previously initialized bytes of a released block for failure
forensics; that grants no consumer stale-read rights. #60's adversarial harness
reads live payloads only. No harness reads uninitialized spare capacity as a
consumer expectation. Release authorization does not protect against a malicious
host corrupting exported memory.

## Fresh integrated validation — 2026-10-28

Stable implementation revision: `fc72d290b0cea1095b0b9df86c476ebc06f1499f`.
The starting working tree was clean. Compiler, adapters, shared tests, runtime,
manifests, lockfile and fixtures stayed unchanged; only documentation/evidence
packaging followed. No new test seam or implementation fix was necessary.

[Durable command evidence](m3-allocation-infrastructure-validation-2026-10-28.json)
records exact commands, exit codes, per-target summaries, full transient-log
hashes, 207 executable-input hashes and rebuilt package hashes. Complete gate logs
remain under `target/m3-integrated-63/`; preserved measurement logs are linked
above. The focused chain had a 300-second tool deadline, the sequential recorder
600 seconds (360 per child), and the fast/full chain 1,200 seconds. These bound
whole command groups, not individual gate durations. Every group completed; no
gate timed out or failed. One initial inline orchestration command failed shell
parsing before executing any gate; the subsequent focused shell chain succeeded.

| Command (repository root) | Actual outcome |
| --- | --- |
| `cargo check --offline --all-targets` | Passed |
| `cargo test --offline -p was-compiler --lib codegen::allocator -- --include-ignored` | 38 passed, zero failed/ignored |
| `cargo test --offline -p was-compiler --lib codegen::region -- --include-ignored` | 23 passed, zero failed/ignored; M2 noninterference |
| `cargo test --offline -p was-compiler --lib lifetime -- --include-ignored` | 42 passed, zero failed/ignored |
| `cargo test --offline -p was-compiler --lib codegen::memory_tests -- --include-ignored` | 21 passed, zero failed/ignored |
| `cargo test --offline -p was-compiler --test typed_arrays_lowering` | 2 passed |
| `cargo test --offline -p was-compiler --test typed_arrays_runtime raw_host_retained_typed_arrays_survive_recursive_reentry_traps_and_rhs_growth -- --ignored --exact` | 1 passed |
| `cargo test --offline -p was-compiler --test closures_runtime reentrant_live_contexts_and_retained_handles_survive_inner_traps_and_growth -- --ignored --exact` | 1 passed |
| `python tests/support/record_m3_allocator.py target/m3-integrated-63/measurements` | Passed sequentially; 38 correctness tests, then exactly one ignored test per release-profile accounting/search/timing command; stable hashes |
| `cargo test --offline` | 1,212 passed, 241 intentionally ignored, zero failed; 69 unit/integration targets and one empty doctest target |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed; production portable core |
| `cargo test --offline -p was-compiler --test workspace_contract` | 3 passed |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed; current-source `pkg/` rebuilt before browser/full ignored tests |
| `node tests/browser.cjs pkg` | 25 groups passed |
| `cargo test --offline --workspace -- --include-ignored` | 1,466 passed, zero failed/ignored; 80 unit/integration targets and one empty doctest target |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed |

Environment: Windows x64, Intel Core Ultra 5 125H, Node 24.12.0 / V8
13.6.233.17-node.37, rustc/Cargo 1.98.0, Python 3.13.14, wasm-pack 0.15.0,
installed Wasm target and cached dependencies. `wasm-bindgen` was not on PATH;
wasm-pack resolved compatible cached tooling with `--mode no-install` and printed
its usual “Installing wasm-bindgen” message. No separate install/fetch command ran.
The only packaging notice concerned optional manifest metadata. No prerequisite
or correctness gate remains pending. #61/#62's earlier timeouts remain historical
limitations, not retrospective passes; the completed run here supplies final-source
evidence independently.

## Review and final checks

Independent Standards and Spec reviews compared all 16 staged documentation/evidence
paths with task-start revision `fc72d290b0cea1095b0b9df86c476ebc06f1499f`.
Standards reported **zero documented-rule violations and zero judgement-call smell
findings**; Spec reported **zero missing, incorrect or out-of-scope requirements**.
Both audited existing evidence rather than rerunning tests: executable/log/package
hashes, actual target counts, named symbols, local paths, all ten timing medians
and historical/private-scope boundaries. No implementation fix was requested.

Final local checks resolved **168 Markdown link paths and 53 named symbols**,
verified all 207 executable-input hashes, 16 gate-log hashes, rebuilt package and
measurement provenance, and confirmed no implementation/test/runtime/manifest or
fixture diff. `git --no-pager diff --cached --check` and
`git --no-pager diff --check` passed. The companion JSON records those commands
and outcomes. No compatibility fixtures or historical M0/M1/M2/#62 records were
regenerated; only new #63 samples and current index/pointer documentation changed.

## Decision and deferred work

The approved M3 allocation/release/reuse, failure-safety, characterization and
ordinary/M2 noninterference obligations reconcile with final tests and fresh
measurements. The private milestone is validated without changing production
behavior, fixtures, defaults, dependencies or public contracts.

Production activation, M2 migration, M4 resizing/reallocation, general array or
capture reclamation, heap rewind, page return, trap recovery, host roots/releases,
GC/RC and wider lifetime/ownership proofs remain separately authorized work. A
free list answers how to reuse released storage, not when language objects die.
