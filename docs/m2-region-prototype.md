# M2 reusable temporary region prototype (#54)

Implemented as an explicitly test-invoked experiment. Ordinary compilation,
numeric presets, CLI and browser adapters retain revision-1 storage behavior.
There is no public option, feature, AST field or new dependency. Adversarial
hardening under #55 is validated below; measurements (#56) and integrated M2
release evidence (#57) remain follow-on work. This prototype is not public
activation or a performance claim.

## Selection and checked lowering

`crates/compiler/src/codegen/region.rs::compile` is private under `cfg(test)`.
It uses existing layout, `ValidatedProgram` body checking/caching and assembly.
`lifetime::temporary_literals` exposes borrowed positive M1 facts only; M1 remains
classification, not storage selection. Eligibility is exactly the
[M1 invocation-wide closed subset](m1-lifetime-analysis.md#closed-positive-subset),
including unsupported and unreachable syntax. Checked lowering still validates
all bodies and children independently of those facts.

Each selected function reserves every candidate's individually aligned footprint:
number length N uses `8 + 8*N`, bool uses `align8(8 + N)`, and empty arrays use 8.
Checked summation includes unreachable suffixes. `Selection::CapacityExceeded`
is a deterministic private whole-function baseline fallback, not a compilation
error. No management header is stored in linear memory: management overhead is
one private mutable i32 global per selected function and one i32 local per
invocation. Global zero remains the ordinary allocator cursor. No selected
functions means no emitted region machinery.

## Ownership and completion

The first executed literal lazily takes and clears its function's cache slot,
or provisions the full block through the existing checked allocator. Ordinary
allocations cannot use its interior. Each literal has a fixed distinct offset;
its descriptor is initialized before element evaluation, and existing checked
lowering evaluates/stores elements once, left-to-right. Reuse reinitializes all
executed arrays. Reserved unreachable slices do not execute their initializers.

Explicit scalar/bare returns and void fallthrough retire the block only on normal
completion, after evaluating a scalar return. No executed literal means no
acquisition or retirement. Retirement preserves any scalar already on the operand
stack. There is no heap rewind, cross-function pooling, intra-invocation reuse,
relocation or growth of an acquired region.

A trap skips retirement and leaves the cache empty: the entire owned block is
quarantined for the instance lifetime, preserving completed writes. Later calls
provision another block. Repeated traps can exhaust memory; bounded reuse applies
to successful calls, not trapping workloads. Failed provisioning traps without
per-literal fallback and preserves the allocator cursor and existing storage.
Front-loaded reservation/growth/failure and whole-block retention after traps are
accepted experimental resource differences.

Experimental temporary addresses discovered through exported raw memory do not
acquire revision-1 instance-lifetime retention rights after normal retirement.
Ordinary allocations and legitimately published handles retain those rights.
Descriptor checks and host view-refresh obligations are unchanged.

## Test map

Library unit tests keep activation inside `cfg(test)`; integration dependencies
cannot activate it. Node tests remain ignored by default and use bounded process
support with finite guest workloads. Runtime scenarios run at None, Basic,
Standard and Aggressive, using the existing optimization boundary and policy
mapping. They check deterministic bytes, enabled-preset byte identity and ordinary
bytes before/after experimental compilation.

- `region::selected_function_has_private_cache_and_checked_array_stores`:
  Wasm validation, independent cache-global and typed-store decoding.
- `region::tests`: independent aligned sizes and offsets, unreachable capacity,
  acquisition/clear/allocation/store/return order, no heap rewind, deterministic
  bytes, enabled-preset parity, export selection, exclusions, no-machinery byte
  identity and independent forged-tree diagnostics. Capacity overflow uses
  synthetic footprints through the actual storage-selection transition, avoiding
  a multi-GiB AST allocation. The private `ValidatedProgram::with_region_selection`
  hook applies a test selector after layout checking and before the existing
  checked lowering/assembly path. Overflow fallback is checked against ordinary
  bytes and decoded allocations/globals; fallback still rejects malformed children.
- `region::runtime_tests`: number/bool/empty arrays and bool alignment boundaries,
  simultaneous literals, changing parameter initialization, reads/replacements,
  all five numeric compounds, early/bare returns and void fallthrough, no executed
  literal, successful reuse without region-driven heap growth, trap quarantine
  during reads and initialization, completed writes and ordinary retained handles.
  A capped one-page module checks front-loaded provisioning failure without
  cursor corruption or per-literal fallback.

## Validation

Run the focused prototype including its ignored Node cases:

```sh
cargo test --offline -p was-compiler --lib codegen::region -- --include-ignored
```

Also preserve M1, memory and typed-array regressions and the repository's
[offline fast/full validation workflow](validation.md#finite-direct-commands).
Rebuild browser bindings before current-source browser or full ignored validation.
Adversarial isolation evidence is recorded under #55 below. Measurements belong
to #56, with integrated M2 evidence in #57.

### Implementation validation — 2026-10-27

All commands below completed successfully offline. The initial decoded test failed
against baseline emission (one global instead of the required two), then passed
with the private storage path. Focused tests were rerun after the review fix.

| Command | Result |
| --- | --- |
| `cargo check --offline -p was-compiler --lib` | Passed; production core builds without activation |
| `cargo test --offline -p was-compiler --lib codegen::region -- --include-ignored` | 8 passed, none failed/ignored; includes 3 bounded Node cases |
| `cargo test --offline -p was-compiler --lib lifetime -- --include-ignored` | Passed, including Node regressions |
| `cargo test --offline -p was-compiler --lib codegen::memory_tests -- --include-ignored` | 21 passed, none failed/ignored |
| `cargo test --offline -p was-compiler --test typed_arrays_lowering` | 2 passed |
| `cargo test --offline` | Passed; default ignored policy preserved |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed |
| `cargo test --offline -p was-compiler --test workspace_contract` | 3 passed |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed; current-source package rebuilt |
| `node tests/browser.cjs pkg` | 25 groups passed |
| `cargo test --offline --workspace -- --include-ignored` | Passed; all packages and ignored runtimes selected after browser rebuild |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed |

The browser build printed its usual wasm-bindgen tooling-resolution message and
optional missing package metadata notice; no separate installer was invoked.
Compatibility fixtures were not regenerated. These checks validate #54's working
prototype and ordinary-path noninterference, not the follow-on M2 exit gates.

Two independent review axes examined the implementation against starting commit
`3c237d637a674c41cc4a224aa7002129846a4d50`, excluding the pre-existing roadmap edit.
Standards found one duplicated alignment-policy constant; it was replaced by
`abi::ALLOCATION_ALIGNMENT`. Spec reported zero findings. No public activation,
proof widening or revision-1 ownership change is introduced.

### Adversarial hardening — 2026-10-27 (#55)

No storage-policy defect was exposed and no production lowering change was
needed. The only compiler-boundary addition is the private `cfg(test)` selection
hook described above; layout remains authoritative before selection, and all
selected or fallback bodies still use the existing checked lowering.

| Guarantee | Evidence in `codegen::region` |
| --- | --- |
| Full reinitialization, both scalar-array layouts, empty arrays, individually aligned slices, all five numeric compounds, scalar/bare returns and void fallthrough | `reuse_reinitializes_distinct_aligned_literals_and_preserves_ordinary_handles`; changing inputs plus poisoned idle bool storage verify every descriptor and bool element, including the 8/9-element alignment boundary |
| Lazy first acquisition, no acquisition for unreachable literals, cached blocks survive pre-acquisition traps | `acquisition_is_lazy_after_scalar_contract_and_absent_for_unreachable_literals`; repeated failed contracts and unreachable bodies, including at capped capacity |
| Unreachable suffix capacity without executing initializers; unsupported unreachable forms remain ineligible | `checked_capacity_includes_aligned_empty_and_unreachable_literals`, `excluded_forms_and_no_candidates_emit_no_region_machinery`, and the `early` runtime case |
| Full acquired-block quarantine on initialization/access traps, including formerly cached blocks | `traps_quarantine_owned_blocks_and_keep_completed_initialization_and_writes`; twelve distinct read-trap blocks plus initializer/bool blocks remain byte-for-byte intact across later calls and growth; only a new successful block is reused |
| Recursive/reentrant excluded callers, caught inner traps, retained ordinary number/bool/string arrays and live captures | `reentrant_recursive_callers_keep_retained_arrays_and_captures_isolated`; eligible callees stay call-free, callbacks repeat reuse around suspended callers, later invocations and memory growth |
| Checked footprint limits and deterministic whole-function fallback | `checked_capacity_includes_aligned_empty_and_unreachable_literals`, `unrepresentable_inventory_discards_all_slices_and_keeps_a_deterministic_reason`, `capacity_overflow_lowers_the_whole_function_with_baseline_storage`; bounded synthetic overflow reaches checked assembly, matches ordinary bytes and emits two ordinary allocations with no cache global |
| Representable provisioning failure without retry, cursor changes or damage to retained storage | `failed_front_loaded_reservation_preserves_cursor_and_ordinary_storage`; one-page cap leaves room for a first literal and subsequent ordinary allocations, but not the complete block |
| Accepted front-loaded failure/growth ordering, not baseline resource parity | `front_loaded_reservation_precedes_earlier_initializer_trap` and `capped_later_literal_reservation_changes_failure_and_growth_before_access_traps`; independent cursor/descriptor/page observations distinguish failed provisioning, successful whole-block quarantine and baseline prefix allocation before the earlier access traps |
| Forged annotations/IDs and hidden, skipped or unreachable children remain checked; unchanged error ordering | Existing annotation/missing-return tests plus `malformed_unreachable_candidate_and_invalid_function_id_preserve_error_order`, `forged_binding_ids_and_skipped_children_keep_ordinary_diagnostics`, `mixed_selected_and_excluded_hidden_bodies_keep_source_order_errors`, `sparse_binding_ids_still_select_and_lower_checked_storage`; positive literal facts do not bypass lowering, malformed binding IDs remain conservative, and layout errors win before selection |

The following offline commands passed on the final code changes. Runtime cases
remain ignored by default; the explicit runs below selected them. Compatibility
fixtures and revision-1 baseline tests were not changed.

| Command | Result |
| --- | --- |
| `cargo check --offline -p was-compiler --lib` | Passed |
| `cargo test --offline -p was-compiler --lib codegen::region -- --include-ignored` | 17 passed, none failed/ignored; includes seven bounded Node scenarios at all four presets |
| `cargo test --offline -p was-compiler --lib lifetime -- --include-ignored` | Passed, including Node cases |
| `cargo test --offline -p was-compiler --lib codegen::memory_tests -- --include-ignored` | 21 passed, none failed/ignored |
| `cargo test --offline -p was-compiler --test typed_arrays_lowering` | 2 passed |
| `cargo test --offline -p was-compiler --test typed_arrays_runtime raw_host_retained_typed_arrays_survive_recursive_reentry_traps_and_rhs_growth -- --ignored --exact` | 1 passed |
| `cargo test --offline -p was-compiler --test closures_runtime reentrant_live_contexts_and_retained_handles_survive_inner_traps_and_growth -- --ignored --exact` | 1 passed |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed; browser package rebuilt before browser/workspace runtime validation |
| `node tests/browser.cjs pkg` | Passed |
| `cargo test --offline --workspace -- --include-ignored` | Passed; includes workspace architecture and revision-1 retention checks |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed |

Independent standards/spec review compared the scoped changes with
`eb60f1eadb71bf06e9cdb28caa8277eea8106454`, excluding pre-existing roadmap/example
edits. Standards reported zero hard violations and one low-priority suggestion to
deduplicate diagnostic-parity assertions, deferred to keep test expectations local.
Spec reported zero findings. These correctness checks do not supply #56's measured
exit gate, authorize public activation, or replace #57's integrated decision.
