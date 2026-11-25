# #96: Binaryen stock profile feasibility

**Decision needed before adapter work:** the two bindings match upstream, but
stock O1, O2, and O3 all remove the unused registered import. This violates the
[M9 external-interface contract](../../was_compiler_memory_and_optimization_roadmap.md#13-milestone-m9--optional-binaryen-integration).
No profiles or guards were changed to make the probe pass. Neither adapter was
implemented.

## Versions and settings

| Distribution | Pin | Upstream provenance |
| --- | --- | --- |
| crates.io `wasm-opt` | `0.116.1`, `wasm-opt-sys` and `wasm-opt-cxx-sys` `0.116.0` | The Rust wrapper's [`v0.116.1` gitlink](https://github.com/brson/wasm-opt-rs/tree/v0.116.1) and the sys release's [`v0.116.0` gitlink](https://github.com/brson/wasm-opt-rs/tree/v0.116.0) both point at `11dba9b1c2ad988500b329727f39f4d8786918c5`. The sys package bundles the native source. |
| npm `binaryen` | `116.0.0` | npm `gitHead` is [`f078b7a96d1fa0d9460839ca84c9e7a5ef795c45`](https://github.com/AssemblyScript/binaryen.js/tree/f078b7a96d1fa0d9460839ca84c9e7a5ef795c45), whose `binaryen` gitlink points at the same commit. The npm lock records the package integrity. |

The common upstream commit is Binaryen [`version_116`](https://github.com/WebAssembly/binaryen/tree/version_116).
The newer Rust `0.133.0` has no corresponding published npm `133.0.0`, so this
probe uses the verified v116 pair rather than comparing unmatched releases.

Each row uses core `Basic`, `Standard`, or `Aggressive` to produce **one** input
for the corresponding O1, O2, or O3 run. Both bindings use the upstream default
pass pipeline, optimize level 1/2/3, shrink level 0, and no debug info, extra
passes, or fast-math. Rust uses the stock `new_opt_level_*` constructors with
optional DWARF build support disabled; JS uses `module.optimize()` with explicit
levels because the JS defaults are not the stock O2 tuple. JS runs under Node
using the package's compiled Binaryen binding, not the bundled CLI executable.

The existing backend emits the Core Wasm MVP subset, including private mutable
globals and memory32. Both optimizers are limited to MVP (`mvp_features_only()` /
`Features.MVP`). Independently, `wasmparser` validates every input and output
with `WasmFeatures::MVP`, rejects a start section, and compares ordered import
and export names, kinds, and resolved external types (not local indices). This
probe does not test runtime semantic equivalence or browser worker loading.

## Reproduce offline

Run from this directory. Prerequisites: Rust/Cargo 1.88+ (the declared minimum
for the locked dependencies), a C++17 toolchain and linker, Node.js 24+ with npm,
and cached package contents
for both lockfiles. Populate a cold cache separately, with network approval
(`cargo fetch --locked` and `npm ci --ignore-scripts --no-audit --no-fund`).
There is no tooling installation or optimizer executable prerequisite. The
recorded run used Linux x86-64, Rust 1.100.0, Clang 23.1.3, Node 26.13.1,
npm 11.20.0, and core source at `cc5fdfc5470ef638287b138e7bffc4a70ee83201`.

```sh
npm ci --offline --ignore-scripts --no-audit --no-fund
CXX=clang++ CARGO_PROFILE_DEV_DEBUG=0 cargo build --locked --offline \
  --target-dir ../../../target/binaryen-96
../../../target/binaryen-96/debug/binaryen-96-probe ../../../target/binaryen-96/reproduced
```

Use a fresh output directory on each run. The probe runs the whole matrix before
returning `2` for a contract rejection (expected here) or `0` if the guard accepts
every output. Other nonzero statuses indicate an incomplete run, such as a
prerequisite or optimizer failure. Each Node subprocess has a 15-second deadline.
Compare generated files with `diff -r artifacts ../../../target/binaryen-96/reproduced`.

## Results and smallest reproducer

The [raw results](results.txt) and [paired Wasm files](artifacts/) retain the
actual output, including the rejected modules.

| Core-emitted case | Stock levels | Input → Rust / JS | Guard | Rust vs. JS |
| --- | --- | --- | --- | --- |
| `unused-import` | O1, O2, O3 | 61 → 44 / 44 bytes | Fail: both drop `host.unused` | Identical in each pair |
| `memory-only` | O1, O2, O3 | 721 → 166 / 166 bytes | Pass: ordered `memory`, `__alloc` exports unchanged | Identical in each pair |

All 12 optimized modules validated as MVP and none added a start function. The
`memory-only` case uses `ExportOptions::None`: its number-array literal still
requires the memory ABI, so it exports memory32 (1 initial page, 65,535 maximum)
and `__alloc: (i32) -> i32`, but no user function.

For the failing case, compile `fn main() -> number { return 1; }` with `main`
exported and register `unused` as `host.unused: (number) -> bool` without
calling it. The [61-byte O1 input](artifacts/unused-import-O1-input.wasm) imports
`host.unused: (f64) -> i32`; the [native](artifacts/unused-import-O1-rust.wasm)
and [JS](artifacts/unused-import-O1-js.wasm) outputs have no imports. All three
modules still export `main: () -> f64`. O2 and O3 reproduce the same removal.
Byte parity on this sample does not establish ABI preservation or authorize the
native or browser slice.
