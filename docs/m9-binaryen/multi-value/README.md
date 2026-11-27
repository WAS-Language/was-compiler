# M9 multi-value replay — 2026-11-27

**The M9 compatibility gate passed.** Both adapters use MVP plus multi-value;
Binaryen v116, `local-peephole-v1`, and the public revision-1 ABI are unchanged.
The replay does not settle M4/M5 performance, add Wasmtime, or claim a speedup.
The [MVP-only #99 run](../README.md#historical-mvp-only-result) remains historical.

## Correctness results

| Gate | Result |
| --- | --- |
| Offline workspace with ignored cases, after rebuilding `pkg/` | 1,737 passed across 97 summaries, none failed or ignored |
| Rebuilt browser smoke and worker protocol | 25 browser groups and 9 worker tests passed, including resizable execution at O1–O3 |
| CLI opt-in, publication, and runtime | 3 feature-disabled and 8 feature-enabled tests passed |
| Shared guard | 5 passed; private multi-value accepted on either side, sign-extension instructions and start sections rejected, ordered resolved ABI checked |
| Offline Clippy and standalone Wasm | `-D warnings` passed for default all targets, Binaryen CLI binary/tests, and both browser library configurations on `wasm32-unknown-unknown`; standalone core Wasm built |
| Actual playground in Chromium | All four paths passed: off, O0, missing asset, active |
| Matched replay | All 20 rows passed: 5 identical O0 bypasses and 15 repeatable, byte-identical active CLI/Chromium pairs |

Every pre-Binaryen CLI/Chromium pair matched. Native and Chromium independently
checked before/after behavior against the same raw ABI oracle, using their own
WebAssembly instances. The guard validated both modules and compared ordered,
resolved imports and exports on each active request. The four historical fixtures
were unchanged, and all 38 retained historical Wasm hashes reproduced. The new
replay records all 60 Wasm hashes, including the browser's O0 output.

The [new fixture](../../../tests/fixtures/m9/resizable-reentry.was) suspends an
indexed write while the host reenters the same instance. Forced growth relocates
the payload without changing the handle. The callback observes `[10, 20, 30]`
before returning `42`; the pending write then produces `[10, 42, 30]`. A second
write reenters to pop two items and mutate the first. When the callback returns,
the now-invalid index traps; no post-store callback runs. The array remains
`[11]`, and a subsequent guest call on the same instance returns `12`. The oracle
checks effect order and reacquires payload addresses without fixing them to
specific values or inspecting private allocator state. This is bounded coverage,
not a proof of equivalence for every program.

See [native publication status](native-status.json), [Chromium replay](chromium-replay.txt),
[Wasm hashes](SHA256SUMS), and [build and harness fingerprints](provenance.sha256).

## Sizes and spot timings

Sizes in bytes were identical across active levels and adapters:

| Fixture | O0 bypass | O1–O3 input → output |
| --- | ---: | ---: |
| Scalar | 81 | 59 → 56 |
| Memory only | 739 | 721 → 701 |
| Contracts and fixed-storage reentry | 4,576 | 4,587 → 4,548 |
| Resizable constructor | 2,356 | 2,350 → 2,354 |
| Resizable reentry | 3,112 | 3,058 → 3,083 |

Times below are medians in milliseconds from seven samples per level for the
**contracts** case only, following the [historical timing boundaries](../README.md#size-and-timings).
Post-processing and guest timing are separate; the new resizable cases were not
timed. These fixed-order spot measurements do not establish a regression or gain.

| Level | Native post-process | Chromium post-process | Node guest off → on | Chromium guest off → on |
| --- | ---: | ---: | ---: | ---: |
| O1 | 1.166 | 4.6 | 2.457 → 2.459 | 2.5 → 2.5 |
| O2 | 1.137 | 2.9 | 2.457 → 2.452 | 2.5 → 2.5 |
| O3 | 1.126 | 2.4 | 2.455 → 2.457 | 2.5 → 2.5 |

Raw samples: [native post-processing](native-post.csv), [Node guest](node-guest.json),
and [Chromium](chromium-measurements.json).

## Reproduce offline

Recorded on Linux x86-64 (AMD Ryzen 7 5700X), Rust 1.100.0, Clang 23.1.3,
Node 26.13.1, Chromium 158.0.8127.88, wasm-pack 0.15.1 and wasm-bindgen 0.2.130.
The run used the uncommitted multi-value adapter and replay changes atop
`e16293f`. Binaryen packages match the [#99 pins](../README.md#provenance-and-offline-replay).
Chromium needs access to local sockets and the harness HTTP loopback server;
its own sandbox remains enabled.

Use the [#99 offline build and validation commands](../README.md#provenance-and-offline-replay)
with the current sources and separate ordinary/feature-enabled target directories.
Use an absolute `CARGO_TARGET_DIR` for the two `wasm-pack` builds if setting it,
since wasm-pack runs Cargo from `crates/browser`. Replace the historical replay
commands with the following (use a fresh output directory for each run):

```sh
node tests/m9-native.mjs target/m9-workspace/debug/was-compiler target/binaryen-96/debug/was-compiler target/m9-multi-value-replay
node tests/playground-http.mjs docs/probes/binaryen-96/node_modules/binaryen/index.js examples/browser/binaryen-check target/m9-multi-value-replay
(cd target/m9-multi-value-replay && sha256sum -c ../../docs/m9-binaryen/multi-value/SHA256SUMS)
```

Both runners should exit 0. For spot timings use the historical measurement
commands with the new replay directory; timing samples need not reproduce.
