## What to build

Implement M5 literal replacement as a complete vertical slice on the concat/byte-length foundation:

```text
string_replace_all(source: string, needle: string, replacement: string) -> string
```

This is a fixed-arity, exact-type, positional-only named builtin. Matching is exact, case-sensitive UTF-8 content matching, left-to-right and nonoverlapping, without rescanning inserted text. Empty needles trap. Replacing `aa` with `b` in `aaa` yields `ba`. There are no regexes, normalization, locale rules, replacement templates, coercions, optional arguments, string `+`, or new method/postfix syntax.

The authoritative design is in the **local checkout**: `docs/adr/0010-immutable-string-operations-and-fragment-construction.md` and confirmed M5 roadmap §9 in `docs/was_compiler_memory_and_optimization_roadmap.md`. These documents are currently uncommitted; do not assume remote links resolve or mistake design confirmation for release evidence. The essential accepted semantics are reproduced here.

Evaluate source, needle, and replacement exactly once, left-to-right, before operation-specific checks. Then structurally validate strings in argument order, check operation constraints (including nonempty needle), check result size, and allocate as necessary. Existing function-entry and host-return validation stays unchanged. Argument traps stop later evaluation; completed effects and allocations are not rolled back. Structural checks do not become full UTF-8 scans: valid immutable UTF-8 remains a trusted-host obligation.

Strings remain immutable revision-1 descriptor values; aliases, interned literals, and array-held strings do not change. Initial storage policy: no-match or content-identical replacement returns the validated source; zero-byte results may reuse a shared empty string. Other results use a new descriptor and copied payload, preferably in one allocation. Construct directly into the final payload without per-match intermediate strings. Straightforward bounded-memory search is allowed: M5 does not require linear-time substring search. Broader reuse, fusion, and general string reclamation are not authorized.

## Acceptance criteria

- [ ] Implement the exact three-string signature through source analysis and checked typed-input lowering, integrating the foundation without requiring slicing or join.
- [ ] Reserve `string_replace_all` for global user functions and host-language registration. Keep it an ordinary lexer identifier so lexical bindings may shadow it; noncallable shadows give the ordinary call error without builtin fallback. Arbitrary nonempty host module/field names remain allowed.
- [ ] Reject named arguments and direct builtin pipeline stages. Preserve existing user-function pipelines, lexical-callable shadows, and calls within supported region-local placeholder stages.
- [ ] Keep replacement outside defaults/contracts' pure-expression whitelist, including skipped/unreachable source and forged typed input.
- [ ] Preserve once-only argument evaluation before builtin checks; structurally validate source, needle, replacement in order even on reuse paths, then reject empty needle. Fast paths must not suppress effects or validation of an otherwise unused operand. Retain existing entry/host-return checks and no-rollback behavior.
- [ ] Implement exact, case-sensitive, left-to-right nonoverlapping replacement without rescanning inserted text. Cover no matches, adjacent/overlapping candidates, deletion, replacement containing needle, identical source/needle/replacement contents, empty source, embedded NUL, and multibyte UTF-8.
- [ ] Apply the accepted initial reuse policy only after validation. Check content identity rather than assuming handle identity implies all required behavior; leave aliases/interned values unchanged.
- [ ] Use widened checked sizing before result allocation, preserving memory32 and allocator bounds with no arbitrary new guest size cap. Build non-reused results directly into final storage with bounded auxiliary state and no intermediate strings/collections.
- [ ] Add no implicit host imports, M4 reclaimable string storage, release of published strings, automatic fusion, lifetime-analysis integration, or mandatory full UTF-8 scans.
- [ ] Preserve `Never` compatibility and check every child independently, including skipped/unreachable/unexported code. Reject forged arity, operand types, and result categories through checked emission rather than trusting public typed annotations.
- [ ] Discover required helpers structurally with `Never`/forged annotations and nested callable bodies. Keep planning deterministic and preserve absent-M5 bytes/helper order except deliberate new-name reservation errors.
- [ ] Preserve semantic/diagnostic parity across presets, enabled-preset byte identity, and adapter byte parity within each preset—not blanket O0–O3 byte identity.
- [ ] Extend proposed frontend, forged-input/independent decoded-Wasm, and bounded runtime tests below. Use `wasmparser::Validator` plus independent assertions; cover malformed unused operands, checked expansion, retained handles after traps, growth/reentry, live stack prefixes, and repetitive adversarial search cases bounded for correctness tests.
- [ ] Update relevant implemented-scope docs without claiming full M5 completion or a linear-time search guarantee. Report actual focused validation; parent owns measurement methodology, integrated adapter/browser/release gates, and milestone acceptance.

## Implementation handoff

### Read first

- `AGENTS.md`, `docs/agents/domain.md`, and `GLOSSARY.md`.
- Local ADR 0010 and confirmed roadmap §9 named above, especially initial storage policy and round-4 replacement-complexity boundaries.
- `docs/memory-abi.md`, `docs/limitations.md`, and the delivered concat/byte-length code/tests.
- `docs/integer-specialization.md` before representation/preset changes; avoid them here.

### Main files and symbols

Existing paths/symbols below are verified; **proposed** locations depend on the foundation's final organization.

- `crates/compiler/src/host.rs::HostFunctions::register`; `semantic.rs::SemanticAnalyzer::{analyze, analyze_call}`: reserved host names, fixed builtin signature, shadowing, expected types, and argument flow.
- `crates/compiler/src/lexer.rs::Lexer::identifier` and `parser.rs::Parser::{parse_primary, parse_call_arguments}`: ordinary identifier-call handling is sufficient; do not add a keyword or AST expression shape.
- `crates/compiler/src/semantic/call_binding.rs::{analyze_bound_call, analyze_pipeline}`: existing rejection of non-user named calls and direct builtin stages.
- `crates/compiler/src/semantic/placeholders.rs::Region::expression`: fixed-signature builtin constraints already provide region-local string parameter evidence. Do not introduce caller-derived inference or reuse array-polymorphic builtin rules.
- `crates/compiler/src/semantic/pure_expression.rs::check_pure_expression` and `codegen/pure.rs::check`: preserve Len-only builtin admission.
- `crates/compiler/src/typed_ast.rs::{BuiltinFunction, TypedExprKind::Call}`; `semantic/inference.rs::LexicalWalker::expr` and `codegen/walker.rs::walk_expression`: existing call-child traversal suffices.
- `crates/compiler/src/codegen/call.rs::{emit, predicted_result}` and `expression.rs::emit_as`: independent call checks, ordered emission, and flow.
- `crates/compiler/src/codegen/memory.rs::{MemoryPlan, MemoryVisitor::enter_expression, Helper::ValidateString, Helper::StringEqual, emit_alloc}` and `codegen/abi.rs`: structural validation, equality infrastructure, allocation bounds, and descriptor constants. There is no existing substring replacement/search implementation to reuse.
- `crates/compiler/src/codegen/layout.rs::Layout::with_exports`, `function.rs::BodyEmitter`, `validation.rs::ValidatedProgram::lower`, and `mod.rs::assemble_inner`: integrate planned helper indices and checked assembly through the delivered foundation.
- **Proposed:** extend `crates/compiler/src/codegen/string.rs` if the foundation creates it; otherwise use its actual narrow helper seam. No mandatory standalone prefactor or generic search framework.
- **Proposed tests:** extend `crates/compiler/tests/string_operations_frontend.rs`, `string_operations_lowering.rs`, and `string_operations_runtime.rs`.

Representative existing tests:

- `crates/compiler/src/codegen/memory_tests/runtime.rs::node_string_utf8_empty_nul_interning_and_distinct_descriptor_content_equality`.
- `crates/compiler/src/codegen/memory_tests/runtime.rs::node_host_memory_roundtrips_reentrant_allocator_growth_and_reference_mutation`.
- `crates/compiler/src/codegen/memory_tests/runtime.rs::node_invalid_exported_and_host_descriptors_trap_before_effects`.
- `crates/compiler/tests/resizable_arrays_lowering.rs::pop_remove_remain_forbidden_in_defaults_contracts_skipped_and_unreachable_nodes`.
- `crates/compiler/tests/resizable_arrays_frontend.rs::constructor_name_is_reserved_for_hosts_but_not_external_module_or_field_names`.

### Focused commands

Run from the repository root, offline, after the proposed test targets exist:

```sh
cargo test --offline -p was-compiler --test string_operations_frontend
cargo test --offline -p was-compiler --test string_operations_lowering
cargo test --offline -p was-compiler --test string_operations_runtime -- --include-ignored
cargo test --offline -p was-compiler --lib codegen::memory
cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown
git --no-pager diff --check
```

Keep Node execution tests ignored by default. Use `tests/support/process.rs` with explicit subprocess deadlines (30 seconds is the existing resizable harness precedent), concurrent pipe draining, and bounded guest workloads. Adversarial search cases must terminate comfortably; subprocess deadlines are not guest fuel or process-tree cancellation, and `--include-ignored` is not a timeout. Apply a finite outer tool timeout to each Cargo command (for example 120 seconds) and report timeouts explicitly. Node.js 24+, the installed Wasm target, and cached dependencies are prerequisites; do not install/fetch silently. Format only edited Rust files with `rustfmt --edition 2024 --config skip_children=true` and their actual paths. Parent owns complete adapter/browser rebuild gates, integrated validation, measurements (including adversarial replacement timing), and publication.

### Direction, open questions, and execution routing

Use a sizing/validation pass followed by direct final-payload construction; reuse the foundation's checked allocation/copy infrastructure. Preserve byte matching semantics under the valid-UTF-8 contract and avoid per-match allocations. A straightforward bounded-memory search is acceptable; no need to block delivery on an advanced search algorithm. Helper factoring and exact search organization are implementation choices; validation order, empty-needle traps, initial reuse policy, and no inserted-text rescanning are settled.

Suggested model: `openai-subscribed/gpt-6.1-sol`; requested effort: **high, provisional**. The catalog does not expose effort support; verify availability rather than assuming support. Rationale: replacement combines search correctness, checked expansion arithmetic, direct final construction, and strict fast-path/trap behavior across forged and divergent trees. Escalate to the parent if the model/effort is unavailable, the foundation seam cannot support the contract, or implementation appears to require ABI changes, dependencies, whitelist expansion, broader reuse, unbounded auxiliary storage, or changed absent-M5 bytes. Do not silently change semantics or promise linear-time search/performance acceptance.
