## What to build

Measure M5 runtime and native compilation costs independently, using ordinary, noninstrumented compilation/output. Replacement is #75 and join is #77; native blocking edges are configured on this issue. Measurements must not depend on private observation counters being enabled.

The primary construction comparison is repeated immutable concatenation versus fragment accumulation followed by one final `string_join`. Include bounded size families, empty/nonempty separators, empty fragments, multibyte UTF-8 and array-growth boundaries. Add adversarial repetitive sources/needles for `string_replace_all`; M5 permits straightforward bounded-memory search and does not require linear-time substring search. Replacement is exact, case-sensitive, left-to-right and nonoverlapping, never rescans inserted text, and traps on an empty needle (`aaa`, replacing `aa` with `b`, yields `ba`).

Runtime measurements use fresh ordinary instances according to an explicit sampling contract. Native compiler measurements separately exercise source-to-Wasm and/or prepared typed-program compilation. Do not confuse native Cargo release optimization with the language's numeric preset, engine module compilation with native compiler timing, or a benchmark smoke run with a timing baseline.

## Acceptance criteria

- [ ] Record bounded repeated-concat versus accumulation/join guest workloads with independent correctness results outside timing. Include a controlled nonoptimized copy-based concat baseline if reporting its expected cumulative copying behavior; do not demand quadratic copying from optimized concat or a guaranteed join speedup.
- [ ] Keep runtime timing and native compiler timing separate. Use ordinary noninstrumented modules/instances, no marker allocations or accounting callbacks in timed regions. State included JS/Wasm crossings, JS loops, guest validation/allocation/copy/growth and warmup/tiering effects; exclude/report setup, compilation, instantiation, oracle construction, decoding and reporting explicitly.
- [ ] Native compilation measurements use deterministic M5 source families and explicit source/typed boundaries, preserve output destruction timing, and validate outside timed loops. Existing scalar/calls/captures benchmarks remain distinct; their smoke success is not M5 compiler-time evidence.
- [ ] Measure adversarial repetitive replacement inputs/needles, including no-match/overlap pressure and expanding or shrinking replacements. Preserve checked result sizing and direct final-payload construction; do not silently impose a linear-search algorithm requirement or guest size cap.
- [ ] Preserve complete raw samples, repetitions/order/warmup methodology, source and ordinary-module byte sizes/hashes, corpus/harness/recorder hashes, revision plus dirty status, tool versions, CPU/OS/Node/V8, relevant environment overrides and exact commands/bounds. Verify executable inputs remain stable; use fresh output directories without overwriting historical evidence.
- [ ] Run measurements sequentially without competing builds/benchmarks; repeat under comparable conditions before interpreting changes. Report variability and unexplained material regressions rather than selecting only favorable runs. No arbitrary performance threshold, statistical-significance claim without evidence, or guaranteed speedup.
- [ ] Keep round-4 accounting interpretation explicit even though this is timing: n counts fragment occurrences including empty ones; B counts fragment bytes by occurrence plus output separator occurrences. Separate preexisting inputs and fragment-production costs from accumulation/join overhead; array-handle relocation is not string-payload copying. Separately retained repeated joins are separate constructions.
- [ ] Link deterministic test-only evidence for O(n + B) aggregate storage/copying and no intermediate join strings from #79 when available. Timing alone proves neither claim. Transient peaks and completed-call retained storage remain separate; report unavailable storage metrics rather than infer them from runtime or pages.
- [ ] No production telemetry, new public allocator API, fusion, string reclamation, mandatory physical allocation counts or M4 release disposition. Preserve existing immutable string/host retention semantics and within-level adapter/preset contracts.

## Implementation handoff

### Read first

- `AGENTS.md`; `docs/agents/domain.md`; `GLOSSARY.md`; `docs/adr/0010-immutable-string-operations-and-fragment-construction.md`; `docs/was_compiler_memory_and_optimization_roadmap.md` §9. Design documents may still be uncommitted; essential measurement/acceptance rules are embedded above.
- `docs/benchmarks.md` timing contract and provenance rules; `docs/validation.md` finite direct commands.
- `docs/benchmarks/m4-resizable-arrays-2026-10-29/README.md`; `docs/m4-release-readiness.md` and its pending material timing signal.
- Actual replacement/join implementation contracts from #75 and #77 before defining workload expectations.

### Main verified anchors

- `crates/compiler/benches/compiler.rs`: `validated_source`, `lex`, `parse`, `semantic`, `typed_compile`, `source_compile`; Divan `black_box_drop` includes output destruction while preparation/validation stays outside timing.
- `crates/compiler/benches/support/corpus.rs`: current deterministic scalar/calls/captures corpus, not an M5 corpus.
- `tests/support/m4_resizable_measure.cjs`: `timedCase`, `timing`; fresh ordinary instances, separate warmup, 3 repetitions/5 samples per case, reversed case order on alternating repetitions, assertions after timing. These are precedent, not mandatory M5 sample counts.
- `tests/support/record_m4_resizable.py`: `source_hashes`, `main`; sequential bounded recording, raw logs, fresh directory protection and stable-source verification. It does not collect native compiler timing.
- `crates/compiler/src/codegen/resizable_measurement_tests.rs`: `record_m4_guest_timing`; ordinary-module execution is separate from observed accounting.
- `tests/support/process.rs`: explicit direct-child deadlines.

### Direction and open questions

New M5 workload files, native benchmark cases/targets and runtime recorder are **proposed**. Prefer existing Divan and bounded Node patterns; no new benchmarking dependency or generic framework. Choose source sizes that distinguish compiler source-size cost from a constant-size guest loop's runtime parameter. State whether fragment production is outside timing or measured as a separate end-to-end case. Decide sample counts/order and practical external bounds after a smoke run, without weakening offline requirements. Escalate a material unexplained signal, unstable inputs, counter contamination, or need for an algorithm/ABI change; report rather than optimize speculatively.

### Exact validation and bounds

Existing documented compiler commands, from repository root:

```sh
cargo bench --offline -p was-compiler --bench compiler -- --test
cargo bench --offline -p was-compiler --bench compiler -- --sample-count 100 --sample-size 1 --timer os --color never
```

Use an external 600-second bound per command and report a timeout rather than silently treating partial output as complete. The first is smoke-only; the second has finite samples but no inherent wall-clock timeout. Existing compatibility runtime/measurement commands:

```sh
cargo test --offline -p was-compiler --test resizable_arrays_runtime -- --include-ignored
python tests/support/record_m4_resizable.py target/m4-reproduction
```

The recorder requires a fresh output directory, bounds Cargo children at 600 seconds and Node children at 120 seconds. It reproduces historical M4 accounting/timing, not M5 evidence or M4 release disposition. New M5 commands must be documented exactly once implemented and checked to execute nonzero cases; no invented existing target is prescribed here. Node 24+, Python 3 and cached Rust/tool dependencies are prerequisites. Deadlines are not guest fuel or process-tree cancellation. Finish changed-source checks with `git --no-pager diff --check`; #81 owns integrated gates.

### Suggested execution

Model: `zed.dev/claude-sonnet-4-6`; effort: **medium**, provisional planning recommendation, not an exposed catalog capability. Rationale: a bounded measurement/provenance task using established harness patterns, not compiler algorithm design. Escalate to `openai-subscribed/gpt-6.1-sol` with high provisional effort for ambiguous timing boundaries, accounting interpretation, material unexplained regressions, or proposed compiler/storage changes. Do not decide M4 release acceptance here.
