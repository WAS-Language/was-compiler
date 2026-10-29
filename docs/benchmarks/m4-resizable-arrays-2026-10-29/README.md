# M4 resizable-array growth, reuse and storage costs (#71)

Recorded 2026-10-29 on Windows x64 (10.0.26300), Intel Core Ultra 5 125H,
Node 24.12.0 / V8 13.6.233.17-node.37, Rust/Cargo 1.98.0, Python 3.13.14.
This is current resizable-array evidence, **not** historical typed-array #37's
“M4” measurement phase, and not integrated memory-roadmap M4 release acceptance.
The [resizable contract](../../resizable-arrays.md) and [ADR 0009](../../adr/0009-opt-in-resizable-array-storage.md)
remain authoritative. No production behavior, options, management exports,
ownership rules or dependencies change.

## Reproduction and provenance

From the repository root, with cached dependencies and Node 24+/Python 3 installed,
run sequentially, without competing builds or benchmarks:

```sh
python tests/support/record_m4_resizable.py target/m4-reproduction
```

Choose a fresh output directory; existing JSON/log evidence is never overwritten.
The recorder runs runtime correctness, historical M3 measurement compatibility
(not new M4 evidence), and the new native isolation checks first. It then runs
these **release-profile**, exact, nonzero recording commands in order:

```sh
cargo test --offline -p was-compiler --test resizable_arrays_runtime -- --include-ignored
cargo test --offline -p was-compiler --lib codegen::allocator::measurement_tests -- --include-ignored --test-threads=1
cargo test --offline -p was-compiler --lib codegen::resizable::measurement_tests -- --skip record_m4 --test-threads=1
cargo test --offline --release -p was-compiler --lib codegen::resizable::measurement_tests::record_m4_accounting -- --ignored --exact --nocapture --test-threads=1
cargo test --offline --release -p was-compiler --lib codegen::resizable::measurement_tests::record_m4_guest_timing -- --ignored --exact --nocapture --test-threads=1
```

Rust bounds each Node child at 120 seconds using `tests/support/process.rs`;
the Python recorder bounds each Cargo child at 600 seconds and preserves partial
output on timeout. Neither is guest fuel or process-tree cancellation. The fixture
modules are regenerated at `target/m4-resizable/{ordinary,observed}.wasm`, not
committed. Ordinary compilation uses the baseline numeric policy; Cargo release
optimizes the native recorder, not the language's numeric policy. Compiler timing
is not collected here. The separate compiler benchmark smoke check is not a
compiler timing baseline.

[environment.json](environment.json) records the exact commands, base revision
`64c2cd19c0a39983a160ddd0e7d310ee3365b9d9`, dirty/untracked state, tool versions,
selected environment overrides, and SHA-256 hashes of all compiler sources,
manifest/lockfile, corpus, harness, process helper and recorder. It verifies those
source bytes remain identical after recording. This is dirty-working-tree evidence:
the base revision alone does not contain this experiment. Existing unrelated
`examples/program.was` / `program.wasm` appear in status but are not inputs.
Documentation and subsequent validation logs are outside the executable-source
manifest. Raw [accounting.json](accounting.json) and [timing.json](timing.json)
include OS/CPU/V8, UTC, source/harness/module hashes and all results. Logs retain
complete Cargo stdout/stderr, including the raw JSON. Git checkout newline
conversion can change byte hashes; compare recorded bytes on the same checkout.

The [source snapshot](workload.was) matches `tests/support/m4_resizable_corpus.was`.
That corpus supplies deterministic per-type constructors/method wrappers; the
harness defines and records every bounded accounting call and item index.
String items alternate static `"even"` / `"odd-λ"`, avoiding per-push string
allocation. Numeric items are `3*index - 7.25`, bool items alternate true/false.

## Private observation and isolation

`crates/compiler/src/codegen/resizable_measurement_tests.rs` compiles through the
existing checked backend, then rewrites only explicitly requested unit fixtures:

- Export existing cursor, last identity and allocator/array registry root globals.
- Add two i64 counters, exported only by the observed variant: increment once per
  visited non-null allocator FIND / per-element authority LOOKUP record, immediately
  after the null guard, before comparison. Includes every nested lookup in an
  operation; does not count a null terminator or elapsed time.
- No management operation, host callback, memory allocation or production switch
  is added. Ordinary modules have no `__m4_*` exports or counters. Ordinary
  integration dependencies cannot activate this `cfg(test)` module.

Native tests validate all variants with `wasmparser::Validator`, independently
remove exactly the four counter instructions at every planned target, and require
all original bodies/locals and other sections to match. They check ordinary-byte
determinism, export isolation and collision handling. The accounting harness
executes identical operations on fresh ordinary/observed instances and compares
handles, returned scalars, live elements, pages, every initialized allocator record
and all descriptor/authority bytes after **every call**. Ordinary registry roots
and cursor are not independently exported: paired observed addresses locate its
records. No spare/free payload bytes or stale removed slots are compared/read.
Host observation uses direct loads and does not execute FIND/LOOKUP or increment
counters. i64 counters remain decimal strings; bounded record fields are checked
for safe exact conversion to JS numbers.

## Observation definitions

All accounting snapshots occur after a synchronous guest call completes. Arrays
remain retained in a host ledger; “live” here means the ledger's current logical
contents, **not** a tracing-GC liveness proof. Views and payload addresses are
reacquired. Independently decode every live number, canonical bool and UTF-8
string, compare against a JS sequence oracle, validate registry membership,
identity/capacity/length correspondence and pairwise-disjoint physical extents.
Spare capacity is never made logically readable.

| Field | Definition / temporal classification |
| --- | --- |
| `liveElements`, `liveBytes` | Current logical slot count / width-scaled slot bytes; string slot bytes exclude referenced immutable string data |
| `capacityElements`, `activeCapacityBytes` | Current sum of full acquired capacities divided by each array's width / in bytes; widths number 8, bool 1, string handle 4 |
| `spareCapacityBytes` | Current active capacity minus logical slot bytes; distinct from free payload storage |
| `freeCapacityBytes`, `freeByClass` | Current retained released payload bytes, reusable only in their exact byte class |
| `physicalCapacityBytes` | Current distinct physical payload capacity, active plus free; monotonic in these runs, also its final peak/freshly provisioned cumulative total |
| `allocatorRecords`, `allocatorRecordBytes` | Current physical population and `32 * population`, including free records; no per-acquisition history records |
| `authorityDescriptors`, `authorityObjectBytes` | Current retained array count and `48 * count`: 8-byte descriptor plus 40-byte authority per array; not engine metadata |
| `cursor`, `bumpGrowthBytes` | Absolute current bump address / cursor minus fresh-instance initial cursor; includes ordinary allocations |
| `otherBumpBytes` | Residual bump growth minus physical payloads, allocator records and authority objects; constructor source literals and alignment, not inferred free/live payload |
| `pages` | Current Wasm memory size in 65,536-byte pages; not consumed or live bytes |
| `operationBoundaryPeak` | Per-field maximum over completed-call snapshots only; excludes transient old-plus-new payload residency inside growth |
| `cumulative.acquisitions`, `acquiredCapacityBytes` | Successful constructor/relocation event count and capacity sum, including repeated reuse; not fresh provisioned bytes |
| `relocations`, `reusedAcquisitions` | Cumulative successful payload replacements / acquisitions reusing previously registered free physical blocks |
| `inferredCopiedLiveBytes` | Pre-relocation logical length times width summed over verified relocations; excludes constructor copies, an inference from checked algorithm/observed relocation, not measured load/store counts |
| `comparisons` | Cumulative i64 counts; each sequence entry records operation-local deltas separately |

The independent accounting identity at every snapshot is:

```text
bumpGrowthBytes = activeCapacityBytes + freeCapacityBytes
                + allocatorRecordBytes + authorityObjectBytes + otherBumpBytes
```

Free-chain reconstruction checks unique linked membership within each class,
without exporting or independently checking hidden free-list head globals.
Disjointness includes full allocator record/payload and authority/descriptor
extents; it is not a census of ordinary source literals or static strings.
Actual engine global storage, allocator overhead, RSS and GC costs are unavailable.

## Growth, removal and reuse results

[accounting.json](accounting.json) records **1,233 mixed-type operations** plus
three independent exact-class-stranding sequences. Each type exercises empty and
three-element construction, a 129-push prefix, a second growing array, reserve
followed by no-op reserve(0)/(1), spare-capacity append, front/middle/last removal,
pop, and three additional fill/shrink/reserve/refill arrays. All earlier arrays
remain independently verified after each call. Removal preserves order and
capacity; scalar return values are checked as well as remaining contents.

Independent geometric prefix expectations:

| Type | Pushes | Capacity bytes at 129 elements | Relocations | Inferred copied live bytes |
| --- | ---: | ---: | ---: | ---: |
| number | 129 | 2,048 | 8 | 2,040 |
| bool | 129 | 256 | 5 | 248 |
| string | 129 | 1,024 | 7 | 1,016 |

Each append transition doubles byte capacity. Copy sums equal the geometric series
`finalCapacity - 8`, strictly below `2 * pushes * width`, rather than quadratic
copy growth. This is a bounded policy/copy-volume result, **not** an O(1) total
append-time claim: authority and physical-registry searches still cost work.

| Mixed workload final observation | Value |
| --- | ---: |
| Logical elements / slot bytes | 879 / 3,809 |
| Acquired element capacity / active bytes | 3,456 / 14,976 |
| Active spare / free payload bytes | 11,167 / 560 |
| Distinct physical payload bytes | 15,536 |
| Active / free / total allocator records | 15 / 9 / 24 |
| Allocator record bytes | 768 |
| Retained descriptor / authority bytes | 120 / 600 |
| Initial / final cursor | 40 / 17,232 |
| Bump growth / other bump bytes | 17,192 / 168 |
| Pages | 1 |
| Cumulative acquisitions / reuse / relocations | 103 / 79 / 88 |
| Cumulative acquired / inferred copied-live bytes | 26,472 / 8,844 |
| Allocator / authority comparisons | 22,409 / 4,632 |

Boundary free-capacity peak is 2,072 bytes, spare-active peak 11,235 bytes;
final values are smaller. Monotonic storage metrics have final maxima. The raw
record retains **all** per-field maxima. One page throughout is not a plateau:
cursor, authority count and physical storage increase. There is no reuse of the
48-byte stable array objects or ordinary constructor source literals.

Each stranding case creates ten arrays, moves each through 64-byte then 128-byte
payloads, retaining **720 free payload bytes**, including ten free 64-byte blocks.
A subsequent 512-byte request has no exact-class block and consumes **544 new
bump bytes** (512 payload + 32 record), while releasing another 128 bytes. No
splitting, coalescing or larger/smaller-class borrowing is implied by total free
capacity. Mixed acquisitions demonstrate actual payload-address reuse, not merely
stable pages or repeated logical contents.

Search work is explicit: in each ten-array stranding sequence, reserve on arrays
0 through 9 incurs authority comparison deltas **20,18,16,14,12,10,8,6,4,2**,
consistent with two lookups at ranks 10 through 1 in the newest-first authority
registry. Allocator FIND also linearly traverses physical records; nested validation,
replacement and release all contribute, even if capacity already suffices. The
counter definitions/isolation tests establish what is counted; timings do not
isolate individual comparisons.

## Fresh ordinary-instance guest timing

[timing.json](timing.json) retains **180 raw samples**: 12 type/case combinations,
3 repetitions, 5 fresh-instance samples each; case order reverses in repetition 2.
Warmup executes one full case on separate discarded ordinary instances. Every
sample starts from a fresh **uninstrumented** instance of the ordinary module.
Both modules are validated/compiled outside timing; the observed module is never
instantiated by timing. Setup, instantiation, expected-sequence construction,
decoding, assertions, hashing and reporting are outside `hrtime.bigint()`.

Timed regions include JS loops and JS/Wasm crossings, parameter checks, item
selection (including bool/string remainder), authority/allocator searches,
mutation, allocation/copy/growth as applicable and return transport. This is not
isolated Wasm instruction latency. Cases are 512 pushes from empty; 4,096 spare
push/pop pairs with 128 elements reserved; 2,048 ordered-remove/push pairs at length
64 (indices cycle 0/32/63); and 4,096 reserve(0)/(1) calls on the oldest of 32 arrays.
Final logical contents and retained companion arrays are checked outside timing.

Medians of 15 samples each, **ns per iteration** (a pair for paired cases):

| Type | Append | Spare push/pop | Ordered remove/push | No-op reserve, oldest/32 |
| --- | ---: | ---: | ---: | ---: |
| number | 115.62 | 160.74 | 266.36 | 225.73 |
| bool | 113.87 | 156.91 | 192.09 | 222.78 |
| string | 132.42 | 173.34 | 284.77 | 176.03 |

Ordinary module: **6,126 bytes**, observed: **6,305 bytes**. The 179-byte difference
includes exports and two counter globals/increments, not measured engine overhead.
Exact hashes are in both raw files and provenance. No matched old/new compiler
comparison, cross-type speed ranking, statistical significance, tier stability,
optimizer benefit, or generally bounded execution cost is claimed.

## Retention limits and validation

This successful workload retains all constructed arrays. Even when language/host
references are dropped, the array registry retains descriptors, authority and their
current active payloads; there is no unreachable-array collector. Referenced
immutable strings are never reclaimed. Failed construction can retain an acquired
unpublished payload and an unpublished 48-byte object; it is not part of this
successful-workload census. [#70 failure evidence](../../resizable-arrays.md#issue-70-validation)
checks retained unpublished acquisitions and later recovery, separately from this
recording. No failed-construction byte total is inferred from pages here. Growing
peaks, new classes, growing array counts or failed constructions can keep increasing
storage. Payload reuse is not GC, heap rewind, page return or a universal memory
bound, and does not establish release suitability.

Recorded prerequisite logs: [runtime-correctness.log](runtime-correctness.log)
(38 passed), [m3-correctness.log](m3-correctness.log) (5 passed), and
[isolation.log](isolation.log) (3 passed). Each release-profile recording log
([accounting](accounting.log), [timing](timing.log)) selects exactly one passing
test.

Additional final checks, run sequentially after measurement:

| Command | Result |
| --- | --- |
| `cargo check --offline -p was-compiler --all-targets` | Passed |
| `cargo bench --offline -p was-compiler --bench compiler -- --test` | Passed; [smoke output](bench-smoke.log), not compiler timing evidence |
| `cargo test --offline --workspace` | 1,302 passed, 0 failed, 300 ignored across 86 test-result targets; [complete output](workspace-tests.log) |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed; [output](clippy.log) |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed; [output](core-wasm.log) |
| `node --check tests/support/m4_resizable_measure.cjs` | Passed |
| Edited-file Rust formatting and Git diff whitespace checks | Passed |

Only terminal blank lines were removed from the ancillary benchmark/workspace
validation logs for Git whitespace hygiene; measurement logs are unmodified.
The full ignored workspace suite was not run. Browser artifacts were not rebuilt:
production code and adapters are unchanged, and the only existing implementation
file edit is a `cfg(test)` module declaration. The focused runtime runs above
provide their explicit ignored coverage, not full workspace/rebuilt-browser gates.

The instrumentation agent reproduced red before implementing the missing private
seam; native isolation checks then passed. The corpus/accounting/timing harness
was exercised at the ticket's established private-unit and bounded Node seams.
Independent Standards and Spec reviews against starting commit `64c2cd19` each
reported **zero findings**. Both verified source/provenance hashes and raw-log
consistency; Spec additionally checked every reported median, accounting identity,
peak and comparison sum. Reviewers did not rerun benchmarks or witness sequential
execution. Integrated M4 release acceptance remains pending.
