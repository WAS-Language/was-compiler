# M5 integrated measurement reconciliation (#81)

## Result and decision boundary

Two fresh deterministic construction recordings reproduce **all 103 construction
rows and 34 controlled concat baselines**, including their complete accounting,
correctness, lifetime and produced-fragment records, byte-for-byte against both
historical [#79 recordings](../m5-fragment-construction-2026-10-30/README.md).
Three fresh ordinary timing recordings retain **630 native and 1,845 runtime raw
samples each**. Their executable inputs, compiled executables and corpus modules
are identical to [#80](../m5-strings-2026-10-30/README.md), not a changed-code experiment.

The bounded **O(n + B) aggregate guest storage/copy-volume result** is reproduced;
it is not an O(n + B) running-time claim. **Material unexplained timing variability
remains explicitly escalated for disposition. Integrated M5 acceptance is pending
that disposition**, not granted by successful execution or favorable medians.
[M4's pending performance-signal disposition](../../m4-release-readiness.md) is
unchanged. No optimization, default, ABI, retention, search algorithm, fusion,
builder API or reclamation change follows from this record.

This document reconciles existing evidence only. No builds, tests or benchmarks
were run while writing it. The parent-owned
[validation manifest](../../m5-integrated-validation-2026-10-30.json) owns command
execution/provenance; this README does not replace the integrated release decision.

## Evidence and provenance verification

The fresh recordings use revision
`23441c9c27d9eae9d1a227ad12bba7ecab689217`, with the dirty working-tree state
preserved in each environment record. Construction recordings started at
**23:32:59 and 23:33:03 UTC**, timing recordings at **23:33:07, 23:33:15 and
23:33:22 UTC on 2026-10-30**. These are working-tree records, not isolated
clean-checkout evidence.

| Recording | Provenance | Raw accounting or samples | Complete timing summaries |
| --- | --- | --- | --- |
| Construction 1 | [environment](construction-1/environment.json) | [accounting](construction-1/accounting.json) | No timing collected |
| Construction 2 | [environment](construction-2/environment.json) | [accounting](construction-2/accounting.json) | No timing collected |
| Timing 1 | [environment](timing-1/environment.json) | [native](timing-1/native.json), [runtime](timing-1/runtime.json) | [summary](timing-1/summary.json) |
| Timing 2 | [environment](timing-2/environment.json) | [native](timing-2/native.json), [runtime](timing-2/runtime.json) | [summary](timing-2/summary.json) |
| Timing 3 | [environment](timing-3/environment.json) | [native](timing-3/native.json), [runtime](timing-3/runtime.json) | [summary](timing-3/summary.json) |

Read-only SHA-256/size reconciliation checked:

- All **240 manifest input hashes**, **283 manifest artifacts**, **34 command
  stdout/stderr references**, and **six freshly rebuilt browser artifact hashes**
  against the files on disk. All match.
- All five fresh environment records' input/artifact hashes, and the historical
  #79/#80 records against available files. Each construction record covers **101
  inputs**; each timing record covers **233**. The larger manifest scope is not a
  claim that the timing recorder hashed 240 inputs. All current input hashes match;
  before/after stable-input checks are recorded as successful.
- All **233 timing input paths and hashes** match across the three fresh and three
  historical #80 runs: **zero additions, removals or changed hashes**. Both native
  executable hashes also match across all six runs and the current executables:
  benchmark `ec393d86ad7671396264b89300a1196aa73e5c384af40b11d82c55b4ae40b636`,
  CLI `d057b779b748a5c338ab509591bbe389b754c6735e10cdee0b115b52852ee98a`.
- The root [input snapshot](inputs.zip), all three fresh timing ZIPs and all three
  historical #80 ZIPs have exactly their corresponding source-hash path sets;
  **1,638 ZIP entries** in total match their recorded SHA-256 values. Construction
  inputs are covered by their narrower hash sets and the root snapshot; there is
  no claim of separate per-construction ZIPs.
- All **21 native source/module pairs** and the ordinary runtime source/module
  match across the six timing runs. Native sources span **94–4,866 bytes** and
  modules **918–4,289 bytes**. Runtime source is **878 bytes**, SHA-256
  `cac918716bed5a258097c65c16446fb13084ed69e11aa10aa01919307389cb60`;
  runtime module is **4,258 bytes**, SHA-256
  `a31dcab3a7cf82108b603591e61d3510780b463bd9ee39626e9ad0d8225576a1`.
- Both fresh construction modules match each other and both historical #79
  environment records: ordinary **4,966 bytes**, SHA-256
  `6c03e85eeb90ae19652798b15c20dc99c8f52fc78b8dd735f23c57040f9c8bc0`;
  observed **5,580 bytes**, SHA-256
  `f79909aa3b87e5a2384b69e622922cd12325d6d5f3f97bb7aa8c39a3c6519027`.
  Corpus snapshots and all 101 construction input hashes match #79 as well.

**Archival limitation:** the four historical #79 `ordinary.wasm`/`observed.wasm`
files are absent from its two run directories. Historical module comparison is
therefore against the recorded size/hash pairs, not a rehash or byte comparison
of those absent files. The fresh modules are present and independently rehashed.
There are no fresh hash mismatches. Full accounting bytes match all four records,
SHA-256 `51cf4533fbb8b919d17dd7584e99860945d8258aa643872d2ea533d2bd222c05`.

Environment remains **Windows x64 10.0.26300, Intel Core Ultra 5 125H, 18 logical
CPUs, Node 24.12.0 / V8 13.6.233.17-node.37, rustc/Cargo 1.98.0, Python 3.13.14**.
Timing environment overrides record only processor identity/count. All fresh
recordings completed successfully; none of their logs records a Cargo build-lock
wait. The manifest records sequential task execution without competing task
builds/benchmarks. The [pre-run process snapshot](processes-before.log) has no
Cargo/rustc/rustdoc entry but does contain Node processes; it does not identify
their workloads or prove system idleness. There was no continuous load, power,
thermal, GC or engine-tier monitoring. Input identity is not machine-state control.

## Deterministic construction: full reproduction, unchanged interpretation

The [#79 methodology](../m5-fragment-construction-2026-10-30/README.md) is inherited
unchanged: fresh paired ordinary/observed instances, independent content/layout
oracles, complete memory/handle/cursor parity, decoded instrumentation isolation,
and actual completed byte-store counters. Six patterns/separators at 17 sizes
**0, 1, 2, 3, 4, 5, 8, 9, 16, 17, 32, 33, 64, 65, 129, 257, 1025**, plus the
initial tracer, give 103 rows. Both one- and five-byte controlled concat fragment
widths at every size give 34 baselines. No rows or small-size overheads are omitted
from the byte-for-byte accounting comparison.

`n` counts fragment occurrences, including empties; `B` counts fragment UTF-8 bytes
by occurrence plus every output separator occurrence. Inputs are preexisting in
these accumulation rows, so fragment production costs zero there. Their physical
input byte-range union is reported once outside runtime storage. The separate
produced-fragment interval records a six-byte concat result (**16 reserved bytes**)
and its later 12-byte join result (**24 reserved bytes**) separately.

For four-byte handle slots, `C = max(8, next_power_of_two(4n))`. Geometric acquired
capacities sum to `2C - 8`; relocation copies `C - 8` handle bytes. Each physical
capacity has a 32-byte record, in addition to the eight-byte source descriptor and
48-byte stable descriptor/authority object. Retained reservations partition as:

```text
8 + 48 + 32 * physical_record_count
  + active_capacity + free_capacity + result_reservation
result_reservation = align8(8 + B) when join materializes, otherwise 0
```

For n >= 2, each row checks reserved storage <= `256(n + 1) + B + 8`, and handle
plus string copying <= `8n + B`. Singleton reuse/empty-result policy are current
implementation details, not mandatory future allocation semantics. The controlled
None-preset concat baseline preserves its first empty-accumulator reuse: for
n >= 1 and fixed nonempty fragment width `f`, copied string bytes are
`f * (n(n + 1)/2 - 1)`. Reservations are rounded per allocation, not after summing
logical payloads. The mixed-fragment timing corpus is not this controlled baseline.

Representative one-byte, empty-separator accounting, reproduced exactly:

| n | B / join payload copying | Handle relocation copying | Construction reserved | Concat payload copying | Concat reserved |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 3 | 3 | 8 | 160 | 5 | 32 |
| 17 | 17 | 120 | 496 | 152 | 336 |
| 129 | 129 | 1,016 | 2,496 | 8,384 | 9,856 |
| 1,025 | 1,025 | 8,184 | 17,824 | 525,824 | 537,600 |

Handle relocation is not string-payload copying. String allocation/descriptor
counters are subsets of physical requests, not additional reserved storage.
Registry accounting includes physically retained free capacities once each;
alignment, authority and records are explicit, not unexplained residual overhead.
Private nontrapping modular i64 counters are interpreted as unsigned BigInt;
these finite workloads remain below **67,108,864**, with no wrapped sample used.

**Transient versus retained:** at n=1,025, completed-call active capacity is
**8,192 bytes**, while the measured old-plus-new acquisition peak is **12,288**.
Free old buffers remain physically bump-retained; physical monotonic high water
is the completed construction's reserved total, not active residency or engine
RSS. Category peaks are not summed as if simultaneous. Separately retained second
joins have their own deltas. The lifetime case again records actual memory growth
**65,536 → 393,216 bytes** and two retained join results surviving a caught trap.
No page plateau, constant retained-memory bound, general liveness or reclamation
claim follows. M4 registry search work prevents interpreting this storage/copy
bound as a running-time proof.

## Ordinary timing: inherited sample boundaries

[#80's complete sampling contract](../m5-strings-2026-10-30/README.md#runtime-workload-and-sampling-contract)
is unchanged. Language numeric preset is **None**; native Cargo bench optimization
is a separate setting. Ordinary modules have no private observation exports or
counters. Timings cannot establish allocations, transient/retained storage, copy
volume or absence of intermediate strings; those conclusions use the separate
construction evidence above.

Runtime retains **123 cases**: 108 repeated-concat/accumulation/fixed-array-join
cases over 18 counts and two separators, plus 15 replacement cases. Each run uses
three forward/reverse/forward repetitions, five samples per case, eight calls per
sample, and two separate fresh-instance eight-call warmup batches per case per
repetition. Every measured batch has a fresh instance and preencoded inputs;
module/engine state is shared, not a guaranteed stable tier. The timer includes
the JS loop, crossings, handle stores and guest validation/allocation/copy/growth;
it excludes compiler/process/module compilation, instantiation, input production,
encoding, oracles, decoding and assertions. Eight results remain retained within
each batch; this is not a single-construction storage measurement.

Native retains **42 source/prepared-typed boundary/workload pairs** (seven families
at sizes 1/16/64), three repetitions, five samples and ten compilations per sample,
with two untimed warmup compilations per pair per repetition and reversed second
repetition. A new compiler and output destruction are timed; source/typed
preparation, corpus export, validation and reporting are not. These are not
backend-substage or engine-compilation timings.

For each fresh run, independent grouping of raw samples reproduced every reported
sample count, minimum, median and maximum in `summary.json`: **15 samples per case**,
**630 native / 1,845 runtime**, with unique boundary/case/repetition/sample identities.
All cases and raw distributions remain linked above; tables below use the same
illustrative n=8/n=256 and native size-64 slices as #80, not a favorable new subset.

### Fresh guest medians

Units: **microseconds per invocation**, retaining four decimal places from the
recorded batch-normalized medians. `empty`/`separated` refer to the separator.

| Workload | Timing 1 | Timing 2 | Timing 3 |
| --- | ---: | ---: | ---: |
| repeated, empty, n=8 | 0.3875 | 0.4375 | 0.3625 |
| accumulation/join, empty, n=8 | 1.2125 | 1.3750 | 1.0750 |
| existing-array join, empty, n=8 | 0.2750 | 0.3000 | 0.2375 |
| repeated, separated, n=8 | 0.5250 | 0.5625 | 0.4875 |
| accumulation/join, separated, n=8 | 1.7750 | 1.3125 | 1.0750 |
| existing-array join, separated, n=8 | 0.2625 | 0.3125 | 0.2625 |
| repeated, empty, n=256 | 49.9000 | 50.1875 | 49.5500 |
| accumulation/join, empty, n=256 | 23.7500 | 25.3125 | 21.3750 |
| existing-array join, empty, n=256 | 2.1625 | 2.4125 | 2.2500 |
| repeated, separated, n=256 | 208.2125 | 217.2000 | 186.5875 |
| accumulation/join, separated, n=256 | 21.2375 | 23.8000 | 20.3250 |
| existing-array join, separated, n=256 | 2.6375 | 3.8750 | 2.5000 |
| replacement no-match, source 1,023 bytes | 24.8125 | 28.9375 | 23.7000 |
| replacement overlap, source 1,023 bytes | 1.8625 | 2.1500 | 1.7875 |
| replacement expanding, source 1,023 bytes | 5.1500 | 5.9125 | 4.9375 |
| replacement shrinking, source 1,023 bytes | 2.8125 | 3.3000 | 2.7250 |

The raw replacement case suffix `_1024` names a workload parameter; its source is
1,023 bytes, not 1,024. The adversarial family includes repetitive 63/255/1023-byte
all-`a` sources, 33-byte almost-matching needles, overlap pressure, expansion and
shrinkage, plus nonoverlap/case-sensitive/UTF-8 sentinels. No-match remains visibly
costly; direct final allocation and constant auxiliary state do not imply linear
search time. No new size cap, algorithm change or linear-time claim is authorized.
Small accumulation remains slower than repeated concat in these n=8 rows; n=256
shows lower accumulation medians. Neither observation guarantees a speedup.

### Fresh native medians

Units: **microseconds per compilation**, size-64 families. Sizes 1 and 16 are
included in all-case ratios below, not omitted from interpretation.

| Boundary / family (64) | Timing 1 | Timing 2 | Timing 3 |
| --- | ---: | ---: | ---: |
| source / concat empty | 717.18 | 811.10 | 656.88 |
| source / concat separated | 615.15 | 838.48 | 722.42 |
| source / join empty | 416.39 | 420.71 | 341.84 |
| source / join separated | 384.61 | 356.06 | 325.41 |
| source / replace no-match | 384.51 | 466.50 | 374.42 |
| source / replace expand | 400.09 | 628.99 | 368.81 |
| source / replace shrink | 387.22 | 599.36 | 378.04 |
| typed / concat empty | 133.25 | 156.04 | 129.24 |
| typed / concat separated | 149.41 | 232.12 | 136.55 |
| typed / join empty | 115.60 | 137.41 | 117.22 |
| typed / join separated | 118.06 | 202.38 | 119.43 |
| typed / replace no-match | 85.62 | 174.07 | 88.19 |
| typed / replace expand | 85.61 | 178.38 | 87.59 |
| typed / replace shrink | 88.30 | 87.84 | 84.11 |

## All-case median ratios and material variability

Each cell is the **minimum–maximum of per-case median ratios**, across all 42
native or all 123 runtime cases, rounded to four decimal places. These are not
ratios of aggregate medians, pooled samples, confidence intervals or significance
tests. Numerator/denominator direction is explicit; no run is discarded.

| Fresh comparison | Native | Runtime |
| --- | ---: | ---: |
| Timing 2 / Timing 1 | 0.8583–2.0974× | 0.7394–2.3061× |
| Timing 3 / Timing 2 | 0.4771–1.0233× | 0.3953–1.3182× |
| Timing 3 / Timing 1 | 0.8130–1.1744× | 0.6056–1.4318× |
| Per-case maximum / minimum across fresh runs | 1.0113–2.0974× | 1.0129–2.5294× |

The complete fresh-versus-historical matrix uses historical #80 runs H1/H2/H3,
not just the most favorable historical denominator:

| Comparison | Native | Runtime |
| --- | ---: | ---: |
| Timing 1 / H1 | 0.6006–1.2859× | 0.4412–1.2000× |
| Timing 1 / H2 | 0.3702–0.9376× | 0.2429–1.0241× |
| Timing 1 / H3 | 0.5045–1.0687× | 0.3023–1.3125× |
| Timing 2 / H1 | 0.5155–2.1845× | 0.5882–2.2632× |
| Timing 2 / H2 | 0.3707–1.5586× | 0.3223–1.5288× |
| Timing 2 / H3 | 0.6064–1.8091× | 0.3188–2.0638× |
| Timing 3 / H1 | 0.4883–1.1778× | 0.5150–1.6500× |
| Timing 3 / H2 | 0.3685–0.8341× | 0.2143–1.0833× |
| Timing 3 / H3 | 0.5101–0.9594× | 0.3527–1.3404× |

Across all nine comparisons, native ratios span **0.3685–2.1845×**, runtime
**0.2143–2.2632×**. Within fresh runs the largest single-sample/case-median ratio
is **3.8011× native** (Timing 1, source compile/join separated size 1) and
**23.3256× runtime** (Timing 1, accumulation empty n=1). Historical #80's larger
outliers and median variability remain evidence; fresh repetitions do not erase
them or establish a stable performance budget.

This is a **material-variable performance signal requiring explicit disposition**.
With unchanged source, executable and module identities, these differences do
not establish a causal compiler regression or improvement. Scheduling, thermal,
power, JIT/GC and shared engine-tier effects remain possible influences, not
identified causes. Tiny-call batch measurements are especially sensitive to
crossing/timer noise. No arbitrary percentage gate, guaranteed speedup or release
approval is inferred. Successful measurement execution and deterministic accounting
must not be substituted for disposition of the timing signal.

## Recorded integrated gates, not rerun here

The manifest records all 17 sequential top-level commands with exit code zero,
including both construction and all three timing recorders. Each construction
recorder executes the unchanged **6 + 3 + 2** focused tests with ignored runtime
coverage enabled. Final evidence includes **five Python recorder tests**, **67
core string-slice tests**, **nine CLI string-slice tests**, default tests (**1,359
passed, 322 intentionally ignored**), strict native Clippy, portable Wasm core
build, three architecture tests, fresh browser rebuild, **25 browser smoke groups**,
full ignored workspace execution (**1,701 passed, zero failed/ignored**), strict
browser-target Clippy and whitespace checking. Exact argv, bounds, durations and
stdout/stderr remain in the manifest and [gate logs](gates/).

The recorders retain #79/#80's unchanged child bounds: Cargo 600 seconds,
construction/Node timing 120 seconds, simple timing-recorder metadata/CLI children
30 seconds. These are direct-child deadlines, not guest fuel or process-tree
cancellation. Default test success alone would not establish runtime coverage;
the explicit full-workspace and rebuilt-browser records supply that evidence.
These execution results support integrated correctness, not automatic integrated
M5 performance acceptance. **M5 remains pending explicit signal disposition;
M4's decision remains unchanged.**
