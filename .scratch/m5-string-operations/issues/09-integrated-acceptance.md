## What to build

Perform final-source integrated M5 validation and write a durable contract-to-test map, measurement-reproduction record and explicit acceptance conclusion. The immutable-slice gate is #76, linear construction evidence is #79, and ordinary measurements are #80; native blocking edges are configured on this issue. Both the immutable-operation slice and fragment-construction slice must pass their own gates before M5 completion.

This is integration/release evidence, not authorization for broader string optimization, automatic fusion, mutable strings, production telemetry, reclamation or disposition of M4's pending performance signal. Preserve historical evidence rather than rewriting earlier subset results as final-source passes.

Essential confirmed surface: positional-only named builtins `string_concat(string, string) -> string`, `string_replace_all(string, string, string) -> string`, `string_slice_bytes(string, number, number) -> string`, `string_byte_len(string) -> number`, and `string_join(string[], string) -> string`. No string `+`, `len(string)` overload, direct builtin pipeline stages, named/default builtin arguments or pure-default/contract whitelist expansion. New names reserve global functions/host-language registrations, not arbitrary external module/field names; lexical shadowing remains allowed. Strings are immutable trusted-host-valid UTF-8 with revision-1 descriptors and retained handles.

Operations evaluate arguments once left-to-right before operation-specific checks, structurally validate in specified order, check constraints/result sizes and allocate only as needed. Replacement is literal, nonoverlapping, left-to-right, does not rescan insertions and traps on empty needle. Slices use finite integral strict half-open byte bounds, accept -0, never clamp, and trap at endpoints inside a Unicode scalar even for empty slices. Join observes the captured array after separator effects, validates separator and complete array storage before entries in index order, and performs sizing/validation before copying without post-argument callbacks. Required validation precedes reuse; traps do not roll back completed effects.

## Acceptance criteria

- [ ] Produce a contract-to-test map naming actual source/forged-tree/independent decoded/runtime/CLI/browser/private-observation tests. Distinguish source inventories, historical results, newly executed results and gaps. Cover skipped/unreachable children, pure-context rejection, name migration/shadowing, UTF-8/numeric boundaries, malformed descriptors, aliases/interned strings, effect/trap order, retained handles, growth/reentry and preset parity.
- [ ] Verify both separately required slice gates on final sources. Initial reuse paths remain validated: empty-operand concat returns the other input; no-match/content-identical replacement and full-range slicing return source; one-entry join returns its entry; zero-byte results may share empty text. Other results initially use new descriptors/copied payloads, including proper nonempty slices. No broader reuse without observable-equivalence/lifetime evidence.
- [ ] Reproduce the user-specified construction criteria: controlled nonoptimized copy-based repeated concat shows expected cumulative copying; accumulation plus one join establishes O(n + B) aggregate storage/copying; final materialization writes fragments/separators directly without intermediate concatenated strings; safe immutable representation reuse is permitted; cover empty fragments/arrays, separators, multibyte UTF-8 and array-growth boundaries; test-only counters categorize string payload allocations/copies separately from array growth/descriptors/overhead; ordinary runtime and native compilation measurements are separate with methodology/environment and no guaranteed speedup.
- [ ] Preserve round-4 accounting: n counts fragment occurrences including empty ones; B counts fragment bytes by occurrence plus output separator occurrences. Attribute retained array-growth allocations, descriptors and new string payloads; count array-handle relocation separately. Report preexisting inputs and fragment-production costs separately from accumulation/join overhead. Separately retained repeated joins are separate constructions. Separate transient storage peaks from completed-call retained storage. No mandatory physical allocation counts, optimized-concat quadratic requirement, constant-retained-memory claim or forced linear-time replacement search.
- [ ] Execute all documented offline correctness/portability/architecture gates sequentially on stable final sources, freshly rebuild browser bindings before browser-dependent tests, and run full workspace tests including ignored cases. Record commands, bounds, exit codes, durations and actual nonzero per-target counts; a timeout or missing prerequisite is an unverified gate, not a pass.
- [ ] Require current-source CLI/browser/core bytes and WAT identity within each preset, diagnostic/host-registration parity and selected/no exports. Enabled presets match each other, not necessarily baseline bytes. Preserve ordinary output bytes/helper ordering for programs without M5 operations except deliberate new-name reservation errors; preserve fixtures rather than regenerate to hide changes.
- [ ] Reproduce deterministic accounting and separate ordinary runtime/native compilation measurements sequentially without competing work. Preserve complete raw samples/logs, environment, exact methodology and executable/corpus/harness/module hashes, revision/dirty state and stable-input checks. Aggregate-suite recorder execution is not isolated timing evidence. Include adversarial repetitive replacement search without asserting a linear-search requirement.
- [ ] Confirm test-only observations cannot activate through production/adapters/integration dependencies; no production telemetry, public controls, implicit host imports, arbitrary guest string cap or enrollment of published strings in M4 recycling. Node encoding remains fixed-storage and decoding copy-based; hosts refresh views and reacquire movable fragment-array payload/length when required.
- [ ] State focused correctness, deterministic construction evidence, performance characterization and integrated M5 acceptance separately. Escalate unexplained material regressions or failed claims rather than silently accepting them. Explicitly preserve M4's pending release decision; M5 completion must not imply M4 performance acceptance.

## Implementation handoff

### Read first

- `AGENTS.md`; `GLOSSARY.md`; `docs/agents/domain.md`; `docs/adr/0010-immutable-string-operations-and-fragment-construction.md`; `docs/was_compiler_memory_and_optimization_roadmap.md` §9. Design documents may still be uncommitted; essential semantics/acceptance criteria are embedded above.
- `docs/validation.md`; `docs/benchmarks.md`; `docs/memory-abi.md`; `docs/resizable-arrays.md`.
- `docs/m4-contract-test-map.md`; `docs/m4-release-readiness.md`; `docs/m4-integrated-validation-2026-10-30.json`; `docs/benchmarks/m4-resizable-arrays-2026-10-29/README.md` for evidence/provenance patterns, not M5 results.
- Final contracts, test inventory and evidence from #73, #74, #75, #76, #77, #78, #79 and #80 before choosing exact M5 focused commands.

### Main verified anchors

- `crates/cli/tests/resizable_arrays_adapters.rs`: core/CLI byte/WAT/export/diagnostic parity and bounded execution patterns.
- `crates/browser/tests/resizable_arrays_adapters.rs`: `browser_input`, `assert_surface`, `rebuilt_browser_resizable_bytes_wat_diagnostics_and_execution_match_native`; a native fixture pass alone does not execute rebuilt browser output.
- `crates/compiler/tests/memory_benchmarks.rs`; `crates/compiler/src/codegen/resizable_measurement_tests.rs`: ordinary isolation, decoded instrumentation checks and recording separation.
- `tests/support/record_m4_resizable.py`: stable-source hashes/fresh evidence directories; `tests/support/process.rs`: bounded children.
- `crates/compiler/benches/compiler.rs`: separate native compiler timing boundaries; `tests/support/m4_resizable_measure.cjs`: ordinary-instance runtime timing.

### Exact validation and bounds

From repository root, run in this order on stable sources with an external 600-second bound per command (the documented #72 precedent), no competing builds/benchmarks:

```sh
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown
cargo test --offline -p was-compiler --test workspace_contract
wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline
node tests/browser.cjs pkg
cargo test --offline --workspace -- --include-ignored
cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings
git --no-pager diff --check
```

Prerequisites: cached dependencies, Node 24+, installed `wasm32-unknown-unknown`, wasm-pack and compatible cached wasm-bindgen tooling; Python 3 for recording. Do not install/fetch tools silently. Existing `pkg/` presence proves no freshness. Default tests omit browser selection/ignored runtimes. Do not use Bacon/watchers as finite agent gates.

Run final M5 focused commands and isolated measurement commands recorded by #76, #79 and #80 before and/or after the full gates as appropriate; these targets are **proposed until implemented**, so none is fabricated here. Existing native benchmark smoke and finite-sample baseline commands are:

```sh
cargo bench --offline -p was-compiler --bench compiler -- --test
cargo bench --offline -p was-compiler --bench compiler -- --sample-count 100 --sample-size 1 --timer os --color never
```

Run separately with an external 600-second bound; the current corpus is not M5 evidence unless extended explicitly. Existing M4 compatibility reproduction is `python tests/support/record_m4_resizable.py target/m4-reproduction` with a fresh directory, 600-second Cargo children and 120-second Node recording children. It neither supplies M5 measurements nor resolves M4 acceptance. All child deadlines are direct-child bounds, not guest fuel/process-tree cancellation.

### Direction, open questions and escalation

Proposed deliverables are an M5 contract map, final-source validation manifest and separate measurement archives; choose actual paths consistently with existing documents. Audit the actual tests, not only historical summary counts. If a gate fails, isolate whether the failure is introduced or unrelated; fix scoped regressions with tests, do not alter unrelated code or regenerate compatibility fixtures to force success. Missing transient accounting, nonlinear construction, incomplete adapter freshness, changed executable inputs or unexplained material regressions block the corresponding conclusion and must be surfaced explicitly.

M4 historical status is not an open decision for this ticket: #72 records all eight execution gates passing, including 1,602 workspace tests with none failed/ignored, while identical-code timings remained materially variable and integrated release acceptance stayed pending explicit performance-signal disposition. Carry that status forward without disposing of it.

### Suggested execution

Model: `openai-subscribed/gpt-6.1-sol`; effort: **high**, provisional planning recommendation, not an exposed catalog capability. Rationale: cross-layer contract audit, stable-source provenance, fresh browser/full-ignored execution and separation of correctness, measurement and release conclusions. Escalate any failed acceptance criterion, material unexplained regression or requested scope/ABI/ownership expansion; never turn partial execution or M5 success into implicit M4 release acceptance.
