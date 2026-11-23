# M6 final-source release readiness — #87

**Historical #87 evidence.** See the [later default-preset decision](m8-release-readiness.md#default-preset-decision)
for current activation and M6 performance disposition. At #87, integrated
release acceptance was pending. The final-source native tests and
range-record elimination evidence passed. Strict native Clippy failed on an
unrelated existing lint; browser smoke, rebuilt-browser parity and the full
ignored workspace run remain unverified. Performance disposition is also pending.
The only implemented M6 optimization is the direct range-loop slice approved in
[ADR 0011](adr/0011-bounded-m6-range-scalar-replacement.md). These measurements
used `None` as the byte baseline.

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
The browser M6 matrix is present and typechecked but was not executed.

Once the offline prerequisite and unrelated Clippy blocker are resolved, rerun
any affected gates on stable final executable sources. In particular, rerun
strict native Clippy and the documented
[browser build](validation.md#finite-direct-commands), then run
`cargo test --offline --workspace -- --include-ignored`, which includes browser
smoke and per-preset parity. For focused verification or troubleshooting, run
`node tests/browser.cjs pkg` or
`cargo test --offline -p was-compiler-browser --test m6_range_adapters -- --include-ignored`
after the same fresh build.

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

1. **Correctness gates:** resolve the existing strict-native-Clippy failure and
   make wasm-bindgen CLI `0.2.129` available offline; complete the fresh-browser
   smoke, per-preset adapter verification and full ignored workspace gates.
2. **M6 performance:** explicitly accept the observed compile/size and longer-loop
   costs and unexplained variability as trade-offs for the record elimination, or
   request further controlled measurements or scoped changes before acceptance.
3. **M4 and M5 performance:** explicitly dispose of the pending signals in the
   [M4 release record](m4-release-readiness.md) and
   [M5 release record](m5-release-readiness.md). The latest comments on
   [#72](https://github.com/WAS-Language/was-compiler/issues/72#issuecomment-6521885957)
   and [#81](https://github.com/WAS-Language/was-compiler/issues/81#issuecomment-6535983467)
   close evidence work, not the performance decisions. No approval was recorded
   in #87 or the slice completion notes.

Completing this evidence record does not approve M4, M5, or M6. Closure
specialization, string fusion, temporary-array reuse, compact layouts, broader
range replacement and later milestones remain deferred. No heap reset, general
reclamation, ownership or ABI change, or new public control is authorized.
