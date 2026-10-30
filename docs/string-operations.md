# Immutable string operations — issues #73–#75

## Implemented scope

```text
string_byte_len(source: string) -> number
string_concat(left: string, right: string) -> string
string_slice_bytes(source: string, start: number, end: number) -> string
string_replace_all(source: string, needle: string, replacement: string) -> string
```

These fixed-arity, exact-type, positional-only named builtins implement the M5
concat/byte-length foundation (#73), strict byte slicing (#74), and literal
replacement (#75). They use ordinary identifier/call syntax, not new keywords or AST
expression shapes. Global user functions and host-language registration cannot
use these names; lexical values/callables may shadow them. A noncallable shadow
produces the ordinary call error. Host module/field names remain unrestricted.

All four builtins are excluded from pure defaults/contracts and direct builtin
pipeline stages. Existing lexical callable stages and parenthesized placeholder
stages can call them in their bodies. String `+`, string `len`, methods,
join and fusion remain absent. M5 completion, integrated adapter/
release validation and measurements remain separate parent-owned work.

The accepted design is recorded in local ADR 0010 and memory-roadmap §9; design
confirmation is not release evidence. This document records implemented #73–#75.

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

## Compiler boundaries and Rust migration

`BuiltinFunction` adds `StringByteLen`, `StringConcat`, `StringSliceBytes` and
`StringReplaceAll`;
downstream exhaustive matches must handle them. Existing `TypedExprKind::Call`
represents all four operations.
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
