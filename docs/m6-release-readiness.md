# M6 final-source release readiness — #87

**Current disposition: the bounded M6 slice is in the experimental default,
and the current-source correctness gates pass.** The [default-preset decision](m8-release-readiness.md#default-preset-decision)
accepts the observed compile-time, size and longer-loop trade-offs in exchange
for deterministic range-record elimination and the measured short-loop benefit.
This is not a general speedup or a causal explanation of timing variability.
The only implemented M6 optimization is direct range-loop scalar replacement
as approved in [ADR 0011](adr/0011-bounded-m6-range-scalar-replacement.md).
Explicit `None` (`-O0`) and direct codegen remain the byte baseline.

## Current-source recheck — 2026-11-27

The recheck used `e16293fa5aafa4717547de2c62586c3b9980cf7e` plus pending adapter,
benchmark and documentation changes. Compiler implementation and M6 fixtures
were unchanged during this review. `wasm-pack` 0.15.1 rebuilt ordinary `pkg/`
offline using the locally cached wasm-bindgen CLI **0.2.130**, matching the current
lockfile. All 25 browser smoke groups passed (Node 26.13.1). Workspace all-target
typecheck, strict native and browser-target Clippy (`-D warnings`), and the
standalone core Wasm build passed.

After rebuilding `pkg/`, `cargo test --locked --offline --workspace -- --include-ignored`
passed **1,737 tests across 97 result summaries**, none failed or ignored. This
includes all three CLI and the rebuilt browser M6 adapter tests, both private
allocation tests, and the benchmark corpus correctness preflight. The adapter
cases check bytes/WAT, diagnostics and bounded execution at each preset;
the allocation checks still distinguish the one 32-byte baseline record from
zero records on the eligible enabled path. No new timing run was made: the
performance trade-offs are those already recorded below and accepted for the
experimental default. The official Linux M4/M5 baseline separately removed the
historical Windows timing dependency.

The remaining sections preserve the original #87 evidence, including its failed
or skipped gates. The new run does not retroactively change those results.

## Final-source provenance

Run on 2026-11-22 on a clean `main` at
`64f9f05bd8d8585aa52f5a6473c2c427dd488c27` (tree
`cf84a569892b1765714b12f06c6c6538acd8fd2e`). #87 changes only documentation and
evidence records; no executable inputs or goldens changed during the gates and
measurements. The lockfile SHA-256 was
`af7880ab794e4a61cf78a04cc10b1478084132648d39a4cfb8495d0ec0294145`.
Linux `7.2.15-1-omarchy`, AMD Ryzen 7 5700X, `x86_64-unknown-linux-gnu`, Rust
`1.100.0` (`0ef1bc386`, LLVM 23.1.5), Cargo `1.100.0` (`3acc79c0c`),
`wasm32-unknown-unknown` installed, Node on `PATH` `v26.13.1` (V8
`14.6.202.34-node.40`), wasm-pack `0.15.1`. As in #86, guest timing used
`/usr/bin/node` `v26.12.0` (V8 `14.6.202.34-node.39`), not the `PATH` Node.

## Offline gates and execution counts

The [command ledger](benchmarks/m6-integrated-87/gates.txt) records exact
commands, wall-clock durations, exit codes, and outer time limits. Commands
ran sequentially from the repository root and did not fetch tools. A failed
gate did not prevent independent checks; browser-dependent checks required a
successful fresh build.

| Gate | Limit / elapsed / exit | Result |
| --- | --- | --- |
| Workspace all-target typecheck | 120 s / 0.17 s / 0 | Passed. Compiles browser test targets; does not execute them. |
| Default native tests | 180 s / 24.85 s / 0 | 1,367 passed, 0 failed, 326 ignored across 82 result targets. |
| Strict native all-target Clippy | 120 s / 3.59 s / 101 | Failed: `clippy::needless_range_loop` in [`named_defaults.rs:487`](../crates/compiler/tests/named_defaults.rs#L487), also reported by #83/#84. The file is unchanged since before #82. No suppression or unrelated edit. |
| Portable core build | 120 s / 0.06 s / 0 | Passed for `wasm32-unknown-unknown`. |
| Workspace contract | 120 s / 0.24 s / 0 | 3 passed: workspace/dependency/consumer checks. |
| Fresh offline browser rebuild | 180 s / 0.12 s / 1 | Rust Wasm compilation finished; wasm-pack failed with `Not able to find or install a local wasm-bindgen`. The lockfile requires CLI `0.2.129`; no matching local CLI was found. |
| Browser smoke and adapter execution | Not run | No fresh generated `pkg/`; no smoke groups or browser parity results claimed. |
| Full workspace including ignored tests | Not run | Requires the fresh browser build. The native-only substitute is below. |
| Native tests including ignored (substitute) | 300 s / 32.71 s / 0 | 1,693 passed, 0 failed, 0 ignored across 82 result targets. Includes compiler and CLI, not browser. |
| Strict browser-target library Clippy | 120 s / 0.50 s / 0 | Passed. Does not validate generated JavaScript or browser execution. |
| Workspace inventory (`--list`) | 120 s / 0.66 s / 0 | 1,714 tests listed across 96 targets, including 21 browser cases outside the native run. Inventory, not a pass count. |

The [contract-to-test map](m6-contract-test-map.md) identifies coverage and its
limits. The native run includes all 32 `for_frontend`, 23 `for_lowering`, 12
`for_loops`, 3 CLI `m6_range_adapters`, 2 private range-allocation, and 1 corpus
preflight tests. The CLI matrix checks Wasm/WAT bytes and diagnostics against the
core within each preset; the bounded runtime test executes CLI-generated Wasm.
Decoded and byte checks establish `None`/direct-codegen compatibility, enabled
preset identity, ineligible controls with unchanged bytes, and preserved module
surface and helper ordering on the tested inputs. This is not a claim of universal
`None`/enabled byte identity: other enabled optimizations can still change bytes.
The browser M6 matrix was typechecked but not executed in #87.

#87 called for rerunning strict native Clippy and completing the offline browser
and full workspace gates. The [current-source recheck](#current-source-recheck--2026-11-27)
records the later results. For future reruns, use the documented
[browser build and validation commands](validation.md#finite-direct-commands).

An executable input change invalidates affected earlier evidence; regenerate
browser output and rerun the relevant correctness and measurement checks rather
than carrying forward results indiscriminately.

## Final-source allocation and measurement reconciliation

The [reconciliation ledger](benchmarks/m6-integrated-87/reconcile.txt) records
all bounded commands, durations and exits. Two runs of the private allocation
suite passed both tests each. The standalone Node observations reproduced the
same 136-line trace (`0a26623288efc0b86c356b4dcd05fc28441f5f9684fe0128a71e45f7f7ecb663`)
and all eight module digests in [#84](m6-range-allocation-evidence.md#recorded-run--2026-11-22).
A normally completed eligible construction, even if empty, reserves one 32-byte
range record at `None` and zero when enabled. Binding/call controls still
allocate; invalid completed operands and noncompleting operands allocate no
range record. The observer separates operand, capture and body allocations.
This is not whole-loop allocation freedom, heap reclamation, an RSS or page-growth
measurement, or an allocation-exhaustion test.

Ordinary timing used the [#86 sampling contract](benchmarks/m6-ranges-86/README.md)
unchanged: two sequential sessions, no competing task builds or benchmarks,
CPU 4 (`performance` governor, SMT sibling not reserved), fresh guest instances
per sample, and separate source and typed native compiler timings. Background
load and thermal state were not controlled. The recorder executable and
`/usr/bin/node` hashes match [#86's provenance](benchmarks/m6-ranges-86/environment.txt).
Each session retained all 336 native and 336 guest batches: session 1
([native](benchmarks/m6-integrated-87/timing1/native.csv),
[guest](benchmarks/m6-integrated-87/timing1/guest.csv)) and session 2
([native](benchmarks/m6-integrated-87/timing2/native.csv),
[guest](benchmarks/m6-integrated-87/timing2/guest.csv)). No observed module
was timed. All inputs and ordinary module digests and the size table reproduce
[#86](benchmarks/m6-ranges-86/run1/modules.sha256) ([sizes](benchmarks/m6-ranges-86/run1/sizes.csv)).
Both sessions compared `None` output byte-for-byte against the retained
[pre-#82 CLI modules](benchmarks/m6-ranges-86/pre82.sha256) for all three workloads;
the pre-#82 CLI was not rebuilt or timed in #87. Enabled presets emit identical
bytes per workload. The direct module grows 1,152 → 1,241 bytes (+89) because the
loop emits direct setup and the range/memory infrastructure is retained. Binding
(1,158 bytes) and call (1,165 bytes) controls do not change.

Medians below are per call, with enabled ranges spanning the three individual
preset medians, not pooled samples:

| Direct workload | Session 1 `None` / enabled | Session 2 `None` / enabled |
| --- | ---: | ---: |
| Guest, 4 trips (ns) | 28.25 / 11.24–11.25 | 28.99 / 11.24–11.25 |
| Guest, 1,024 trips (ns) | 690.25 / 698.02–698.22 | 689.04 / 698.12–698.32 |
| Native source-to-Wasm (µs) | 18.65 / 20.26–20.55 | 19.57 / 21.05–21.48 |
| Native typed-to-Wasm (µs) | 11.91 / 13.14–13.18 | 12.14 / 13.50–13.57 |

The fallback guest medians stayed near 28–29 ns (4 trips) and 689–691 ns
(1,024 trips); enabled native compilation medians increased for both fallbacks
at both timing boundaries in both sessions despite unchanged output. Slow
samples recur: the enabled 4-trip direct loop reached 75.22 ns in session 1,
and binding `None` reached 118.01 ns. These results reinforce #86's short-loop
benefit, long-loop slowdown and compile/size costs without explaining the tails
or isolating pass-specific compilation cost from full-preset cost. They do not
establish a general speedup, a regression threshold, or a causal explanation.

## Decisions still required

This heading is retained for historical links. At #87, correctness and performance
were still pending. The [current-source recheck](#current-source-recheck--2026-11-27)
closes the correctness gaps on current sources. The
[experimental default decision](m8-release-readiness.md#default-preset-decision)
accepts the M6 performance trade-offs for that scope, and the
[official Linux M4/M5 baseline](benchmarks/m4-m5-aa-2026-11-27/baseline.md)
supersedes the historical Windows timing dependency.

Closure specialization, string fusion, temporary-array reuse, compact layouts,
broader range replacement and later milestones remain deferred. No heap reset, general
reclamation, ownership or ABI change, or new public control is authorized.
