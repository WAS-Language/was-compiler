# M6 ordinary range-loop measurements (#86)

On 2026-11-22, two sequential sessions measured ordinary output from the M6
compiler. These measurements neither replace [#84's allocation proof](../../m6-range-allocation-evidence.md)
nor establish release acceptance. No timed module was instrumented. `None`
remains the default; the required benefit is range-record elimination, not
faster execution or smaller modules.

## Workloads and boundaries

- **Corpus:** the [native harness](../../../crates/compiler/benches/m6_ranges.rs)
  emits three [fixed WAS sources](../../../crates/compiler/benches/support/m6_corpus.rs).
  `direct` iterates a range expression; `binding` stores it first; `call`
  gets it from an internal function. Each sums the same exclusive progression
  `0..n`, with the zero computed from the unknown numeric parameter to avoid
  unrelated exact-i32 literal specialization. The two ineligible controls
  retain their range records. The only user export is `sum`; there are no hosts.
- **Native compilation:** a Cargo bench-profile executable, using `Instant`,
  measures source-to-Wasm and pre-analyzed typed-to-Wasm separately. Both include
  construction/configuration of a new compiler and destruction of the black-boxed
  output buffer. Source timing includes parsing, analysis and typed-program
  destruction; typed timing reuses a borrowed program. All corpus preparation,
  validation, byte comparison, module writes and output formatting are outside
  timing. Per case and round: 64 untimed compiles, then seven batches of 128.
- **Guest execution:** the [Node harness](../../../tests/support/m6_range_timing.cjs)
  compiles ordinary modules once per workload/preset and instantiates a fresh
  guest per sample. It checks the empty and nonempty results, then makes 2,048
  untimed same-instance warmup calls. Each timed batch makes 4,096 calls into
  that instance at either 4 or 1,024 iterations per call. It includes JS loop,
  Wasm export crossing, checksum accumulation and whatever guest work occurs
  during those calls (including any allocation/growth). It excludes process
  startup, module loading/compilation, instantiation, warmup and assertions.
  Outside the timer, the checksum must match `calls × n(n−1)/2` exactly. No
  memory reset occurs after warmup and no allocator or memory counter is read.
- **Module size:** raw `.wasm` file lengths, not WAT, compressed bytes or native
  machine code. Independent [`for_lowering` coverage](../../../crates/compiler/tests/for_lowering.rs)
  checks that the direct loop drops the constructor/validator calls while range
  and memory infrastructure, helper order and non-code sections stay in place.
  The measured direct module gained **89 bytes** (1,152 → 1,241): the direct
  body emits its own operand validation/setup while the shared helpers are
  retained. Neither control changed (binding: 1,158; call: 1,165).

Each stage runs two rounds in opposite case order per process, with all four
presets measured separately, not substituted by a representative. There are 336
native and 336 guest batches per session. The bounds are synthetic, not a
production workload or a steady-state engine-tier guarantee.

## Reproduce

From the repository root with cached Cargo dependencies and Node installed.
Replace `BENCH` with the executable printed by Cargo, and choose a CPU available
on the current machine. Run the sessions in order without other task builds or
benchmarks. The recorder requires new session directories and keeps intermediate
Wasm/source files under `target/`.

```sh
cargo bench --offline -p was-compiler --bench m6_ranges --no-run
BENCH=target/release/build/was-compiler/563b1ae62aae54ea/out/m6_ranges-563b1ae62aae54ea
mkdir -p target/m6-ranges-86/reviewed
for run in run1 run2; do
    dir="target/m6-ranges-86/reviewed/$run"
    timeout 45s taskset -c 4 "$BENCH" --record-samples "$dir" &&
    timeout 45s taskset -c 4 /usr/bin/node tests/support/m6_range_timing.cjs "$dir" &&
    (cd "$dir" && sha256sum *.was *.wasm) > "$dir/modules.sha256" || exit 1
done
cmp target/m6-ranges-86/reviewed/run1/modules.sha256 target/m6-ranges-86/reviewed/run2/modules.sha256
cmp target/m6-ranges-86/reviewed/run1/sizes.csv target/m6-ranges-86/reviewed/run2/sizes.csv
```

The recorded sessions pinned both processes to Linux logical CPU 4 (SMT sibling
12 was not reserved). There were no competing task builds or benchmarks during
measurement. Both sessions shown are from the reviewed recorder; two preliminary
sessions from before the recorder's CLI/setup cleanup remain in `target/m6-ranges-86/run{1,2}`
on the measurement machine. No batches from the reviewed recorder were dropped. Background system load and thermal state were not controlled;
see [environment and source/tool hashes](environment.txt). The exact inputs are
in the corpus and the exported input and module hashes matched between sessions.
`Basic`, `Standard`, and `Aggressive` are byte-identical per workload. `None`
also matches current direct codegen and independently matches the pre-#82 CLI
for all three sources, with only `sum` selected. The historical CLI was built
separately after preliminary timing and was not timed. The same historical
output also matched the reviewed sessions:

```sh
mkdir target/m6-ranges-86/pre82
git archive d9c59d1474a52cbdec8b23046313aa09184ac627 | tar -xf - -C target/m6-ranges-86/pre82
cargo build --offline --manifest-path target/m6-ranges-86/pre82/Cargo.toml \
    --target-dir target/m6-ranges-86/pre82-target -p was-compiler-cli
for workload in direct binding call; do
    target/m6-ranges-86/pre82-target/debug/was-compiler \
        "target/m6-ranges-86/reviewed/run1/$workload.was" --export sum \
        -o "target/m6-ranges-86/pre82/$workload.wasm" &&
    cmp "target/m6-ranges-86/pre82/$workload.wasm" \
        "target/m6-ranges-86/reviewed/run1/$workload-None.wasm" || exit 1
done
```

## Results and disposition

[Full summaries](results.md) show the median and min–max of **all** 14 batches
per case in each session, normalized to time per call. Raw records:
[run 1 guest](run1/guest.csv), [native](run1/native.csv), [sizes](run1/sizes.csv),
[hashes](run1/modules.sha256); [run 2 guest](run2/guest.csv), [native](run2/native.csv),
[sizes](run2/sizes.csv), [hashes](run2/modules.sha256); [historical baseline hashes](pre82.sha256).

- **Guest, 4 trips:** direct-loop median fell from about 28 ns to 11 ns per
  call. Both fallback medians stayed near 27–28 ns. Some batches were several
  times slower, including for byte-identical presets and in both sessions. For
  example, enabled `Basic` reached 68–78 ns/call on the direct loop despite its
  11 ns/call median. The timing data do not identify a cause for these tails.
- **Guest, 1,024 trips:** enabled direct-loop medians were approximately
  0.698 µs versus 0.689 and 0.690 µs at `None`, a small repeatable slowdown on
  this machine. Fallback medians stayed near 0.688–0.691 µs, with slow batches
  in byte-identical controls (up to 0.842 µs on the call fallback). The result
  does not establish a general effect or its cause.
- **Native:** enabled medians were higher at both boundaries for every workload
  in both sessions, even for byte-identical fallback output. Direct source
  medians were 21.21–21.49 µs versus 19.33–19.62 µs at `None`, and direct typed
  medians 13.57–13.75 µs versus 12.07–12.28 µs. Slow batches included 18.07 µs
  for direct typed `Basic` and 27.18 µs for call source `Basic`. The comparison
  is between full preset policies, not isolated passes; it does not attribute
  the native cost increase to scalar replacement.

The compilation cost increase, larger direct module, longer-loop slowdown and
unexplained guest variability are carried forward for **explicit disposition**. No speed threshold,
causal attribution for timing noise, general speedup or release decision is made
here. The separately demonstrated 32-byte record elimination still stands.
