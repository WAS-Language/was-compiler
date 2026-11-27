# Application benchmarks — initial Linux baseline, 2026-11-27

Two deterministic import-free programs complement the M4/M5 microbenchmarks:

- **Report:** format each status with `string_concat`, append it to a resizable
  fragment array, and produce one semicolon-separated report with `string_join`.
  The dataset cycles `ok`, `alerta`, `ação`, `revisão`. The oracle reads the
  revision-1 string descriptor and checks both byte length and exact UTF-8 text.
- **Aggregate:** iterate readings by range index and call a scoring function with
  two preconditions. Reading `i` is `(37*i + 11) % 100`. Scores are weighted by
  3 for readings at least 80, by 2 for readings from 50 to 79, and by 1 otherwise.
  The oracle checks the exact scalar sum.

Each program embeds 16, 128, or 1,024 entries in its source. This scales both
compiler input and guest work without host-side encoding or file I/O in a
sample. All six sources are compiled in `None` (O0) and the public default
(`Basic` in this run). Only `main` is selected as a user export; ABI-required
memory exports remain. No Binaryen, private observation helpers or new dependencies
are involved. These are application-shaped workloads, not production traffic.

## Reproduce offline

From the repository root, with cached dependencies and Node.js 24+. Each output
directory must be new; artifact generation will not overwrite an existing run.

```sh
cargo bench --locked --offline -p was-compiler --bench applications -- --check target/applications-check
cargo bench --locked --offline -p was-compiler --bench applications -- --record target/applications-run
```

`--check` generates six `.was` and twelve `.wasm` files, validates the modules
with `wasmparser`, and executes each against an independent Node oracle without
collecting samples. `--record` requires that same preflight before any timer,
then records `native.csv` and `runtime.json`. The Node subprocess deadline is
120 seconds per invocation; this is not guest fuel. To recheck already generated
artifacts without recompiling, run `node tests/application-bench.mjs check DIRECTORY`.

| Measurement | Included | Excluded |
| --- | --- | --- |
| Native source → Wasm | Compiler construction, export selection, compilation, output destruction | Source generation, module validation, file I/O, process startup |
| Wasm size | Raw emitted byte count | WAT, packaging |
| Engine module compilation | `new WebAssembly.Module(bytes)` | File read, instantiation; engine caching may occur, so this is **not** cold compile |
| Instantiation | `new WebAssembly.Instance(module)` | Engine compile and export lookup |
| Guest execution | One zero-argument call to `main` on a fresh instance, including the JS → Wasm crossing | Host input encoding/copy, string descriptor reads, decoding and result assertions |

Both collectors run three repetitions with forward/reverse/forward case order,
two warmups per case per repetition, and five samples per case per repetition.
A Rust `Instant` sample is a batch of ten builds; divide `elapsed_ns` by
`iterations`. A Node sample uses `process.hrtime.bigint()` and contains one
measurement per stage (`compileNs`, `instantiateNs`, `executeNs`). Node checks
all cases before timing and verifies each result after the call timer stops.

## First recording

Linux x86-64 `7.2.15-4-omarchy`, AMD Ryzen 7 5700X, Rust/Cargo 1.100.0,
Node 26.13.1 / V8 14.6.202.34-node.40. Used the `--record` command above with
`taskset -c 0` and output `target/applications-baseline-2026-11-27`; the CPU
was not exclusively reserved. The governor was `performance`. The worktree
was based on `e16293fa5aafa4717547de2c62586c3b9980cf7e` with the benchmark and
separate adapter/evidence edits. Production compiler source was unchanged.
There were 180 native samples and 180 runtime samples **per stage**. Source and
module bytes matched a separate `--check` run.

Medians below are in microseconds per operation, except **Wasm**, which is bytes.

| Application | Entries | Preset | Wasm | Source → Wasm | Engine compile | Instantiate | Execute |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| Report | 16 | O0 | 3,869 | 61.0 | 7.1 | 18.9 | 4.9 |
| Report | 16 | Default | 3,869 | 63.6 | 7.2 | 17.9 | 4.9 |
| Report | 128 | O0 | 5,060 | 101.1 | 7.4 | 18.4 | 28.0 |
| Report | 128 | Default | 5,060 | 109.8 | 7.3 | 20.1 | 28.0 |
| Report | 1,024 | O0 | 14,692 | 400.7 | 9.0 | 20.3 | 93.2 |
| Report | 1,024 | Default | 14,692 | 458.3 | 9.8 | 24.6 | 93.5 |
| Aggregate | 16 | O0 | 1,542 | 43.2 | 5.0 | 16.4 | 1.1 |
| Aggregate | 16 | Default | 1,495 | 48.7 | 4.6 | 16.0 | 0.9 |
| Aggregate | 128 | O0 | 3,223 | 77.7 | 4.7 | 16.0 | 2.9 |
| Aggregate | 128 | Default | 2,544 | 91.9 | 4.6 | 14.7 | 2.7 |
| Aggregate | 1,024 | O0 | 16,665 | 335.0 | 5.6 | 15.7 | 20.1 |
| Aggregate | 1,024 | Default | 10,931 | 417.9 | 5.3 | 18.1 | 20.3 |

[Native samples](native.csv) · [Node samples](runtime.json) · [Artifact hashes](SHA256SUMS)
· [Source and executable fingerprints](provenance.sha256). Compare the generated
source and Wasm hashes on replay; timing samples are not expected to hash-match.
The smaller aggregate modules at the default preset are a byte-count result.
The spot timings do not establish a speedup, regression, or timing budget, and
this corpus does not replace the [official M4/M5 baseline](../m4-m5-aa-2026-11-27/baseline.md).
