# M5 ordinary string runtime and native compilation costs (#80)

This is measurement infrastructure and bounded timing evidence, not a compiler
optimization, allocation proof, performance budget or integrated M5 release decision.
M4's [pending performance-signal disposition](../../m4-release-readiness.md) is
unchanged. No production implementation, ABI, allocator controls, numeric default,
string retention, fusion or reclamation changes belong to this task.

## Confirmed seams and correctness

The user approved ordinary source-to-Wasm/Node execution, source/prepared-typed
native compiler benchmarks, and recorder/provenance tests. Tests preceded each
new slice: the runtime corpus and harness initially failed as missing inputs;
the recorder initially failed to import; native corpus tests preceded its
implementation. These are harness/corpus contracts, not a new compiler feature.

- [`m5_measurements.rs`](../../../crates/compiler/tests/m5_measurements.rs)
  validates deterministic ordinary Wasm, source/typed parity, enabled-preset byte
  identity and the exact public exports. Its ignored Node test checks **123 cases**
  and an additional empty-needle trap, with no timing collection.
- [`m5_benchmarks.rs`](../../../crates/compiler/tests/m5_benchmarks.rs)
  validates 21 deterministic source-size workloads, increasing source/module sizes,
  source/typed/preset contracts, native raw-sample scheduling/format, exported corpus
  parity and overwrite refusal. Its ignored Node test executes **84** ordinary
  modules against independent expected contents (21 workloads × four presets).
- [`test_record_m5_strings.py`](../../../tests/support/test_record_m5_strings.py)
  rejects directory reuse, changed/added/removed inputs, zero/missing/duplicate/
  reordered native/runtime samples, and nonpositive durations.

Expected construction content comes from JavaScript array join, not guest concat
or join. Replacement uses JavaScript literal `replaceAll` plus worked literals:
`aaa / aa / b → ba`, case-sensitive `AaAa / a / xx → AxxAxx`, and multibyte text.
All outputs and retained inputs are decoded/checked after possible memory growth,
outside timers. Existing [operation contracts](../../string-operations.md) remain
authoritative for checked sizing, direct final copying, reuse and trap order.

## Runtime workload and sampling contract

[`m5_string_timing.was`](../../../tests/support/m5_string_timing.was) has constant
source size; fragment counts are runtime input, not source expansion. It exports:

| Entry | Included guest work |
| --- | --- |
| `repeated` | Iterate preexisting fixed string array; concatenate separator then fragment, retaining authorized empty-operand fast paths |
| `accumulate` | Construct empty resizable string array, push all preexisting fragment handles through geometric growth, then one final join |
| `join_existing` | Join the preexisting fixed array, separating final materialization from accumulation overhead |
| `replace` | Literal replacement with normal validation, sizing and direct final construction |

Counts are **0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 64, 128, 256**,
including both sides of string-handle array growth boundaries. Each occurrence
cycles through `é`, empty, `🦀`, `a`, with empty or `·` separator: **108 cases**.
The replacement family uses repetitive all-`a` sources of 63/255/1023 bytes:
33-byte almost-matching `aaaa…ab` needles (no match), 16-byte all-`a` needles
(overlap pressure), two-byte `aa` expanded to `aaéa` (inserted text is not rescanned),
and `aa` removed (shrinking), plus the three worked sentinels: **15 cases**.
These are external harness bounds, not guest size limits or a linear-search demand.

[`m5_string_timing.cjs`](../../../tests/support/m5_string_timing.cjs) compiles one
ordinary module in Node before any timers, rejects imports/observation exports,
and first runs all correctness cases. It then performs **3 repetitions × 5 samples
× 8 invocations per case**, **1,845 raw samples** total. Case order is forward,
reverse, forward. Each sample starts with a fresh ordinary instance and newly
encoded preexisting inputs. Two separate fresh-instance batches of eight calls
warm up each case before its five measured batches in each repetition. The module,
engine caches and tiering state are shared across all instances; the correctness
pass also warms the module. This does not establish a stable engine tier.

`process.hrtime.bigint()` surrounds only the eight-call JS loop. Included: JS loop,
spread calls/JS-Wasm crossings, output-handle stores, guest parameter/helper
validation, iteration, allocation, copying and any memory growth. Excluded: native
compiler/process startup, engine module compilation, instantiation, fragment
production, host encoding, oracle construction, decoding, assertions and reporting.
JS JIT/GC, Wasm tiering, host scheduling and thermal/power effects may influence
samples. Setup between samples can affect cache/GC state even when excluded.
Eight invocations retain eight results in that instance; this is not an amortized
single-construction storage metric. No marker allocations or callbacks run inside
timers. Engine compilation is not native compiler timing.

### Accounting boundaries — not inferred from timing

`n` counts fragment occurrences, including empties. `B` counts fragment bytes by
occurrence plus all output separator occurrences. Each case records both, plus
logical preexisting input UTF-8 bytes by occurrence; this is not a physical
allocation or retained-storage observation. Preexisting strings/arrays are encoded
outside timing and never charged as new guest fragment-production work. Production
is not measured here. Accumulation includes handle-slot growth/relocation; relocating
handles is not copying string payloads. Separately retained repeated joins are
separate constructions, not free reuse of one output.

**Transient peak storage, completed-call retained storage, payload-copy bytes and
allocation counts are unavailable in these ordinary modules.** Neither timings,
module size nor pages establish O(n + B) storage/copying or absence of intermediate
join strings. The separately owned [#79 deterministic evidence](../m5-fragment-construction-2026-10-30/README.md)
now records the bounded O(n + B) construction proof and independently decoded
absence of intermediate join strings. #80 does not substitute timings for that
proof; its mixed-fragment timing rows are not #79's controlled concat-copy baseline. We do not report expected cumulative copy volume for concat, so this
harness does not add/claim a controlled always-copying baseline. Authorized empty
reuse is preserved; no quadratic-copy requirement or guaranteed join speedup follows.

## Native compiler workload and sampling contract

The dedicated [`m5_strings` benchmark](../../../crates/compiler/benches/m5_strings.rs)
uses the ordinary public compiler and the default language numeric preset **None**.
Cargo's optimized **bench** profile is separate from that language setting.
[`m5_corpus.rs`](../../../crates/compiler/benches/support/m5_corpus.rs) grows literal
statements/calls with sizes **1, 16, 64** for seven families: concat/join with empty
or nonempty separators, replacement no-match, expanding and shrinking. These are
21 source-size workloads, distinct from the constant-source runtime corpus and
from the historical scalar/calls/captures benchmarks.

- `source_compile`: prepared source; new compiler and source-to-Wasm pipeline.
- `typed_compile`: prepared typed program; new compiler and checked compilation.

Both include Wasm-buffer destruction through `black_box_drop`; inputs are black
boxed. Corpus construction, preparation, parity/Validator checks, file exports,
formatting and reporting stay outside timers. No AST clone or observer is used.
The raw recording mode uses `std::time::Instant`: **3 repetitions × 5 samples ×
10 compilations × 42 boundary/workload pairs**, **630 raw samples**. Each pair has
two untimed warmup compilations per repetition. The full pair order is reversed
on repetition two. Native samples include the small loop/match/black-box overhead;
they are not private backend-stage timings. Divan's normal summaries remain useful
but expose no raw per-sample export in the pinned version; they are not mislabeled
as raw samples. The recorder's explicit raw mode supplies those samples instead.

## Reproduction and provenance

Run from the repository root, cached dependencies/tools only, with **no competing
builds or benchmarks**. Node 24+, Python 3 and cached Rust/Divan dependencies are
required. Choose a new directory for every recording; even an empty existing
directory is refused. Exact commands:

```sh
python -B -m unittest discover -s tests/support -p test_record_m5_strings.py
cargo test --offline -p was-compiler --test m5_measurements --test m5_benchmarks -- --include-ignored
cargo bench --offline -p was-compiler --bench m5_strings -- --test
cargo bench --offline -p was-compiler --bench m5_strings -- --sample-count 100 --sample-size 1 --timer os --color never
python tests/support/record_m5_strings.py target/m5-80-run-1
python tests/support/record_m5_strings.py target/m5-80-run-2
python tests/support/record_m5_strings.py target/m5-80-run-3
```

Use an external **600-second** bound per Cargo command/recorder invocation. Each
recorder Cargo child has 600 seconds, Node timing/correctness has 120 seconds,
and simple version/metadata/CLI children have 30 seconds. Rust correctness tests
also use the shared bounded process helper. Bounds are direct-child deadlines,
not guest fuel or process-tree cancellation. A timeout/failure leaves logs and
`complete: false`; it is not accepted as a partial timing baseline.

The [recorder](../../../tests/support/record_m5_strings.py) serializes correctness,
release executable builds, ordinary runtime compilation, nonzero 42-case Divan
smoke, raw native sampling and Node timing. It validates complete sample identities
and order. It records exact argv/bounds/durations, raw stdout/stderr, revision and
dirty status, tool/CPU/OS/Node/V8 versions, and relevant present `CARGO_*`, `RUST*`,
`NODE_*`, `DIVAN_*`, compiler/CI/processor environment overrides. Missing keys
were unset, not presumed default tooling configuration. Source hashes cover
tracked and untracked executable files under `crates`, `tests`, `runtime`, plus
Cargo manifest/lockfile. They are checked before and after recording. Native/CLI
executables are hashed and checked unchanged; source/module byte sizes and hashes
cover every exported ordinary corpus artifact. `inputs.zip` preserves exact
executable source bytes, including unrelated uncommitted work, without enrolling
those files in this task's commit. ZIP entry hashes must equal `sourceHashes`.

An initial `target/m5-timing-80-smoke` run established practical bounds and nonzero
case counts; it is not the repeated timing baseline. Its raw output remains local.
Final recordings use fresh directories and do not overwrite historical evidence.
Archived raw bytes are protected from Git newline normalization by local attributes.

## Recorded evidence and interpretation

Three complete sequential recordings are retained, not just the favorable one:

| Recording | Provenance | Native raw samples | Runtime raw samples | Summaries |
| --- | --- | --- | --- | --- |
| 1 | [environment](run-1/environment.json) | [630 samples](run-1/native.json) | [1,845 samples](run-1/runtime.json) | [all cases](run-1/summary.json) |
| 2 | [environment](run-2/environment.json) | [630 samples](run-2/native.json) | [1,845 samples](run-2/runtime.json) | [all cases](run-2/summary.json) |
| 3 | [environment](run-3/environment.json) | [630 samples](run-3/native.json) | [1,845 samples](run-3/runtime.json) | [all cases](run-3/summary.json) |

The recordings started at **23:05:58, 23:07:50 and 23:09:09 UTC on 2026-10-30**,
at revision `3e91f5c6b074f1f02e6c905a6d402c07b22c8697` with uncommitted #80
infrastructure and separately owned #79 test-only work. Dirty status is preserved
in each environment record. All **233 executable-input hashes**, both compiled
executable hashes, all 21 native source/module pairs and the ordinary runtime
source/module bytes match across all three recordings. Each recorder's before/
after hashes and archived ZIP entry hashes pass. Run 1's benchmark **preparation
build** waited for another build lock; no task build ran during its native/Node
timing phases. Runs 2 and 3 have no build-lock waits. Before run 3, a process-list
check found no competing Cargo/rustc/rustdoc or repository benchmark process.
There was no continuous system-load, power or thermal monitoring; ordinary editor/
OS background work was not disabled. These are repeatable input identities, not a
claim of controlled machine conditions.

Environment: **Windows x64 10.0.26300, Intel Core Ultra 5 125H (18 logical CPUs),
Node 24.12.0 / V8 13.6.233.17-node.37, rustc/Cargo 1.98.0, Python 3.13.14**.
Recorded relevant overrides were processor identity/count only; no `NODE_*`,
`RUST*`, `CARGO_*` or `DIVAN_*` override was present. Ordinary runtime source is
**878 bytes**, SHA-256 `cac918716bed5a258097c65c16446fb13084ed69e11aa10aa01919307389cb60`;
module is **4,258 bytes**, SHA-256
`a31dcab3a7cf82108b603591e61d3510780b463bd9ee39626e9ad0d8225576a1`.
Native sources span **94–4,866 bytes**, modules **918–4,289 bytes**; exact bytes,
sizes and hashes are archived under every `native-corpus/` and environment record.
Each run also preserves all raw command stdout/stderr, exact executable-source
snapshot `inputs.zip`, commands, bounds and success codes. None timed out.

### Guest results

Medians below are **microseconds per invocation**, computed from 15 raw eight-call
samples per case in each recording; they are not isolated instruction latency.
`empty`/`separated` name the separator, not a promise that every fragment is empty.

| Workload | Run 1 | Run 2 | Run 3 |
| --- | ---: | ---: | ---: |
| repeated, empty separator, n=8 | 0.400 | 0.913 | 0.525 |
| accumulation/join, empty separator, n=8 | 1.275 | 2.513 | 1.763 |
| existing-array join, empty separator, n=8 | 0.313 | 0.550 | 0.313 |
| repeated, nonempty separator, n=8 | 0.538 | 1.250 | 0.588 |
| accumulation/join, nonempty separator, n=8 | 2.088 | 2.650 | 1.600 |
| existing-array join, nonempty separator, n=8 | 0.350 | 0.638 | 0.363 |
| repeated, empty separator, n=256 | 52.475 | 57.725 | 53.888 |
| accumulation/join, empty separator, n=256 | 21.975 | 24.763 | 25.450 |
| existing-array join, empty separator, n=256 | 2.288 | 2.650 | 2.638 |
| repeated, nonempty separator, n=256 | 215.200 | 239.788 | 240.175 |
| accumulation/join, nonempty separator, n=256 | 24.250 | 23.563 | 30.275 |
| existing-array join, nonempty separator, n=256 | 2.750 | 3.413 | 2.938 |
| replacement no-match, source 1,023 bytes | 27.438 | 27.538 | 34.800 |
| replacement overlap pressure, source 1,023 bytes | 1.963 | 2.050 | 2.175 |
| replacement expanding, source 1,023 bytes | 5.988 | 5.750 | 5.938 |
| replacement shrinking, source 1,023 bytes | 3.100 | 3.150 | 4.163 |

Small accumulation includes a substantial setup/growth cost and is slower than
repeated concat in these n=8 rows. The n=256 construction rows show lower observed
medians for accumulation/join than repeated concat, without establishing a
universal speedup or using time as a copy-volume proof. Repetitive no-match
replacement is visibly costly within this bounded corpus; no search-algorithm or
size-cap change is authorized. Full empty/singleton/boundary cases, replacement
sizes, min/max and every raw sample remain in the linked records.

### Native results

Medians below are **microseconds per compilation** for the size-64 source family,
from 15 ten-compilation raw samples per boundary/case. Sizes 1 and 16 and complete
min/max/sample distributions remain in each native record.

| Boundary / family (64) | Run 1 | Run 2 | Run 3 |
| --- | ---: | ---: | ---: |
| source / concat empty | 557.72 | 1,020.15 | 942.28 |
| source / concat separated | 615.05 | 1,133.02 | 944.88 |
| source / join empty | 330.68 | 671.21 | 434.64 |
| source / join separated | 381.76 | 645.17 | 437.99 |
| source / replace no-match | 393.64 | 662.97 | 480.86 |
| source / replace expand | 358.74 | 657.66 | 420.76 |
| source / replace shrink | 359.35 | 526.69 | 397.40 |
| typed / concat empty | 131.53 | 200.97 | 167.38 |
| typed / concat separated | 132.81 | 240.67 | 160.10 |
| typed / join empty | 126.47 | 158.42 | 170.60 |
| typed / join separated | 116.47 | 198.01 | 184.13 |
| typed / replace no-match | 85.21 | 119.34 | 109.04 |
| typed / replace expand | 84.95 | 115.50 | 126.38 |
| typed / replace shrink | 84.59 | 113.00 | 114.14 |

### Material variability — escalation, not a release decision

With identical executable/source/module hashes, **run-2/run-1 case medians span
0.64–2.92× native and 0.57–3.68× runtime**. The additional run does not eliminate
the signal: **run-3/run-2 spans 0.43–1.42× native and 0.27–3.07× runtime**.
Across all three, each native case's maximum/minimum median ratio is at least
1.30× and reaches 2.92×. Within-run single-sample outliers reach **6.34× the case
median natively** and **281.98× at runtime** (run 3, repeated separated n=16).
The tiny-call batches are particularly sensitive to timer/crossing/scheduling
noise. These ranges are descriptive, not statistical-significance claims.

This is **material, unexplained measurement variability**, not evidence of a
changed-code compiler regression: there is no changed implementation experiment
between these recordings. Neither the slow nor fast run is discarded. No power,
thermal, scheduling or tier-state evidence identifies a cause. No arbitrary
performance threshold, stable performance budget, optimization/default change,
linear-time replacement claim or M4/M5 release acceptance follows. #81 must account
for this signal alongside #79's deterministic proof and integrated correctness
rather than treating successful measurement execution as performance approval.

## Validation and two-axis review

Final bounded validation ran sequentially after the three recordings, without
concurrent task measurements. [Exact commands/durations](validation/commands.json)
and complete stdout/stderr are archived under `validation/`.

| Command | Actual result |
| --- | --- |
| Python recorder unittest command above | **5 passed**, including first-failing target-triple/stale-CLI regression |
| Focused `m5_measurements` + `m5_benchmarks -- --include-ignored` | **7 passed**, 0 failed/ignored; rerun in every recording |
| `cargo check --offline --all-targets` | Passed during implementation |
| `cargo test --offline` | **1,359 passed, 0 failed, 322 intentionally ignored**, 80 result targets; default workspace members |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed |
| `cargo bench --offline -p was-compiler --bench compiler -- --test` | **45 existing cases** smoke-passed; not M5 compiler timing evidence |
| M5 Divan finite 100-sample command above | **42 cases** completed; summaries retained separately, not labeled raw samples |

Default Cargo success does not execute ignored runtime/browser cases. New ignored
Node cases were explicitly executed; current-source browser/full ignored workspace
gates remain #81's integrated responsibility. No production core/adapter changes
or golden regeneration were made by #80. The full default suite includes concurrent
#79 test-only work; counts are actual execution, not tests attributed solely to #80.
Only edited Rust files were individually formatted. Whitespace/link/archive checks
are completed before commit.

Independent Standards and Spec reviews used the task-start revision as the WIP
baseline and excluded separately owned #79/user changes. Standards identified a
stale-CLI risk under configured Cargo target triples. A red/green recorder test
now covers it; both CLI and benchmark paths come from Cargo's reported executable
artifacts, never a guessed output path. Re-review found no remaining standards
findings; Spec found no scope/contract findings. Final evidence received a separate
read-only reconciliation. Review does not dispose of the material timing signal.
