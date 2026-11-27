# Official Linux M4/M5 performance baseline — 2026-11-27

**Decision:** Use this Linux baseline for M4 and M5 performance evaluation.
The earlier Windows measurements are preserved as historical evidence, but they
no longer serve as acceptance criteria or block either milestone. No repeat on
Windows is required. This replaces the reference; it does not explain the
historical variability or establish a cross-platform timing guarantee.

**Reference revision:** `e16293fa5aafa4717547de2c62586c3b9980cf7e`. The working tree
also held M9 adapter and replay edits; the compiler, workspace manifests/lockfile,
and M4/M5 workloads and harnesses used for timing matched the revision. No Binaryen
post-processing is used. The timed guest modules and the native M5 workload use
`None` (O0). The [A/A record](README.md) specifies the hardware, affinity, toolchain,
timers, boundaries, and limitations. [Input and executable hashes](environment.json.gz)
identify the measured runs; compare source/module hashes on rebuild, not host
executable hashes across different paths or build environments.

[Baseline CSV](baseline.csv) freezes **177 cases**: 12 M4 guest, 21 M5 source
compile, 21 M5 prepared typed compile, and 123 M5 guest. Per case, the center is
the median of six run medians (15 raw samples per run). The min and max are also
run medians; the paired A/A column is the largest within-session slower/faster
ratio. These are reference values, not fixed release thresholds. Regenerate from
the archived samples:

```sh
python -B docs/benchmarks/m4-m5-aa-2026-11-27/summarize.py docs/benchmarks/m4-m5-aa-2026-11-27/samples.json.gz --baseline
```

For a future candidate, rebuild the fixed revision and the candidate with the
same toolchain, prepare artifacts before timing, and interleave fresh baseline,
candidate, and A/A runs on the same machine across three sessions. Escalate a case
only if the median slowdown is at least 15%, exceeds measured paired A/A noise,
and meets both conditions in all three sessions. Do not compare a future single
run directly to the frozen CSV as a pass/fail test.

## Matched M5 native comparison against #80

A = `23441c9c27d9eae9d1a227ad12bba7ecab689217`, B = the reference revision above.
Both were built offline in the release bench profile using Rust/Cargo 1.100.0.
The historical checkout used its own locked dependencies; `cargo tree --locked
--offline -p was-compiler --prefix none` matched after the root checkout path
was excluded. The archive's only overlay was the current `m5_strings.rs` and
`support/m5_corpus.rs` benchmark sources, to pin `None` and use identical timed
code and workloads on both sides. No historical compiler source was changed.

The [comparison driver](compare_native.py) ran A1/B/A2, B/A2/A1, then A2/A1/B,
with a 30-second gap between sessions. Each run was a separate process on the
same logical CPU 0 with the same per-case warmups and 15 samples. Both executables
were stable, had different hashes, and all 42 exported corpus files matched byte
for byte across the nine runs. B is divided by the center of its session's two
A medians, and the noise check uses the slower/faster ratio of those same A runs.

| Boundary | Median B/A across cases, sessions 1 / 2 / 3 | Range across individual case/session ratios | Cases meeting triage in all sessions |
| --- | --- | --- | ---: |
| M5 source-to-Wasm | 1.003 / 0.989 / 1.001 | 0.924–1.086 | 0 / 21 |
| M5 prepared-typed-to-Wasm | 1.025 / 1.016 / 1.026 | 0.920–1.058 | 0 / 21 |

No individual case reached 1.15× in any session. This bounds the observed
revision-to-revision comparison on this setup; it does not prove unchanged
costs everywhere or explain the historical Windows variability. Re-running
historical versus current guest Wasm would not isolate a compiler revision effect
for these workloads: the M4 and M5 guest modules already match byte for byte.
The Linux comparison applies only to the measured workloads and toolchain. The
15% rule is for triage in matched runs, not a fixed timing budget or proof of no
regression.

See [per-case A/B medians](native-ab.csv) and [raw A/B samples, environment,
commands and hashes](native-ab.json.gz) (5,670 samples).

### Rebuild the comparison offline

From the repository root, with cached dependencies and a fresh output path:

```sh
mkdir -p target/m4-m5-baseline/ref-80
git --no-pager archive 23441c9c27d9eae9d1a227ad12bba7ecab689217 Cargo.toml Cargo.lock crates tests runtime examples grammar.bnf README.md | tar -xf - -C target/m4-m5-baseline/ref-80
cp crates/compiler/benches/m5_strings.rs target/m4-m5-baseline/ref-80/crates/compiler/benches/
cp crates/compiler/benches/support/m5_corpus.rs target/m4-m5-baseline/ref-80/crates/compiler/benches/support/
cargo bench --locked --offline --manifest-path target/m4-m5-baseline/ref-80/Cargo.toml -p was-compiler --bench m5_strings --no-run --message-format=json
cargo bench --locked --offline -p was-compiler --bench m5_strings --no-run --message-format=json
python -B docs/benchmarks/m4-m5-aa-2026-11-27/compare_native.py <historical-bench> <current-bench> target/m4-m5-baseline/native-ab 0
```

Use the `m5_strings` `compiler-artifact.executable` path reported by each build.
Run the second build and copy the overlay from the frozen reference tree. The
lockfile hashes are `af7880ab794e4a61cf78a04cc10b1478084132648d39a4cfb8495d0ec0294145`
(A) and `5476a13ee8b68dae7a0d710c1afb06ac63bf98fe3a78dba6289486842809ce52` (B).
Timing samples need not reproduce exactly.
