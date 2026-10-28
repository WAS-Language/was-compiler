# M3 private allocation/release cycle (#58)

This first slice implements emitted-Wasm allocation, explicit release and exact-class
reuse behind `cfg(test)`. It is not a production memory manager, source feature,
public compiler option or adapter API. The remaining M3 tickets expand runtime,
failure, growth and measurement evidence; #58 does not claim their exit gates.
[#63's integrated contract-to-test and validation record](m3-allocation-infrastructure.md)
now accepts the final private milestone, including #59–#62's evidence and fresh
ordinary/M2/browser noninterference gates. It does not authorize production consumers.
The approved scope is [roadmap §7](was_compiler_memory_and_optimization_roadmap.md#7-milestone-m3--allocation-infrastructure-and-reclamation-policy).

## Activation and assembly boundary

`crates/compiler/src/codegen/allocator.rs::compile(program, exports, config)` is
private to library unit tests. It uses `ValidatedProgram::with_policy` with baseline
numeric policy, existing layout/checking and cached user/helper bodies. The fixture
must already require ordinary memory (an array or string suffices); scalar-only
fixtures return `Error::MissingMemory`. Checked errors are preserved inside
`Error::Checked`; management-export collisions are rejected privately.

`codegen::assemble_inner` accepts the optional test-only append configuration.
Ordinary `assemble` always supplies `None`; production has no configuration
parameter or allocator module. Private types, functions, globals and exports append
after all existing entries, including local callables. Imports, user bodies,
memory/remainder/range helpers, callable indices, memory limits and static data stay
unchanged. `memory::Helper::ALL` and `abi::Signature` are unchanged.

M2 is not a client: no selection, cache, retirement or quarantine rule changes.
Ordinary raw descriptors and host-retained handles keep revision-1 lifetime rights.
Valid memory bounds do not establish release authority.

## Frozen private Wasm transport

These exports exist only in explicitly compiled experimental modules:

| Export | Wasm signature | Meaning |
| --- | --- | --- |
| `__m3_class` | `(i64 request) -> i32` | Validate request and return class 0–28, without mutation |
| `__m3_acquire` | `(i64 request) -> (i32 payload, i64 identity)` | Acquire one live block; private multivalue return |
| `__m3_release` | `(i32 payload, i64 identity) -> ()` | Validate exact authority and return block to its class |
| `__m3_observe` | `(i32 selector, i32 argument) -> i64` | Nonmutating observations below |

Requests use signed i64, not an already truncated i32: negative values and values
above 2^31 trap before narrowing or mutation. JavaScript must pass an in-domain
BigInt; as with any raw Wasm i64 boundary, values outside the signed i64 transport
range are converted modulo 2^64 by JavaScript and cannot be detected after that
conversion. The private caller must not pass such values. Return addresses are
unsigned memory32 (`payload >>> 0` in JS). Identities are u64 bit patterns carried
by i64: use `BigInt.asUintN(64, identity)` for display, but the returned signed
BigInt can be passed directly back to release. The source ABI remains single-result.

Capacity is the smallest power of two at least the request, with minimum eight:
29 classes, eight through 2^31 inclusive. Alignment is eight bytes. Every zero-byte
request acquires a distinct live block in class 0, with a new identity. Supported
capacity does not guarantee backing availability. Reuse is LIFO within exactly
one class; there is no splitting, borrowing from larger classes or coalescing.
Payload is not cleared. Consumers initialize every requested byte before reading
or publishing it; capacity does not extend their requested extent.

## Physical records and authority

A fresh block reserves `32 + capacity` bytes in one call to the existing checked
bump allocator. The aligned record precedes the payload and is outside its usable
extent. Fields are little endian:

| Record offset | Type | Meaning |
| --- | --- | --- |
| 0 | u32 | Next physical record in registry, zero terminates |
| 4 | u32 | Next free record in this class, zero terminates |
| 8 | u32 | Class index 0–28 |
| 12 | u32 | Active flag: 1 active, 0 released |
| 16 | u64 | Current acquisition identity |
| 24 | u64 | Current requested extent |
| 32 | bytes | Start of usable payload |

One record is reused with its physical block: no acquisition history allocation.
Private globals hold last-issued identity (i64), registry head (i32), physical
population (i64), then 29 i32 free-list heads. They are not exported. The ordinary
bump cursor remains global 0. Internal registry lookup, head read and head write
helpers follow the four exported management functions.

Release traverses only registered records to find an exact payload match, then
checks current identity and active state before any mutation. Unknown/interior,
static and host-created addresses without membership cannot supply authority.
Release derives the class from the record, not caller input. Lookup is O(physical
record population). This is not protection against arbitrary metadata corruption
by a host with access to exported linear memory, and identities do not make stale
payload reads safe.

## Failure ordering and deterministic configuration

`Config::default()` starts last-issued identity at zero and imposes no additional
backing ceiling. `Config { last_identity, backing_limit }` is fixed at compilation:

- `last_identity: u64` seeds the initial last-issued identity, skipping earlier IDs
  permanently. Set it near `u64::MAX` for bounded exhaustion tests. There is no
  runtime setter or reset. Issuing `u64::MAX` succeeds once; every subsequent
  acquisition traps, even when a suitable free block exists.
- `backing_limit: Option<u64>` caps the absolute post-reservation ordinary bump
  cursor for *fresh M3 reservations only*. It is checked with widened arithmetic
  before calling the real allocator. It does not cap ordinary allocations, reduce
  memory limits, disable free-list reuse or simulate rollback. It supplies a
  deterministic failure without exhausting host resources.

Acquisition validates request and identity availability before touching free lists
or backing storage. Fresh capacity/record sizing is widened and checked before
narrowing. The real allocator performs address-limit/growth checks before committing
its cursor. Only after a successful combined reservation does M3 initialize every
record field, publish registry membership, advance population and commit the new
identity. Reuse renews metadata/identity without new backing. There are no fallible
calls after mutation under the private metadata invariants. Invalid release traps
before writes. Completed operations survive later traps: active blocks are not
swept, automatically pooled, or recovered on another call. No heap rewind, page
return, general trap rollback or unlimited-trap memory bound is promised.

## Frozen observation schema

`__m3_observe(selector, argument)` never mutates state. Results use i64 bits;
unsigned capacities, addresses, counts and pages fit positive i64. Identity selectors
return the same u64 bit encoding as acquire.

| Selector | Argument | Result |
| --- | --- | --- |
| 0 | ignored | Ordinary bump cursor, includes ordinary and experimental reservations |
| 1 | ignored | Current memory size in 65,536-byte pages |
| 2 | ignored | Number of registered physical records |
| 3 | exact payload | Block state: 0 unknown, 1 active, 2 released |
| 4 | exact payload | Current identity; zero for unknown |
| 5 | exact payload | Rounded payload capacity; zero for unknown |
| 6 | exact payload | Requested extent at latest acquisition; zero for unknown |
| 7 | class index | Free-list head *record* address; zero if empty |
| 8 | ignored | Last-issued identity |

Unknown addresses for selectors 3–6 return zero without interpreting caller-adjacent
bytes. Invalid selectors and class indices trap without mutation. Records and
free-list links may be inspected in exported memory by trusted test code using the
independent layout above. Cursor is consumed backing storage, not live payload;
population times 32 is record storage, not all module metadata. Pages alone cannot
establish a storage plateau. No performance or search-cost counters are added here;
#62 adds its separate explicitly requested measurement instrumentation, described
in [the measurement record](benchmarks/m3-allocation-infrastructure-2026-10-28/README.md#registry-comparisons-and-instrumentation-isolation).
The frozen selectors and noninstrumented compilation remain unchanged.

## Tests and reproduction

Library unit tests are necessary: integration dependencies cannot activate
`cfg(test)`. `allocator/tests.rs` checks Wasm validation, transport, request checks,
record offsets/instructions, single checked backing call, lookup-before-release
loads, checked diagnostic parity, export collisions, deterministic bytes and
unchanged existing indices/raw bodies/data with a host/import, memory, remainder,
range and captured local callable fixture. `allocator/runtime_tests.rs` executes
seven bounded Node workloads using `crate::test_process`; they remain ignored by
default. They cover two live initialized blocks, renewed identity on reuse,
ordinary retained arrays, all class boundaries without giant allocation, zero
requests, exact-class reuse, double/stale/wrong/unknown/interior release and
same-instance preservation after invalid requests, exhaustion and configured
backing failure. This is correctness smoke evidence, not the full M3 workload and
measurement acceptance gate.

```sh
cargo test --offline -p was-compiler --lib codegen::allocator -- --include-ignored
cargo test --offline -p was-compiler --lib codegen::memory
cargo test --offline -p was-compiler --lib codegen::region -- --include-ignored
cargo clippy --offline -p was-compiler --all-targets -- -D warnings
```

The first focused test failed against ordinary assembly with missing
`__m3_acquire`, then passed with the private append path.

### Validation — 2026-10-27

| Command | Result |
| --- | --- |
| `cargo check --offline --all-targets` | Passed |
| Focused allocator command above, including ignored tests | 13 passed, none failed/ignored; seven bounded Node cases |
| Focused memory command above | 20 passed, eight ignored under the default policy |
| Focused region command above, including ignored tests | 23 passed, none failed/ignored |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed; current-source package rebuilt |
| `node tests/browser.cjs pkg` | 25 groups passed |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed |
| `cargo test --offline --workspace -- --include-ignored` | Passed on retry with cached builds, then passed again after review fixes; all workspace packages and ignored runtimes selected |

The initial `cargo test --offline --workspace -- --include-ignored` run reached
its 180-second tool deadline near the end of compiler integration tests, without
reported failures. That interrupted run alone is not full-suite evidence.
The browser build printed the usual wasm-bindgen tooling-resolution and optional
package-metadata notices; no separate install/fetch command was invoked.

### Two-axis review

Independent Standards and Spec reviews used baseline `e8303a3` (the separately
committed pre-existing M2 validation/M3 plan and example deletion). Standards
identified raw function/type indices in the new emitter; those now retain
`FunctionIndex`/`TypeIndex` to encoder boundaries. Spec identified one test snapshot
reading spare capacity outside its initialized nine-byte extent; the snapshot now
reads only the management record and those nine bytes. Both were low-priority
findings, corrected before the final focused allocator, Clippy and full ignored
workspace runs. No allocator correctness or unauthorized-scope finding was raised.
Both independent reviewers rechecked the correction and reported zero outstanding
Standards and Spec findings.

## Storage plateaus and allocator costs (#62)

[The dated evidence](benchmarks/m3-allocation-infrastructure-2026-10-28/README.md)
defines current/peak/cumulative accounting, fixed-demand mixed-class reuse,
stranded free capacity, linear registry comparisons including failed releases,
and separate noninstrumented guest-operation timings. `compile_instrumented`
privately opts into one comparison counter; ordinary and uninstrumented modules
remain unchanged. The finite recorder uses stable-source hashes, fresh instances,
nonmutating cursor observations and sequential release-profile measurements.
This does not change release authority, activate production consumers or complete
integrated M3 acceptance.

## Acquisition failure and trap-retention evidence (#61)

`allocator/failure_tests.rs` adds six bounded Node cases at the private emitted-Wasm
seam; they remain ignored by default. `allocator/failure_order_tests.rs` adds four
Validator/independent decoded-order cases. No allocator implementation, production
ABI, M2 policy, public activation or recovery protocol changes.

| Obligation | Evidence |
| --- | --- |
| Signed request rejection before narrowing/mutation, zero and adjacent maximum-class boundaries | `rejected_request_domain_preserves_all_free_classes_and_live_contents`; drains two free classes after every rejected request while keeping another block live |
| Monotonic unsigned identities through exhaustion on reuse and fresh paths | `exhausted_identity_never_consumes_free_or_fresh_storage_even_after_releases`; fixed initial seed, BigInt authority, final identity issued by reuse, repeated failures and valid subsequent releases without resetting identities |
| Real backing failure with live/free records present, no partial metadata reservation | `real_backing_growth_failure_preserves_live_free_and_unpublished_storage`; successful growth to two pages followed by actual failed `memory.grow`, unchanged cursor/population/identity/all free heads/records, sentinel at the unreserved cursor, ordinary retained array and live payload checks, subsequent free-block reuse |
| Checked record-plus-payload address bound without giant allocation | `metadata_plus_payload_address_limit_traps_without_large_memory_or_partial_commit`; test module seeds only its initial ordinary cursor to eight bytes below the 65,535-page address ceiling, then even a zero request's 32+8 reservation fails before growth; no runtime cursor setter |
| Preserve the actual static memory minimum when capping growth | `bounded_memory_preserves_a_multi_page_static_minimum`; two-page static fixture grows to three, then fails without damaging static/live contents |
| Completed release and earlier effects survive later traps; active allocations are never swept | `completed_release_survives_later_traps_while_active_blocks_are_never_swept`; a callback in a suspended ordinary guest invocation acquires A/B and releases B, then guest checked indexing traps; four repetitions preserve ordinary writes/host effects, reuse B and retain distinct active A blocks across further successful calls and acquisition traps |
| Independent ordering oracle | Four `failure_order_tests` cases check signed-domain guards, exhaustion before free-head access, widened capacity+32/unsigned bounds before narrowing, one real backing call before metadata publication, and ordinary widened address/growth checks before cursor commit |

The test-local module cap preserves the original memory minimum and permits one
additional page; it does not use `Config::backing_limit` to fake real allocator
failure. The address-bound seed is a separate synthetic initial-state scenario,
not proof of a reachable small-memory production cursor. Accepted requests cannot
make `capacity + 32` overflow u32 under the frozen 2^31 domain; the unreachable
size-overflow guard is checked by decoding, not by widening the request contract.
All JS views are recreated after possible growth. No memory shrinking, trap
rollback, automatic release or bounded growth for unlimited retained-active traps
is promised. The finite trap workload deliberately retains four active blocks.

A sensitivity check temporarily removed the test memory maximum: the real-growth
failure case failed with a missing expected `WebAssembly.RuntimeError`. Restoring
the cap made it pass. No production code was changed for this negative control.

Focused reproduction:

```sh
cargo test --offline -p was-compiler --lib codegen::allocator::failure_tests -- --include-ignored
cargo test --offline -p was-compiler --lib codegen::allocator::failure_order_tests
cargo test --offline -p was-compiler --lib codegen::allocator -- --include-ignored
cargo test --offline -p was-compiler --lib codegen::region -- --include-ignored
cargo test --offline -p was-compiler --lib codegen::memory_tests -- --include-ignored
```

### Validation — 2026-10-28 (#61)

- Combined failure filter: 10 passed, none failed/ignored (six Node scenarios and
  four decoded checks); allocator filter: 33 passed, none failed/ignored.
- M2 region filter: 23 passed, none failed/ignored. Ordinary memory filter: 21
  passed, none failed/ignored.
- `cargo check --offline -p was-compiler --all-targets` and
  `cargo clippy --offline --all-targets -- -D warnings`: passed.
- The exact `codegen::allocator::failure_tests` filter selected six Node cases:
  all passed, none failed/ignored.
- Explicit `typed_arrays_runtime` case
  `raw_host_retained_typed_arrays_survive_recursive_reentry_traps_and_rhs_growth`
  and `closures_runtime` case
  `reentrant_live_contexts_and_retained_handles_survive_inner_traps_and_growth`:
  one passed each, none failed/ignored.
- `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown`:
  passed.
- `cargo test --offline`: attempted once, timed out after 180 seconds during CLI
  integration tests; not a completed suite pass. No longer-timeout retry was run.
  Browser bindings/full ignored workspace gates were not rerun for these test-only
  changes; they remain part of #63's integrated validation.
- Edited-file Rust formatting and `git --no-pager diff --check`: passed.

### Two-axis review (#61)

Independent Standards and Spec reviews compared the working tree, including the
new test files, against starting commit
`9f4cd20472e9574e00d0be7826c42302a6e04c3b`. Both reported zero findings. Each
independently reran the ten-case combined failure filter and whitespace check.
The reentrant callback's completed release followed by a guest indexing trap was
confirmed to satisfy the release-then-trap retention requirement.

This is #61 failure/continuation evidence, not #62 measurements or #63's integrated
M3 release gates. Earlier validation records retain their original scope.

## Size-class and growth evidence (#59)

This slice adds evidence only, through the frozen private transport and observation
schema. It changes no allocator implementation, production compilation, ordinary
array storage or M2 behavior. It does not establish M3's later failure, measurement
or integrated release gates.

- `decoded_rounding_covers_every_capacity_and_adjacent_request` uses a literal
  29-capacity oracle and checks the emitted rounding recurrence for zero, one,
  exact boundaries and adjacent requests (89 cases). The upper adjacent request
  is rejected. Large classes do not require allocating multi-GiB buffers.
- `decoded_free_list_dispatch_has_one_distinct_global_per_class` checks independent
  class heads and matching read/write dispatch for all 29 classes.
- `decoded_reservation_math_preserves_large_capacities_and_unsigned_addresses`
  checks widened capacity/reservation arithmetic, eight-byte alignment, unsigned
  backing transport and five synthetic address/ceiling cases. These are not
  successful large backing allocations. All decoded fixtures are Wasm-validated.
- `simultaneous_zero_requests_remain_active_until_explicit_release` holds six
  distinct minimum-class allocations, releases two and reacquires exactly those
  addresses with fresh identities, without touching their zero-byte payloads.
- `mixed_classes_preserve_live_extents_and_metadata_across_unordered_reuse` holds
  five blocks in each of four classes, then two free and three live blocks per
  class. Mixed release/reacquisition checks membership rather than reuse order,
  renewed identities, disjoint physical extents, initialized requested contents
  and unchanged records during payload writes. Reuse initializes the new requested
  extent before reading; spare capacity is never read or treated as usable extent.
- `adjacent_free_blocks_are_not_coalesced_split_or_borrowed` leaves adjacent
  16-byte-class blocks free, requires fresh 32-byte storage, then leaves both
  classes free while requiring fresh eight-byte storage. The original blocks
  remain independently reusable in their exact classes.
- `real_memory_growth_preserves_initialized_live_blocks_identities_and_free_records`
  causes two actual growth operations with requests of 65,537 and 131,073 bytes.
  It checks increased pages and detached old buffers/views, refreshes views, and
  verifies independently initialized live contents/identities, a retained ordinary
  array and a free record from before growth. That free block is then reused with
  a renewed identity. The workload reserves less than one MiB, not giant buffers.

All four new Node tests use the existing bounded subprocess helper and remain
ignored by default. These tests are regression evidence over an already implemented
contract: they passed against the unchanged #58 allocator, without a production
red/green change.

### Validation — 2026-10-28 (#59)

| Command | Result |
| --- | --- |
| `cargo test --offline -p was-compiler --lib codegen::allocator::tests` | 9 passed, none failed/ignored |
| `cargo test --offline -p was-compiler --lib codegen::allocator::runtime_tests -- --include-ignored` | 11 passed, none failed/ignored |
| Each of the seven new test names above, passed separately to `cargo test --offline -p was-compiler --lib <name>` (plus `-- --include-ignored` for Node tests) | Each selected exactly one test and passed |
| `cargo check --offline -p was-compiler --all-targets` | Passed |
| `cargo test --offline -p was-compiler --lib codegen::region -- --include-ignored` | 23 passed, none failed/ignored |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed; current-source package rebuilt |
| `node tests/browser.cjs pkg` | 25 groups passed |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed |
| `cargo test --offline --workspace -- --include-ignored` | 1,451 passed across 81 targets, none failed/ignored |
| `git --no-pager diff --check` | Passed |

The full workspace run also included three concurrently developed private
invalid-release tests outside #59's changes. They were preserved, not included in
this ticket's commit; the aggregate count describes the tested worktree rather
than #59 alone. The browser build printed the existing wasm-bindgen tooling
resolution and optional package-metadata notices; no separate install/fetch
command was invoked.

### Two-axis review (#59)

Independent reviews examined only this ticket's two test files and this document
against baseline `bd98992`. Spec reported zero findings. Standards reported zero
hard violations and one low-priority possible duplication between the local
runtime verification helpers. That optional extraction is deferred: the scenarios
keep their distinct metadata, extent and retained-array checks local, without
introducing a shared testing framework. No allocator defect or core change was
required.
