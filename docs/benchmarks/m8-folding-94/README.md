# M8 isolated scalar-folding measurements (#94)

Measured on 2026-11-23 after #92 and #93. **Folding reduced the frozen corpus by
527 Wasm bytes (15.3%), but did not establish a guest-time benefit.** The warmed
`wide_closed` median regressed in both sessions; byte-identical controls also
showed substantial timing variation. This is measurement evidence, not release
acceptance or a new regression threshold.

## Comparison and reproduction

The [#91 baseline](../m8-pre-folding-91.md) defines all 13 workloads, the independent
result oracle, and sampling boundaries. This run reused those inputs unchanged.
Both arms use `Basic`'s M6 direct-range scalar replacement and M7 integer
specialization. `FoldOff` disables only closed-scalar folding; `FoldOn` enables it.
Both run through the same checked compiler code in a single bench-profile test
executable. The toggle is private; the benchmark adds no public option, guest
instrumentation, or change to preset behavior. `None` is not a comparison arm.
Unlike the #91 executable, this one compiles the core with `cfg(test)`, which
also enables private test-only backend state and checks. Both arms share them;
byte parity with ordinary output does not guarantee native timing parity with a
production build. The native numbers apply to this test build.

From the repository root with cached dependencies and Node 24+, choose an available
CPU and a new directory. Finish other builds, tests, and benchmarks before collecting:

```sh
cargo test --offline -p was-compiler --lib baseline_corpus_isolates_m8_with_m6_and_m7_enabled
cargo test --offline -p was-compiler --test m8_benchmarks -- --include-ignored
python3 -B -m unittest discover -s tests/support -p test_record_m8_baseline.py
taskset -c 6 python3 -B tests/support/record_m8_baseline.py --isolated target/m8-folding-replay
```

The recorder runs two sequential sessions, reversing the workload/variant order
in the second. Each retains 364 native batches (seven batches of 128 compiles per
workload, variant, and source/typed boundary, after 64 warmup compiles). The source
boundary includes analysis; the typed boundary prepares the typed tree outside
measurement. Both include compiler construction, export selection, checked
compilation, and Wasm buffer destruction. Each session also retains 1,200 guest
samples (20 per timed workload, variant, and boundary). Node times instantiation,
first call on a fresh instance, and 4,096 calls on a fresh instance after 2,048
warmup calls, separately. It excludes engine module compilation and I/O. The
signed-zero, subnormal, and infinity cases are verified, not guest-timed.
The `preset` column in the CSVs retains the #91 schema; here it names the folding
variant. Reported medians and min–max ranges are per session, not pooled.

## Deterministic evidence

- Both sessions agreed on every source and module hash, module length, and memory
  checkpoint. The sources match #91, and all 13 `FoldOff` modules match #91's
  `Basic` module hashes. The private `FoldOn` bytes match public `Basic` on final
  sources. Preflight also checks typed/source parity, ordinary enabled-preset
  parity, and module validity. The Node oracle checks imports, exports, independent
  results, signed zeros, alternate bool input, and growth in both arms.
- The eight no-fold controls are byte-identical across the toggle: `fraction_open`,
  `wide_open`, `bool_open`, `nonfinite_root`, `division_root`, `integer_local`,
  `range_direct`, and `array_growth`. The direct range stays at one page; the array
  control reaches three pages after 6,144 calls and four after 8,192. These are
  observed page sizes, not allocation counts. Scalar modules export no memory.
- The decoded `wide_closed` body changes from 31 `f64.add` and 16 `f64.mul`
  instructions to none. This verifies activation, not a speedup.

| Workload | FoldOff bytes | FoldOn bytes | Saved bytes |
| --- | ---: | ---: | ---: |
| `fraction_closed` | 69 | 49 | 20 |
| `wide_closed` | 521 | 49 | 472 |
| `bool_closed` | 56 | 42 | 14 |
| `negative_zero` | 60 | 49 | 11 |
| `subnormal_tie` | 59 | 49 | 10 |
| **All 13 modules** | **3,441** | **2,914** | **527** |

Most of the savings come from the synthetic 16-leaf `wide_closed` expression.
The corpus total is not a size forecast for application modules.

## Timing signals and limitations

**Material guest regression signal, not a confirmed cause.** For `wide_closed`,
warmed medians were **6.72 → 8.43 ns/call** forward (+25.4%) and
**6.49 → 8.22 ns/call** reverse (+26.5%). The ranges overlap: off/on were
6.69–22.45 / 8.42–8.44 ns forward and 6.48–20.06 / 8.20–8.97 ns reverse. There
was no corresponding repeatable instantiation shift; first-call medians were
150 → 145 ns and 160 → 150 ns, with overlapping ranges. Keep the warmed
regression signal visible rather than assuming that fewer instructions make it
faster or resolving it from these samples alone.

**Native cost.** The most consistent typed-compile increases were small:
`wide_closed` rose 0.9% / 0.5% and `nonfinite_root` 1.4% / 1.8% forward/reverse,
with disjoint observed batch ranges in both sessions. The latter emits identical
Wasm and shows that declining a fold can still have compiler cost. Their source
medians changed by less than 1% in both sessions. Other native differences were
small or less consistent; this run did not establish a material native regression.

**No established guest-time benefit.** `bool_closed` warmed medians improved from
7.35 to 6.92 ns in both sessions, but ranges overlapped. `fraction_closed` warmed
medians improved sharply forward and only slightly in reverse. The unchanged
`integer_local` control went from 7.13 to 5.53 ns forward and 5.53 to 7.14 ns
reverse, with disjoint ranges in both sessions despite identical module bytes;
other unchanged controls also changed direction. These results show that neither
matching median direction nor disjoint sample ranges alone establishes a folding
effect. The timings include the JS loop and JS/Wasm crossings; a fresh instance
is not a fresh engine or a guarantee of a particular JIT tier. The allocating
array control can grow during warmed measurement. CPU affinity and the governor
were fixed, but background activity and thermal state were not controlled.

## Provenance and raw evidence

The sessions used logical CPU 6 of an AMD Ryzen 7 5700X with the `performance`
governor, Linux `7.2.15-1-omarchy` (x86-64), Rust/Cargo `1.100.0`, and Node
`26.13.1` / V8 `14.6.202.34-node.40`. The measured checkout was
`41a67da93cd9dd95dfa95afe2cf3917b8c4fa89f` plus the #94 harness changes and existing
local `Cargo.lock` / browser manifest changes (browser dependency versions
`js-sys` 0.3.107 and `wasm-bindgen` 0.2.130). Those browser dependencies were
not built for the compiler-only benchmark target or committed with #94. Input
and executable hashes matched before and after collection. No comparison of
timings to the historical #91 run is used for attribution.

To reconstruct the exact measured dependency inputs in a scratch checkout of
#94, base `Cargo.lock` and `crates/browser/Cargo.toml` on `41a67da`, change only
`js-sys` from `0.3.106` to `0.3.107` and `wasm-bindgen` from `0.2.129` to
`0.2.130` in the browser manifest, then run
`cargo update --offline -p js-sys --precise 0.3.107` with Cargo 1.100.0. This
recipe reproduced both measured files byte-for-byte; verify against the archived
[input hashes](run/inputs-before.json). The measured compiler/harness files
are the #94 changes themselves. The collector retains the full tracked diff in
its local output directory; the published bundle records its digest without
including the unrelated local edits as a patch.

- [Environment, commands, deadlines, and timestamps](run/environment.json),
  [Cargo artifact](run/cargo-artifact.json), [compiler](run/rustc.txt),
  [Cargo](run/cargo.txt), [Node/V8](run/node.txt), and [preflight](run/bench-smoke.txt).
- [Git status before](run/before-git-status.txt) and [after](run/after-git-status.txt),
  [diff hashes](run/git-diffs.json), and [input hashes before build](run/inputs-prebuild.json),
  [before collection](run/inputs-before.json), and [after collection](run/inputs-after.json).
- [Forward](run/forward/artifacts.json) and [reverse](run/reverse/artifacts.json)
  artifact manifests; [forward sizes](run/forward/sizes.csv) and
  [reverse sizes](run/reverse/sizes.csv); [forward growth](run/forward/growth.csv)
  and [reverse growth](run/reverse/growth.csv).
- [Per-session medians and min–max ranges](run/summary.csv), with raw native
  [forward](run/forward/native.csv) and [reverse](run/reverse/native.csv) batches
  and raw guest [forward](run/forward/guest.csv) and [reverse](run/reverse/guest.csv)
  samples. Generated Wasm and the executable are not checked in.

Release acceptance is a separate decision. These data neither resolve the guest
regression signal nor supersede the pending M6/M7 performance decisions.
