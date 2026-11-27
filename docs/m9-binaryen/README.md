# M9 integrated Binaryen release gates (#99)

## Current envelope and release evidence

Both adapters and the shared guard allow MVP plus multi-value for the private
resizable allocator. The pinned Binaryen version, pass list, and public ABI are
unchanged. The [expanded replay](multi-value/README.md) passed all 20 CLI/Chromium
rows, including resizable mutation, traps and host reentry, plus the actual
playground and applicable #99 gates. The MVP-only results below are historical.

## Historical MVP-only result

**Release blocked in this run.** Both adapters rejected ordinary compiler output using
`resizable(...)` at every active level. Nine active native/Chromium pairs for
MVP-compatible inputs were repeatable, byte-identical, and passed the exercised
semantic and revision-1 ABI checks. Those passes do not establish equivalence
for all supported features. No profile change, fallback, or release approval is
part of this run.

## Blocker: the resizable allocator requires multi-value

The [minimal source](../../tests/fixtures/m9/resizable.was) is:

```was
fn main() -> number[] { return resizable([42]); }
```

Binaryen-off compilation and O0 bypass produce usable modules; the raw host
reads the single number `42` through the revision-1 descriptor in both Node and
Chromium. The public result is still an `i32` handle. The private allocator
`Acquire` signature is `(i64) -> (i32, i64)` (payload pointer and acquisition
identity; see [`abi.rs`](../../crates/compiler/src/codegen/abi.rs)). The O1 WAT
contains `(type (;6;) (func (param i64) (result i32 i64)))`. The baseline module
therefore needs Wasm multi-value before Binaryen even runs a pass. This falsifies
the MVP-only input assumption for this supported language feature.

- **Native O1–O3:** exits 1 during Binaryen input validation, citing disabled
  multi-value function results and tuples; no Wasm is published.
- **Chromium O1–O3:** the same post-processor used by the playground worker
  rejects the compiler output at `module.validate()`; no optimized bytes are
  returned. The shared guard would also reject multi-value in either input or
  output under the MVP policy used in this run.

See the [per-level inputs and diagnostics](artifacts/). There is no optimized
pair, semantic comparison, or timing for these rejected requests. This is not an
optimized-byte discrepancy to waive. In that run, the workaround was to leave
Binaryen off or use O0 for programs that needed resizable arrays. See the
[current envelope and release evidence](#current-envelope-and-release-evidence) for
the follow-up.

## Review follow-up

The native CLI now requires one Binaryen core on active requests. It supplies
that default except on Windows, where the CLI requires `BINARYEN_CORES=1`
in the launch environment rather than relying on a runtime update reaching
C `getenv`. The Windows path was not executed in this Linux run. During review, a local Linux fault probe
returned `EAGAIN` from the second `pthread_create` after the first worker had
started. The old CLI timed out after three seconds: Binaryen v116's pool holds
its readiness mutex while joining workers on partial startup failure. The
one-core path completed without creating workers. With the fix, both the
unset and explicit `BINARYEN_CORES=1` runs succeeded under the probe; an
explicit `2` exited 1 before reaching pool startup and preserved the existing
destination. This is a native adapter restriction, not a change to the profile.
The fault probe is not a permanent test: it relies on Linux `LD_PRELOAD` and
thread scheduling. The portable CLI regression instead covers the unset and
explicit multi-core environment boundaries.

Malformed `BINARYEN_PRINT_FULL` previously let the native validation printer
abort while reporting an invalid module. The CLI now rejects it with exit 1,
before creating optimizer scratch files. The playground worker also retains the
optimizer's failure detail and explains how to retry without post-processing;
it does not automatically fall back. The focused CLI and worker regressions
cover these error paths. All 38 retained Wasm hashes reproduced in the
follow-up and the resizable rows remain blocked. The optional suggestion to
rename the fixture-only export selectors was deferred to avoid unrelated churn
in the two replay harnesses.

## Activation, parity, and contracts

| Gate | Result |
| --- | --- |
| Ordinary offline workspace, including ignored tests | Pass: 1,737 tests across 97 result summaries, 0 failed or ignored, fresh native target directory and rebuilt browser `pkg/` |
| Browser compiler smoke | Pass: 25 groups |
| Native Clippy | Pass: default `--all-targets` and Binaryen-enabled CLI binary/tests, `-D warnings` |
| Standalone core Wasm build | Pass: `was-compiler --lib --target wasm32-unknown-unknown` |
| Browser-target Clippy | Pass: ordinary and `playground-binaryen` library configurations, `-D warnings` |
| Native activation/publication | Pass: 3 feature-disabled tests (including unavailable, source diagnostics, and O0); 7 enabled tests (including inactive, O0, active WAT/Wasm, failure without fallback, and differential runtime) |
| Shared browser guard | Pass: 4 tests; MVP validation, start section rejection, and ordered resolved external types |
| Actual worker protocol and headless Chromium | Pass: 8 Node worker tests and all four real playground cases: off, O0, missing asset, active. HTTP requests confirm lazy loading; the missing asset publishes no download. In Chromium, displayed WAT is checked against the download, and an instantiation tap on the unmodified production worker confirms it instantiates those exact bytes. The Node worker protocol independently checks the same byte agreement. |
| Matched release matrix | Blocked: 4 cases × O0–O3; all pre-Binaryen native/Chromium inputs match. All nine published active pairs match and pass differential checks; both adapters reject the three resizable active rows. |

On/off comparisons hold the compiler preset fixed. The native runner compares
feature-enabled inactive bytes against the ordinary CLI, checks O0 bypass, and
repeats each successful active request. Chromium independently compiles the same
source, checks its bytes against the native input, then uses the worker's
post-processor twice and compares both outputs to the CLI artifact. The unchanged
shared guard compares ordered imports/exports and resolved external types
(including memory limits and global mutability), validates both modules within
MVP, and rejects a start section. The runtime oracle also checks exact export
order, an unused ordered import, memory-only exports, growth and descriptor
layout, string UTF-8 payloads and string-array slots, invalid consumed bool
bytes, scalar import normalization, floating remainder edge cases (including
NaN, signed zero, infinities, subnormals and extremes), host call and trap order,
and reentrant mutation of a fixed array during a compound write. The composition
case exercises defaults, contracts, pipelines, placeholders, captures, range
iteration and membership, match, and value loops. It also tests short-circuiting
with host effects and numeric array equality with NaN and signed zero. These
are bounded differential cases, not a proof over all programs.

## Size and timings

Each of O1, O2 and O3 produced the same sizes in both environments:

| Case | Before | After |
| --- | ---: | ---: |
| Scalar | 59 B | 56 B |
| Memory only, no user exports | 721 B | 701 B |
| Contracts and fixed-storage reentry | 4,587 B | 4,548 B |
| Resizable constructor | 2,350 B | Rejected |

The following medians are milliseconds, from seven samples per level for the
**contracts** case only. Native post-processing uses an optimized release build
and measures the production `optimize` call including temporary files and the guard, not CLI startup or core
compilation. Chromium measures the worker's post-processing function with
warm assets, including guards, excluding fetch/startup and WAT rendering. Both
post-processing measurements exclude guest execution. Guest timings exclude
instantiation; each sample calls `hot` 2,000 times against a warmed instance
(a 200-iteration guest loop per call), checking its checksum after each timed
interval. Timing is an
opt-in replay of retained modules, separate from correctness gates. These are fixed-order
spot measurements, not comparative environment benchmarks or evidence of a
speedup. Chromium timer resolution and run-to-run variability matter at this
scale.

| Preset / profile level | Native post-process | Chromium post-process | Node guest off → on | Chromium guest off → on |
| --- | ---: | ---: | ---: | ---: |
| O1 | 1.152 | 3.9 | 2.454 → 2.456 | 2.6 → 2.5 |
| O2 | 1.127 | 2.8 | 2.460 → 2.451 | 2.5 → 2.6 |
| O3 | 1.126 | 2.8 | 2.452 → 2.452 | 2.5 → 2.5 |

See the [raw native post-processing CSV](artifacts/native-post.csv),
[Node guest samples](artifacts/node-guest.json), and
[Chromium samples](artifacts/chromium-measurements.json).

## Provenance and offline replay

Recorded 2026-11-26 on Linux x86-64 (AMD Ryzen 7 5700X), Rust 1.100.0,
Clang 23.1.3, Node 26.13.1, npm 11.20.0, Chromium 158.0.8127.88, wasm-pack
0.15.1, and wasm-bindgen 0.2.130. The original matrix was recorded in
`1577058` (#99). The follow-up reran it with the reviewed adapter changes and
replaced the timing samples; the compiler core, guard, and profile are unchanged.
The native CLI uses workspace-locked
`wasm-opt` 0.116.1 (default features disabled) and `wasm-opt-sys` /
`wasm-opt-cxx-sys` 0.116.0. Chromium uses npm `binaryen@116.0.0`. Both bundle
Binaryen `version_116`, commit `11dba9b1c2ad988500b329727f39f4d8786918c5`;
see [the package gitlinks and identity check](../probes/binaryen-96/README.md#versions-and-settings).
The profile is `local-peephole-v1`: only `optimize-instructions` (no default
passes), optimize level 1, 2 or 3 as selected, MVP, shrink level 0, fast math
off, debug info off. The native corpus and timing use `BINARYEN_CORES=1` and
`BINARYEN_PASS_DEBUG=0`, with `BINARYEN_PRINT_FULL` unset during the recorded run. See [build, corpus, and harness fingerprints](provenance.sha256),
[worktree status at measurement](worktree-status.txt), and
[retained Wasm hashes](artifacts/SHA256SUMS). Fixture export selection and ordered
host signatures are declared in [`m9-corpus.mjs`](../../tests/m9-corpus.mjs).

Use cached dependencies, the locked wasm-bindgen CLI on `PATH`, and fresh build
and artifact directories from the repository root. In the recorded run, the
native Binaryen build reused the locked C++ objects in `target/binaryen-96`.

```sh
npm ci --prefix docs/probes/binaryen-96 --offline --ignore-scripts --no-audit --no-fund
CARGO_TARGET_DIR=target/m9-browser wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline
CARGO_TARGET_DIR=target/m9-browser wasm-pack build crates/browser --target web --out-dir ../../examples/browser/binaryen-check --out-name was_compiler --mode no-install --no-opt --offline --features playground-binaryen
CARGO_TARGET_DIR=target/m9-workspace cargo test --locked --offline --workspace -- --include-ignored
cargo clippy --locked --offline --all-targets -- -D warnings
cargo build --locked --offline -p was-compiler --lib --target wasm32-unknown-unknown
cargo clippy --locked --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings
cargo clippy --locked --offline -p was-compiler-browser --features playground-binaryen --target wasm32-unknown-unknown --lib -- -D warnings
cargo test --locked --offline -p was-compiler-cli --test cli binaryen
CXX=clang++ CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=target/binaryen-96 cargo build --locked --offline -p was-compiler-cli --features binaryen
CXX=clang++ CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=target/binaryen-96 cargo test --locked --offline -p was-compiler-cli --features binaryen --test cli binaryen -- --include-ignored
CXX=clang++ CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=target/binaryen-96 cargo clippy --locked --offline -p was-compiler-cli --features binaryen --bin was-compiler --tests -- -D warnings
cargo test --locked --offline -p was-compiler-browser --features playground-binaryen --lib
node tests/playground-worker.mjs docs/probes/binaryen-96/node_modules/binaryen/index.js examples/browser/binaryen-check
```

To reproduce the historical blocker, use the archived MVP-only sources rather
than the current tree. The next two commands **both exit 1** on that snapshot,
after writing the artifacts for its full matrix. Run Chromium even when the native corpus
returns 1; it consumes the retained inputs/status to test independently. Pass a
new output directory to the native runner each time.

```sh
node tests/m9-native.mjs target/m9-workspace/debug/was-compiler target/binaryen-96/debug/was-compiler target/m9-replay
node tests/playground-http.mjs docs/probes/binaryen-96/node_modules/binaryen/index.js examples/browser/binaryen-check target/m9-replay
(cd target/m9-replay && sha256sum -c ../../docs/m9-binaryen/artifacts/SHA256SUMS)
```

Run measurements separately after generating the correctness artifacts:

```sh
node tests/m9-guest-bench.mjs target/m9-replay
node tests/playground-http.mjs docs/probes/binaryen-96/node_modules/binaryen/index.js examples/browser/binaryen-check target/m9-replay --measure
env -u BINARYEN_PRINT_FULL BINARYEN_CORES=1 BINARYEN_PASS_DEBUG=0 M9_ARTIFACTS=target/m9-replay CXX=clang++ CARGO_TARGET_DIR=target/m9-review-release cargo test --locked --offline --release -j 6 -p was-compiler-cli --features binaryen --bin was-compiler m9_post_processing_time -- --ignored --nocapture
```

Compare bytes and gate outcomes on replay, not timing samples. Do not treat
passing smoke tests, modest size savings, or a shared upstream version as
release acceptance.
