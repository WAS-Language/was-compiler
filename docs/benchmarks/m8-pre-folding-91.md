# M8 pre-folding baseline (#91)

**Measured 2026-11-22, after explicit authorization for #91.** No production
compiler source, output, defaults, presets, or public options changed. This
record does not authorize M8 implementation or establish release acceptance.
The [confirmed M8 design](../was_compiler_memory_and_optimization_roadmap.md#m8-interview-decisions-round-1-confirmed)
and [ADR 0012](../adr/0012-fold-scalars-during-checked-lowering.md) govern what can
fold. Use the frozen corpus and sampling settings for a matched final-source run.

## Corpus and independent expectations

Generate one ordinary module per row with only `main` selected as a user export
and no host registrations. Except for the range control, use `fn main(x: number)`
with explicit result types. Pass `1.5` for the numeric pairs and `3` for the bool
pair. A shared input signature keeps the JS/Wasm call boundary comparable; the
closed cases deliberately ignore the argument. Use these fixed bodies:

| Workload | WAS body / generator | Expected | Role |
| --- | --- | --- | --- |
| `fraction_closed` | `return (1.5 + 2.25) * 6.5;` | `24.375` | Closed binary64 arithmetic without an M7 exact-i32 arithmetic subtree |
| `fraction_open` | `return (x + 2.25) * 6.5;` | `24.375` | Same operators and result; the binding read keeps every operator open |
| `wide_closed` | Balanced `+` tree of 16 copies of `(1.5 + 2.25) * 6.5` | `390` | Larger closed expression; no guest loop or local accumulator |
| `wide_open` | Same tree with `(x + 2.25) * 6.5` leaves | `390` | Matched source scaling, with every operator dependent on `x` |
| `bool_closed` | `return !((3 < 2) == (4 == 4));` | `true` | Closed comparisons, bool equality and negation; integer comparisons may already specialize under M7 |
| `bool_open` | `return !((x < 2) == (x != 4));` | `true` | Comparable no-fold bool expression; use `x = 1` as a second untimed input and expect `false` |
| `negative_zero` | `return -(1.5 - 1.5);` | `-0` | Binary64 signed-zero check |
| `subnormal_tie` | `return D * 0.5;`, where `D` is `0.` plus 323 zeros and `5` | `+0` | `D` rounds to the least positive subnormal; multiplying by half ties to even zero |
| `nonfinite_root` | `return H * 10;`, where `H` is `1` plus 308 zeros | `+Infinity` | Finite literal, nonfinite operation, no eligible operator below the root |
| `division_root` | `return 1.5 / 2;` | `0.75` | Closed but excluded operator with no eligible operator beneath it |
| `integer_local` | `let a = 120; let b = a + 23; return b < 200;` | `true` | No closed operator tree; M7 can specialize ordinary locals, addition and comparison |
| `range_direct` | Reuse [M6 `direct`](../../crates/compiler/benches/support/m6_corpus.rs), rename `sum` to `main`, pass `4` | `6` | No closed operator tree; M6 scalar replacement changes constructor use across presets |
| `array_growth` | `let values = [x * 1.5, x]; return values[0];` | `2.25` | No-fold, memory-positive control under every preset |

For each balanced tree, recursively join equal halves with parenthesized `+`,
starting with the parenthesized leaf. The same literal, return type and argument
must be used at every repetition and on final sources. Rows with special values
(`negative_zero`, `subnormal_tie`, `nonfinite_root`) are native/size workloads and
untimed guest correctness checks, not guest timing workloads. Use `Object.is`
for both zero signs and verify bool results as raw Wasm `i32` (`1`/`0`). Expected
values and checksums are worked constants, not outputs from another preset.

## Checks and sampling

The [Rust corpus and preflight](../../crates/compiler/benches/support/m8_corpus.rs),
[native sampler](../../crates/compiler/benches/m8_scalars.rs),
[raw Node host](../../tests/support/m8_scalar_timing.cjs) and
[recorder](../../tests/support/record_m8_baseline.py) follow the offline
[M6 harness](m6-ranges-86/README.md) and [M5 recorder](m5-strings-2026-10-30/README.md)
patterns without changing those historical workloads. Verification uses the
public compiler and ordinary Wasm; no dependencies or production APIs were added.

- **Preflight and bytes.** Prepare typed programs outside timers. Compare source
  and typed bytes, validate them with `wasmparser::Validator`, verify imports and
  exports, and record raw Wasm lengths and SHA-256 digests per workload/preset.
  Check `None` against direct codegen with the same export selection and require
  byte identity among `Basic`, `Standard`, and `Aggressive`. Run the Node result
  oracle on every workload/preset before collection. Check the direct range's
  constructor call separately from the scalar rows; helper presence alone does
  not establish allocation or its absence.
- **Native compilation.** For each workload, measure source-to-Wasm and prepared
  typed-to-Wasm under `None` and `Basic`, separately. Use 64 untimed compiles,
  then seven batches of 128 compiles per boundary. Include compiler construction,
  export selection, compilation and output destruction. Black-box inputs and
  outputs. Exclude corpus generation, pre-analysis for the typed boundary,
  preflight validation, writes and reporting. Record each batch's total ns and
  invocation count; normalize to ns/compile in the summary.
- **Guest timing.** Compile each ordinary Wasm module in Node before timing.
  For each timed workload/preset, take 20 samples per timing boundary. Use a
  fresh instance per sample and keep phases in separate sample sets. Time one
  synchronous `new WebAssembly.Instance(module)` for instantiation, one export
  invocation for first call on a fresh instance, and 4,096 same-instance calls
  after 2,048 untimed warmup calls for warmed throughput. The call batch includes
  the JS loop, JS/Wasm crossings, and numeric checksum accumulation. Perform
  checks outside timers. Exclude process startup, module file I/O and engine
  module compilation. First call means fresh **instance**, not a cold engine or
  module, because engine state may carry over. Warmed throughput is a measured
  condition, not proof of a settled JIT tier. Retain raw elapsed ns and counts.
- **Growth checks.** Outside timing, observe exported `memory.buffer.byteLength`
  immediately after instantiation and after 1, 2,048, 6,144 and 8,192 calls on fresh
  ordinary instances per workload/preset, reacquiring the buffer at each
  checkpoint. Scalar modules should have no memory export. The array control
  must grow by the final checkpoint. The direct range control should grow under
  `None` but stay at its initial size under enabled presets. Check completed
  results in each phase. Report actual bytes/pages, not inferred allocation
  counts: unchanged pages can hide reservations and do not measure transient
  peak storage. Checkpoints 2,048 and 6,144 bracket the warmed call interval;
  timing for the allocating controls may include growth and is distinct from
  scalar steady-state timing. Use [M6's independent record-allocation evidence](../m6-range-allocation-evidence.md)
  for attribution, not timing or page deltas. Never time an instrumented module.

Collect two sequential sessions without overlapping builds, tests or benchmarks.
Use forward workload/preset order in session 1 and reverse it in session 2. Time
`None` and `Basic`; check bytes, guest results and growth at all four presets.
Use fresh output directories and bounded subprocesses. Retain every completed
sample with workload, preset, boundary, session/order, sample index and count;
report medians and min–max per session, not pooled means or selectively dropped
outliers. Reconcile corpus/source/module identities and size/growth observations
across sessions before interpreting timings.

## Provenance and later comparison

Capture Git HEAD and status, any measured uncommitted diff, input hashes
(including compiler sources, manifests, lockfile, generated WAS, harness and
recorder), exact commands and deadlines, executable hashes, `rustc -vV`, Cargo
version, Node and V8 versions, OS/CPU, affinity, UTC timestamps, and relevant
build/profile/power settings. Preserve preflight, raw timing and growth records
and size/module manifests. Recheck input hashes after collection and report
missing prerequisites or uncontrolled machine conditions; do not fetch tooling.

`None` is the byte baseline, **not** the M8-off half of the future experiment.
Pre-folding `Basic` already selects M6 and M7. Compare the eventual private
M8-on/off variants on the same final revision with M6/M7 settings held fixed,
rerunning this protocol under matched conditions. Require no-fold controls to
retain their bytes across that toggle; do not require byte identity between
`None` and `Basic`. Keep the range control and the integer-comparison case
visible alongside the M7-only `integer_local` control instead of attributing
existing differences to M8. An unchanged control can still incur pass cost; the historical run is not a
substitute for the isolated comparison. Move the test's unfolded-operator
assertion to the explicit folding-off path when the M8 toggle is added. Do not
set a regression threshold after seeing data.

## Reproduce offline

Run from the repository root with cached dependencies, Python 3 and Node 24+.
Run checks before the collector, not alongside it. Use an available logical
CPU in place of 6 and a new output directory under `target/`.

```sh
cargo test --offline -p was-compiler --test m8_benchmarks -- --include-ignored
python3 -B -m unittest discover -s tests/support -p test_record_m8_baseline.py
taskset -c 6 python3 -B tests/support/record_m8_baseline.py target/m8-pre-folding-replay
```

The collector builds a fresh bench-profile executable offline in its own target
directory, hashes inputs on both sides of the build, and runs an untimed smoke
check before collecting. It bounds metadata commands to 30 s, preparation and
Node commands to 120 s, and the build and native timing commands to 600 s. An existing directory or an inconsistent
schedule, manifest, or result fails collection. Local Wasm artifacts are not
checked in; the source generator and file hashes identify the exact inputs.

## Recorded evidence

Both sessions ran on logical CPU 6 of an AMD Ryzen 7 5700X under Linux
`7.2.15-1-omarchy`, with Rust/Cargo `1.100.0` and Node `26.13.1` / V8
`14.6.202.34-node.40`. The CPU governor was `performance`; background activity
and thermal state were not controlled. The measured compiler was
`85a6aa1e4ce9ea735267637865dde8cabb190e3a` plus benchmark-only workspace changes.
The [environment and commands](m8-pre-folding-91/run/environment.json),
[bench-profile artifact record](m8-pre-folding-91/run/cargo-artifact.json),
[Cargo stdout](m8-pre-folding-91/run/bench-build.stdout.log),
[Cargo stderr](m8-pre-folding-91/run/bench-build.stderr.log),
[inputs before the build](m8-pre-folding-91/run/inputs-prebuild.json),
[inputs before the timings](m8-pre-folding-91/run/inputs-before.json) and
[inputs after](m8-pre-folding-91/run/inputs-after.json) record provenance. The
[Git status](m8-pre-folding-91/run/before-git-status.stdout.log) and
[tracked diffs](m8-pre-folding-91/run/git-diffs.json) show the measurement worktree;
measured input and executable hashes were unchanged across sessions.

- **Deterministic checks:** all 52 workload/preset modules passed the result and
  layout oracle in each session. The [module manifest](m8-pre-folding-91/run/forward/artifacts.json),
  [sizes](m8-pre-folding-91/run/forward/sizes.csv), and
  [growth records](m8-pre-folding-91/run/forward/growth.csv) match the reverse
  session. All three enabled presets produced identical bytes per workload.
- **Size:** `wide_closed` is 521 bytes at both timed presets; `wide_open` is
  409 bytes at both. `bool_closed` is 84 → 56 bytes, `integer_local` 81 → 62,
  and `range_direct` 1,153 → 1,242 from `None` to `Basic`. These are existing
  preset differences, not M8 effects.
- **Growth:** no scalar module exports memory. The array control grows from one
  to four pages at all presets after 8,192 calls. The direct range grows from
  one to five pages at `None` and stays at one at each enabled preset. At call
  6,144, the array controls and `None` direct range are already at three and
  four pages, respectively, so their warmed timing can include memory growth.
  These page counts are not allocation counts.
- **Timing:** each session retains 364 native batches and 1,200 guest samples.
  For example, `wide_closed` forward/reverse `Basic` source medians are 35.12 /
  34.83 µs per compile; typed medians are 14.00 / 13.90 µs. Guest medians at
  `Basic` are 1.56 / 1.59 µs for instantiation, 155.5 / 150 ns for first call, and
  6.49 / 6.71 ns per warmed call. The latter includes JS batch overhead and
  can reflect engine optimizations on unchanged Wasm; it is not an estimate of
  M8's potential benefit.

See the [per-session medians and ranges](m8-pre-folding-91/run/summary.csv) and
raw timings: [forward native](m8-pre-folding-91/run/forward/native.csv),
[forward guest](m8-pre-folding-91/run/forward/guest.csv),
[reverse native](m8-pre-folding-91/run/reverse/native.csv),
[reverse guest](m8-pre-folding-91/run/reverse/guest.csv). The focused Rust and
Node tests, Python recorder tests and `cargo test --offline --workspace` passed.
The workspace run left 346 ignored cases; the focused run executed the M8 Node
oracle explicitly. No fresh browser build or release gate is claimed here.
