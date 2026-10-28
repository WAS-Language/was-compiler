# Final-source M3 reproduction (#63)

Recorded 2026-10-28, separately from [#62's historical samples](../README.md), using
its unchanged workload, observation formulas and timing boundaries. See the
[integrated contract and validation map](../../../m3-allocation-infrastructure.md)
for private milestone scope and ordinary/M2 noninterference. Neither recording
activates production reclamation or authorizes M4/ordinary-array consumers.

## Reproduction and provenance

Run sequentially from the repository root with no competing builds/benchmarks:

```sh
python tests/support/record_m3_allocator.py target/m3-integrated-63/measurements
```

The recorder first runs all private allocator tests including ignored cases, then
three release-profile measurements, each selecting exactly one ignored test:

```sh
cargo test --offline -p was-compiler --lib codegen::allocator -- --include-ignored --test-threads=1
cargo test --offline --release -p was-compiler --lib codegen::allocator::measurement_tests::record_m3_accounting -- --ignored --exact --nocapture --test-threads=1
cargo test --offline --release -p was-compiler --lib codegen::allocator::measurement_tests::record_m3_registry_search -- --ignored --exact --nocapture --test-threads=1
cargo test --offline --release -p was-compiler --lib codegen::allocator::measurement_tests::record_m3_guest_timing -- --ignored --exact --nocapture --test-threads=1
```

All four commands passed. [environment.json](environment.json) records exact argv,
exit codes, versions, clean starting status and revision
`fc72d290b0cea1095b0b9df86c476ebc06f1499f`. It verifies unchanged executable hashes
before/after recording. All source hashes and four fixture/module artifact hashes
also match #62's manifest exactly, despite its earlier dirty-revision provenance.
Documentation/evidence packaging occurred afterwards; no executable inputs changed.

[correctness.log](correctness.log) selects 38 passing tests, none failed/ignored.
[accounting.log](accounting.log), [search.log](search.log) and [timing.log](timing.log)
each select one passing release-profile test. The outer command had a 600-second
deadline, the recorder bounds each Cargo child at 360 seconds, and Rust bounds
Node children at 60 seconds. No timeout occurred. These are subprocess deadlines,
not guest fuel or process-tree control. Generated Wasm remains under `target/`;
[workload.was](workload.was) and module hashes allow reproduction without committing
binaries.

## Results and limits

[accounting.json](accounting.json) and [search.json](search.json) have identical
`results` to #62's recording:

- Five fixed classes, four peak blocks per class: **20 records, 640 record bytes,
  992 payload-capacity bytes and 1,632 aligned backing bytes**. Cursor, population
  and metadata remain constant over 256 post-warmup cycles, with initialized live
  contents checked throughout. Stable pages are not substituted for storage reuse.
- Eight free class-16 blocks retain **128 free payload bytes**; request 17/class 32
  still needs **64 fresh backing bytes** and adds **15 active rounding-waste bytes**.
  Stranded free capacity is separate from active waste. Storage/pages do not shrink.
- With 512 records, head/middle/tail/absent lookup attempts **1/257/512/512 record
  comparisons**. Completed comparisons survive later traps; observation searches
  also count and are isolated from release deltas. Release is linear, not constant
  time. The counter is absent from ordinary/noninstrumented variants.
- Ordinary/private/instrumented modules remain **730/2,186/2,226 bytes**; instrumentation
  adds 40 emitted bytes and one logical i64 global, not measured engine overhead.

[timing.json](timing.json) preserves all **150 noninstrumented raw samples**:
three repetitions, five fresh-instance samples per case/repetition, 2,048 warmup
and 4,096 timed release/reacquire pairs. Medians below use 15 samples per row,
**ns per pair**, with #62 retained for comparison rather than overwritten:

| Population | Rank | #62 median | #63 median |
| ---: | ---: | ---: | ---: |
| 1 | 1 | 139.87 | 99.76 |
| 32 | 1 | 216.43 | 100.83 |
| 32 | 17 | 136.69 | 93.21 |
| 32 | 32 | 162.28 | 132.69 |
| 128 | 1 | 113.70 | 87.08 |
| 128 | 65 | 283.37 | 213.84 |
| 128 | 128 | 476.51 | 355.93 |
| 512 | 1 | 140.53 | 84.81 |
| 512 | 257 | 821.14 | 613.60 |
| 512 | 512 | 1,443.55 | 1,179.25 |

Deeper searches cost more here, consistent with independently counted comparisons;
head variation and uniformly lower medians on unchanged modules demonstrate the
limits of attributing wall-clock differences to code changes. This is neither an
optimizer speedup nor an isolated-Wasm latency comparison. Timing includes JS
iteration, two JS/Wasm calls and multivalue transport; it excludes compilation,
instantiation, validation, initialization, warmup, observations, instrumentation
and reporting. V8 tier stability and hardware-independent thresholds are not claimed.

The environment remains Windows x64 / Intel Core Ultra 5 125H, Node 24.12.0 / V8
13.6.233.17-node.37 and rustc/Cargo 1.98.0. Larger demand, new classes or repeated
traps retaining active allocations can grow storage and metadata. Bounded successful
explicit-release cycles do not establish general bounded memory, fragmentation
resistance, page return, stale-read safety or production suitability.
