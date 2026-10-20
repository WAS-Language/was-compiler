# Proven integer specialization: #38 contracts and experimental #39/#40/#41 subset

## Status and authorization

Issue #38 established bounded workloads, reproducible unoptimized measurements,
and the broader proof contract below. User-approved issue #39 implemented exact
parsed signed-i32 literals and proven addition/subtraction subtrees. User authorization
via ticket #40 extends that implementation to eligible never-reassigned ordinary
numeric local storage and read-fact propagation in `Basic`, `Standard`, and
`Aggressive`. User authorization via ticket #41 adds proven numeric comparisons
and safe unary negation independently of ordinary-local storage, composing with
existing proofs when locals are eligible. This subset remains **experimental**: cumulative measurements show a
module-size benefit, but mixed timing results include material regression signals
pending explicit user acceptance. Historical [#39 correctness gates](#correctness-validation-2026-10-17)
and review against `09dcb5c` retain their #39-only scope. Final [#40 correctness
gates passed](#correctness-validation-2026-10-20), and its [review is complete](#review-completion-40).
Neither correctness validation, implementation authorization nor measurements
establish release acceptance. `None` remains the default and
byte-compatibility baseline.

The approved #39/#40/#41 subset is a narrow scheduling exception ahead of deferred
WP12, not general WP13 authorization. #41 preserves the existing proof interface,
checked lowering and public boundaries; it adds no broader optimization authority.
The [extension plan](was_language_extensions_plan.md#issue-38--proven-integer-specialization-baseline-exception)
reconciles this exception with the ordered work packages. The broader optimizer
candidates and WP13 entry gates remain unchanged.

<a id="implemented-subset-39-literalsadd-sub-40-ordinary-locals"></a>

## Implemented subset (#39 literals/add-sub; #40 ordinary locals; #41 comparisons/negation)

- Literal eligibility uses the parsed binary64 value: exact signed-i32 integer,
  excluding negative zero. Decimal spelling alone neither qualifies nor excludes it.
- Add/sub specializes only when both children have proofs and conservative bounds
  keep every intermediate result within signed i32 without producing negative zero.
  Lower with `i32.const`, `i32.add` and `i32.sub`, not constant folding.
- Independent eligible subtrees specialize even within an unsupported or unsafe
  parent, converting exactly with `f64.convert_i32_s` at every unspecialized consumer.
  An unsupported parent therefore need not retain byte-identical children.
- #40 stores eligible ordinary numeric lets in `i32` slots and propagates initializer
  bounds through their reads and later eligible add/sub/local chains. Eligibility
  requires a proven initializer and no ordinary or compound writes anywhere in the
  owning body, including skipped/unreachable children. An ordinary loop-body let
  may qualify independently on each execution; reassigned/loop-carried state does not.
- Parameters, iteration bindings, normalized-call synthetic bindings, and
  captured/promoted bindings are excluded. Their reads, call results and memory
  loads supply unknown facts. Each user/helper body has separate binding ownership
  and write discovery; helper-local eligible lets may specialize without changing
  captured cells. Slot order and indices are preserved, including sparse IDs.
- #41 uses signed `i32.lt_s/le_s/gt_s/ge_s` and `i32.eq/ne` only when both
  numeric operands have integer proofs. Operand representation is separate from the
  boolean result: no numeric widening is applied to the comparison result. Boolean,
  string and array comparison paths are unchanged.
- #41 proves unary negation only when operand bounds exclude zero and `i32::MIN`.
  Emit `i32.const 0`, the checked integer operand, then `i32.sub`, without folding.
  Propagate the negated bounds through supported expressions and eligible locals.
  `-0`, `-(1 - 1)` and negated signed-i32 minimum retain `f64.neg`; eligible children
  still specialize and widen at that fallback boundary. Source `-1` now qualifies,
  while `-2147483648` retains f64 because its positive literal is outside signed i32.
- Multiplication, division and remainder remain f64. Unknown inputs, NaN, infinities,
  fractional/out-of-range values and unsupported expressions supply no proof.
- `Compiler::compile` / `compile_source` select a private numeric policy from the
  preset. The typed-AST optimization boundary still borrows without rewriting.
  Direct `codegen::compile` and `codegen::compile_with_exports` retain baseline policy.
- Private per-body `Facts` in `crates/compiler/src/codegen/representation.rs` caches
  conservative proofs for borrowed immutable expression nodes, including unknown
  results. Node addresses are lookup keys only, never emitted-order authorities;
  each user/helper body owns its cache. No public AST annotation or new IR is added.
- Facts guide representation, not acceptance. Checked emission still validates every
  node, malformed literals/annotations, and unreachable/skipped children in existing
  order. Unknown facts fall back rather than introducing errors or runtime guards.

Language types, public AST and all ABI boundaries remain unchanged; only eligible
private local slot types change under #40. Function parameters/results, array elements,
captured cells and range records keep their existing representation. There is no
branch/caller/contract refinement, constant folding, algebraic rewrite, or
preliminary baseline compilation.

## First-pass representation contract (#38; implemented by #39/#40/#41)

The table below is the bounded #38 design envelope implemented across #39/#40/#41.
It authorizes neither general WP13 nor extra arithmetic or refinement.

The language retains one `number` type with IEEE-754 binary64 semantics. Signed
`i32` is a private representation only where static proof establishes identical
observable behavior; integer-looking syntax alone is insufficient.

| Eligible operation or binding | Required proof / boundary |
| --- | --- |
| Literal | Its parsed binary64 value is an exact signed-i32 integer and is not negative zero; spelling is not the proof. |
| Ordinary local declaration/read | Uncaptured and never reassigned, initialized by a proven value. An ordinary local declared inside a loop may qualify independently on each execution. |
| Addition / subtraction | Both operands are proven integers and conservative bounds keep every intermediate result within signed i32, preserving zero sign. |
| Numeric comparison | Both numeric operands are proven integers; preserve the existing boolean result. |
| Unary negation | Operand bounds exclude both zero and `i32::MIN`; negation must neither manufacture negative zero nor overflow. |

Track integrality, conservative bounds in `[-2147483648, 2147483647]`, and
negative-zero possibility. Use conservative analysis arithmetic so calculating
bounds cannot itself overflow. Bounds describe possible values, not folded
runtime results. Propagate facts through supported expressions and eligible
bindings only; derive no refinements from branches, callers, or contracts.

Unknown or unsupported values remain `f64`: parameters/call results, memory loads,
captured reads, mutable locals, loop-carried state, and iteration bindings are not
eligible ordinary local storage. Multiplication, division, remainder, and other
unsupported operations retain existing lowering. An eligible subtree may still
specialize independently and convert exactly back to `f64` at an unspecialized
consumer. Negative-zero uncertainty or unproven bounds requires fallback, not a
new error or a speculative runtime guard.

Function signatures and the host ABI remain unchanged (`number = f64`), as do
array elements, captured cells, and range records. Keep public AST/types unchanged.
Compiler orchestration selects policy; private per-function representation facts
may guide checked lowering. Preserve validation of mutable/forged typed input and
all unreachable/skipped children; reuse the existing checked boundary rather than
adding another IR, duplicate validator, or preliminary unoptimized compilation.
Constant folding and algebraic rewriting are outside this pass.

## Preset and compatibility gates

- **Baseline:** `None` stays default and preserves baseline bytes, as do direct
  codegen entry points. #38's recorded cross-preset byte identity is historical.
- **Enabled subset:** `Basic`, `Standard`, and `Aggressive` use the same #39/#40/#41 policy
  and produce identical bytes to each other. They may differ from `None`. Programs
  with no eligible subtree retain existing bytes; unsupported parents alone do not
  establish this condition because their children can specialize independently.
- **Across all levels:** require semantic and diagnostic parity, including error
  variant, message, span, and first-error order, rather than cross-preset byte identity.
- **Within each level:** library, CLI, and current-source rebuilt-browser outputs
  remain byte-identical for matching input, imports, exports, and options.

Preserve once-only left-to-right evaluation, effects, alias visibility, traps and
trap order, signed zero, NaN/infinities, and deterministic layout. Inspect actual
specialization via existing WAT output and independent decoded instruction/local
layout tests, not new public flags, browser methods, or AST annotations.

## Workloads and correctness evidence

The integrated harness comprises:

- `tests/support/integer_corpus.rs`: shared deterministic bounded source corpus.
- `tests/support/integer_measure.cjs`: independent execution verification and
  separately scoped guest timing.
- `crates/compiler/tests/integer_baselines.rs`: compiler-owned integration target.
- `crates/compiler/benches/integer_specialization.rs`: separate Divan target using
  existing development tooling, without changing the historical `compiler` target.

The corpus contains 27 bounded workloads; verification covers 108 workload/preset
pairs across all four levels. Independent worked expectations live in
[`integer_measure.cjs`](../tests/support/integer_measure.cjs), not in the
representation proof analysis. Corpus case names describe workloads, not implemented
specialization: comparison/negation cases can shrink through eligible children
and local storage without specializing those consumers.

| Cases | Count | Coverage / independent expectation |
| --- | ---: | --- |
| `literal_{1,32}`, `locals_{1,32}` | 4 | Literal arithmetic and never-reassigned local chains; 143 per block, totals 143 / 4576. |
| `compare_{1,32}`, `negate_{1,32}` | 4 | Comparisons yield 3 / 96; restricted negation yields -143 / -4576. |
| `loop_local` | 1 | 32 bounded range yields, fresh ordinary locals per iteration; total 4576. |
| `fraction`, `negative_zero`, `negate_zero`, `positive_zero` | 4 | 3.75, -0, -0, +0 respectively. |
| `overflow`, `underflow_i32`, `negate_min` | 3 | Intermediate and final results leave i32: 2147483649 / -2147483650; minimum negation yields 2147483648. |
| `unknown`, `mutable`, `capture` | 3 | Unknown input yields 8.5, reassigned fractional local yields 3.5, shared capture observes reassignment and yields 144. |
| `multiply`, `divide`, `remainder` | 3 | Unsupported operations yield 63, 3.5, -1. |
| `nan`, `infinity`, `subnormal` | 3 | NaN, positive infinity, and `Number.MIN_VALUE`. |
| `effects`, `trap_order` | 2 | Result -6 with marks `[3, 9]`; bounds trap after mark `[1]`, before RHS mark 2. |

Every module declares `env.mark: number -> number`, unused except by effect cases,
and exports `main(x: number) -> number`. The harness supplies the host import even
when unused. Ordinary calls use `x = 7.5`; `unknown` additionally checks -0,
2147483647, both infinities, NaN and `Number.MIN_VALUE`. Host registration and
measurement introduce no production-output change. Subprocess deadlines are not
guest fuel.

Record independent expected values, effect sequences, and traps before inspecting
optimized output. Use `Object.is` for signed zero and `Number.isNaN` for NaN.
Require valid Wasm plus independent decoded representation checks; valid Wasm alone
does not prove numeric equivalence. Unsupported/unsafe operations must retain their
baseline paths, even when eligible child subtrees specialize. Differential execution
must compare both `None` and enabled levels against the independent oracle, not only each other.
Default Cargo success does not establish ignored Node execution coverage.

## Measurement and release decision

[Benchmark procedure](benchmarks.md#issue-38-integer-specialization-baseline-and-comparison)
owns commands, timing isolation, environment records, and the evidence handoff.
Keep compilation cost, emitted module size, and guest execution time separate.
The #38 baseline predates transformations; its then-no-op preset comparison is a
historical harness/parity check, not an optimization speedup.

For #39 and later comparisons, measure matched `None` and enabled configurations on
identical corpus inputs, toolchains, runtime, machine, and harness protocol.
Repeat measurements and preserve raw samples, full command output, source/module
hashes, environment, revision and dirty-tree identity. Record implementation
revision differences explicitly; changes to workloads or harnesses require fresh
matched measurements rather than comparison with incompatible historical data.

Shipping requires a demonstrated module-size **or** guest-time benefit, with
compilation trade-offs and all measured regressions reported. More integer
instructions alone is not a benefit. There is no invented percentage threshold;
material regressions need explicit user acceptance. Inconclusive or regression-only
evidence keeps the work experimental pending a decision, not release success or
permission to change the default. Correctness and diagnostic/adapter gates remain
mandatory even when measurements show a benefit.

## Evidence status

The unoptimized baseline was recorded on 2026-10-17: two compiler runs, two guest
runs, 108 verification cases and matching before/after input hashes. See the
[measurement summary](benchmarks/integer-specialization/summary.md),
[environment record](benchmarks/integer-specialization/environment.json), and
[interpretation](benchmarks.md#recorded-baseline--2026-10-17). Timings show substantial
run-to-run variation, not an enabled optimization benefit or release result.
Historical benchmark records retain their original scope and measurement boundaries.

The [#39 matched measurement record](benchmarks/integer-add-sub-39/README.md)
measures the historical #39 working-tree literal/add-sub subset, not #40 local
storage or the broader #38 design.
24 of 27 modules shrink and none grow; summed independent corpus module sizes
fall from 13,414 to 9,702 bytes (27.67%), not a production-application estimate.
Compilation medians show regressions; guest timings are noisy, with reversals and
material regression signals, not a demonstrated overall speedup. The record reports
108 successful independent workload/preset oracle checks and focused validation,
not complete diagnostics, forged-tree, workspace or current-source CLI/browser gates.
The separate correctness validation below passed; it does not broaden the historical
measurement record's scope. Material regressions still require explicit user
acceptance; this work remains experimental, with no release claim or default change.
Review against `09dcb5c` is complete.

## Correctness validation (2026-10-17)

**Historical #39 only:** these gates and the #39 review do not validate #40.
See the separate [final #40 correctness evidence](#correctness-validation-2026-10-20).

Final integration correctness gates passed with the commands below, as reported by
the integration owner. The [raw final workspace test output](integer-specialization-validation-2026-10-17.txt)
records 1,261 tests passed, zero failed and zero ignored. Browser bindings were
rebuilt from current source before browser and full ignored-suite execution.

| Exact command (repository root) | Result |
| --- | --- |
| `cargo check --offline --workspace --all-targets` | Passed |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed; current-source browser package rebuilt |
| `node tests/browser.cjs pkg` | Passed: 25 groups |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed |
| `cargo test --offline -p was-compiler -- --include-ignored` | Passed |
| `cargo test --offline -p was-compiler-cli -p was-compiler-browser -- --include-ignored` | Passed; together 1,261 tests, zero failed/ignored |

These are correctness gates, not a release decision or acceptance of noisy material
timing regressions. Explicit user acceptance remains pending, and `None` remains
the default. Review against `09dcb5c` is complete.

## #40 measurements and validation status (2026-10-20)

The [new #40 measurement record](benchmarks/integer-locals-40/README.md) compares
matched current `None` and `Basic`, cumulatively including #39 and #40. The synthetic
27-module sum is **13,414 → 9,701 bytes (−27.68%)**. Against historical #39 Basic
(9,702 bytes), the incremental total is only **one byte smaller**: `locals_32` saves
64 bytes, `compare_32` grows 64 bytes, and `loop_local` saves one byte. Conversion
boundaries can offset local-storage savings; historical timings are not matched
incremental #40 controls.

The record reports 108 independent-oracle checks and independent WAT/local-storage
inspection, not final workspace, diagnostic or current-source browser release gates.
Guest timings are mixed/inconclusive and compilation measurements show material
regression signals. Separate [final correctness gates passed](#correctness-validation-2026-10-20);
#40 remains experimental pending explicit acceptance of material regressions and a
release decision. Ticket #40 authorizes implementation,
not release acceptance or a default change. Preserve the evidence verbatim, including
the README's warning about inherited stale #39 narrative in raw recorder output.

`crates/compiler/tests/integer_locals.rs` covers decoded storage/read chains, sparse
IDs, write exclusions, helper ownership, synthetic-call storage, checked forged-input
errors and ignored bounded runtime cases. CLI/browser adapter tests cover the matching
local-chain contract. Their final execution is recorded separately below, without
broadening the historical raw measurement evidence's validation scope.

## Correctness validation (2026-10-20)

Final #40 integration correctness gates **passed**, as reported by the integration
owner. The [raw final workspace test output](integer-locals-validation-2026-10-20.txt)
records **1,276 tests across 81 targets, zero failed and zero ignored**. Browser
bindings were rebuilt from current source before browser and full ignored-suite
execution. This documentation update records the final results; it did not rerun
the compiler validation commands.

| Exact command (repository root) | Result |
| --- | --- |
| `cargo check --offline --workspace --all-targets` | Passed; rerun on the final tree |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed on the final tree |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed; current-source browser package rebuilt |
| `node tests/browser.cjs pkg` | Passed: 25 groups |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed |
| `cargo test --offline --workspace -- --include-ignored` | Passed: 1,276 tests, 81 targets, zero failed/ignored |

Earlier runs are preserved, not replaced by the successful log:

- [Initial run](integer-locals-validation-2026-10-20-initial.txt): stale inference
  storage expectation.
- [Second run](integer-locals-validation-2026-10-20-second.txt): stale pipeline
  ordinary-local slot expectation.
- [Third run](integer-locals-validation-2026-10-20-third.txt): Windows error 206
  from oversized browser-test Node arguments; fixed by passing the payload via stdin.

The integration owner verified the exact measurement input manifest: no production
inputs changed after the measured hashes, with no changed inputs. Test expectation
and adapter-harness fixes do not turn the historical raw measurements into a new
collection or broaden their scope. Final correctness evidence is separate from
benchmark evidence and does not accept material timing regressions or authorize
release/default changes; `None` remains default.

## #41 measurements and correctness validation (2026-10-20)

User-authorized #41 implements all six numeric comparisons with proven integer
operands and unary negation whose operand bounds exclude zero and signed-i32
minimum. Expression-only tests and a separate measurement supplement exercise
these operations without ordinary-local storage; integrated cases also compose
with #40 local proofs. This does not authorize other arithmetic or refinement.

The [matched measurement record](benchmarks/integer-comparisons-negation-41/README.md)
records two sequential repetitions per level, current `None` versus current `Basic`,
using the unchanged 27-case historical corpus plus an expression-only supplement.
The main corpus totals **13,414 → 9,538 bytes (−28.90%)**, cumulatively #39+#40+#41.
Historical #40 Basic totals 9,701 bytes, so the incremental size delta is **−163 bytes
(−1.68%)**: `compare_32` saves 192 bytes while `negate_32` grows 32 bytes. This is not
a matched incremental timing experiment. Expression-only modules use zero declared
Wasm locals and total 12,073 → 6,106 bytes. Main 108 and supplement 24 independent
oracle/validator/decoded checks passed; all 27 `None` source/module identities match
historical #38. Production hashes match the final integration tree. The failed flat
supplement attempt and successful balanced retry are both preserved.

**Trade-offs:** guest timing is mixed, not a stable speed benefit. `negate_32`
guest medians regress 42.24% / 57.09%, with typed compilation +72.78% / +97.15%.
`compare_32` guest medians improve 28.70% / 9.32%, but typed compilation rises
51.41% / 152.00%. Concurrent builds/tests, thermals, background load and JIT variation
were not controlled. Explicit regression/release acceptance remains pending;
`None` stays default and all enabled presets remain experimental.

Final correctness gates ran offline on current sources:

```sh
cargo check --offline --all-targets
cargo test --offline -p was-compiler --test integer_specialization --test integer_locals -- --include-ignored
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown
wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline
node tests/browser.cjs pkg
cargo test --offline --workspace -- --include-ignored
cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings
```

All commands passed after updating the existing enabled-preset fixed-byte oracle
for the intentional #41 `-1` encoding (`i32.const 0`, operand, `i32.sub`, widening),
without changing its independent `None`/direct-codegen byte oracle. The
[initial failed run](integer-specialization-validation-41-2026-10-20-initial.txt)
and [final full workspace log](integer-specialization-validation-41-2026-10-20.txt)
are preserved separately. Focused tests observed red before green for comparisons
and negation, then passed decoded instructions, negative controls, forged/skipped
errors and bounded independent runtime expectations. Browser smoke checks passed
25 groups with freshly rebuilt bindings. The full workspace passed **1,287 tests
across 81 targets, zero failed and zero ignored**.

## Review completion (#41)

Two independent read-only standards/spec reviews covered the uncommitted tree
against initial commit `839899af3a966a9eb9cc109380e028e0a9edaef8`, including new
measurement evidence and both workspace validation logs.

### Standards

Zero documented-standard violations. One low-priority heuristic suggestion:
CLI/browser comparison-negation suites duplicate eight scenario source strings;
a shared test-only data module could reduce maintenance drift. This is deliberately
deferred to preserve the current package-owned harnesses without expanding #41 into
a shared test-data refactor. It is not a hard violation or release blocker.

### Spec

Zero actionable findings: no missing/partial requirements, scope creep or incorrect
implementation identified. Review independently checked the final 1,287-test log,
timed/final production hashes, expression-only independence and explicit unaccepted
measurement regressions. Review completion does not authorize release, a default
change or acceptance of material regressions.

## Review completion (#40)

Review against initial `dc619db46e9dac3694adb7cc5855203a18233426`, covering the
uncommitted #40 tree, completed with **zero hard standards findings and zero spec
findings**, as reported by the review owner. One low-priority suggestion to reduce
duplicate adapter scenarios was deliberately deferred. The [review record](integer-specialization-review-40.md)
is forthcoming. Review completion is not regression acceptance, release acceptance
or permission to change the default.

## Review completion (#39)

The [two-axis review against `09dcb5c`](integer-specialization-review-39.md)
records the standards findings, fixes, zero spec findings, final split validation
and timeout/provenance details. Release acceptance remains a separate decision.
