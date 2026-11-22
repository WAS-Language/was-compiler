# M7 integer specialization: current-source evidence (#88)

**Measured 2026-11-22; M7 remains experimental.** Current-source `Basic` reproduces
#42's size benefit on the M7-focused set, costs more to compile at both measured
boundaries, and has mixed guest timings. This record changes no production code,
option, ABI, default, or release decision. It does not resolve M4, M5, or M6.

## Results against #42

[Per-workload medians](results.md) retain both repetitions and all 27 historical
workloads. Only `loop_local` contains a `for` with a direct range iterable in the
[unchanged corpus](../../../tests/support/integer_corpus.rs). The other 26 are
**M7-focused**, not an isolated measurement of the integer proof pass: `Basic`
still selects M6 even when a workload has no eligible direct loop. `trap_order`
contributes compiler timings and size but is verification-only for guest execution.

- **Size, M7-focused:** 12,206 → 8,360 bytes (−3,846; −31.51%), the same as [#42](../integer-integrated-42/README.md).
  Of the 26 modules, 23 shrink, three are unchanged, and none grow. This is an
  unweighted sum of independent Wasm modules, not the size of a combined app.
  Compared with [#42's identities](../integer-integrated-42/results.json), all 27
  source byte counts and SHA-256 values match at both presets. All 27 `None`
  module identities and 26 `Basic` module identities match; the only `Basic`
  difference is `loop_local`.
- **Compilation cost, M7-focused:** `Basic` increases the median in both repeats
  for all 26 cases at both boundaries. Across individual repeats, source changes
  run from +0.76% to +23.20%, and typed from +1.13% to +69.60%. For example,
  `literal_32` rises +23.20% / +22.09% at the source boundary and +69.60% / +68.29%
  at the typed boundary. In #42, excluding `loop_local`, the count of cases slower
  in both repeats was 22/26 source and 24/26 typed. Repeated higher cost is not a
  measurement of the cost of any one pass.
- **Guest timing, M7-focused:** 11 of 25 timed cases have higher medians in both
  repeats, five have lower medians in both, and nine reverse direction. In #42's
  same subset the counts were 10 higher, four lower, and 11 mixed/equal. Here
  `compare_32` (−58.99% / −59.44%) and `effects` (−31.62% / −32.79%) improve in
  both; `locals_32` rises +11.41% / +1.14% and `positive_zero` +9.57% / +7.98%.
  Repeated signs alone do not establish statistical significance or causation.
- **`loop_local`, M6/M7 composition:** 1,208 → 1,267 bytes (+59); #42's pair was
  1,208 → 1,178, and the current `Basic` module is 89 bytes larger than that
  historical `Basic` module. Current `Basic` source and typed medians are higher
  in both repeats (+10.99% / +9.36% and +10.95% / +14.19%), while guest medians are
  lower (−12.73% / −14.41%). #42's guest medians were higher (+17.57% / +9.11%).
  Do not attribute the runtime difference to either pass individually.

Variability is visible even on **byte-identical** modules: `fraction` shows guest
median increases of +55.68% / +57.89%, but its minima are near 28 ns/call at both
presets and the sample ranges overlap (repeat 1: 28.03–59.02 `None`, 28.02–101.75
`Basic`). `locals_1` reverses from +30.06% to −18.42% when preset order reverses.
These are reasons not to treat a measured difference as an intrinsic effect of
specialization. No samples or cases from the completed collection were discarded,
and no acceptance threshold was selected. #42 ran on a different machine, OS,
Rust toolchain and Node/V8 version; it establishes workload and module continuity,
not a timing control for isolating a historical change.

## Measurement protocol and provenance

- Revision [`750046c5ef4999e1b85dd25332667186a400fd78`](run/git-head.txt), with an
  unrelated, pre-existing roadmap edit and untracked #88 evidence
  ([status](run/git-status.txt)). No measured production source or harness change.
  [Input SHA-256 hashes](run/inputs.json) include source, workspace manifests,
  lockfile, harnesses, collector and both executables; before/after checks
  [matched](run/stability.json). The [premeasurement Cargo artifact records](run/cargo-artifacts.jsonl)
  identify the same executable paths and both successful builds (`fresh: false`).
- Linux x86-64 `7.2.15-1-omarchy`, AMD Ryzen 7 5700X (16 logical CPUs),
  Node `26.13.1` / V8 `14.6.202.34-node.40` ([oracle log](run/verify.txt)),
  [rustc](run/rustc.txt) and [Cargo](run/cargo.txt) `1.100.0`,
  [Python](run/environment.json) `3.15.2`. The collector and its children ran
  with affinity to logical CPU 6. No power/thermal, background-load or settled
  JIT tier controls. No agent-launched builds, other tests or benchmarks
  overlapped timing.
- Oracle first: all **108** workload/preset pairs run against worked expectations
  (including `Object.is` for signed zero and NaN, expected effects, and the bounds
  trap before RHS effects). Source identity is equal across presets; `Basic`,
  `Standard` and `Aggressive` have identical modules. Guest timing logs repeat
  result/effects checks and match the oracle's module and source identities;
  Divan preflight sizes match the oracle too.
- Repetition 1: compiler `None` then `Basic`, guest `None` then `Basic`.
  Repetition 2: compiler `Basic` then `None`, guest `Basic` then `None`.
  Divan retains min/max/median/mean (100 OS-timer samples of one compilation
  per case and boundary, not raw samples). Both boundaries include fresh
  compiler construction, host declaration and output destruction; source also
  includes frontend work, while typed reuses a pre-analyzed program. Corpus
  generation and Wasm validation are outside the timer.
- Guest timing retains 20 raw ns/main-call samples per timed case, each a batch
  of 64 calls on a fresh instance, after 128 warmup calls on a separate instance
  of the same compiled module. The timer includes the JS batch loop, JS/Wasm
  calls and `effects` host bookkeeping, not module compilation, instantiation,
  warmup, assertions, or startup. This is the historical protocol, not a
  steady-state engine guarantee.

Raw logs in [`run/`](run/) retain [module/source identities](run/verify.txt),
complete Divan tables ([1 None](run/compiler-1-None.txt),
[1 Basic](run/compiler-1-Basic.txt), [2 Basic](run/compiler-2-Basic.txt),
[2 None](run/compiler-2-None.txt)) and guest samples ([1 None](run/guest-1-None.txt),
[1 Basic](run/guest-1-Basic.txt), [2 Basic](run/guest-2-Basic.txt),
[2 None](run/guest-2-None.txt)). Each command has an adjacent JSON record with
its argv, UTC interval, 120-second deadline, and exit status.

## Reproduce offline

From the repository root, with cached dependencies and Node 24+, prebuild both
executables before timing:

```sh
cargo test --offline -p was-compiler --test integer_baselines --no-run --message-format=json
cargo bench --offline -p was-compiler --bench integer_specialization --no-run --message-format=json
```

Use the `executable` fields for the respective targets as `TEST` and `BENCH`.
Choose an available logical CPU in place of 6. The collector requires a **new**
output directory, reuses the existing parsers, and runs no Cargo commands that
build or test. The earlier direct invocation without Divan's `--bench` only
listed compiler cases; the entire sequence was rerun with `--bench`, and nothing
from that initial attempt is included in `run/`.

```sh
taskset -c 6 python3 docs/benchmarks/integer-current-88/record.py collect docs/benchmarks/integer-current-88/new-run "$TEST" "$BENCH"
python3 docs/benchmarks/integer-current-88/record.py report docs/benchmarks/integer-current-88/new-run
```

The `report` command only reads logs. Pass `docs/benchmarks/integer-current-88/run`
instead of `new-run` to regenerate the checked-in table without timing again.

## Validation scope

`cargo check --offline --workspace`, the two historical parser self-tests
(7 tests), `python3 docs/benchmarks/integer-current-88/test_record.py`
(3 report CLI tests), `cargo test --offline -p was-compiler --test integer_baselines --
--include-ignored --test-threads=1` (3 tests), and the default
`cargo test --offline --workspace` (1,369 passed, 345 ignored) passed. The offline
browser rebuild compiled the Rust Wasm but could not find the matching local
wasm-bindgen `0.2.129` CLI. Fresh-browser parity and the full ignored workspace
suite were not run. This evidence is not integrated M7 release validation.
