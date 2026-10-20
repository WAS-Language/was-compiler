# Issue #41: matched comparisons/negation evidence — 2026-10-20

**Experimental measurements, not release/default/regression acceptance.** Current
`Basic` demonstrates cumulative module-size savings, but compilation costs rise
and guest timings include substantial regression signals and reversals. Concurrent
build/test work was permitted on this machine: these are not idle-machine timings
or an attribution of timing changes solely to #41.

## Scope and measured identity

This compares **current None versus current Basic**, cumulatively including:

- #39 parsed exact-i32 literals and proven add/sub intermediates;
- #40 eligible never-reassigned ordinary numeric local storage/read proofs;
- #41 all six numeric comparisons (`< <= > >= == !=`) when both operands are
  proven signed integers, and unary negation when operand bounds exclude zero
  and `i32::MIN`. Negation results compose with add/sub and eligible local proofs.

The public `number` type and function/host ABI remain f64. Unknown parameters,
calls, mutable/captured reads and memory loads do not acquire proofs. Unsupported
arithmetic still falls back; eligible children may specialize independently.
Zero must fall back to preserve negative zero; minimum must fall back to avoid
signed-i32 overflow. No guards, folding, branch refinement or default change is
claimed. The global integer-specialization docs still describe #39/#40 at the time
of collection; this owned directory records the implemented #41 working tree
without editing those documents.

Measured dirty-tree HEAD: `839899af3a966a9eb9cc109380e028e0a9edaef8`, not a
pristine-commit measurement. See [revision](collection-1/git-head.txt),
[initial status](collection-1/git-status-before.txt),
[losslessly compressed binary tracked diff](collection-1/git-diff-before.txt.gz) and
[final collection status](collection-1/git-status-after.txt). Untracked file
contents are not included in Git's tracked diff. Main collection ran
**17:05:39–17:06:16 UTC**; successful expression supplement finished
**17:10:53 UTC** on 2026-10-20. Per-command UTCs are authoritative.

Production and historical measurement inputs were unchanged before/after every
recorded measurement command and across both successful collections; the
inspection also matches the timed hashes. See
[main stability](collection-1/stability.json),
[supplement stability](expressions-2/stability.json),
[inspection stability](inspection/stability.json) and the original collection-final
[audit](audit.json). The later integration reconciliation below separately compares
current files without changing those immutable records.
The production group includes all adapter/compiler `src/**/*.rs` (including inline
unit tests), runtime and Cargo manifests/lockfile. Other owners' unrelated test/doc
edits are not all frozen by that group. Stable hashes do **not** establish an idle
CPU or controlled power/thermal state.

All task-authored evidence/scripts are confined here. No production files, shared
tests, historical evidence or global docs were edited; no staging/commit or full
validation was performed. Cargo generated caches under root `target/`.

## Final integration reconciliation — 2026-10-20

At **17:24:26 UTC**, the current **78 production and seven historical measurement
file hashes** match all seven reference snapshots: main before/after, successful
expression supplement before/after, CLI inspection before/after, and the original
`final-source-state.json`. See the new
[final-integration-source-state.json](final-integration-source-state.json), which
records full current hashes, reference-manifest hashes and per-reference differences:
**`changed_inputs: []` throughout**.

The integration owner reported running rustfmt on `expression.rs` and
`representation.rs` after the original audit. Comparing actual bytes finds **no
observable change**, not even a formatting-only hash delta, in either file:

| Production file | Timed SHA-256 | Current integration SHA-256 |
| --- | --- | --- |
| `crates/compiler/src/codegen/expression.rs` | `75570251a7401ccb31dd4ed2655e92cabdfc9f19cf47acbfef35967d53893284` | `75570251a7401ccb31dd4ed2655e92cabdfc9f19cf47acbfef35967d53893284` |
| `crates/compiler/src/codegen/representation.rs` | `b7f72be04cd8bf0aa26c87b5f995305754b20d4979f9f259af40824fd8dda734` | `b7f72be04cd8bf0aa26c87b5f995305754b20d4979f9f259af40824fd8dda734` |

There are therefore **zero changed production files to classify**. The reported
rustfmt invocation is consistent with an idempotent run; this reconciliation does
not claim an unobserved source edit. Timed and current source identity coincide,
but the saved timings remain the original measurements and their original machine
conditions, not a new run. **No timings, builds or compiler tests were rerun**.

Before updating this README, every artifact covered by the original
`artifact-hashes.json` still matched, including the original source snapshot and
`audit.json`. The owner's attempted rerun of `audit.py` stopped at its create-new
`final-source-state.json` destination; the artifact comparison finds no resulting
evidence changes. The old audit is a historical collection-final audit, not an
in-place updatable integration report. It was **not rerun by this follow-up**.

The requested README reconciliation is the sole intentionally changed artifact
covered by that old manifest. Original raw evidence, original `audit.json`,
`final-source-state.json` and `artifact-hashes.json` remain unchanged. New
[final-integration-audit.json](final-integration-audit.json) records the exception
with old/new README hashes; new
[final-integration-artifact-hashes.json](final-integration-artifact-hashes.json)
covers the reconciled directory, including the original manifest and all new
records, except itself. Use the old manifest for the pre-reconciliation README
identity and the new manifest for its current identity.

**Integration validation status, reported by the owner, not independently run here:**
global docs are being updated outside this directory. A full workspace attempt
stopped at the preexisting fixed-byte enabled-negation oracle in
`tests/block_values.rs`; the owner reports updating only enabled expected bytes
for #41 while preserving the O0 fixed oracle. The full workspace rerun is **pending
at this handoff**, not passed. These test/doc edits are outside the recorder's
production/historical measurement hash groups; unchanged hashes do not assert
that all repository files are unchanged. No release/default/regression acceptance
follows from this reconciliation.

Append-only follow-up commands, from repository root:

```sh
python -B docs/benchmarks/integer-comparisons-negation-41/reconcile_integration.py snapshot
# Update only this README, then create the new audit/manifest:
python -B docs/benchmarks/integer-comparisons-negation-41/reconcile_integration.py audit
```

The script rejects existing destinations. A later integration snapshot needs new
filenames rather than overwriting either generation of evidence.

## Final-commit packaging reconciliation — 2026-10-20

The raw tracked-diff snapshot is now stored as
[collection-1/git-diff-before.txt.gz](collection-1/git-diff-before.txt.gz), replacing
only its plain `.txt` representation. Its eight space-only Git diff context lines
are faithful raw output, not malformed measurement data; lossless compression
avoids treating those payload lines as trailing whitespace in the staged patch.
Gzip uses **mtime 0**, an empty embedded filename and compression level 9. The
compressed stream is verified against its original bytes before deleting the
plain file, and deterministic recompression is checked byte-for-byte.

**Historical command metadata, audits and artifact manifests remain unchanged.**
They describe the original **decompressed** path
`collection-1/git-diff-before.txt`, byte length and SHA-256. Decompressing the new
`.gz` must recover those exact raw bytes and digest; no context whitespace, line
endings, measurements or timings were normalized or edited. Old manifests and
snapshots remain historical, not current on-disk packaging inventories. In
particular, do not rerun the old create-new audits against this packaged directory.

New [packaging-reconciliation.json](packaging-reconciliation.json) records the
path replacement, original raw and gzip digests, exact verification, and retained
artifact comparisons against both earlier manifests. Apart from this explicitly
repackaged raw file, **README.md is the only retained original artifact whose hash
changes**. New
[final-commit-artifact-hashes.json](final-commit-artifact-hashes.json) inventories
the packaged directory, including the original immutable manifests and all new
records, excluding itself. [package_evidence.py](package_evidence.py) records the
operation reproducibly and refuses existing output files; it does not stage or
commit. Historical collection/reconciliation scripts remain unchanged.

**Later integration status, reported by the owner:** the final full workspace
suite passed **1,287 tests across 81 targets**; two-axis review completed with no
hard findings and one low-priority duplication suggestion deferred. See the
integrator's [#41 review completion](../../integer-specialization.md#review-completion-41).
The earlier “pending” integration paragraph above is its historical snapshot, not
the latest owner-reported status. This packaging task did not rerun those checks,
benchmarks or timings, and makes no release/default/regression acceptance claim.

**Staging caution:** `git diff --cached --check` continues to inspect the old staged
snapshot until the integrator stages the plain-file deletion, gzip addition,
README update and new packaging records. No index changes are made by this task.

## Protocol, tooling and commands

The unchanged 27-case corpus and #39 recorder were reused through the
[destination-only launcher](collect.py), following #40's pattern. Only the exact
historical output-location assertion is redirected to the new `collection-1`;
root/new-directory safety, command order, hashes, deadlines, oracle checks, parsers
and summaries remain original. [Launcher metadata](launcher.json) captures both
script hashes and the redirection. Both original recorders remain unchanged.

**Inherited stale narrative:** raw `collection-1/summary.md`, `summary.json` and
`environment.json` retain the original #39 heading and “Locals remain f64” text.
Those descriptions are false for this newer tree and are superseded by this README
and [results.md](results.md). Raw generated evidence is deliberately not rewritten.

Both collections run two sequential matched repetitions: repetition 1 **None →
Basic**, repetition 2 **Basic → None**, for compiler then guest measurements.
There were no benchmarks launched in parallel by this task. Another owner was
allowed to run builds/tests concurrently; exact overlap, CPU contention, power
and thermals were not measured or controlled. Cargo lock/build delays and startup
are outside measured inner loops but may still affect subsequent machine state.

- Compiler: optimized Cargo bench build; 100 Divan samples × one compilation per
  case, separately source-to-Wasm and typed-to-Wasm. Setup/validation/input creation
  are outside timing; fresh compiler construction, host registration, applicable
  pipeline work and output destruction are inside. Inputs/outputs cross black-box
  boundaries. Raw reports retain min/max/median/mean; individual compiler samples
  are not exposed by this Divan CLI and are not fabricated.
- Guest: 128 warmup calls on a separate instance, then 20 samples × 64 calls,
  fresh instance per sample with shared compiled module/JIT state. Reports retain
  **every raw ns/main-call sample**. Timing includes JS/Wasm crossing, batch loop
  and host effect bookkeeping, but excludes compilation, instantiation, warmup,
  verification, hashing and reporting. Finite warmup does not prove settled tiering.
- Commands: offline only (`--offline`, `CARGO_NET_OFFLINE=true`), cached tooling;
  no installation/download. Recorder commands retain 120-second deadlines; Node
  has a 60-second subprocess deadline. Outer collectors were bounded at 1,200
  seconds. These are direct-child deadlines, not guest fuel/process-tree control.
- Environment: Windows x64 `10.0.26300`, Intel Core Ultra 5 125H, 18 logical CPUs,
  reported RAM 16,730,726,400 bytes; rustc/Cargo 1.98.0, MSVC host, LLVM 22.1.8;
  Python 3.13.14; Node 24.12.0 / V8 13.6.233.17-node.37, no Node flags. Exact
  environment/PATH/commands are retained in [main environment](collection-1/environment.json),
  tool logs and each guest report.

Commands actually invoked from repository root:

```sh
python docs/benchmarks/integer-comparisons-negation-41/collect.py
python docs/benchmarks/integer-comparisons-negation-41/collect_expressions.py
# Above initially failed into expressions-1; after balancing the corpus,
# its destination changed to expressions-2 and the same command succeeded.
python docs/benchmarks/integer-comparisons-negation-41/inspect_wat.py
python docs/benchmarks/integer-comparisons-negation-41/report.py
python docs/benchmarks/integer-comparisons-negation-41/audit.py
git --no-pager diff --check
```

These collectors refuse to overwrite existing outputs. Future reproduction needs
new reviewed destinations, not deletion/replacement of this record. Every nested
command is retained as an argument array with exit/deadline metadata. Main commands
include the focused `integer_baselines` contract, ignored oracle verification and:

```sh
# Each measured twice with INTEGER_BASELINE_LEVEL=None / Basic set per command:
cargo bench --offline -p was-compiler --bench integer_specialization -- --sample-count 100 --sample-size 1 --timer os --color never
cargo test --offline -p was-compiler --test integer_baselines record_integer_guest_timing -- --ignored --exact --nocapture --test-threads=1
```

The supplement is a standalone evidence-only Cargo workspace, not a production
workspace member, with cached dependencies and its own lockfile/target directory.
[expressions.rs](expressions.rs) copies the existing benchmark **unchanged except
for its corpus module path**. [expression_measure.cjs](expression_measure.cjs) copies
the original Node harness, replacing only oracle names/values/comments; its timing,
effect, special-input and trap machinery is unchanged. Unused historical branches
are not supplement coverage; those controls are exercised by the main collection.
[node_bridge.py](node_bridge.py) only supplies the recorded stdin payload with a
60-second bound. Supplemental machine/runtime records accompany every guest run.

## Concrete results and trade-offs

Full all-case results are in [results.md](results.md), with derived percentages
and direction classifications in [results.json](results.json). Raw collections,
not rounded Markdown cells, own full precision and spread. Compiler medians below
are **µs/compilation**, guest medians **ns/main call**, pairs **repetition 1 / 2**.

| Workload | Bytes None → Basic | Typed None | Typed Basic | Guest None | Guest Basic |
| --- | ---: | ---: | ---: | ---: | ---: |
| `literal_32` | 1156 → 542 | 32.79 / 37.04 | 77.49 / 108.4 | 85.9375 / 96.875 | 164.0625 / 139.0625 |
| `locals_32` | 1828 → 1214 | 89.19 / 100.9 | 136 / 203.6 | 121.875 / 134.375 | 132.8125 / 175.78125 |
| `compare_32` | 4164 → 2430 | 159.3 / 185.2 | 241.2 / 466.7 | 359.375 / 570.3125 | 256.25 / 517.1875 |
| `negate_32` | 1668 → 1118 | 79.29 / 90.64 | 137 / 178.7 | 125.78125 / 209.375 | 178.90625 / 328.90625 |
| `expr_compare_32` | 10420 → 5236 | 278.6 / 617.1 | 697 / 1029 | 1021.09375 / 728.90625 | 871.875 / 730.46875 |
| `expr_negate_32` | 1044 → 469 | 27.89 / 50.64 | 56.24 / 71.94 | 160.9375 / 106.25 | 96.09375 / 198.4375 |

**Cumulative size:** 24/27 historical-corpus modules shrink, three unchanged
(`fraction`, `negate_min`, `subnormal`), none grow against current None. Unweighted
sum: **13,414 → 9,538 bytes (−28.90%)**. The separate six-case expression supplement
sums to **12,073 → 6,106 bytes (−49.42%)**. These are synthetic, independent-module
sums, not weighted production traffic or a combined application-size estimate.

**Incremental size versus historical #40 Basic:** historical corpus source hashes
match; total **9,701 → 9,538 bytes (−163 bytes, −1.68%)**. Exact changed lengths:

| Case | #40 Basic → current Basic | Delta |
| --- | ---: | ---: |
| `compare_1` | 140 → 134 | −6 |
| `compare_32` | 2622 → 2430 | −192 |
| `negate_1` | 92 → 93 | +1 |
| `negate_32` | 1086 → 1118 | +32 |
| `remainder` | 224 → 226 | +2 |

Others have unchanged lengths, not necessarily hashes. Integer negation is emitted
as `i32.const 0; operand; i32.sub`, rather than the one-op `f64.neg`: extra opcode
bytes can outweigh removed widening/storage costs. WAT confirms new negation
propagation/local storage, but **instruction counts alone are not a size or timing
benefit**. This historical size comparison is **not matched incremental #41 timing**;
none of the historical #39/#40 timings is used as a timing control.

**Compiler regressions:** historical-corpus typed medians rise in both repetitions
for 12/27 cases; none fall in both. `locals_32` is **+52.48% / +101.78%**;
`compare_32` **+51.41% / +152.00%**; `negate_32` **+72.78% / +97.15%**.
Source compilation also rises in both runs for these three: respectively
**+17.41% / +71.72%**, **+8.71% / +17.62%**, **+14.54% / +85.65%**.
Expression-only typed costs also rise: comparison **+150.18% / +66.75%**, negation
**+101.65% / +42.06%** at size 32. These are cumulative observed costs under the
recorded conditions, not isolated #41 pass-cost estimates.

**Guest regression signals and reversals:** only `compare_32` improves in both main
repetitions (**−28.70% / −9.32%**); 13/26 timed cases worsen in both, the rest are
mixed/equal. `negate_32` worsens **+42.24% / +57.09%**, `locals_32`
**+8.97% / +30.81%**, `capture` **+64.52% / +25.63%**, and `effects`
**+33.96% / +26.96%**. Expression comparison reverses **−14.61% / +0.21%**;
expression negation reverses **−40.29% / +86.76%**. All cases/spread are retained,
not selected as outliers. Background builds/tests, run-to-run variation, JS/Wasm
and timer overhead, and V8's own constant optimization/tiering prevent a stable
speedup or intrinsic-regression conclusion. Smaller modules are the demonstrated
benefit; there is **no stable general guest-speed claim**.

## Expression-only and independent inspection evidence

The supplement has **no ordinary local declarations or local reads**, even for
accumulation; independent decoded assertions verify **zero Wasm locals** in every
module (the unused f64 parameter is not a local declaration). Balanced expression
trees keep the bounded workload practical on Windows without increasing stack
limits. Both sizes use the same worked oracle:

- Six comparisons of `-(120 + 30)` against negative literals, all true with distinct
  weights `1,2,4,8,16,32`: expected **63 / 2016**. Decoding requires one / 32 each
  of `i32.lt_s`, `i32.le_s`, `i32.gt_s`, `i32.ge_s`, `i32.eq`, `i32.ne` in enabled
  modules. This exercises signed negative operands, not unsigned coincidental parity.
- Negation terms `-(120 + 30) + 7`: expected **−143 / −4576**. Enabled modules have
  no `f64.neg` and contain the expected integer subtraction path, independently
  of integer local storage.
- `-(0 - 0)` yields **−0**, checked with `Object.is`; exactly one `f64.neg` remains.
- `-((0 - 2147483647) - 1)` yields **2147483648**. The inner minimum is *proven*
  via subtraction; the outer unsafe negation retains exactly one `f64.neg`. This
  strengthens the historical `negate_min` control whose initial unary-minus literal
  is itself unproven. No trapping float-to-int conversion is introduced.

[emit.rs](emit.rs) validates all 24 modules with `wasmparser::Validator`, checks
source/typed byte parity and enabled-preset byte identity, and independently decodes
instructions/local declarations. The [raw decode report](expressions-2/emit.txt),
all `.was`/`.wasm`/`.wat` artifacts and payloads are preserved under
[expressions-2/artifacts/](expressions-2/artifacts/). All 24 independent guest oracles
pass, with 12 timed workload/repetition combinations per measured level.

Post-timing [CLI inspection](inspection/counts.md) reuses #40's inspector verbatim.
All 16 independently emitted CLI binaries match the main timed library module
hashes; all 32 binary/WAT commands and the offline CLI build passed. Main-only WAT:
`compare_32` has 32 each signed gt/eq/le comparisons, widening conversions fall to
129 (historical #40: 321); `negate_32` has 64 i32 locals / one f64 local, 33 widenings
and **zero f64.neg** (historical #40: 32). `negate_zero` retains one `f64.neg` after
widening; historical `negate_min` retains two. Full WAT is authoritative for opcodes
not listed in the copied count table. This is CLI/library artifact parity, **not
current-source browser parity**, diagnostics validation or a full release gate.

## Checks, failures and evidence inventory

- Original matched-recorder self-tests: **3 passed**; baseline-recorder self-tests:
  **4 passed**. [Logs](matched-self-test.txt) / [logs](baseline-self-test.txt) and
  command metadata retained. Simulated failure/timeout self-tests are not actual
  measurement failures.
- Focused native `integer_baselines` contract: **1 passed, 2 ignored** as expected;
  those ignored verify/timing cases were then explicitly run. Main **108 oracle
  checks passed**, including signed zero, NaN, infinities, subnormal, i32 bounds,
  unknown-input special values, alias/capture/mutation, unsupported arithmetic,
  exact effects as specified below, and expected bounds-trap order.
  Specifically `effects` returns −6 with marks **[3, 9]**; `trap_order` traps after
  **[1]**, before RHS mark 2, and receives **no guest timing**. Unknown additionally
  checks −0, i32 maximum, ±infinity, NaN and `Number.MIN_VALUE` against independent
  expected results. Original assertions remain intact.
- Main collection: all commands passed, no timeout; stable inputs. All 27 None
  source/module identities equal historical #38. Enabled Basic/Standard/Aggressive
  modules match; repeated guest identities and compiler/guest sizes match per level.
- **Preserved real failure:** [expressions-1/emit.txt](expressions-1/emit.txt) records
  exit `0xc00000fd` (Windows stack overflow), not a timeout, during debug emission of
  a flat repeated expression tree. [Metadata](expressions-1/emit.json), partial
  modules/sources, manifests and incomplete stability record are preserved. No
  timings from that failed attempt enter the results. Only this owned supplement's
  generator was balanced; the retry uses the new `expressions-2` destination.
- Successful supplement: all commands passed, no timeout; stable measured inputs,
  24 oracle/validator/decode checks and both timing repetitions completed. Import/
  dead-code warnings from reusing the historical corpus helper are retained in raw
  build logs, not silently suppressed. No production fix was attempted for the
  failed synthetic flat debug expression.
- No full tests, Clippy, core-Wasm build, browser rebuild or browser tests were run
  by this task. Correctness/diagnostic/adapter release validation belongs to the
  integration owner and is not established by these benchmark checks.

Evidence layout:

- [collection-1/](collection-1/): full main command stdout/stderr bytes plus JSON
  argument/UTC/exit/timeout metadata; before/after hashes for every command;
  environment, revision, status, tracked diff, verification, Divan reports and
  every raw guest sample.
- [expressions-1/](expressions-1/): failed attempt, retained verbatim.
- [expressions-2/](expressions-2/): successful separate expression corpus, source/
  module hashes, all presets' artifacts, decode report, raw timings and manifests.
- [inspection/](inspection/): post-timing CLI parity, WAT, counts and hashes.
- [results.md](results.md) / [results.json](results.json): complete derived tables,
  historical size deltas and all timing-direction classifications; generated by
  [report.py](report.py), not a new timing experiment.
- [audit.json](audit.json): original collection-final command/input/artifact/link/
  whitespace checks, including explicit retained failures.
  [artifact-hashes.json](artifact-hashes.json) covers the original directory and
  **pre-reconciliation README**, not this README's updated bytes.
- [final-integration-source-state.json](final-integration-source-state.json),
  [final-integration-audit.json](final-integration-audit.json) and
  [final-integration-artifact-hashes.json](final-integration-artifact-hashes.json):
  append-only current-source reconciliation, integrity audit and current artifact
  manifest; original evidence is not replaced. Git newline conversion can affect
  byte hashes; compare the appropriate saved manifest before relying on an altered
  checkout.

**Decision remains separate:** cumulative size benefit and incremental historical
size improvement are demonstrated; material timing regression signals remain
unaccepted. This record does not authorize shipping, changing defaults or general
optimizer work.
