# Immutable string operations — issues #73–#75 and #77

## Implemented scope

```text
string_byte_len(source: string) -> number
string_concat(left: string, right: string) -> string
string_slice_bytes(source: string, start: number, end: number) -> string
string_replace_all(source: string, needle: string, replacement: string) -> string
string_join(parts: string[], separator: string) -> string
```

These fixed-arity, exact-type, positional-only named builtins implement the M5
concat/byte-length foundation (#73), strict byte slicing (#74), and literal
replacement (#75), and fragment join (#77). They use ordinary identifier/call syntax, not new keywords or AST
expression shapes. Global user functions and host-language registration cannot
use these names; lexical values/callables may shadow them. A noncallable shadow
produces the ordinary call error. Host module/field names remain unrestricted.

All five builtins are excluded from pure defaults/contracts and direct builtin
pipeline stages. Existing lexical callable stages and parenthesized placeholder
stages can call them in their bodies. String `+`, string `len`, methods,
and fusion remain absent. [#76 adapter validation](#issue-76-first-slice-adapter-validation--2026-10-30)
covers the first four operations. #77 adds fragment construction through existing
fixed/resizable arrays and join. [#81 integrated validation](m5-release-readiness.md)
now records both final-source slice gates, reproduced deterministic construction,
ordinary runtime/native measurements and rebuilt-browser parity. The
[contract-to-test map](m5-contract-test-map.md) distinguishes inventories from actual
execution and gaps. The [official Linux baseline](benchmarks/m4-m5-aa-2026-11-27/baseline.md)
replaces historical Windows timing for M4 and M5 performance evaluation.
Historical validation sections below retain their original subset scope.

The accepted design is recorded in local ADR 0010 and memory-roadmap §9; design
confirmation is not release evidence. This document records implemented #73–#75 and #77.

## Evaluation and storage contract

Arguments execute exactly once, left-to-right, before operation-specific checks.
Concat then structurally validates left followed by right, even when an empty
operand permits reuse. Existing parameter-entry and host-return checks still run
at their existing boundaries. An argument trap stops later argument execution;
completed effects/allocations remain after traps.

Byte length returns the unsigned descriptor byte length exactly as `number`, not
a scalar/grapheme count, and allocates no result string. Embedded NUL is one byte.
Concat returns the other validated handle when either operand is empty (both empty
returns the right). Otherwise it uses widened result-length arithmetic and the
ordinary checked bump allocator for one descriptor-plus-payload allocation, then
copies both payloads without modifying inputs. There is no arbitrary new size cap,
host import, M4 reclaimable payload storage, or general string reclamation.

The revision-1 descriptor ABI and instance-long retention rights remain unchanged.
Aliases, interned literals and strings held in arrays remain immutable. Reassignment
changes a binding, not the old value. Hosts refresh views after possible growth;
valid immutable UTF-8 remains their obligation. Structural validation does not scan
UTF-8. Fresh identity is not a general language guarantee; broader reuse is outside
this slice.

### Strict byte slicing (#74)

Slicing uses half-open `[start, end)` UTF-8 byte offsets, not scalar/grapheme
indices. Both numbers must be finite integers with
`0 <= start <= end <= byte_length`; negative zero is accepted and bounds never
clamp. Both endpoints must be Unicode-scalar boundaries, even for empty slices.
Under the trusted-host valid-UTF-8 obligation, interior endpoints are checked for
continuation-byte markers. Zero/end skip payload loads; no full UTF-8 scan is added.

Source, start and end execute once, left-to-right, before structural source
validation and bounds/boundary checks. Existing entry and host-return checks are
unchanged. Full ranges return the validated source. Proper slices allocate one
new descriptor-plus-copied-payload result (also for empty proper slices); they are
not views into source storage. Sizing is widened and checked before allocation.
No callbacks occur between validation and copying. Aliases and retained results
keep the existing immutable, instance-long lifetime across growth/reentry/traps.

### Literal replacement (#75)

Replacement matches exact, case-sensitive UTF-8 bytes left-to-right, without
overlapping matches or rescanning inserted text: replacing `aa` with `b` in
`aaa` produces `ba`. Empty needles trap, including on empty sources. There are
no regexes, replacement templates, normalization or locale rules.

All three arguments execute before the helper structurally validates source,
needle, then replacement. It rejects empty needles only after those validations.
No-match and content-identical needle/replacement cases return the validated
source, including when equal contents have distinct handles. Otherwise a sizing
pass precedes one ordinary descriptor-plus-payload allocation, and a second pass
copies directly into final storage. Empty changed results also allocate a descriptor.
Inputs, aliases and array-held strings remain immutable and retained.

Both passes use constant auxiliary Wasm locals and search only the original
source. Result lengths and descriptor-inclusive bounds use widened arithmetic;
unrepresentable expansion traps before allocation. Search is straightforward and
may be quadratic on repetitive inputs; this slice makes no linear-time search or
performance-acceptance claim. There are no per-match collections/intermediate
strings, new host imports, full UTF-8 scans, reclamation or fusion.

## Fragment join (#77)

`string_join(parts: string[], separator: string) -> string` takes exactly two
positional arguments with exact types and ordinary Never compatibility. Its fixed
signature supplies string-array context to `[]` and normal region-local placeholder
constraints; callers add no unification. Lexical shadowing, reservations and pure/
pipeline exclusions match the other string builtins.

Capture parts once, then evaluate separator once before inspecting the captured
array. Separator-time writes, reserve/push/pop/remove, nested calls and host reentry
are visible in that array's current payload/length. Rebinding the original variable
does not redirect join. Both fixed and resizable arrays work without resize authority.
After arguments, validate separator, complete outer storage, then each entry in
index order, including empty/otherwise-unused entries. Even zero/singleton joins
validate separator. Trusted-host UTF-8 validity/immutability remains an obligation;
structural checks add no UTF-8 scans.

Join inserts separators only between entries: `[]` and `[""]` yield empty text;
`["", ""]` with comma yields `","`; `["a", "", "b"]` yields `"a,,b"`.
Empty separators, raw multibyte UTF-8 and embedded NUL work byte-for-byte. After
validation a singleton returns its entry. All other current results allocate one
ordinary descriptor plus payload, including zero-byte results (shared-empty reuse
is permitted by the contract, but not used here). Positive multi-entry results are
fresh copies, not fragment/separator aliases.

A sizing/validation pass uses unsigned widened lengths and checks the descriptor-
inclusive result against existing memory/allocator bounds before allocation. Outer
validation bounds count by memory bytes / four; string lengths are memory-bounded,
so the fragment sum plus separator product and header fit u64. A second pass copies
directly to final payload with constant auxiliary Wasm locals. No callbacks occur
during either pass or allocation/copying; memory growth preserves numeric offsets.
No fragment snapshot collection, intermediate concatenated strings, array mutation,
implicit imports, new type, ownership protocol, release of published strings or M4
reclaimable string storage is introduced. Earlier effects survive subsequent traps.

### Issue #77 validation — 2026-10-30

Confirmed seams: frontend analysis, checked typed-input/decoded lowering, bounded
Node execution, and CLI/current-source browser parity. Initial frontend/adapter
tracers failed on undefined `string_join`; the backend tracer then failed on the
missing checked builtin dispatch before implementation passed.

| Boundary | Evidence |
| --- | --- |
| `string_operations_frontend` | 16 tests: exact arity/types, contextual empty arrays, placeholder constraints, Never/skipped checks, reservations, shadowing and pure/pipeline exclusions |
| `string_operations_lowering` | 24 tests: forged nodes, contracts/defaults, Never-only discovery, nested helpers, independent decoded validation/copy/control/layout and constant-local checks, deterministic preset and absent-M5 compatibility |
| `string_operations_runtime -- --include-ignored` | 27 tests: fixed/resizable UTF-8/NUL/empty joins, captured identity and separator effects, ordered validation, sizing overflow, independent expected content/identity, aliases, retained results, traps, captures, recursion, reentry and actual growth |
| CLI `string_operations_adapters -- --include-ignored` | 9 tests: core/CLI bytes/WAT/diagnostics within each preset, host names, explicit/no exports and bounded execution |
| Browser `string_operations_adapters -- --include-ignored` | 5 tests after fresh `wasm-pack` rebuild: native/current-source browser parity and bounded execution |

Executed offline from the root, with 600-second bounds per build/test command:

- `cargo check --offline --all-targets`
- `cargo test --offline -p was-compiler --test string_operations_frontend`
- `cargo test --offline -p was-compiler --test string_operations_lowering`
- `cargo test --offline -p was-compiler --test string_operations_runtime -- --include-ignored`
- `cargo test --offline -p was-compiler-cli --test string_operations_adapters -- --include-ignored`
- `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline`
- `cargo test --offline -p was-compiler-browser --test string_operations_adapters -- --include-ignored`
- `cargo test --offline`
- `cargo test --offline --workspace -- --include-ignored`
- `cargo clippy --offline --all-targets -- -D warnings`
- `cargo clippy --offline --workspace --lib --bins -- -D warnings`
- `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown`
- `node tests/browser.cjs pkg` — all 25 smoke groups passed.

All listed commands passed. The first all-target Clippy attempt was blocked by
seven errors in concurrently added, unrelated string-observation tests; after
that separate work was corrected, the same command passed on retry. This task did
not modify those files. Edited Rust files were formatted individually; whitespace
checks passed.

Two-axis review found zero documented-standard breaches and zero spec findings.
One low-priority suggestion to share identical CLI/browser scenario data is deferred;
adapter-specific tests remain local without a new testing framework.

This establishes #77 correctness/adapter evidence, not construction-scaling
measurements, integrated M5 acceptance, or disposition of M4's pending performance
signals. No fusion, new ownership protocol, production counters or general string
reclamation is authorized by these results.

## Compiler boundaries and Rust migration

`BuiltinFunction` adds `StringByteLen`, `StringConcat`, `StringSliceBytes` and
`StringReplaceAll` and `StringJoin`;
downstream exhaustive matches must handle them. Existing `TypedExprKind::Call`
represents all five operations.
Semantic signatures supply exact string contexts and compose existing Never/flow
rules. No new source type, typed fields, configuration or dependencies are added.

Checked lowering independently checks arity, operands, result category and all
children, including skipped/unreachable code. Structural memory discovery finds
calls even with Never/forged annotations and inside nested callable bodies.
`codegen/memory.rs` appends only the required M5 helpers after existing memory helper
groups, leaving absent-M5 helper order and bytes unchanged. Helpers validate only
after arguments execute; no callbacks occur during sizing/allocation/copying.

## Focused validation — 2026-10-30

The three public seams are source analysis, checked typed input/decoded Wasm, and
bounded Node execution. Tests use independent expected values/instruction checks
and `wasmparser::Validator`. Node tests remain ignored by default, use explicit
30-second subprocess deadlines, and run bounded workloads at every preset.

- `string_operations_frontend`: 4 passed; signatures/inference/captures/placeholders,
  exact call structure, reservation/shadowing, purity/pipeline rejection, Never and
  skipped-child validation.
- `string_operations_lowering`: 7 passed; forged input, decoded validation/reuse/
  widened allocation/copy order, no byte-length allocation, structural discovery,
  nested arguments/live stack prefixes, captures and preset behavior.
- `string_operations_runtime -- --include-ignored`: 6 passed; empty/ASCII/NUL/UTF-8,
  reuse/immutable aliases, argument effects/traps, post-entry malformed descriptors,
  actual growth, reentry/caught traps, retained handles and nested stack/call state.
- `cargo test --offline -p was-compiler --lib codegen::memory`: 20 passed,
  8 ignored. Default success alone does not validate those ignored memory cases.

The initial test run failed at compilation before the new builtin variants existed;
the completed focused run passed all 17 new tests, including all six ignored Node
cases. This focused evidence does not claim full M5 or integrated release acceptance.

Additional checks:

- `cargo clippy --offline --all-targets -- -D warnings`: passed (default members).
- `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown`: passed.
- `cargo test --offline --workspace`: incomplete; the combined Clippy/build/test
  command reached its 120-second outer deadline while Cargo was compiling test
  targets. No full-workspace success is claimed; a longer bounded run is needed.
- `cargo test --offline -p was-compiler --test abi --test emitter`: 4 passed,
  2 Node cases ignored; baseline fixture bytes and preset determinism passed.
- Edited-file rustfmt and `git --no-pager diff --check`: passed.

Two-axis review used starting commit `473c93e871ee4e2466c3f930985de340159367d1`
and the implementation working diff plus new tests/docs, excluding preexisting
user design/scratch changes. Standards: zero hard findings and zero reported
smells. Spec: zero findings against #73, local ADR 0010 and memory-roadmap §9.
Reviewers did not independently rerun validation. Browser rebuild, integrated
adapter coverage, full ignored workspace execution and measurements remain pending
parent-owned gates.

## Issue #74 focused validation — 2026-10-30

- Test-first source-analysis slice failed with `UndefinedFunction` before frontend
  registration; it then passed. The decoded-Wasm test failed with
  `UnknownFunctionReference` before checked emission/helper integration; it then
  passed. The six new runtime cases likewise failed before builtin registration.
- `cargo test --offline -p was-compiler --test string_operations_frontend`:
  5 passed, covering fixed signatures, region-local placeholder constraints,
  shadowing/reservation, named/direct-stage rejection, purity and skipped children.
- `cargo test --offline -p was-compiler --test string_operations_lowering`:
  10 passed, covering forged arity/types/categories/children, Never discovery,
  helper-local discovery, decoded validation/conversion/reuse/copy ordering,
  validator success, once-only argument order and enabled-preset byte identity.
- `cargo test --offline -p was-compiler --test string_operations_lowering --test string_operations_runtime -- --include-ignored`:
  passed (8 lowering tests at that point, all 12 runtime tests); later lowering
  additions passed in the separate 10-test run above. Runtime coverage includes
  every endpoint in multibyte/NUL strings, numeric extremes, empty interior traps,
  exact memory-end endpoints, post-entry malformed descriptors, host-validation
  ordering, immutable aliases, copied payloads, real guest allocation growth,
  reentry/caught traps and retained handles across all four presets.
- `cargo test --offline --workspace`: 1,317 passed, 0 failed, 312 ignored across
  89 targets. This is default-suite evidence, not full ignored/browser coverage.
- `cargo clippy --offline --all-targets -- -D warnings`: passed.
- `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown`:
  passed.
- `cargo test --offline -p was-compiler --lib codegen::memory`: 20 passed,
  8 ignored. `cargo test --offline -p was-compiler --test abi --test emitter`:
  4 passed, 2 ignored; independent absent-M5 baseline fixtures remain unchanged.
- Edited-file rustfmt and `git --no-pager diff --check`: passed.

Current-source CLI/browser adapter parity, browser rebuild, the complete ignored
workspace suite, measurements and integrated M5 acceptance remain parent-owned
work, not claims made by this slice. Preexisting uncommitted ADR/roadmap/glossary
and scratch work are preserved separately from the #74 implementation.

Two-axis review compared the implementation working diff against starting commit
`3a538a2a833426e792a07955b0359438532cb264`, excluding preexisting design/scratch
changes. Standards: zero violations and zero actionable smells. Spec: zero
findings against #74, ADR 0010 and roadmap §9. The spec reviewer independently
reran the combined frontend/lowering/runtime command with `--include-ignored`:
27 passed, none failed/ignored. No integrated M5 release claim is made.

## Issue #75 focused validation — 2026-10-30

- Test-first frontend checks failed before builtin registration/reservation, then
  passed. New checked-lowering and runtime cases initially failed with
  `UnknownFunctionReference` before backend integration, then passed.
- `cargo test --offline -p was-compiler --test string_operations_frontend --test string_operations_lowering --test string_operations_runtime -- --include-ignored`:
  44 passed (10 frontend, 16 lowering, 18 runtime), none failed/ignored.
  Replacement covers literal UTF-8/NUL/overlap/deletion behavior, content reuse,
  ordered effects and validation, malformed unused operands, forged input, Never,
  nested callables, retained values, growth/reentry, live prefixes and bounded
  repetitive searches at every preset. Expansion tests exceed memory32 and its
  descriptor-inclusive bound using small input storage, checking no heap advance
  or memory growth on failure and successful later use.
- `cargo test --offline --workspace`: 1,328 passed, 0 failed, 318 ignored across
  89 targets. The first attempt reached its 240-second outer deadline with no
  reported failures; a same-limit repeat after compilation completed successfully.
  This is default-suite evidence, not full ignored/browser coverage.
- `cargo clippy --offline --all-targets -- -D warnings`: passed.
- `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown`:
  passed.
- `cargo test --offline -p was-compiler --lib codegen::memory`: 20 passed,
  8 ignored. `cargo test --offline -p was-compiler --test abi --test emitter`:
  4 passed, 2 ignored; independent absent-M5 baseline fixtures remain unchanged.
  An initial combined gate command reached its outer deadline after reporting
  these checks; the complete sequential gate command was rerun and exited successfully.
- Edited-file rustfmt and `git --no-pager diff --check`: passed.

Two-axis review used starting commit
`59ea45aa6bb0537fa93baa2d872e62d2f5475657` and the implementation working diff,
excluding preexisting design/scratch work. Standards: zero hard violations, one
low-priority duplicated-allocation-emission suggestion deferred to avoid expanding
this slice into a cross-operation refactor. Spec: zero findings against #75,
local ADR 0010 and roadmap §9. The standards reviewer independently reproduced
all 44 focused tests; the spec reviewer performed static review only.
Current-source integrated adapter/browser release gates, measurements (including
adversarial replacement timing), join and full M5 acceptance remain parent-owned
work. Preexisting uncommitted ADR/roadmap/glossary and scratch work stay separate
from this implementation.

## Issue #76 first-slice adapter validation — 2026-10-30

### Scope and contract-to-test map

This slice adds compatibility tests, not another implementation. Confirmed seams
are the core `Compiler`, CLI file/output adapter and freshly rebuilt browser
`Compiler.compile`/`compileWat`. The two adapter targets are
`crates/cli/tests/string_operations_adapters.rs` and
`crates/browser/tests/string_operations_adapters.rs`. The core prerequisite suites
were reviewed and rerun; no compiler or runtime changes were needed.

| Contract | Evidence |
| --- | --- |
| All four operations, immutable UTF-8/empty/NUL, aliases/interned literals and array-held strings | CLI `cli_string_copy_marshaling_utf8_empty_nul_aliases_and_edge_results_execute_at_each_preset`; browser `rebuilt_browser_string_values_strict_slicing_and_replacement_match_native`; core runtime literal/reuse/copy cases |
| Strict slicing, including empty interior endpoints, numeric extremes and -0; literal replacement overlap/deletion/no-rescan/no-match/content reuse | Browser value test independently enumerates byte endpoints and checks worked replacement results; CLI values and prerequisite runtime cases cover both operations |
| Within-preset bytes/WAT parity, enabled-preset identity, deterministic output, selected/no exports and explicit imports | CLI `cli_string_bytes_wat_imports_and_selected_or_no_exports_match_core_at_each_preset`; browser `browser_string_native_fixtures_validate_surface_diagnostics_and_preset_parity` and both rebuilt-browser tests; independent decoded imports/exports plus `wasmparser::Validator` |
| Global/host-language reservation, arbitrary nonempty external names, callable shadows and noncallable no-fallback | CLI host/diagnostic/surface tests; browser registration errors, external NUL/Unicode/whitespace names, lexical callable execution and diagnostic matrix. CLI delimiter limitations are not imposed on core/browser |
| Exact positional arity/types; named calls, pure defaults/contracts and direct builtin stages rejected; unchanged error ordering/spans | CLI diagnostic matrix and `cli_string_errors_preserve_raw_utf8_spans_and_structure_before_child_order` explicitly check variants and UTF-8 byte offsets with CRLF/tabs; browser compares native rendered diagnostics at every preset for both compile methods; prerequisite frontend/forged-input purity tests |
| Once-only left-to-right effects, argument traps, validation before reuse and no rollback | CLI `cli_string_ordered_effects_traps_growth_and_retained_handles_execute_at_each_preset`; browser `rebuilt_browser_string_order_validation_retention_growth_and_reentry_match_native`; post-entry descriptor corruption isolates builtin validation from entry checks, including otherwise unused operands |
| Retained handles after caught traps, reentry, actual allocation growth and refreshed views | Browser effects/lifetime test checks guest concat growth and detached old buffers during reentry, retains published strings and array-held strings; CLI lifetime case and core per-operation growth/reentry suites complement it |
| Existing destinations unchanged on compilation/registration errors; explicit copy marshaling and unchanged API surface | CLI diagnostics and reserved-host-name tests preserve sentinel artifacts; CLI/browser value tests use existing `runtime/abi.cjs` copies; decoded exports/imports reject additional runtime management APIs/imports |
| Checked frontend, forged typed input and independent decoded lowering | Prerequisite `string_operations_frontend` (10), `string_operations_lowering` (16), and runtime (18) cases cover signatures, Never/skipped children, purity, helper discovery/order, validation-before-reuse, widened sizing and direct final-payload construction |

Native and CLI diagnostics retain their source labels; browser expectations use
`<browser>`. Raw span and first-error assertions are independent of rendering.
Node/browser cases remain ignored by default and use bounded subprocess helpers:
CLI children have 30-second deadlines; browser children have 60-second deadlines.
Bounded workloads are not guest fuel or process-tree cancellation. All execution
cases run at every preset; O0 is not required to equal enabled-preset bytes.

### Executed validation

All commands ran offline from the repository root with external bounds of at
most 600 seconds per Cargo/build invocation. Node.js 24.12.0 and cached tooling
were available. Browser execution followed a successful current-source rebuild:

- `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline`: passed; no tooling/dependency installation was requested.
- `cargo test --offline -p was-compiler-browser --test string_operations_adapters -- --include-ignored`: **3 passed**, none failed/ignored, against that rebuilt package.
- `cargo test --offline -p was-compiler-cli --test string_operations_adapters -- --include-ignored`: **6 passed**, none failed/ignored.
- `cargo test --offline -p was-compiler --test string_operations_frontend --test string_operations_lowering --test string_operations_runtime -- --include-ignored`: **44 passed**, none failed/ignored.
- `cargo test --offline`: passed; its ignored cases are not execution evidence.
- `cargo test --offline --workspace -- --include-ignored`: **1,655 passed, 0 failed, 0 ignored across 91 targets** (including the empty doc-test target); local full-run log is `target/issue-76-workspace-tests.log`.
- `cargo clippy --offline --all-targets -- -D warnings`: passed.
- `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown`: passed.
- `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings`: passed.
- `node tests/browser.cjs pkg`: **25 groups passed** against the same rebuilt package.
- File-only Rust formatting and `git diff --check`: passed.

These are compatibility tests against the delivered #73–#75 behavior, not a
production red-to-green change. Existing behavior passed after correcting test
assumptions; no compiler defect or semantic policy change was introduced.

### Acceptance boundary

The first immutable-operation adapter slice is validated. Hosts still preserve
valid immutable UTF-8 and refresh views after growth; checks are structural, not
full UTF-8 scans. Marshaling remains explicit/copy-based and raw handles retain
revision-1 instance lifetime. No join, fusion, new ownership API, M4 performance
disposition or general reclamation is implied.

`string_join`, fragment-construction instrumentation and ordinary-instance
measurements (including adversarial replacement timing), and final integrated M5
acceptance remain pending. Historical prerequisite and unrelated milestone records
retain their original scope. Preexisting glossary, ADR, roadmap design and scratch
work are preserved separately from this ticket's changes.

### Two-axis review

Review compared only this ticket's staged changes with starting commit
`36bfcf5f58aab8f528128d0bc973b880a99d0371`; preexisting user design/scratch
changes were excluded. Standards: zero hard violations; one low-priority suggestion
to replace string-valued expected-error tags with a test-local enum was implemented
and confirmed resolved by the reviewer. Spec: zero findings against #76 and the
confirmed immutable-operation contracts. Reviewers performed static review, not
independent reproduction of the full validation. The adjusted CLI suite and Clippy
passed again; local documentation checks resolved all 333 relative paths.
