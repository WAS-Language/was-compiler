# Memory roadmap M0 — contract and regression baseline

**Implemented and validated: 2026-10-26.** This is M0 of the
[memory/optimization roadmap](was_compiler_memory_and_optimization_roadmap.md),
not issue #37's historical measurement-only M0. It formalizes
[memory/ownership contract revision 1](memory-abi.md#contract-revision-1--2026-10-26)
without changing compiler/adapter behavior, emitted ABI, allocation policy,
production exports or dependencies. Later milestones are not authorized here.

## Contract-to-test map

Existing assertions and compatibility fixtures are retained. Node/browser tests
remain ignored by default; their explicit execution is recorded below.

| Implemented guarantee | Regression evidence |
| --- | --- |
| Eight-byte untagged descriptors, independent little-endian bytes, deterministic modules and unchanged baseline fixture | [`abi.rs`](../crates/compiler/tests/abi.rs): `memory_abi_preserves_baseline_bytes_and_determinism_at_every_level`, `static_descriptors_match_independent_little_endian_bytes` |
| Numeric-array aliases, fresh literals, rebinding, checked indices including negative zero | [`memory_tests/runtime.rs`](../crates/compiler/src/codegen/memory_tests/runtime.rs): `node_arrays_sum_len_mutation_alias_reassignment_and_fresh_literals`, `node_all_invalid_indices_trap_and_negative_zero_is_valid_for_reads_and_writes` |
| Immutable UTF-8 strings, embedded NUL, interned literals and content equality across distinct handles | Same runtime suite: `node_string_utf8_empty_nul_interning_and_distinct_descriptor_content_equality` |
| Raw host roundtrips and separately allocated payloads; descriptor bounds/alignment checks do not establish provenance | Same runtime suite: `node_host_memory_roundtrips_reentrant_allocator_growth_and_reference_mutation`, `node_invalid_exported_and_host_descriptors_trap_before_effects`; [`typed_arrays_runtime.rs`](../crates/compiler/tests/typed_arrays_runtime.rs): `raw_host_array_results_validate_storage_without_scanning_elements` |
| Bool/string-array widths, canonical consumed values, lazy validation, live alias iteration and checked replacement | `typed_arrays_runtime.rs`: `boolean_layout_lazy_consumption_replacement_and_iteration`, `string_handles_are_lazy_and_equality_compares_contents` |
| Once-only element effects and complete pre-RHS destination checks; completed effects survive traps | `memory_tests/runtime.rs`: `node_memory_expression_order_live_prefixes_and_assignment_bounds_before_rhs`; `typed_arrays_runtime.rs`: `construction_captures_reentry_and_traps_keep_completed_effects` |
| Aligned minimum allocation, actual growth, overflow and failed-growth cursor safety | `memory_tests/runtime.rs`: `node_allocator_alignment_reservation_growth_overflow_and_large_arrays`, `node_guest_large_array_growth_and_bounded_allocator_exhaustion` (bounded capped-memory failure, not resource exhaustion of the machine) |
| Distinct recursive/reentrant capture contexts; retained numeric arrays and host-created strings survive inner traps/growth and later calls | [`closures_runtime.rs`](../crates/compiler/tests/closures_runtime.rs): `reentrant_live_contexts_and_retained_handles_survive_inner_traps_and_growth` |
| Raw-host-retained bool/string arrays survive recursion, caught guest and allocator traps, growth and later calls; suspended pre-RHS writes resume at the original checked destination | **New:** `typed_arrays_runtime.rs`: `raw_host_retained_typed_arrays_survive_recursive_reentry_traps_and_rhs_growth` |
| Node explicit-copy isolation and view refresh, scalar/accessor snapshots, aggregate budgets before allocation | [`runtime.rs`](../crates/compiler/tests/runtime.rs): `node_abi_host_copies_and_memory_growth_refresh`; [`typed_array_marshaling.rs`](../crates/compiler/tests/typed_array_marshaling.rs): bool/string copies, accessor snapshot/budget and raw-import roundtrip tests |
| CLI/library bytes and current-source browser/core bytes, diagnostics and raw typed-array execution agree within each preset | [`cli/tests/abi.rs`](../crates/cli/tests/abi.rs), [`cli/tests/typed_arrays_adapters.rs`](../crates/cli/tests/typed_arrays_adapters.rs), [`browser/tests/typed_arrays_adapters.rs`](../crates/browser/tests/typed_arrays_adapters.rs), [`tests/browser.cjs`](../tests/browser.cjs) |
| Valid Wasm, independent decoded memory/export contracts, no profiling exports, preset/analysis parity and independent allocation results | [`memory_benchmarks.rs`](../crates/compiler/tests/memory_benchmarks.rs), [`memory_measure.cjs`](../tests/support/memory_measure.cjs) |

The new retention test runs at all four optimization levels using the existing
bounded subprocess helper and `wasmparser::Validator`. Its raw host saves handles
inside an invocation that subsequently traps, not just successful return values.
Independent byte/handle assertions cover both array payloads, immutable host-created
UTF-8 strings, and preserved writes. Growth occurs during both bool and string
assignment RHS execution; stale JavaScript views are explicitly rejected and
refreshed. The host catches inner failures and resumes the suspended outer call.
No source feature or production lowering changes were necessary.

## Compatibility and future-feature boundary

Raw hosts retain existing instance-lifetime storage guarantees without new root
registration, release or nonretention requirements. Structurally valid storage
is not implicitly runtime-owned. Explicit Node marshaling remains copying;
the browser compiler adapter does not become a guest-value marshaler. No new
ownership protocol, descriptor field, version export or pinning mechanism exists.

Resizable arrays, shrink-during-iteration traps, movable payloads and immutable
string operations remain **future proposals**, documented in the roadmap rather
than represented as implemented revision-1 features. Their operation APIs and
management protocols need later specifications. Future storage movement must
respect retained handles and outstanding pre-RHS checked writes. M0 adds no
escape analysis, reclamation or numeric specialization.

## Memory measurements

The existing bounded generator/harness now has **19 workloads**: the original 17
plus `construct_bool_3` and `construct_string_3`. Each new workload constructs 128
three-element arrays and returns 384. Independent aligned reservations are
`128 * align8(8 + 3) = 2,048` and `128 * align8(8 + 3*4) = 3,072` bytes respectively.
String literal payloads are statically interned, not dynamically constructed.

Records in [`benchmarks/m0-memory-2026-10-26/`](benchmarks/m0-memory-2026-10-26):

- [`environment.json`](benchmarks/m0-memory-2026-10-26/environment.json): exact commands,
  exit codes, base revision, tool versions and corpus/harness/test/runtime/package
  SHA-256 hashes. Measurements used default `None` optimization and the debug
  Rust integration caller, with uncommitted M0 changes atop
  `c295129e3aa4b8bda7ba1a9f20647fde24329bdb`.
- [`markers.txt`](benchmarks/m0-memory-2026-10-26/markers.txt): independent results,
  aligned consumption, module/source sizes and hashes for all 19 cases.
- [`capacity.txt`](benchmarks/m0-memory-2026-10-26/capacity.txt): separate fresh,
  unmarked instances' initial/final memory pages, with matching source/module hashes.

| Workload | Module bytes | Aligned bump bytes/call | Unmarked pages before → after |
| --- | ---: | ---: | --- |
| construct_bool_3 | 1,079 | 2,048 | 1 → 1 |
| construct_string_3 | 1,107 | 3,072 | 1 → 1 |
| page_crossing | 803 | 131,072 | 1 → 3 |
| capture_shared | 946 | 48 | 1 → 1 |
| capture_iteration | 929 | 8,232 | 1 → 1 |

**Measurement limits:** `__alloc(0)` markers allocate at least eight bytes and can
cause growth. Consumption is `after - before - 8`, including padding and capture
storage; it is not object size or live bytes. Page observations use a separate
unmarked run and measure capacity, not occupancy or process RSS. General allocation
counts and peak live bytes are **unavailable**, not zero or inferred from pages.
The focused suite also executes the existing timing smoke case, but this milestone
records no timing comparison, speedup, performance budget or reclaimed-byte claim.
See [benchmark timing boundaries](benchmarks.md#measurement-isolation-and-commands).

## Validation 2026-10-26

All commands below exited 0. Fast/full checks were sequential with a 180-second
limit per command; no retries or timeouts. The browser build preceded smoke and
workspace tests, so generated bindings reflect current source. Only documentation
and evidence packaging changed after these test runs.

| Command | Result |
| --- | --- |
| `cargo test --offline -p was-compiler --test typed_arrays_runtime -- --include-ignored` | 6 passed; none failed/ignored; includes the new all-preset regression |
| `cargo test --offline -p was-compiler --test memory_benchmarks -- --include-ignored --test-threads=1` | 5 passed; none failed/ignored; independent checks for all 19 workloads |
| `cargo test --offline` | 1,145 passed, 205 ignored; 69 unit/integration targets and one empty doctest target |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed |
| `cargo test --offline -p was-compiler --test workspace_contract` | 3 passed |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed; current-source package rebuilt |
| `node tests/browser.cjs pkg` | 25 groups passed |
| `cargo test --offline --workspace -- --include-ignored` | **1,363 passed, none failed/ignored**; 80 unit/integration targets and one empty doctest target |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed |

Separate marker/capacity commands each passed one selected test; exact commands
are in the environment record. The focused new test also passed individually.
[Validation evidence](benchmarks/m0-memory-2026-10-26/validation.txt) retains the
fast/full commands, per-target summaries, exit codes, durations and browser output;
complete transient logs are under `target/m0-2026-10-26-*.log`.

Tools: rustc 1.98.0, Cargo 1.98.0, Node 24.12.0, wasm-pack 0.15.0; the Wasm target
was installed. `wasm-bindgen` was absent from PATH, but the documented no-install
build succeeded with wasm-pack's tooling resolution (its log prints “Installing
wasm-bindgen”; no separate installer was invoked). Packaging reported only optional
missing `description`, `repository` and `license` fields. Compatibility fixtures
were not regenerated. M0's correctness/measurement gates passed; this is not a
release/default change or approval to implement later milestones.
