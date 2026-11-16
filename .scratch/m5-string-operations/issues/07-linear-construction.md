## What to build

Establish deterministic efficient-construction evidence for implemented fragment accumulation plus one final `string_join`, using #78's private observation seam. Join is #77; native blocking edges are configured on this issue.

Compare repeated immutable concatenation with fixed-size fragment accumulation followed by a single join. Prove the construction/accounting contract, not a guaranteed runtime speedup or constant retained-memory bound. Use existing `string[]` and explicit M4 resizable arrays, not a new mutable builder, concatenation fusion or string reclamation.

Essential join semantics: `string_join(parts: string[], separator: string) -> string` accepts fixed or resizable arrays without requiring resize authority. Evaluate parts then separator once before inspecting the captured array's current contents; separator effects may mutate/resize it, while binding reassignment does not redirect the captured handle. Validate separator and complete array storage, then consumed string handles in index order, even on reuse paths. No callbacks follow argument evaluation. Size/validate first, then copy directly to final storage using widened checked memory32 arithmetic. Unrepresentable sizes trap before result allocation. Inputs remain immutable, valid UTF-8 under the trusted-host contract.

Initial policy: one-entry join returns its validated entry; zero-byte results may share an empty string; other results use a new descriptor/copied payload. Separators occur only between entries: `[]` and `[""]` yield empty text; `["", ""]` joined with `,` yields `,`; `["a", "", "b"]` yields `a,,b`. Required validation precedes reuse. These are initial policies, not permanent physical-layout restrictions.

## Acceptance criteria

- [ ] **Baseline concatenation:** demonstrate expected cumulative copying for a controlled nonoptimized, copy-based repeated-concat baseline over fixed-size fragments. Do not require optimized implementations to allocate/copy quadratically, disable authorized fast paths indiscriminately, or introduce fusion.
- [ ] **Linear construction:** establish O(n + B) aggregate storage and copying for one fragment-accumulation workload through one final join. Count n as fragment occurrences, including empty ones; B as fragment bytes by occurrence plus all output separator occurrences. Use multiple bounded sizes and independent formulas/inequalities, not one favorable sample or pages alone.
- [ ] **No intermediate strings:** when final materialization is required, join writes each fragment/separator occurrence directly into the final result payload without intermediate concatenated strings. Independently decoded lowering plus counted execution establishes the claim; join uses constant auxiliary Wasm locals rather than a second allocated collection.
- [ ] **Storage reuse:** test permitted immutable representation reuse with required validation and lifetime/alias preservation. Acceptance is representation-independent: no mandatory physical allocation counts or assumption that fresh handle identity is guaranteed.
- [ ] **Correctness:** cover empty arrays/fragments, empty/nonempty separators, multibyte UTF-8, duplicate occurrences, and fragment-array geometric growth boundaries; fixed and resizable arrays produce independently expected contents. Include argument-effect/captured-array behavior and retained inputs/results across growth or caught traps.
- [ ] **Instrumentation:** deterministic test-only counters distinguish string payload allocations/copies from array growth, descriptor storage and other overhead. Count array-handle relocation separately from string-payload copying. Attribute retained array-growth allocations, descriptors and newly allocated string payloads to the workload without double-counting physical storage.
- [ ] **Accounting boundary:** report preexisting inputs separately, never charge them repeatedly as new allocation; report fragment-production costs separately from accumulation/join overhead. Repeated joins with separately retained outputs are separate constructions. Report transient storage peaks separately from completed-call retained storage; no constant retained-memory, general reclamation or page-plateau claim.
- [ ] **Performance complement:** identify the separately owned ordinary runtime/native compilation measurements in #80 and link its evidence if already available. Pending timing work does not block this deterministic-evidence ticket; final integrated acceptance requires both. No guaranteed speedup; deterministic linearity is not inferred from timings.
- [ ] Keep observation transport private and prove ordinary-module isolation; no production telemetry or public allocator controls. Preserve byte/helper compatibility for programs without new operations. Do not change or dispose of M4's pending release decision.

## Implementation handoff

### Read first

- `AGENTS.md`; `docs/agents/domain.md`; `GLOSSARY.md`; `docs/adr/0010-immutable-string-operations-and-fragment-construction.md`.
- `docs/was_compiler_memory_and_optimization_roadmap.md` §9: user-specified efficient-construction criteria and round-4 accounting. These design documents may still be uncommitted; the operative criteria are embedded above.
- #77's actual join implementation/evidence and #78's counter definitions before selecting instrumentation sites.
- `docs/resizable-arrays.md`; `docs/benchmarks/m4-resizable-arrays-2026-10-29/README.md`; `docs/benchmarks.md`; `docs/validation.md`; `docs/m4-release-readiness.md`.

### Main verified anchors

- `crates/compiler/src/codegen/resizable_measurement_tests.rs`: `modules`, `observe`, isolation/decoded increment tests, `record_m4_accounting`.
- `tests/support/m4_resizable_measure.cjs`: `accountingSession`, `operation`, `accounting`; independent content/registry oracles and geometric relocation checks. Current copied-byte metric is inferred array copying, not string copying, and peaks exclude intra-call residency.
- `tests/support/m4_resizable_corpus.was`; `tests/support/record_m4_resizable.py`: deterministic fixture/provenance patterns.
- `crates/compiler/tests/memory_benchmarks.rs`: deterministic compilation/preset/public-surface checks.
- `tests/support/process.rs`: bounded child execution.

### Direction and open questions

Build a small deterministic size family first, then add empty/multibyte/growth boundaries. Keep independent expected formulas and decoded direct-copy assertions separate from production constants/algorithms. New M5 corpus/harness/tests and their commands are **proposed**, not existing targets. Settle how baseline copy semantics are controlled, what is preexisting versus produced inside a workload, and how transient peaks are observed with #78's actual transport. Broader reuse needs equivalence for handles, allocation failure, growth, trap order and lifetime safety; equal contents alone are insufficient. Escalate a nonlinear accounting result, missing attribution, inability to exclude intermediate strings, or any requested representation/ownership expansion rather than weakening the claim.

### Exact validation and bounds

Run from repository root, offline, sequentially. Existing compatibility reproduction, not an M5 proof:

```sh
python tests/support/record_m4_resizable.py target/m4-reproduction
```

Choose a fresh directory; the recorder refuses existing JSON/log evidence. Each Cargo child has a 600-second bound and Node recording children have 120-second bounds. No competing build/benchmark runs. The recorder does not measure compiler time.

Existing focused/native checks, each with an external 600-second command bound:

```sh
cargo test --offline -p was-compiler --lib codegen::resizable::measurement_tests -- --skip record_m4 --test-threads=1
cargo test --offline -p was-compiler --test resizable_arrays_runtime -- --include-ignored
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
git --no-pager diff --check
```

Record exact nonzero M5 focused/recording commands after those tests exist; no current M5 target is asserted here. Use finite guest workloads and explicit Node deadlines, not unbounded loops. Cached Node 24+/Python/Rust dependencies only. Subprocess deadlines are not guest fuel or process-tree cancellation. Full integrated gates belong to #81.

### Suggested execution

Model: `openai-subscribed/gpt-6.1-sol`; effort: **high**, provisional planning recommendation, not an exposed catalog capability. Rationale: independent algorithmic/accounting evidence across string and M4 storage boundaries, including transient versus retained attribution. Escalate failed linearity, unclear baseline semantics, or new production instrumentation needs before widening scope.
