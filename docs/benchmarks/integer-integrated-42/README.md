# Issue #42: integrated integer-specialization evidence

**Collection completed 2026-10-20. Release/regression acceptance PENDING.**
The cumulative #39/#40/#41 pass remains experimental; `None` remains the default.
Smaller modules are demonstrated. These observations do **not** justify a general
speed claim, an isolated incremental timing claim for any pass, or regression
acceptance. Separate final correctness gates passed; see the
[correctness and decision record](../../integer-specialization.md#42-integrated-validation-and-release-readiness-2026-10-20).
Correctness success is not release acceptance.

## Measured identity and isolation

Measured base HEAD is **`0e1b5225a6d63c18244ab8c06fc460dd2d7a0020`**, plus the
recorded dirty **test-only** changes, not a clean-commit measurement. See
[HEAD](collection-1/git-head.txt), [pre-run status](collection-1/git-status-before.txt),
[tracked diff](collection-1/git-diff-before.txt), and
[post-run status](collection-1/git-status-after.txt). The tracked diff changes only:

- `crates/browser/tests/integer_specialization_adapters.rs`
- `crates/cli/tests/cli.rs`
- `crates/compiler/tests/integer_locals.rs`

Status also records the new `tests/support/integer_composition_cases.rs` and
untracked #42 evidence/launcher files. Git diff does not capture untracked file
contents. These are added test scenarios, **no production changes**. They do not
extend the timed historical corpus. Do not identify the measured tree with a later
packaging commit or infer validation of those new tests from this collection.

[Before hashes](collection-1/hashes-before.json) and
[after hashes](collection-1/hashes-after.json) are identical.
[Stability](collection-1/stability.json) records completed commands, unchanged
monitored inputs and no changed-input entries; per-command `inputs-*-before.json`
and `inputs-*-after.json` snapshots are retained. These are the recorder's
production/measurement-input checks, not a complete content snapshot of every
untracked file. [results.json](results.json) includes those hashes, complete
recorded command metadata and SHA-256 hashes of the raw evidence inputs.

The [launcher](launcher.json) records **no agent-launched concurrent builds, tests
or benchmarks during collection**. OS/background activity, power, thermals and
settled V8 JIT tiering remain uncontrolled. This improves the collection's
agent-work isolation but does not establish a noise-free causal experiment.
Historical #41 background-load caveats are historical, not evidence of concurrent
agent work in #42.

`collect.py` reuses the historical
[`integer-add-sub-39/record_matched.py`](../integer-add-sub-39/record_matched.py)
unchanged (SHA-256
`d20dbc781d22247b7a5f57890880db834e3649cf013d9e402acc57b4d63c5dd6`),
redirecting **only its destination assertion**. Timing, corpus, parsing and oracle
checks are unchanged. Its raw `summary.md`, `summary.json` and `environment.json`
still contain #39's stale “locals remain f64” narrative. **This README supersedes
that narrative, not the numerical records**: the measured enabled pass includes
#39 exact literals/add-sub, #40 eligible ordinary-local storage/read proofs, and
#41 proven comparisons/safe negation. Language `number` and public ABI remain f64;
only proven private representations specialize.

The local [Git attributes](.gitattributes) preserve exact evidence bytes across
Windows checkouts; the verbatim Git diff snapshot is displayed as binary so its
intentional context-line whitespace is not treated as edited-source whitespace.
This packaging rule changes neither raw evidence nor recorded hashes. The final
staged-byte reconciliation checks the archive against the audited working files.

## Environment and timing boundaries

[Environment](collection-1/environment.json), [runtime records](collection-1/summary.json)
and [toolchain](collection-1/rustc.txt):

- Windows `10.0.26300`, `win32/x64`; Intel Core Ultra 5 125H, 18 logical CPUs;
  16,730,726,400 bytes total memory.
- Node `v24.12.0`, V8 `13.6.233.17-node.37`, no Node flags.
- rustc `1.98.0` (`88d9e12ae`, 2026-08-18), host `x86_64-pc-windows-msvc`,
  LLVM `22.1.8`; Cargo `1.98.0` (`797e8a9bc`, 2026-08-05).
- Python `3.13.14`, MSC v.1944 AMD64; `CARGO_NET_OFFLINE=true`.
- Launcher interval: `2026-10-20T18:37:18.338601+00:00` through
  `2026-10-20T18:37:30.440199+00:00`.

Two sequential repetitions, reversing preset order:

1. compiler None → Basic, then guest None → Basic;
2. compiler Basic → None, then guest Basic → None.

Verification precedes timing. Each level has two compiler runs of 54 measurements
(27 source-to-Wasm and 27 typed-program-to-Wasm), and two guest runs (26 timed
cases and one verify-only `trap_order`). `Basic` represents the enabled presets;
verification checks `Standard` and `Aggressive` identities, but does not time them.

Compiler logs retain **Divan aggregates**, not raw per-compilation samples:
100 samples, sample size 1, OS timer, min/max/median/mean. Corpus generation,
parsing prerequisites for typed input, and validation are outside the appropriate
timing boundaries; fresh compiler/host setup and output destruction are inside.
Source compilation includes frontend work; typed compilation starts with an
already analyzed program. Parsed aggregates are retained in results.json; raw
Divan text remains authoritative for its displayed precision and spread.

Guest logs retain **20 raw ns/main-call samples**, each batching 64 calls, after
128 warmup calls on a separate warm instance. Each measured sample uses a fresh
instance with shared compiled module/JIT state. Timing excludes source generation,
compiler work, Node startup, Wasm module compilation, instantiation, warmup,
assertions and hashing. It includes JS/Wasm calls, batch-loop overhead and JS host
mark bookkeeping for `effects`. Finite warmup is not proof of settled JIT state.
Raw guest samples are not raw compiler samples or interchangeable with Divan
aggregates. See the [timing contract](../../benchmarks.md#separate-harness-and-timing-boundaries).

## Complete results and tradeoffs

[results.md](results.md) contains **all 27 cases**, exact byte counts, both
repetitions' compiler/guest medians, absolute and percentage deltas and direction
classifications; [results.json](results.json) preserves full parsed precision,
aggregates, identities and provenance. Delta is Basic − None; percentage is
`100 * (Basic / None - 1)`. Positive time means slower. “Higher/lower both” means
strictly positive/negative in both repeats; reversals or equality are mixed/equal.
These are descriptive signs, not statistical significance tests.

**Demonstrated size benefit:** independent-module unweighted sum
**13,414 → 9,538 bytes (−3,876; −28.90%)**. 24/27 shrink; `fraction`, `negate_min`
and `subnormal` are unchanged; none grow. This is a synthetic corpus sum, not
weighted production traffic or a combined application.

**Compilation regressions are material:** source medians rise in both repetitions
for **23/27** cases (four mixed/equal); typed medians rise for **25/27** (two
mixed/equal). Neither boundary has a case that falls in both. Examples:

| Case | Source change, repeats 1 / 2 | Typed change, repeats 1 / 2 |
| --- | ---: | ---: |
| `literal_32` | +35.58% / +34.51% | +68.07% / +118.33% |
| `locals_32` | +8.35% / +42.26% | +69.64% / +63.68% |
| `compare_32` | +7.93% / −25.27% | +142.26% / +140.27% |
| `negate_32` | +54.71% / +62.69% | +124.02% / +69.21% |

**Guest regression signals:** **11/26** timed cases worsen in both repetitions,
**4/26** improve in both (`locals_1`, `literal_32`, `compare_32`, `negate_32`),
**11/26** are mixed/equal. No guest time is invented for `trap_order`.
`loop_local` worsens **+17.57% / +9.11%**; `negative_zero`
**+11.36% / +9.30%**; `negate_zero` **+16.28% / +9.52%**;
`positive_zero` **+15.12% / +6.10%**. Even `locals_32` worsens
**+5.19% / +1.92%** despite its smaller module. In contrast, `compare_32` improves
**−50.41% / −48.88%**, `literal_32` **−14.75% / −11.86%**, and `negate_32`
**−12.35% / −5.06%**. Keep every case and both repeats; do not select apparent
wins or discard regressions as outliers. Two reversed-order repetitions with
uncontrolled background/thermal/JIT effects do not establish a general speedup
or intrinsic-regression estimate. Acceptance remains pending.

**Conversion tradeoffs (historical static evidence, not new #42 measurements):**
all current corpus module hashes match #41, so the independently emitted
[#41 main-only decoded counts](../integer-comparisons-negation-41/inspection/counts.md)
refer to identical binaries. `locals_32` changes 0 i32 / 97 f64 locals to
96 i32 / 1 f64, but adds **33 `f64.convert_i32_s`** widenings. `compare_32`
has 32 i32 / 1 f64 locals and **129 widenings** (the
[#41 account](../integer-comparisons-negation-41/README.md#expression-only-and-independent-inspection-evidence)
records **321** under historical #40). `negate_32` has 64 i32 / 1 f64 locals,
**33 widenings**, and 0 rather than 32 `f64.neg`. `loop_local` adds four
widenings; excluded `mutable`/`capture` still have two each. Safe fallback
`negate_zero` retains one widening and one `f64.neg`; `negate_min` retains two
`f64.neg`. Integer negation uses `i32.const 0; operand; i32.sub`, so more integer
instructions are not automatically smaller or faster. Historical #41 versus #40
sizes even grew for `negate_32` (+32 bytes), `negate_1` (+1) and `remainder` (+2),
while net corpus size fell 163 bytes. These are static historical counts/lengths,
not dynamic conversion frequencies, pass-cost attribution or new inspection runs.

## Matched identities, not historical timing controls

The report checks all identity fields (`sourceBytes`, `moduleBytes`,
`sourceSha256`, `moduleSha256`) for every case against
[#38](../integer-specialization/summary.json) and
[#41](../integer-comparisons-negation-41/collection-1/summary.json):

- Current None equals #38 and #41 None: **27/27 exact identities** each.
- Current Basic equals #41 Basic: **27/27 exact identities**.
- Current None/Basic source identities match: **27/27**.
- Size change versus #41 is **zero** at both levels, not a new incremental win.

Full SHA-256 values are tabulated in results.md, with historical/current fields
in results.json. Current None versus current Basic measures the **cumulative
#39/#40/#41 pass**. Historical #38/#39/#40/#41 timing is not a matched incremental
control; do not subtract historical medians to attribute speed to an individual
change. Historical #41's different timing directions are a caution about
variability, not grounds to average away #42 regression signals.

## Commands, evidence and limits of validation

Run derived reporting from the repository root using the installed executable:

```sh
'C:/Program Files/WindowsApps/PythonSoftwareFoundation.Python.3.13_3.13.3824.0_x64__qbz5n2kfra8p0/python3.13.exe' docs/benchmarks/integer-integrated-42/report.py
```

This standard-library-only command reads immutable evidence and overwrites only
`results.md` and `results.json`. It asserts case coverage, identity and recorded
input stability; it runs no compiler, benchmark or test. It does not mutate raw
records. The script has no timestamps in derived output, so reruns over identical
inputs produce identical files.

**Historical collection commands below are documentation, not instructions to
rerun into the existing evidence directory.** `collect.py` rejects an existing
destination; never delete/overwrite raw evidence to recollect. The invocation was
the same installed Python executable followed by
`docs/benchmarks/integer-integrated-42/collect.py`. It called the recorder and
`tests/support/record_integer_baseline.py` with `--self-test` first. Component
commands recorded under `collection-1/*.json` were:

```sh
cargo test --offline -p was-compiler --test integer_baselines -- --nocapture --test-threads=1
cargo test --offline -p was-compiler --test integer_baselines execute_integer_workloads -- --ignored --exact --nocapture --test-threads=1
cargo bench --offline -p was-compiler --bench integer_specialization -- --sample-count 100 --sample-size 1 --timer os --color never
cargo test --offline -p was-compiler --test integer_baselines record_integer_guest_timing -- --ignored --exact --nocapture --test-threads=1
```

`INTEGER_BASELINE_LEVEL=None` or `Basic` is set per timing command;
`CARGO_NET_OFFLINE=true` throughout. Each recorded command has its argv, UTC
start/end, 120-second deadline, exit code and output path. Guest subprocess
bounds are 60 seconds, not guest fuel or process-tree cancellation. Tool identity
commands are `rustc -Vv`, `cargo --version`, `node --version`; Git snapshots and hashes
are retained alongside them. All exact recorded argv are in results.json.

Evidence inventory:

- [Raw summary JSON](collection-1/summary.json) / [Markdown](collection-1/summary.md).
- [Oracle log](collection-1/verify.txt): **108 workload/preset pairs** (27 × 4),
  independent results/effects/traps; enabled-preset identity and historical None
  identity are recorded. This is collection-oracle evidence, not a release gate.
- [Matched recorder self-tests](matched-self-test.txt): **3 passed**;
  [baseline recorder self-tests](baseline-self-test.txt): **4 passed**.
- Compiler aggregates: [1 None](collection-1/compiler-1-None.txt),
  [1 Basic](collection-1/compiler-1-Basic.txt),
  [2 Basic](collection-1/compiler-2-Basic.txt),
  [2 None](collection-1/compiler-2-None.txt).
- Guest raw samples: [1 None](collection-1/guest-1-None.txt),
  [1 Basic](collection-1/guest-1-Basic.txt),
  [2 Basic](collection-1/guest-2-Basic.txt),
  [2 None](collection-1/guest-2-None.txt).
- [Launcher](launcher.json), [environment](collection-1/environment.json),
  [stability](collection-1/stability.json), before/after hashes and input snapshots.

No Cargo/tests/benchmarks were launched to derive the numerical report. Separate
correctness validation subsequently passed all nine commands in
[validate.py](validate.py), with [argv, deadlines and exit metadata](validation-1/commands.json)
and complete raw logs under `validation-1/`. This included a current-source browser
rebuild **before** browser smoke and full ignored workspace tests: **1,296 passed
across 81 targets, zero failed/ignored**, plus 25 browser smoke groups. Native
default tests passed 1,088 with 195 deliberately ignored; focused numeric suites
passed 31 with none ignored. Both native and wasm32 browser Clippy passed, as did
the portable core build. [Final audit](audit.json) reconciles production and
historical harness hashes with measured inputs and records final test-source hashes.

The collection's default-test command alone does not establish these broader gates.
Validation used the same installed Python executable followed by
`docs/benchmarks/integer-integrated-42/validate.py`; each gate has a 600-second bound,
and the runner refuses to overwrite `validation-1/`. No prerequisite installation
or fetch was needed. Explicit regression/release acceptance remains **PENDING**;
correctness completion does not change that decision.
