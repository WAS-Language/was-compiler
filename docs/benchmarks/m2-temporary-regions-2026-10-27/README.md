# M2 successful reuse and trap quarantine (#56)

Private measurement evidence, recorded 2026-10-27. This is not public activation,
a revision-1 ownership change, general reclamation, or the integrated #57 gate.
The [prototype contract](../../m2-region-prototype.md) remains authoritative.

## Workload and observation boundary

The exact source is [workload.was](workload.was), generated from the private unit
caller in `crates/compiler/src/codegen/region/measurement_tests.rs`. JavaScript
repeats calls; the eligible functions themselves have no loops, host calls,
reentry or widened M1 proof. All variants compile the **same borrowed typed
input**, through checked lowering, at None/Basic/Standard/Aggressive. Each module
passes `wasmparser::Validator`. Native tests independently decode calls, imports,
exports and global counts, verify capacities, deterministic bytes, ordinary-path
noninterference and enabled-preset byte identity.

| Function | Reserved bytes | Executed behavior |
| --- | ---: | --- |
| `numbers(n, i)` | 24 | Two numeric elements, replacement through `+=`, indexed scalar return; index 99 traps after completed initialization/write |
| `aligned(n)` | 48 | Empty numeric array (8), nine bool elements (24 including padding), one numeric element (16); simultaneously live distinct slices |
| `lazy()` | 16 | Returns 7 before its reserved literal; no execution/acquisition |

Before **every** measurement mode, all three module variants execute independent
scalar checks with negative/fractional/zero/positive inputs and an out-of-bounds
trap. A separate marked observation instance checks both descriptor words, both
numeric elements, the completed compound write and retained bytes after later
calls. These instances are discarded before accounting, capacity or timing.
This verifies measured behavior, not universal semantic equivalence. Existing
#54/#55 tests cover the broader adversarial correctness contract.

## Counter definitions

Only `region::instrumentation::compile_instrumented` emits these counters. It
uses the same selector, plans, checked bodies and assembler. Ordinary compilation
and the uninstrumented prototype have no counter globals or inspection exports.
Instrumentation adds **no calls or linear-memory allocations**. Four i64 globals
per selected function are exported as `__m2_<function>_<field>` and read only by
this trusted test harness. It never mutates them.

- `provisions`: successful backing allocations, counted **after** `Alloc` returns
  and the invocation receives the base. Allocation failure counts zero.
- `reuses`: acquisition from a nonempty per-function cache, not fresh provision.
- `literals`: entries into a selected literal's allocation path, **after successful
  block acquisition but before descriptor/element initialization**. A subsequent
  initializer trap would still count that literal entry; a provisioning trap does
  not. This is not a completed-initializer count or a count of reservations.
- `retirements`: normal completion returning an acquired block to its cache;
  scalar results are already computed. A no-acquisition return counts zero.

Samples occur only after synchronous calls return or throw, with no active or
suspended invocation. At this quiescent boundary:

```text
quarantined blocks = provisions + reuses - retirements
cached blocks = retirements - reuses   (0 or 1 per eligible function)
provisioned bytes = provisions * function capacity
reused-block bytes = reuses * function capacity
quarantined bytes = quarantined blocks * function capacity
cached capacity = cached blocks * function capacity
```

These are event-derived whole-block capacities, **not occupancy or live bytes**.
Reused-block bytes sum capacity across reuse events: repeatedly counting the same
block is deliberate, not newly allocated or uniquely reclaimed bytes. The host
observes completed traps but performs no guest cleanup and inserts no callbacks.
Counters use i64 wrapping addition; the bounded runs remain far below wraparound.

Management overhead is zero linear-memory header bytes, one i32 cache global per
selected function (three total), and one i32 invocation local per selected body.
`lazy` reserves a cache slot but never provisions backing storage. The instrumented
variant adds twelve i64 counter globals (96 bytes of logical scalar values) plus
increment instructions/export names. Actual engine storage/RSS overhead is
**unavailable**; logical widths are not physical engine allocation measurements.
Baseline dynamic literal/provision counts and peak live bytes are **unavailable**,
not inferred from pages. Source footprints supply independent marker expectations.

## Recorded results

All four presets agree on accounting, capacity and failure results. Raw JSON files
contain individual snapshots, samples, source/module/harness SHA-256 hashes, sizes,
Node/V8/OS/CPU details and UTC recording times. [environment.json](environment.json)
records base revision, dirty working-tree status, commands/tool versions and input
file hashes. The pre-existing roadmap modification and deleted example were
preserved and are unrelated to these measurements.

- After 256 interleaved successful calls **per function**, `numbers` and `aligned`
  each have **one provision and 255 reuses**: provisioned reusable capacity
  plateaus at **24 + 48 = 72 bytes** after the first pair. Their literal counts are
  256 and 768. `lazy` has zero events after 256 calls, despite its 16-byte plan.
- Eight subsequent `numbers` traps quarantine eight blocks/**192 bytes**. The first
  trap consumes its cached block (a reuse); later traps provision fresh blocks.
  `aligned` retains its independent 48-byte cache throughout. Recovery provisions
  a ninth numeric block; the next success reuses it. Quarantine stays at 192 bytes,
  separate from the recovered 72-byte combined reusable capacity.
- On separate uninstrumented marked instances, 256 calls per function consume
  **18,432 aligned bump bytes baseline versus 72 prototype**. This is allocator
  consumption, not reclaimed/live bytes. Neither number is inferred from pages.
- On separate fresh **unmarked** instances, 4,096 calls per function change pages
  **1 → 5 baseline versus 1 → 1 prototype**. Pages measure capacity only.
- A separately capped instrumented module with one maximum page has only 16 bytes
  left when a 24-byte reservation is attempted. It traps, leaves all counters zero,
  preserves every linear-memory byte and the allocator cursor, and creates **no
  quarantine**. The subsequent marker consumes 8 of the remaining 16 bytes. Its
  page checks are failure diagnostics on a marked instance, not capacity evidence.

### Module size and timing

| Preset | Baseline bytes | Prototype bytes | Instrumented bytes |
| --- | ---: | ---: | ---: |
| None | 1,350 | 1,534 | 2,011 |
| Basic / Standard / Aggressive | 1,314 | 1,498 | 1,975 |

The prototype costs **184 module bytes** over baseline; instrumentation costs a
further **477 bytes**. Failure modules differ only in the explicitly capped memory
section, reducing the instrumented size by two bytes; their hashes are recorded
separately. These are size regressions, not savings hidden by instrumentation.

Timing JSON contains three matched repetitions, alternating variant order, with
10 samples each. Each sample instantiates a fresh **uninstrumented, unmarked**
instance outside timing, then times 256 interleaved pairs (512 guest calls) and
checksum accumulation. Eight warmup calls per variant/repetition occur outside
timing. `hrtime.bigint()` surrounds only the finite JavaScript loop, JS/Wasm calls,
guest execution (including allocation/growth) and checksum. Compilation,
instantiation, validation, assertions, warmup, hashing, accounting and reporting
are excluded. The independent checksum is 68,864. Raw samples retain outliers;
there is no tier-stability claim, performance threshold or general speedup claim.
Recorded aggregate medians across the 30 raw samples (ns/guest call):

| Preset | Baseline | Prototype |
| --- | ---: | ---: |
| None | 117.97 | 99.41 |
| Basic | 122.66 | 105.57 |
| Standard | 131.25 | 122.85 |
| Aggressive | 116.50 | 98.14 |

These observations favor the prototype in this recording, but establish no
representative speedup. Differences between enabled presets with identical bytes
expose run noise rather than different generated behavior. Use the recorded samples, not a cross-machine
comparison, for this bounded workload only.

## Reproduction and validation

Prerequisites: cached Rust dependencies, Node.js 24+, no network installation.
Run from the root, **sequentially**, with no other benchmark/build concurrently.
The Rust caller applies a 60-second direct-child deadline to each Node run; this
is not guest fuel or process-tree cancellation.

```sh
cargo test --offline -p was-compiler --lib codegen::region::measurement_tests -- --include-ignored --nocapture --test-threads=1
```

For isolated recording, each command selects one ignored test (each runs all four
presets). Raw artifacts and exact modules are written under `target/m2-regions/`.
Only source/JSON/log evidence is preserved here; binaries can be regenerated and
checked against the recorded hashes.

```sh
cargo test --offline -p was-compiler --lib codegen::region::measurement_tests::record_m2_accounting -- --ignored --exact --nocapture --test-threads=1
cargo test --offline -p was-compiler --lib codegen::region::measurement_tests::record_m2_capacity -- --ignored --exact --nocapture --test-threads=1
cargo test --offline -p was-compiler --lib codegen::region::measurement_tests::record_m2_failed_provisioning -- --ignored --exact --nocapture --test-threads=1
cargo test --offline -p was-compiler --lib codegen::region::measurement_tests::record_m2_guest_timing -- --ignored --exact --nocapture --test-threads=1
```

Marker consumption is `after - before - 8`. Both `__alloc(0)` calls reserve at
least eight bytes; the final marker's returned address precedes its own reserve.
Thus subtract only the first marker's eight bytes from the address difference.
Markers can cause growth and never share the capacity/timing instances. The
harness independently checks the eight-byte minimum with two zero requests.
Failure-mode markers and raw-memory effect observations are diagnostic only.

Node measurement tests remain ignored by default, private library unit tests,
not public integration or feature activation. The unchanged 19-workload ordinary
memory corpus remains baseline-only; it was not made M1-positive. Integrated
browser/full ignored release validation remains owned by #57.

### Final validation and review

The [command results](validation-commands.json) and `validation-*.txt` logs preserve
successful final offline runs:

| Command / log | Observed result |
| --- | --- |
| Private region suite including Node, one test thread / [log](validation-region.txt) | 23 passed, none failed or ignored; includes all six measurement tests |
| Ordinary `memory_benchmarks` / [log](validation-memory.txt) | 1 passed, 4 intentionally ignored |
| Explicit independent-results/allocation runtime baseline / [log](validation-memory-runtime.txt) | 1 passed, none ignored |
| `cargo test --offline` / [log](validation-default.txt) | 1,196 passed, 219 intentionally ignored across 70 result summaries; zero failures |
| `cargo clippy --offline --all-targets -- -D warnings` / [log](validation-clippy.txt) | Passed |
| Portable core `wasm32-unknown-unknown` build / [log](validation-wasm.txt) | Passed |

The initial instrumentation test failed because `compile_instrumented` did not
exist, then passed after the private seam was implemented. Native measurement
tests and the production library check were rerun during development. Formatting
was limited to edited Rust files. Node syntax and Git whitespace checks passed;
compatibility fixtures were not regenerated.

Independent standards/spec review used base revision
`88f3d42238a78a8820792c506c6ebe5da7c7b33b`, excluding the two pre-existing user
changes. Standards found zero hard violations and one low-priority duplicated
counter-schema suggestion. It was resolved by making `Event::ALL` authoritative
for count/order/index, with exhaustive names, then rechecked with zero remaining
findings. Spec found zero missing/partial requirements, scope additions or
implementation errors. Final recordings and validation ran after that fix. The
ordinary/experimental guest module identities were unchanged by the schema cleanup.
