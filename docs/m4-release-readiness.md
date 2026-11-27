# M4 resizable arrays — integrated validation and release readiness (#72)

## Current performance decision — 2026-11-27

The [official Linux baseline](benchmarks/m4-m5-aa-2026-11-27/baseline.md)
replaces Windows timing for M4 performance evaluation. **The historical timing
signal no longer blocks M4 acceptance.** Existing correctness and deterministic
accounting evidence retain their original scope; this is not a timing guarantee
or an explanation of the historical variability.

## Historical decision — 2026-10-30

**Correctness, portability, architecture, adapter and measurement-execution gates
passed. Integrated M4 release acceptance remains pending explicit disposition of
the material timing signal below.** No implementation regression or contract conflict
was identified. Passing correctness does not resolve unexplained performance costs.

The approved constructor/reserve/push/pop/remove subset is implemented for
`number[]`, `bool[]` and `string[]`. The [contract-to-test map](m4-contract-test-map.md)
accounts for 134 direct M4 tests and shared compatibility/noninterference coverage.
[Roadmap §8](was_compiler_memory_and_optimization_roadmap.md#8-milestone-m4--resizable-typed-arrays),
[ADR 0009](adr/0009-opt-in-resizable-array-storage.md) and
[operation contracts](resizable-arrays.md) remain authoritative for semantics.
Historical #65–#71 records retain their original scope; #68's timeout is not
retrospectively a pass. The fresh completed runs here independently close those
execution prerequisites, not the pending performance decision.

## Stable sources and durable evidence

Implementation revision: `52a28557cb41652581ed247e045ca27a6a5ed75f` (#71).
The task started with a clean working tree. Documentation edits began before the
recorded gate sequence; the recorded dirty state lists those documentation paths.
No compiler, adapter, test, runtime, manifest, lockfile or compatibility fixture was
changed. No new test seam or regression fix was necessary, so no red/green feature
implementation is claimed.

The [validation manifest](m4-integrated-validation-2026-10-30.json) records:

- Exact commands, exit codes, ten-minute command bounds, durations and per-target
  test summaries; full raw gate-log SHA-256 hashes.
- SHA-256 hashes for **220 tracked executable inputs** under `crates/`, `tests/`,
  `runtime/` and root Cargo manifest/lockfile, checked unchanged after gates and
  after both measurement runs. This includes compiler/adapters, shared assets and
  compatibility fixtures, not documentation or generated output.
- Starting/final gate dirty states, tool versions, platform, selected environment
  overrides and freshly rebuilt `pkg/` hashes. Generated package files are not
  committed or evidence of freshness by their presence alone.
- Hashes of archived gate logs and both complete measurement recordings, including
  separate local working-tree and committed Git-blob bytes where checkout newline
  normalization changes them.

[Archived gate logs](benchmarks/m4-resizable-arrays-2026-10-29/integrated-72/gates/)
retain complete output except terminal blank lines removed for Git whitespace
hygiene; the manifest distinguishes raw and archived hashes. Complete raw logs
also remain locally in `target/m4-integrated-72/`. Measurement files were copied
without modification into the working tree; Git normalizes CRLF to LF for each
`environment.json` and `workload.was`, so their committed-byte hashes differ. The
manifest records both forms. Checkouts may again change line endings; compare the
appropriate bytes. [The reproduction record](benchmarks/m4-resizable-arrays-2026-10-29/integrated-72/README.md)
links both runs and all prerequisites. #71's historical samples were not overwritten.

Environment: Windows x64 `10.0.26300`, Intel Core Ultra 5 125H, Node `24.12.0`,
V8 `13.6.233.17-node.37`, rustc/Cargo `1.98.0`, Python `3.13.14`, wasm-pack
`0.15.0`. The lockfile selects wasm-bindgen `0.2.129`; its CLI was not on PATH.
The browser rebuild succeeded with cached compatible tooling under
`--mode no-install --offline`, despite wasm-pack's usual “Installing wasm-bindgen”
status message. No separate install/fetch command ran. The only build notice was
optional package description/repository/license metadata.

## Fresh integrated gates

Commands ran **in this order**, from the repository root, without competing build
or benchmark commands. Each had a 600-second subprocess bound; the overall tool
invocation had a 1,200-second bound and completed. Every command exited **0**;
none timed out or required a retry. Counts below are executed test functions,
not source inventory or sums of historical results.

| Command | Actual outcome |
| --- | --- |
| `cargo test --offline` | **1,301 passed, 0 failed, 286 intentionally ignored**, across 74 test-result targets, including an empty doctest target; default compiler/CLI members only |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed strict native default-member all-target check |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed portable production-core build |
| `cargo test --offline -p was-compiler --test workspace_contract` | **3 passed**, 0 failed/ignored; adapter-free production graph, standalone downstream consumer, shared assets |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed current-source browser rebuild, before dependent tests |
| `node tests/browser.cjs pkg` | **25 smoke groups passed** on rebuilt package |
| `cargo test --offline --workspace -- --include-ignored` | **1,602 passed, 0 failed, 0 ignored**, across 86 test-result targets, including empty targets/doc-tests; compiler, CLI and browser selected |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed strict browser Wasm-target library check |

The full ignored workspace run executes all 46 M4 frontend, 33 forged/decoded
lowering, 38 bounded runtime, 5 private failure, 5 private measurement, 1 inline
helper-planning, 4 CLI and 2 browser tests. It also executes the private M2/M3
suites and ordinary ABI/emitter/diagnostic, typed-array, capture and numeric-preset
regressions. Aggregate-suite measurement recorders are **not isolated timing**;
only the later sequential recordings supply measurement evidence.

The [map](m4-contract-test-map.md) names the actual tests for authority/copy identity,
contextual and placeholder typing, purity, fixed-storage rejection, once-only
argument order, pending ordinary/all-five-compound writes, displaced-buffer reuse,
recursion/reentry/caught traps, post-RHS mismatch recovery, shrinking/shifting
iteration and consumed-element checks. Dedicated adapter tests validate within-level
bytes/WAT/diagnostics and selected/none exports on fresh browser output. Enabled
numeric presets must match each other, not baseline bytes indiscriminately.

Shared import-snapshot mutation tests and M4 ordered-import/export tests provide
compositional coverage; there is no dedicated M4 analyze/registry-mutate/recompile
scenario. Historical golden compatibility is representative, not an exhaustive
proof for every old program. No fixtures were regenerated. The intentional
`resizable` global/host-language-name reservation remains a documented migration;
lexical shadowing and arbitrary host module/field names remain supported.

## Reproduced measurements and escalation

After all gates, the unmodified #71 recorder ran twice, sequentially, with fresh
output directories. Each run passed **38 runtime**, **5 M3 measurement compatibility**,
**3 M4 isolation** tests, then exactly **one** release-profile accounting test and
**one** release-profile uninstrumented guest-timing test. All ten child commands
exited 0. Each Cargo child had a 600-second recorder bound; Node children have
120-second bounds. These are direct-child deadlines, not guest fuel or process-tree
cancellation. No concurrent build/benchmark command was launched by this task.

Both runs' **97 executable measurement input hashes** match #71 exactly. The
ordinary **6,126-byte** and observed **6,305-byte** modules also match #71 exactly.
Both full accounting `results` reproduce #71 identically, including **1,233 mixed
operations**, the three exact-class-stranding sequences, all values/records checked
after each call, and geometric copy-volume bounds. The final mixed workload has
3,809 logical slot bytes, 14,976 active capacity bytes, 560 free payload bytes,
768 allocator-record bytes, 720 stable descriptor/authority bytes and 17,192 bump
growth bytes. No page plateau or general memory bound follows.

Each run retains **180 raw timing samples** on fresh ordinary uninstrumented
instances. The first run's medians are **1.82–2.97× #71**. A second full sequential
recording substantially reduces those medians: number append is **0.85× #71**;
other cases remain **1.13–1.44× #71**. Exact medians and all raw samples are in
[the measurement comparison](benchmarks/m4-resizable-arrays-2026-10-29/integrated-72/README.md).

This is a material, variable signal on **identical source/module bytes and reported
tool versions**. The repeat establishes that first-run cost is not stable; it does
not explain the residual difference from #71. No power/thermal state, scheduler
load, engine tier stability or statistical-significance evidence was collected.
There is no matched changed-code experiment or identified compiler regression to
fix. These samples include JS loops/crossings and runtime work, not isolated Wasm
latency or compiler timing. Neither run alone established suitability. At the
time, #72 left acceptance pending explicit disposition or further controlled
measurement; see the [current performance decision](#current-performance-decision--2026-11-27).

## Controlled timing follow-up — 2026-11-27

The [three-session A/A investigation](benchmarks/m4-m5-aa-2026-11-27/README.md)
reproduced the module and accounting results and observed up to 32.7% median
variation within one pair of identical artifacts. The historical timing baseline
is not comparable to the Linux replay. Future comparisons use fresh matched
runs against the official Linux reference.

## Retained contracts and non-authorizations

- Ordinary literal/host-encoded arrays retain fixed storage and their existing host
  payload lifetime rights. Only explicit fresh `resizable(source)` copies receive
  private runtime resize authority; valid descriptors alone do not grant it.
- Stable handles and the eight-byte descriptor remain. Hosts reacquire opt-in
  payload pointers **and lengths** after possible resize/reentry, including trapped
  calls with completed effects, separately from refreshing views after growth.
  Node encoding stays fixed-storage; decoding is an explicit copy of live elements.
- Pending movable writes validate before RHS effects and resolve current storage
  afterward, preserving numeric old values. A removed index can cause a post-RHS
  trap. Fixed writes keep their saved-address semantics; traps do not roll back.
- Per-acquisition capacity is bounded by the allocator's **2^31-byte** domain;
  reserve sizing/overflow, authority, identity exhaustion, empty pop and invalid
  remove checks may trap. No exact reserve-capacity or successful ceiling allocation
  is promised. Pop/remove retain capacity; no capacity shrinking/query is added.
- Recycling covers displaced owned payloads only, after replacement publication.
  Stable descriptors/authority, unreachable arrays and their current payloads,
  immutable referenced strings and successful unpublished allocations after failed
  construction remain retained. Exact-class fragmentation and linear registry
  searches remain material; completed-call peaks omit transient growth residency.
- This record authorizes no M2 migration, ordinary-array management, host ownership
  registration, general free/GC/RC, heap rewind, page return, public allocator
  controls, new default, broader optimization or unmeasured performance claim.

## Review and completion

Independent Standards and Spec reviews compared the documentation/evidence diff
against the task-start revision. Standards found one archive-integrity issue: four
working-tree hashes differed from Git-normalized blob bytes. The manifest now
records both forms; re-review verified all 18 measurement-file hashes in each form.
Spec found one incorrect repeat timing-ratio range; the summaries were corrected
to 1.13–1.44× and re-reviewed. **No outstanding findings remain on either axis.**
Reviewers reconciled test counts, source/module/accounting hashes, timing medians,
contract names and links from recorded evidence; they did not witness sequential
execution or rerun builds/tests/measurements. Final checks resolved 576 local
link paths, verified all 220 executable-input hashes and all 18 committed
measurement-file hashes, and passed staged/unstaged Git whitespace checks. That
review did not resolve the historical timing signal.
