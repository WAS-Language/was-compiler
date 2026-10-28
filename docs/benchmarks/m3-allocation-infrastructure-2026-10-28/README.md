# M3 storage plateaus and allocator costs (#62)

Private experimental evidence recorded 2026-10-28 on Windows x64, Intel Core Ultra
5 125H, Node 24.12.0 / V8 13.6.233.17-node.37, rustc/cargo 1.98.0.
The [M3 contract](../../m3-private-allocator.md) remains authoritative. This does
not activate production reclamation, complete integrated M3 acceptance, change M2,
or authorize M4/ordinary arrays to release storage.

## Reproduce sequentially

Use cached Rust dependencies, Node 24+, Python 3 and the existing bounded Rust
subprocess helper. From the repository root, with no competing builds/benchmarks:

```sh
python tests/support/record_m3_allocator.py target/m3-reproduction
```

The recorder runs the entire private allocator correctness suite first, then these
three **release-profile** measurements sequentially, selecting exactly one ignored
test each. It writes raw JSON, logs and provenance to the chosen directory.

```sh
cargo test --offline -p was-compiler --lib codegen::allocator -- --include-ignored --test-threads=1
cargo test --offline --release -p was-compiler --lib codegen::allocator::measurement_tests::record_m3_accounting -- --ignored --exact --nocapture --test-threads=1
cargo test --offline --release -p was-compiler --lib codegen::allocator::measurement_tests::record_m3_registry_search -- --ignored --exact --nocapture --test-threads=1
cargo test --offline --release -p was-compiler --lib codegen::allocator::measurement_tests::record_m3_guest_timing -- --ignored --exact --nocapture --test-threads=1
```

The convenient combined filter also selects five nonzero cases (two native and
three ignored Node cases):

```sh
cargo test --offline -p was-compiler --lib codegen::allocator::measurement_tests -- --include-ignored --nocapture --test-threads=1
```

Rust applies a 60-second deadline to each finite Node child. The recorder bounds
each Cargo child at 360 seconds; neither is guest fuel or process-tree control.
Generated modules remain under `target/m3-allocator/`; regenerate rather than commit
binaries. The fixture [workload.was](workload.was) supplies ordinary memory; explicit
management operations are defined in `tests/support/m3_allocator_measure.cjs`.

[environment.json](environment.json) records base revision
`dd5c5ce03804771d464e55990cbce0ff142e81dc`, dirty status, exact argv, tool versions,
SHA-256 hashes of all core Rust sources, manifests/lockfile, subprocess helper,
measurement harness and recorder, and module hashes/sizes. It confirms source hashes
were identical before/after recording. This is dirty-worktree evidence, not a claim
that the base revision alone contains the experiment. Raw JSON additionally records
UTC time, OS/CPU/V8, fixture/harness/module hashes. Documentation and later validation
logs are outside the executable-source hash manifest. i64 observations are decimal
strings in JSON, never lossy floating-point conversions.

## Observation definitions and formulas

All samples are taken after synchronous management calls return or throw. A host
ledger tracks explicit active blocks; it is not a language-level liveness proof.
Every new requested extent is initialized before a consumer reads it. Views are
recreated from the current memory buffer. Every accounting acquire/release checks
all still-live requested bytes, exact identities/types/extents, independent record
fields and disjoint physical extents; no spare payload capacity is read.

For request `r`, capacity `c = max(8, next_power_of_two(r))`. A physical block
reserves exactly `32 + c` aligned bump bytes. All these sizes are multiples of
eight; the initial cursor is already aligned. The frozen record is 32 bytes.

| Field | Meaning and temporal classification |
| --- | --- |
| `activeRequested` | Current sum of requested extents of active acquisitions; zero-byte blocks count as active but contribute zero bytes |
| `activeCapacity` | Current sum of rounded capacities of active blocks |
| `activeRoundingWaste` | Current `activeCapacity - activeRequested`; not free-list capacity |
| `peakRequested`, `peakCapacity`, `peakBlocks` | Maximum simultaneous active requested bytes, capacity and block count over the workload so far |
| `acquisitions` | Cumulative successful acquisitions, including reuse; also checked against last-issued identity for these zero-seeded instances |
| `cumulativeRequested`, `cumulativeCapacity` | Sum of requested extents / capacities over all successful acquisition events; repeated reuse deliberately counts the same storage again |
| `population`, `recordBytes` | Current physical block count and `32 * population` record storage; monotonic, hence also peak in these runs |
| `physicalCapacity` | Current sum of capacities of distinct physical blocks, active or free; also peak/cumulative freshly provisioned payload capacity here |
| `freeCapacity` | Current retained free payload capacity, `physicalCapacity - activeCapacity`; reusable only in its exact class |
| `cursor` | Current ordinary bump cursor, an absolute address; includes pre-existing/ordinary storage |
| `backing` | Current cursor minus initial cursor on this instance; cumulative fresh aligned M3 consumption, because measured instances execute no ordinary allocations |
| `pages` | Current `memory.size`, in 65,536-byte pages; also peak here, not live bytes or consumed backing |

Independently assert `backing = physicalCapacity + recordBytes` at every sample.
Observe cursor/population/pages using `__m3_observe`, without cursor-probe blocks.
Free blocks retain stale requested-extent metadata; it contributes **nothing** to
active requested bytes. Retained free record storage can separately be calculated
as `32 * (population - activeBlocks)`; free capacity excludes those records.

Private management globals contain 136 logical scalar bytes (u64 identity, u32
registry head, u64 population, 29 u32 class heads), fixed independently of
acquisition count, plus the existing ordinary u32 cursor. Actual engine metadata,
RSS and allocator bookkeeping are unavailable; logical widths are not engine
storage measurements. There are no per-acquisition history records.

## Fixed-demand plateau and stranded capacity

[accounting.json](accounting.json) contains initial/warmup snapshots, cycles
1/2/4/8/16/32/64/128/256 and the final release. Assertions check both full and released
states **every cycle**, not only the recorded subset. Requests `[0,9,17,33,65]`
select capacities `[8,16,32,64,128]`. Hold one persistent block per class while
three transient blocks per class repeatedly acquire, initialize, verify and release
in reverse class/batch order. Warmup establishes four physical blocks per class;
256 subsequent batches keep the class populations and simultaneous-live peaks fixed.

| Observation | After warmup and throughout cycles |
| --- | ---: |
| Physical blocks / record bytes | 20 / 640 |
| Physical payload capacity | 992 bytes |
| Aligned fresh backing consumed / absolute cursor | 1,632 / 1,640 bytes |
| Peak simultaneous requested bytes / capacity | 496 / 992 bytes |
| Peak active rounding waste | 496 bytes |
| Between batches: active requested / capacity | 124 / 248 bytes |
| Between batches: free payload capacity / free record bytes | 744 / 480 bytes |
| Memory pages | 1 |

There are **zero fresh backing bytes and zero new records after warmup**. The
plateau is demonstrated by cursor, physical population, capacity and record bytes,
not inferred from the stable page count. Final explicit release leaves 992 free
payload bytes and 640 reusable record bytes; pages and cursor do not shrink. Across
3,860 successful acquisitions, cumulative requested/capacity event sums are
95,728 / 191,456 bytes, which are neither newly consumed nor simultaneously live.
The entire accounting result is identical for noninstrumented and explicitly
instrumented variants on separate fresh instances.

A separate stranded-capacity instance frees eight request-9/capacity-16 blocks:
128 payload bytes and 256 record bytes remain retained. Acquiring request 17 needs
class 32, so it consumes **64 new backing bytes** (32 record + 32 capacity), despite
128 free payload bytes elsewhere. Its active rounding waste is **15 bytes**, a
separate cost from those **128 stranded free bytes**. No splitting, coalescing or
larger-class borrowing is introduced. Increasing peaks, new classes, or traps that
retain active blocks can grow backing/metadata; there is no general fragmentation
resistance or increasing-demand memory bound claim.

## Registry comparisons and instrumentation isolation

Only explicitly calling `compile_instrumented` adds
`__m3_registry_comparisons`, an i64 global initialized to zero. Increment once
**before each attempted exact-payload comparison against a non-null physical
record** in FIND. The null terminator is not a comparison. This counts attempted
comparisons, not successful releases, traversed bytes, identities tested or time.
The shared FIND also serves observation selectors 3–6: their searches count too.
Release deltas are isolated before any such observation; selectors 0/1/2/7/8 and
reading the counter itself perform no registry comparisons. The harness separately
checks observation-search counting. i64 increments wrap, but these finite runs
remain far below wraparound.

Completed comparisons remain counted if absent address, wrong identity or already
released state subsequently traps. Invalid release preserves every observed
management record and initialized requested payload byte, cursor, pages, population,
last identity and all 29 free-list heads; only the explicit measurement global
changes. Snapshots exclude spare capacity and unallocated memory. Instrumentation adds no calls or linear-memory
allocation, and changes neither class policy, identity issuance nor outcomes.
Decoded tests verify every body is identical after removing exactly one four-op
counter increment from FIND, call/load/store/growth operations match, and precisely
one global is added. Ordinary/noninstrumented modules have no counter export;
ordinary compilation remains outside the allocator entirely. A selected user-export
collision is rejected only when instrumentation is requested.

[search.json](search.json) controls populations 0/1/8/32/128/512. Registry order is
newest physical record first. At population N, head/middle/tail successful releases
compare exactly `1 / (floor(N/2)+1) / N` records. An absent lookup compares N,
including zero comparisons with an empty registry. Wrong-identity middle lookups
and double-release tail lookups trap after the same rank-dependent search. At
N=512 the counts are **1 / 257 / 512 / 512** (head/middle/tail/absent).
This characterizes linear release search without changing its authority checks.

Module sizes: ordinary **730**, private noninstrumented **2,186**, instrumented
**2,226** bytes (exact ordinary size is also recorded in `environment.json`). The
counter costs **40 emitted bytes** plus eight logical global-value bytes, not a
measured engine overhead. Instrumented modules are never used for timing.

## Noninstrumented guest-operation timing

[timing.json](timing.json) preserves all 150 raw samples. Ten population/rank cases
run three repetitions, five fresh-instance samples per case/repetition, reversing
case order in the middle repetition. Each sample provisions/initializes N blocks
outside timing, warms up 2,048 release/reacquire pairs, then times 4,096 pairs on
one fixed physical block. Registry rank remains fixed; reuse renews identity.
Cursor/pages/population, final identity and still-live contents are checked outside
timing. The final reacquired extent is initialized before post-timing reads.

`hrtime.bigint()` encloses only a finite JS loop, two JS/Wasm calls per pair,
release search/authority checks, exact-class reuse and multivalue return transport.
It **includes JS iteration/destructuring overhead**; this is not isolated Wasm
instruction latency. Payload initialization/verification, compilation, instantiation,
module validation, correctness probes, warmup, observations, instrumentation,
hashing and reporting are outside timing. There are no allocation markers and no
instrumented timing or overhead subtraction. Wasm is emitted with the private
baseline numeric policy; native release-profile compilation is not evidence of a
language-optimizer speedup. V8 tier stability is not claimed.

Recorded medians (15 samples per row), **ns per release/reacquire pair**:

| Population | Rank | Median |
| ---: | ---: | ---: |
| 1 | 1 | 139.87 |
| 32 | 1 | 216.43 |
| 32 | 17 | 136.69 |
| 32 | 32 | 162.28 |
| 128 | 1 | 113.70 |
| 128 | 65 | 283.37 |
| 128 | 128 | 476.51 |
| 512 | 1 | 140.53 |
| 512 | 257 | 821.14 |
| 512 | 512 | 1,443.55 |

Head timing variation illustrates noise/tiering, not different allocator policy.
Deep searches cost more in this recording, consistent with independently counted
comparisons. No hardware-independent time threshold, comparative speedup, memory
shrinking/page return, compiler optimization benefit or production suitability is
claimed. Correct bounded reuse does not require a speedup.

## Validation and review

The recorder's [correctness.log](correctness.log) reports 38 passed allocator tests,
none failed/ignored; each release-profile measurement log selects one test and
passes. Native tests validate all emitted modules with `wasmparser::Validator` and
independently decode instrumentation; Node validates the measured modules too.

| Additional command | Result |
| --- | --- |
| `cargo check --offline -p was-compiler --all-targets` | Passed |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed; fresh ordinary compiler/browser package |
| `node tests/browser.cjs pkg` | 25 groups passed |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed |
| `cargo test --offline --workspace -- --include-ignored --test-threads=1` | **Timed out at 300 seconds**; full workspace acceptance is unverified, not reported as passing |

The full run was bounded by Python's subprocess timeout. Its captured partial
output was not retained on timeout, so no aggregate test count or failure claim
is made. Process inspection afterwards found no remaining Cargo/target test
processes. A longer full-suite run remains follow-up work; current-source browser
smoke and focused private allocator results above are separate evidence, not a
substitute for that gate. wasm-pack printed its wasm-bindgen tooling-resolution
and optional package-metadata notices; no separate install/fetch command was run.

The initial instrumentation test failed because the opt-in compiler entry point
did not exist; accounting/search/timing tests each failed before their corresponding
harness mode existed, then passed. Formatting was limited to edited Rust files.

Independent standards/spec reviews against the starting revision identified the
same initialization-boundary problem: the first search probe snapshotted whole
memory, including unrequested spare capacity. The probe now copies only management
records and initialized nine-byte requested payloads, and separately compares the
cursor/pages/population/identity/free-list globals through non-searching observations.
Preservation claims were narrowed accordingly. All measurements and source provenance
were regenerated after this correction; the raw records here are from that final
run. Independent rechecks reported **zero remaining Standards findings** and
**zero remaining Spec findings**. Both checked the corrected probe boundaries,
staged-source hashes (allowing CRLF normalization), raw JSON/log consistency,
search ranks and all ten medians. The full-workspace timeout remains a validation
limitation, not integrated M3 acceptance.

The explicit combined measurement filter subsequently passed **5 tests, none
failed/ignored**, confirming that the documented filter selects nonzero cases.
`node --check tests/support/m3_allocator_measure.cjs` and both Git diff whitespace
checks passed. No compatibility fixtures were regenerated.
