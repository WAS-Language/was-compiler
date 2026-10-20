# Issue #40: matched integer-local measurements — 2026-10-20

**Experimental evidence only; no release acceptance.** This collection demonstrates
smaller enabled modules versus the matched `None` baseline, with mixed guest timing
and substantial compilation-cost regression signals. It does not authorize changing
the default or accept material regressions. Correctness, diagnostics, adapter/browser
parity and broader release gates remain separate work.

## Measured identity and scope

- Baseline HEAD: `dc619db46e9dac3694adb7cc5855203a18233426`, confirmed in
  [git-head.txt](collection-1/git-head.txt). Measurements use the **dirty working
  tree atop that commit**, not the pristine commit. The pre-existing #40 compiler
  edits are part of the measured inputs.
- Collection interval: **2026-10-20 14:39:55–14:40:22 UTC**. See
  [environment](collection-1/environment.json), [command metadata](collection-1/)
  and [stability](collection-1/stability.json).
- Production and measurement hashes stayed identical before/after **every recorded
  command**, and before/after the collection: `commands_completed: true`,
  `inputs_unchanged: true`, `changed_inputs: []`. The manifests include compiler,
  CLI/browser source, runtime, Cargo manifests/lockfile, corpus, test/benchmark
  harnesses and both original recorders. They do not claim all unrelated docs/tests
  were frozen during concurrent review.
- The recorder preserved [initial status](collection-1/git-status-before.txt),
  [tracked binary diff](collection-1/git-diff-before.txt),
  [final status](collection-1/git-status-after.txt),
  [input hashes before](collection-1/hashes-before.json) and
  [after](collection-1/hashes-after.json). Untracked files appear in status; Git diff
  does not contain their contents. This task did not edit any source, tests, shared
  recorder or historical records, nor stage/commit anything. Cargo used the usual
  generated `target/` cache; evidence and inspection artifacts are only here.

This is a matched **current `None` versus current `Basic`** comparison. It measures
cumulative enabled literal/add-sub plus integer-local behavior, not an isolated
#40-versus-#39 timing experiment. Historical #39 timings are not matched controls.

## Unchanged recorder and explicit destination adaptation

The requested existing [#39 recorder](../integer-add-sub-39/record_matched.py) is
unchanged. It rejects destinations outside its own directory, so invoking its CLI
directly with the requested sibling directory cannot work. The local
[collect.py](collect.py) launcher imports that original module and redirects **only
its exact output-location assertion** to the fixed, new `integer-locals-40/collection-1`
destination. It retains repository-root and new-directory checks. No timing,
command sequence, deadline, hash, identity, correctness, parsing or summary logic
is replaced. [launcher.json](launcher.json) records the original assertion result,
allowed destination, recorder/launcher hashes, actual invocation and successful
completion. `environment.json` honestly records the launcher invocation.

**Known inherited narrative:** the untouched recorder writes a #39 heading and
“Locals remain f64” into `collection-1/summary.md`, `summary.json` and
`environment.json`. Those descriptive strings are stale for #40 and are **not a
claim about the measured tree**. They are preserved verbatim rather than editing
raw generated records. The numerical records, identities, command outputs and
this README's interpretation apply to #40. See the independent WAT inspection
below for actual storage. The original recorder SHA-256 is recorded in the
launcher and all collection measurement manifests.

To reproduce in a clean new copy of this evidence location, from repository root:

```sh
python docs/benchmarks/integer-locals-40/collect.py
python docs/benchmarks/integer-locals-40/inspect_wat.py
```

Both scripts intentionally refuse to overwrite their existing output directories;
do not delete historical evidence to rerun them. A subsequent collection needs a
separately reviewed new destination/launcher rather than reusing this one.

## Protocol, environment and validation actually run

The same unchanged 27-case corpus and existing timing seams were used:

- **108 independent-oracle checks**: all 27 cases at `None`, `Basic`, `Standard`
  and `Aggressive`. All passed; enabled presets have identical modules. All 27
  `None` source/module identities also match historical #38.
- **Two compiler repetitions per measured level**: each contains 27 source-to-Wasm
  and 27 typed-to-Wasm cases, Divan **100 samples × one compilation** per case.
  Complete CLI reports preserve min/max/median/mean, not individual compiler
  samples unavailable through this CLI. Corpus preparation/validation are outside
  timing; compiler setup, output destruction and applicable pipeline work remain
  inside their established timing boundaries.
- **Two guest repetitions per level**: 26 timed cases plus the verify-only
  `trap_order` control. Each timed case uses 128 warmup calls on a separate warm
  instance, then 20 samples × 64 calls with a fresh instance per sample and shared
  compiled module/JIT state. All raw per-call samples are preserved. Timing includes
  JS/Wasm calls and `effects` bookkeeping, not compilation/instantiation/assertions.
- Commands ran **sequentially**, with repetition 1 `None → Basic`, repetition 2
  `Basic → None`, for compiler and guest measurements. No concurrent benchmark or
  compiler build was launched by this task during collection.
- Recorder command deadlines remained **120 seconds**, with the existing Node
  subprocess bound of **60 seconds**. The outer collection command was bounded at
  1,200 seconds. These are direct-child deadlines, not guest fuel/process-tree
  cancellation. **No collection command failed or timed out.**
- Offline only: `CARGO_NET_OFFLINE=true`, all Cargo commands `--offline`;
  no installations, downloads or tools fetches.

Tooling: Windows x64, OS release `10.0.26300`; Intel Core Ultra 5 125H, 18 logical
CPUs, 16,730,726,400 bytes reported RAM; rustc/Cargo **1.98.0**, rustc host
`x86_64-pc-windows-msvc`, LLVM **22.1.8**; Python **3.13.14**; Node **24.12.0**,
V8 **13.6.233.17-node.37**, no Node flags. Exact versions, PATH/environment overrides,
UTCs and commands are recorded, not inferred from current tooling.

Validation executed:

- Original recorder self-tests: **3 passed**; shared baseline recorder self-tests:
  **4 passed**, with [logs](matched-self-test.txt) / [logs](baseline-self-test.txt)
  and companion command metadata. These include the existing failure/timeout
  preservation self-test; its simulated failures are not collection failures.
- `cargo test --offline -p was-compiler --test integer_baselines -- --nocapture --test-threads=1`:
  **1 passed, 2 ignored** as expected. The two ignored cases were then explicitly
  executed by the recorder (verification once, guest timing four times), not
  counted as covered by the default test alone.
- The ignored `execute_integer_workloads` verification and every compiler/guest
  measurement command succeeded. See their `.json` exit codes and complete `.txt`
  outputs under `collection-1/`.
- Post-timing CLI build and **32 CLI artifact commands** succeeded; 16 independently
  produced CLI binaries matched timed module hashes. Inspection input hashes still
  match the timed collection. This is not browser parity or a full test-suite run.
- Final evidence audit checks command exit/deadline metadata, artifact hashes,
  links/paths and whitespace. See [audit.json](audit.json).

## Results: size benefit, mixed timing and regression signals

Full results for all 27 workloads are in the unchanged generated
[table](collection-1/summary.md) and [JSON](collection-1/summary.json), subject to the
stale-narrative warning above. All pairs below keep repetitions **1 / 2** separate.
Compiler values are **µs per compilation**; guest values are **ns per main call**.

| Workload | Bytes None → Basic | Source None | Source Basic | Typed None | Typed Basic | Guest None | Guest Basic |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `literal_32` | 1156 → 542 | 234.4 / 111.1 | 103.6 / 145.2 | 35.19 / 25.64 | 65.59 / 46.14 | 92.1875 / 98.4375 | 36.71875 / 81.25 |
| `locals_32` | 1828 → 1214 | 494.6 / 375.9 | 445.7 / 472.8 | 95.34 / 48.89 | 151.1 / 127.5 | 84.375 / 70.3125 | 56.25 / 124.21875 |
| `compare_32` | 4164 → 2622 | 949.2 / 659.6 | 625.7 / 783.2 | 183.2 / 159.1 | 321.8 / 155.9 | 184.375 / 213.28125 | 154.6875 / 151.5625 |
| `negate_32` | 1668 → 1086 | 439.6 / 235.2 | 429.5 / 242.3 | 90.54 / 150.2 | 152.2 / 129.3 | 135.9375 / 64.0625 | 62.5 / 62.5 |
| `loop_local` | 1208 → 1178 | 46.99 / 44.69 | 61.54 / 41.09 | 45.39 / 16.29 | 35.99 / 31.99 | 253.125 / 154.6875 | 160.9375 / 468.75 |
| `overflow` | 80 → 66 | 12.59 / 6.799 | 12.39 / 6.199 | 4.899 / 3.299 | 5.799 / 6.299 | 67.1875 / 32.03125 | 67.96875 / 68.75 |
| `remainder` | 236 → 224 | 24.14 / 7.799 | 13.44 / 6.899 | 7.899 / 4.099 | 7.599 / 8.299 | 70.3125 / 46.875 | 84.375 / 70.3125 |

**Size:** 24/27 modules shrank, three were unchanged (`fraction`, `negate_min`,
`subnormal`), none grew. The synthetic sum is **13,414 → 9,701 bytes (−27.68%)**;
this is not weighted production traffic. `locals_32` shrank **33.59%** and
`compare_32` **37.03%** versus current `None`.

For byte context only, historical #39 Basic totaled 9,702 bytes. Current Basic
`locals_32` is **64 bytes smaller** (1278 → 1214), `compare_32` is **64 bytes
larger** (2558 → 2622), and `loop_local` is **one byte smaller** (1179 → 1178).
Thus the aggregate is only **one byte smaller than historical #39**. Other modules
may change hashes without changing size. This is exact historical byte comparison,
**not** a matched incremental timing attribution or a broad #40 size win. New
conversion boundaries can offset local-storage savings.

**Compiler trade-offs:** `literal_32` typed medians rose **86.39% / 79.95%**;
`locals_32` rose **58.49% / 160.79%**. Both-run typed median increases also occur
for `literal_1`, `locals_1`, `negate_1`, `overflow`, `underflow_i32`, `negate_min`,
`subnormal` and `trap_order` (the latter is verify-only for guest timing, not compiler
measurement). Other cases have mixed or improved compilation medians. Only
`trap_order` has source-compilation median increases in both runs; many source
results reverse direction. Do not hide these costs behind module-size savings.

**Guest observations:** medians decreased in both runs for `literal_1`, `locals_1`,
`literal_32`, `compare_32`, `negate_32`, `negative_zero` and `infinity`. They increased
in both runs for `overflow` (**+1.16% / +114.63%**) and `remainder`
(**+20.00% / +50.00%**). `locals_32` reversed from **−33.33%** to **+76.67%**;
`loop_local` reversed from **−36.42%** to **+203.03%**. These are regression signals
and conflicting observations, not excluded outliers or proof of intrinsic costs.

Power, thermals and background activity were not controlled; concurrent docs/test
review was allowed. Stable compiler hashes are not proof of an idle machine.
Run-to-run variation is large, short scalar cases include JS/Wasm overhead, V8
may optimize constants independently and finite warmup is not settled tiering.
**No stable general guest-speed conclusion follows from these two repetitions.**

## Independent storage and conversion inspection

After timing, [inspect_wat.py](inspect_wat.py) used the existing CLI and its
wasmprinter-backed WAT output, not internal representation facts, to inspect eight
exact corpus cases at `-O0` / `-O1`. Source hashes and separately emitted binary
hashes equal the corresponding timed records. [counts.json](inspection/counts.json)
contains hashes, local types and selected opcode counts; [counts.md](inspection/counts.md)
is the complete inspection table. Full `.was`, `.wasm`, `.wat`, per-command logs
and metadata are retained in [inspection/](inspection/). The CLI build and inspection
happened **after** collection and are not included in timing results.

Counts below cover **main only**, excluding its f64 parameter, helper bodies and
other callable bodies. Local counts include any scratch locals; opcode counts are
static occurrences, not dynamic executions or conversion cost estimates.

| Case | Locals None → Basic (i32 / f64) | `f64.convert_i32_s` None → Basic | Observation |
| --- | ---: | ---: | --- |
| `locals_32` | 0 / 97 → 96 / 1 | 0 → 33 | Ordinary a/b/c locals use i32; mutable total remains f64. |
| `compare_32` | 0 / 33 → 32 / 1 | 0 → 321 | Local storage specializes, but comparisons remain f64; 32 each `f64.gt`, `f64.eq`, `f64.le`. |
| `negate_32` | 0 / 65 → 32 / 33 | 0 → 65 | Initial a locals use i32; 32 `f64.neg` remain, as do f64 b locals. |
| `loop_local` | 2 / 4 → 4 / 2 | 0 → 4 | Loop-body locals specialize; three i64 scratch locals remain at both levels. |
| `mutable` | 0 / 1 → 0 / 1 | 0 → 2 | Reassigned local remains f64. |
| `capture` | 2 / 1 → 2 / 1 | 0 → 2 | Captured storage remains on its existing floating cell path. |
| `negate_zero` | 0 / 1 → 1 / 0 | 0 → 1 | Zero local is i32 but is widened before `f64.neg`, preserving −0. |
| `negate_min` | 0 / 1 → 0 / 1 | 0 → 0 | Floating local/negation path retained. |

There are no `i32.trunc_f64_s` or `i32.trunc_f64_u` occurrences in these inspected
main bodies. Selected counts do not claim absence of unrelated conversion kinds
(e.g. range-helper/i64 conversions). `locals_32` retains 32 f64 additions while
its 64 local-chain add/sub operations are i32. This is evidence of storage and
conversion boundaries, **not comparison/negation specialization or a performance
benefit by itself**.

## Raw evidence index and integrity

- [collection-1/](collection-1/): original recorder outputs, with each command's
  raw `.txt` plus `.json` command/start/end/deadline/exit metadata.
- `compiler-1-None`, `compiler-1-Basic`, `compiler-2-Basic`, `compiler-2-None`:
  all complete Divan tables and outside-timing sizes.
- `guest-1-None`, `guest-1-Basic`, `guest-2-Basic`, `guest-2-None`:
  all 20 guest samples/case, machine/runtime identity, source/module hashes and
  correctness/effect/trap results.
- [verify.txt](collection-1/verify.txt): all four presets' independent-oracle
  results, with module/source identities; [default-test.txt](collection-1/default-test.txt)
  preserves default native-contract coverage and ignored counts.
- `inputs-*-before.json` / `inputs-*-after.json`: full input manifests for each
  command; collection-wide `hashes-before.json`, `hashes-after.json`, `stability.json`.
- [environment.json](collection-1/environment.json), [rustc.txt](collection-1/rustc.txt),
  [cargo.txt](collection-1/cargo.txt), [node.txt](collection-1/node.txt): exact tools
  and process environment. Each guest report independently records CPU/OS/runtime.
- [launcher.json](launcher.json) and self-test logs/metadata: explicit adaptation
  and original recorder identity; [inspection/stability.json](inspection/stability.json)
  reconciles inspection inputs to the timed files.
- [audit.json](audit.json): final bounded command/identity/link/whitespace audit.
- [artifact-hashes.json](artifact-hashes.json): SHA-256 and byte lengths of every
  evidence file in this directory, including scripts and this README, excluding
  the manifest itself. Recompute before relying on an altered checkout; Git
  line-ending conversion can change raw artifact hashes.

**Decision remains pending.** A cumulative matched size benefit is demonstrated;
incremental #40 size improvement is negligible in this corpus, and guest timing
is inconclusive. Explicit acceptance of material regressions and separate release
validation have not been supplied by this task.
