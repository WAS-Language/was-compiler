# Final-source M4 reproduction (#72) — 2026-10-30

These are new sequential recordings on unchanged implementation revision
`52a28557cb41652581ed247e045ca27a6a5ed75f`. See the
[integrated release-readiness record](../../../m4-release-readiness.md) for the
passed correctness gates and **pending performance-signal disposition**.
[#71's definitions and boundaries](../README.md) remain authoritative; historical
samples were not overwritten. Accounting observations and uninstrumented timing
remain separate.

## Commands and provenance

After the eight integrated gates, run sequentially from the repository root:

```sh
python tests/support/record_m4_resizable.py target/m4-integrated-72/measurements
python tests/support/record_m4_resizable.py target/m4-integrated-72/measurement-repeat
```

Each unmodified recorder runs these exact commands in order:

```sh
cargo test --offline -p was-compiler --test resizable_arrays_runtime -- --include-ignored
cargo test --offline -p was-compiler --lib codegen::allocator::measurement_tests -- --include-ignored --test-threads=1
cargo test --offline -p was-compiler --lib codegen::resizable::measurement_tests -- --skip record_m4 --test-threads=1
cargo test --offline --release -p was-compiler --lib codegen::resizable::measurement_tests::record_m4_accounting -- --ignored --exact --nocapture --test-threads=1
cargo test --offline --release -p was-compiler --lib codegen::resizable::measurement_tests::record_m4_guest_timing -- --ignored --exact --nocapture --test-threads=1
```

All commands exited **0**, selecting **38 / 5 / 3 / 1 / 1** passing tests in each
run, with zero failures/ignored. The recorder bounds each Cargo child at 600 seconds,
Rust bounds each Node child at 120 seconds, and each recorder tool invocation had a
1,200-second bound. No timeout occurred and no competing build/benchmark command
was launched by this task. No external-machine-load or power-state isolation is
claimed. Choose fresh directories when reproducing; the recorder refuses overwrite.

| Evidence | First run | Repeat |
| --- | --- | --- |
| Revision, dirty state, versions, environment, 97 source hashes and stable-after flag | [environment.json](first/environment.json) | [environment.json](repeat/environment.json) |
| Complete accounting and search/copy/storage results | [accounting.json](first/accounting.json) | [accounting.json](repeat/accounting.json) |
| All 180 raw timing samples per run | [timing.json](first/timing.json) | [timing.json](repeat/timing.json) |
| Exact one-test recording output | [accounting.log](first/accounting.log), [timing.log](first/timing.log) | [accounting.log](repeat/accounting.log), [timing.log](repeat/timing.log) |
| Runtime prerequisite | [runtime-correctness.log](first/runtime-correctness.log) | [runtime-correctness.log](repeat/runtime-correctness.log) |
| Historical M3 compatibility, not independent M4 proof | [m3-correctness.log](first/m3-correctness.log) | [m3-correctness.log](repeat/m3-correctness.log) |
| Observation isolation | [isolation.log](first/isolation.log) | [isolation.log](repeat/isolation.log) |
| Corpus snapshot | [workload.was](first/workload.was) | [workload.was](repeat/workload.was) |

The [integrated manifest](../../../m4-integrated-validation-2026-10-30.json)
hashes both working-tree and committed Git-blob measurement bytes, and archived
gate logs. Git normalizes CRLF to LF in each `environment.json` and `workload.was`;
checkout conversion can change working-tree byte hashes. Both source manifests match
#71's 97 inputs exactly. Both module pairs match each other and #71:

| Module | Bytes | SHA-256 |
| --- | ---: | --- |
| Ordinary | 6,126 | `c89121663e51268435134faab16df589130bcd849846a8ae980736527e4bd912` |
| Observed | 6,305 | `767503dc4218b16cd371c9ada7e405e3c56c0d2b058a5ed48c72f04d696e3156` |

## Deterministic accounting

Both complete accounting `results` are identical to #71, not merely the final page
count. The harness compares ordinary/observed instances and an independent JS
sequence oracle after each completed call, including retained companion arrays.
The **1,233-operation mixed workload**, all three stranding cases, geometric copy
sums, exact-class reuse and linear-search comparison counts reproduce unchanged.

- Final live slot bytes **3,809**, active capacity **14,976**, spare capacity
  **11,167**, free payload **560**, physical payload **15,536**.
- **24 allocator records / 768 record bytes**, **15 stable objects / 720 bytes**,
  **17,192 bump-growth bytes**, **168 other bump bytes**, one page.
- **103 acquisitions / 79 reused / 88 relocations**, **26,472 cumulative acquired
  capacity bytes / 8,844 inferred copied-live bytes**.
- **22,409 allocator / 4,632 authority comparisons**. No-op calls still search;
  geometric copy volume is not an O(1) total append-time proof.
- Every stranding case retains **720 free bytes** but needs **544 new bump bytes**
  for a 512-byte request; free byte classes are not split/coalesced/borrowed.

Completed-call peaks exclude transient old-plus-new residency. Current payloads,
stable objects, unreachable arrays, immutable strings and failed-construction
allocations are not reclaimed generally. The successful-workload census does not
measure failed unpublished storage. Neither stable pages nor payload reuse proves
a total-memory bound, GC, page return or suitability.

## Timing comparison and unresolved signal

Medians of 15 samples per type/case, **ns per iteration** (a pair for paired cases):

| Type / case | Historical #71 | First #72 | Repeat #72 |
| --- | ---: | ---: | ---: |
| number append | 115.62 | 210.74 | 98.83 |
| number spare push/pop | 160.74 | 406.84 | 207.64 |
| number ordered remove/push | 266.36 | 502.05 | 315.62 |
| number no-op reserve oldest/32 | 225.73 | 670.17 | 263.01 |
| bool append | 113.87 | 278.12 | 135.16 |
| bool spare push/pop | 156.91 | 366.92 | 198.24 |
| bool ordered remove/push | 192.09 | 455.66 | 264.11 |
| bool no-op reserve oldest/32 | 222.78 | 427.54 | 251.93 |
| string append | 132.42 | 341.41 | 160.16 |
| string spare push/pop | 173.34 | 478.42 | 225.49 |
| string ordered remove/push | 284.77 | 679.30 | 359.57 |
| string no-op reserve oldest/32 | 176.03 | 431.96 | 253.42 |

Both runs use Node 24.12.0 / V8 13.6.233.17-node.37 on Windows x64 10.0.26300,
Intel Core Ultra 5 125H, with identical recorded overrides (unset). Each has 180
raw samples, three repetitions, five fresh instances per case/repetition and
reversed second-repetition ordering. Warmup uses separate discarded instances.
Only ordinary uninstrumented modules are instantiated for timing; construction,
validation, decoding/assertions and reporting are excluded. JS loops/crossings,
parameter checks, item selection, searches, allocation/copy and return transport
are included. Compiler timing is not measured.

The first medians are 1.82–2.97× historical #71; the repeat reduces them
substantially, but 11 of 12 remain 1.13–1.44× historical medians. Number append is
0.85× historical. Identical module bytes rule out a changed-code explanation for
these runs, **not** engine/host/environment effects. Thermal/power state, scheduler
load, tier stability and significance were not measured. Neither the slow run nor
the faster repeat may be selectively discarded. No speedup, cross-type ranking,
compiler regression or generally bounded execution cost is established.

#72 therefore records all measurement-execution gates passed while leaving
**integrated release acceptance pending explicit performance-signal disposition**
or further controlled measurements. Correctness does not silently accept the
material unexplained signal or authorize any new optimization/default.
