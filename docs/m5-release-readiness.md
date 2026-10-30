# M5 strings — integrated validation and release readiness (#81)

## Decision — 2026-10-30

**Both slice correctness gates, deterministic construction, portability,
architecture, fresh-browser compatibility and measurement execution passed on
final executable sources. Integrated M5 release acceptance remains pending
explicit disposition of material unexplained timing variability.** Successful
recording is not performance approval. No changed-code regression was identified;
there is no basis here for a speculative implementation fix or optimization.

The five positional-only builtins are implemented: `string_byte_len`,
`string_concat`, `string_slice_bytes`, `string_replace_all` and `string_join`.
The [contract-to-test map](m5-contract-test-map.md) names **99 actual Rust tests**
at source, forged-tree, independent decoded, runtime, CLI/browser, private
observation and ordinary-measurement seams. That is source inventory, not an
execution count or 99 independent scenarios. The [implemented contracts](string-operations.md),
[revision-1 ABI](memory-abi.md) and [roadmap §9](was_compiler_memory_and_optimization_roadmap.md#9-milestone-m5--string-operations-and-efficient-construction)
retain their respective implementation/design roles. The separately owned
local/unpublished `docs/adr/0010-immutable-string-operations-and-fragment-construction.md`
and other design edits were preserved; historical proposed/pending wording is not final-source
evidence. Historical #73–#80 records are not rewritten as integrated passes.

**M4's pending performance-signal disposition remains unchanged.** M5 uses M4
fragment arrays but neither approves M4 performance nor changes its release decision.
See [M4 release readiness](m4-release-readiness.md).

## Stable-source provenance

Implementation revision: `23441c9c27d9eae9d1a227ad12bba7ecab689217` (#80).
The task started with existing changes to `GLOSSARY.md`, the memory roadmap,
ADR 0010 and `.scratch/m5-string-operations/`; those were not enrolled in this
change. All task changes are documentation/evidence. Compiler, adapter, runtime,
test, manifest, lockfile and golden fixture sources remain unchanged. No new test
seam or regression fix was needed; no new red/green implementation is claimed.

The [execution manifest](m5-integrated-validation-2026-10-30.json) records exact
argv, start times, 600-second top-level bounds, durations, exit codes, full raw
stdout/stderr hashes and actual per-target counts. It captures **240 executable
inputs**, including binary/text compatibility fixtures, with a complete
[ZIP snapshot](benchmarks/m5-integrated-81/inputs.zip). Eighteen stable-input
checks passed after each command and after all recordings/gates. Test targets are
paired with Cargo's stderr headings in order; empty unit/doc targets remain
visible rather than being counted as execution. All six freshly rebuilt `pkg/`
artifacts are hashed. Generated bindings are not committed.

[Complete archives and measurement reconciliation](benchmarks/m5-integrated-81/README.md)
retain two deterministic recordings, three ordinary timing recordings, all raw
samples/logs, corpus/module bytes, environment and source/executable hashes.
Local `.gitattributes` preserve evidence bytes without checkout normalization;
raw terminal blank lines are allowed only in these evidence logs. Manifest hashes
cover data artifacts recorded at execution completion, not subsequently written
narrative READMEs or the manifest itself. The separately hashed
[one-off execution driver](benchmarks/m5-integrated-81/record_integrated.py.txt)
is an archival copy of `target/record_m5_integrated_81.py`, not an installed tool.
To replay that driver, restore it at that repository-relative path and choose
fresh output/manifest/recording paths; its root calculation assumes that location.
The unchanged #79/#80 recorders remain the normal measurement reproduction entry points.

Environment: Windows x64 `10.0.26300`, Intel Core Ultra 5 125H (18 logical CPUs),
Node `24.12.0`, V8 `13.6.233.17-node.37`, rustc/Cargo `1.98.0`, Python `3.13.14`,
wasm-pack `0.15.0`, installed `wasm32-unknown-unknown`. Dependencies and compatible
wasm-bindgen tooling were cached. Browser build used `--mode no-install --offline`;
no dependency/tool installation command ran. Its optional package metadata notice
is not a failure. Processor overrides are recorded; no relevant Cargo/Rust/Node/
Divan override was present. A pre-run process snapshot found no Cargo/rustc/rustdoc
process. No competing task build/benchmark ran, and no fresh log reports a build-lock
wait. Node background processes existed; system idleness, thermal/power state and
engine tiers were not continuously controlled or measured.

## Newly executed slice and construction gates

Before the final gate sequence, these commands ran sequentially, each with a
600-second outer bound and exit code **0**:

| Command | Actual result | Seconds |
| --- | --- | ---: |
| `python -B -m unittest discover -s tests/support -p test_record_m5_strings.py` | **5 passed**, including artifact-path/stale-CLI regression | 0.482 |
| `cargo test --offline -p was-compiler --test string_operations_frontend --test string_operations_lowering --test string_operations_runtime -- --include-ignored` | **16 + 24 + 27 = 67 passed**, zero failed/ignored; immutable-operation and join gates | 7.064 |
| `cargo test --offline -p was-compiler-cli --test string_operations_adapters -- --include-ignored` | **9 passed**, zero failed/ignored | 23.248 |
| `python tests/support/record_m5_construction.py target/m5-integrated-81-construction-1` | **6 + 3 + 2 focused tests passed**, 103 construction rows and 34 concat baselines | 2.952 |
| `python tests/support/record_m5_construction.py target/m5-integrated-81-construction-2` | Same nonzero counts, identical complete accounting and modules | 4.087 |
| `python tests/support/record_m5_strings.py target/m5-integrated-81-timing-1` | 2 runtime + 5 native correctness tests, 42-case smoke, 630 native + 1,845 runtime raw samples | 6.933 |
| `python tests/support/record_m5_strings.py target/m5-integrated-81-timing-2` | Same nonzero counts and complete samples | 7.282 |
| `python tests/support/record_m5_strings.py target/m5-integrated-81-timing-3` | Same nonzero counts and complete samples | 6.406 |

The unchanged construction recorder records child exit codes and test logs, not
individual child wall-clock durations; the manifest supplies total recorder
durations. Timing recorders also supply individual child durations. Their Cargo
children have 600-second bounds; construction accounting/ordinary Node timing
children have 120-second bounds. Earlier paired tracers keep their shorter bounds.
These are direct-child deadlines, not guest fuel or process-tree cancellation.

## Final-source integrated gates

Immediately after the recordings, these commands ran **in this exact order**,
with no competing task builds/benchmarks or executable-source edits. Every command
had a 600-second outer subprocess bound, exited **0**, and required no retry.

| Command | Actual result | Seconds |
| --- | --- | ---: |
| `cargo test --offline` | **1,359 passed, 0 failed, 322 intentionally ignored**, 80 result targets including empty targets; default compiler/CLI members | 68.868 |
| `cargo clippy --offline --all-targets -- -D warnings` | Strict native default-member all-target check passed | 0.875 |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Portable production core passed | 0.725 |
| `cargo test --offline -p was-compiler --test workspace_contract` | **3 passed**, adapter-free graph, standalone consumer and shared assets | 3.304 |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Fresh current-source browser rebuild, before dependent execution | 19.333 |
| `node tests/browser.cjs pkg` | **25 smoke groups passed** | 1.035 |
| `cargo test --offline --workspace -- --include-ignored` | **1,701 passed, 0 failed, 0 ignored**, 93 result targets including empty unit/doc targets | 248.875 |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Strict browser Wasm library check passed | 3.808 |
| `git --no-pager diff --check` | Whitespace passed | 0.061 |

The full workspace run independently executes the **16 frontend, 24 checked/
decoded, 27 runtime, 9 CLI and 5 browser** string tests, plus all **11 private**
observation/construction and **7 ordinary measurement-correctness** tests. Browser
adapters actually execute rebuilt browser output, not only native fixtures. The
same run covers M2/M3/M4 noninterference and ABI/emitter/diagnostic/numeric regression
fixtures. Recorders invoked by the aggregate suite are **not isolated timing
evidence**; the earlier three sequential recordings alone supply fresh timing.

Both required slices now have final-source correctness/adapter evidence:

- **Immutable operations:** strict UTF-8 byte endpoints (including empty interior
  slices), finite integral half-open bounds/-0, literal nonoverlap/no-rescan
  replacement, ordered structural checks before reuse, aliases/interned/array-held
  values, retained handles, traps, recursion, reentry and real memory growth.
- **Fragment construction:** fixed/resizable arrays, capture before separator
  effects, current payload/length after effects, separator/outer/entry validation
  order, overflow checks, singleton reuse and direct final copying with constant
  locals. Private counted execution and independent decoding complement Validator.

Within each preset, core/CLI/fresh browser bytes, WAT, diagnostics, host naming,
selected/no exports and deterministic output match. Enabled presets match one
another, not necessarily the baseline. All five reserved language names allow
lexical shadowing; arbitrary nonempty external module/field names remain allowed.
No string `+`, `len(string)`, named/default builtin arguments, direct builtin stages
or pure-whitelist expansion is added. Unreachable/skipped checks remain enabled.
Absent-M5 helper/byte compatibility has representative decoded/golden coverage;
no fixtures were regenerated and no universal old-program equivalence proof is claimed.

## Construction and performance conclusions

**Deterministic construction passed.** All 103 rows and 34 baseline accounting
records reproduce #79 byte-for-byte. The observed/ordinary modules are respectively
5,580/4,966 bytes with the same recorded historical hashes. Historical #79 stored
hashes rather than the four module files; those absent files cannot be rehashed.
Fresh modules and all samples are now archived. The [reconciliation](benchmarks/m5-integrated-81/README.md)
records exact identities and complete comparisons.

For one accumulation followed by one join, `n` counts occurrences including empty
fragments, and `B` counts fragment bytes by occurrence plus separator occurrences.
At n=1,025 one-byte fragments with empty separator: join copies **1,025 string
bytes + 8,184 handle-relocation bytes** and reserves **17,824 bytes**; controlled
None-preset repeated concat copies **525,824 string bytes** and reserves **537,600
bytes**. The bounded size family satisfies O(n + B) aggregate guest storage/copying;
this is not an O(n + B) time claim. Decoded loops and actual byte-store counters
establish direct final-payload writing without intermediate concatenated strings.

Accounting separates preexisting input unions, fragment production, stable/source
array descriptors, authority/allocator records, retained active/free capacities,
new string descriptors/payloads and padding without double counting. Handle
relocation is not string copying. A separately retained second join is a separate
construction. At 1,025 entries, completed active capacity is **8,192 bytes** versus
**12,288 bytes** transient old-plus-new capacity. Bump-retained storage, active
capacity, engine RSS and distinct category peaks are not interchangeable or summed
into an invented peak. No constant-retained-memory or reclamation claim follows.

**Performance characterization executed; acceptance remains pending.** All **233
#80 timing input hashes**, both native executable hashes and all corpus modules
match historical #80. Each new run contains all 630 native/1,845 runtime samples,
including adversarial repetitive replacement. Native source/prepared-typed compilation
(including output destruction) remains separate from ordinary fresh-instance Node
runtime (including JS loops/crossings and guest validation/allocation/copy/growth).
Setup/oracles/decoding and module compilation/instantiation remain outside timing;
no private counter module is timed. No linear-time replacement search is required.

Across the three fresh runs, each case's maximum/minimum median ratio spans
**1.0113–2.0974× native** and **1.0129–2.5294× runtime**. Across all nine fresh versus
historical #80 comparisons, per-case ratios span **0.3685–2.1845× native** and
**0.2143–2.2632× runtime**. The complete matrix and raw distributions retain slow
and fast cases alike. There is no changed-code experiment or measured causal
explanation, stable budget, statistical-significance proof or universal speedup.

This is the explicit escalation required by #81: **do not mark integrated M5
release acceptance complete until the material-variable signal receives an
explicit acceptance/disposition decision or further controlled investigation.**
Passing correctness and deterministic construction are separate positive results;
M4 performance acceptance is neither required to be silently granted nor disposed
of here. No arbitrary timing threshold or favorable-run selection substitutes
for that decision.

## Coverage limits and retained boundaries

The [map's gaps](m5-contract-test-map.md#gaps-limits-and-parent-owned-closure)
remain explicit: no dedicated allocator-exhaustion runtime for slice/replace/join;
forged skipped/unreachable matrices and historical byte compatibility are bounded;
operation-specific private payload counters cover concat/join rather than every
string operation. Overflow tests are not successful ceiling allocations or general
exhaustion evidence. No failing correctness claim was found in the approved scope.

Observations stay private `cfg(test)` fixtures and cannot activate through ordinary
production/adapters/integration dependencies. No production telemetry, public
controls, implicit imports, arbitrary guest string cap or M4 recycling of published
strings is introduced. Strings retain immutable trusted-host-valid UTF-8 and
instance lifetime; structural checks do not scan UTF-8. Node encoding stays
fixed-storage and decoding copy-based. Hosts refresh views after growth and
separately reacquire movable fragment-array payload **and length** after possible
resize/reentry, including completed effects before traps. There is no rollback,
mutable text, automatic fusion, new builder/ownership protocol or general reclamation.

## Review and completion

Independent Standards and Spec reviews used the staged task-only diff against
`23441c9c27d9eae9d1a227ad12bba7ecab689217`, excluding preexisting user design/scratch
work. Standards found one low-priority documentation issue: two new ADR links
would not resolve in a checkout without the unpublished design file. Both were
replaced with explicit local/unpublished references; re-review confirmed resolution.
Spec found no actionable findings. **No outstanding findings remain on either axis.**
Reviewers independently reconciled test names/counts, raw timing summaries/ratios,
construction records and evidence hashes, but did not rerun builds or measurements.
The release conclusion remains pending timing-signal disposition; completed review
is not an implicit performance acceptance or closure of that decision.
