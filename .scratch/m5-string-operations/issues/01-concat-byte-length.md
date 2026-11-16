## What to build

Implement the first M5 tracer bullet end-to-end through the existing compiler pipeline:

```text
string_byte_len(source: string) -> number
string_concat(left: string, right: string) -> string
```

Both are fixed-arity, exact-type, positional-only named builtins. Byte length counts UTF-8 bytes, not scalars or graphemes. Do not add string `+`, `len(string)`, methods, coercions, optional arguments, or general postfix chaining.

The authoritative design is in the **local checkout**: `docs/adr/0010-immutable-string-operations-and-fragment-construction.md` and the confirmed M5 section (§9) of `docs/was_compiler_memory_and_optimization_roadmap.md`. These design documents are currently uncommitted; do not assume remote links resolve or that design confirmation itself is release evidence. The essential accepted decisions are reproduced here so implementation does not depend on a published link.

Strings remain immutable revision-1 descriptor values. Concat must leave aliases, interned literals, and strings stored in arrays unchanged. Reassignment changes a binding, not the prior string. Evaluate arguments exactly once, left-to-right, before operation-specific checks; then structurally validate string operands in argument order, check result size, and allocate as needed. Existing function-entry and host-return validation remains unchanged. An argument trap stops later evaluation; completed effects and allocations are not rolled back.

Initial storage policy: concat with an empty operand returns the other validated operand; zero-byte results may reuse a shared empty string. Other concat results get a new descriptor and copied payload, preferably in one allocation. Fresh identity is not a general language guarantee, but broader reuse is not part of this ticket. Byte length validates its source without allocating a result string. Structural descriptor validation does not scan all UTF-8: valid immutable UTF-8 remains a trusted-host obligation.

## Acceptance criteria

- [ ] Both builtins work in source and checked typed-input compilation, including internal calls, captured values, inferred results, and nested expressions, with exact signatures and no conversions.
- [ ] Reserve both names for global user functions and host-language registration. Keep them ordinary lexer identifiers so lexical bindings can shadow them; a noncallable shadow gives the ordinary call error without builtin fallback. Arbitrary nonempty host module/field names remain allowed.
- [ ] Reject named arguments and direct builtin pipeline stages. Preserve ordinary user-function pipelines and lexical callable shadowing; a parenthesized placeholder stage may call a builtin in its body under existing rules.
- [ ] Keep both operations, including byte length, outside defaults/contracts' pure-expression whitelist. Reject skipped/unreachable source uses and forged typed uses without expanding the whitelist.
- [ ] Evaluate concat arguments once in order before builtin validation, validate left then right even when a fast path would not use one operand, and preserve existing entry/host-return checks and trap ordering.
- [ ] Byte length returns the exact unsigned byte length as `number`; cover empty, embedded NUL, ASCII, and multibyte text. `len(string)` and string arithmetic remain rejected.
- [ ] Concat checks widened result-size arithmetic and memory32/allocator limits before result allocation, with no arbitrary new guest size cap or implicit host import. Preserve immutable inputs and retained handles across later allocations, growth, reentry, and caught traps.
- [ ] Implement initial reuse/copy behavior above. Do not release published strings, use M4 reclaimable payload storage for results, introduce general reclamation, or perform concatenation fusion.
- [ ] Preserve `Never` compatibility and flow composition while independently checking every child, including skipped/unreachable/unexported code. Forged typed calls receive independent arity, operand-type, and result-category checks.
- [ ] Structural planning discovers the operations even with `Never`/forged annotations and nested callable bodies; helper availability must not depend only on concrete result types.
- [ ] Programs without M5 operations retain existing bytes/helper ordering, apart from intentional new-name reservation errors. Output remains deterministic. Preserve semantic/diagnostic parity across all presets, enabled-preset byte identity, and adapter byte parity within each preset—not blanket O0–O3 identity.
- [ ] Add frontend, forged-input/decoded-Wasm, and bounded runtime tests at the proposed locations below. Use `wasmparser::Validator` plus independent decoded assertions; cover live stack prefixes, malformed descriptors, argument effects, fast-path validation, aliases/interned literals, growth/reentry/traps, and preset behavior.
- [ ] Update relevant language/API documentation and `AGENTS.md` for implemented scope without claiming all M5 is complete. Record actual focused validation; parent-owned integrated adapter/release validation and measurements remain separate.

## Implementation handoff

### Read first

- `AGENTS.md`, `docs/agents/domain.md`, and `GLOSSARY.md`.
- Local ADR 0010 and roadmap §9 named above, including initial storage policy and measurement boundaries.
- `docs/memory-abi.md`, `docs/limitations.md`, and `docs/compilation-roadmap.md`.
- `docs/integer-specialization.md` before changing preset/representation behavior; avoid such changes here.
- Existing affected implementation and neighboring tests before editing.

### Main files and symbols

Paths below are existing unless explicitly marked **proposed**.

- `crates/compiler/src/lexer.rs::Lexer::identifier`: keep names as identifiers, not keywords.
- `crates/compiler/src/parser.rs::Parser::{parse_primary, parse_call_arguments}`: ordinary `Expr::Call` already fits; no new syntax node is needed.
- `crates/compiler/src/host.rs::HostFunctions::register`: explicit host-language reserved-name list.
- `crates/compiler/src/semantic.rs::SemanticAnalyzer::{analyze, analyze_call}`: builtin signatures, global collision handling, lexical shadow rejection, expected argument contexts, and flow.
- `crates/compiler/src/semantic/call_binding.rs::{analyze_bound_call, analyze_pipeline}`: existing non-user named-call/direct-stage rejection.
- `crates/compiler/src/semantic/placeholders.rs::Region::expression`: fixed-signature non-user calls already constrain placeholders locally; do not copy polymorphic `Len`/`Resizable` rules.
- `crates/compiler/src/semantic/pure_expression.rs::check_pure_expression` and `crates/compiler/src/codegen/pure.rs::check`: only builtin `Len` is admitted; retain rejection of new operations.
- `crates/compiler/src/typed_ast.rs::{BuiltinFunction, FunctionRef, TypedExprKind::Call}`: add builtin variants, retaining the existing call representation.
- `crates/compiler/src/semantic/inference.rs::LexicalWalker::expr` and `crates/compiler/src/codegen/walker.rs::walk_expression`: already traverse call arguments; no new structural traversal/IR is required.
- `crates/compiler/src/codegen/call.rs::{emit, predicted_result}` and `expression.rs::emit_as`: checked call lowering and child flow.
- `crates/compiler/src/codegen/memory.rs::{MemoryPlan, MemoryVisitor::enter_expression, Helper, emit_alloc, emit_typed_len, emit_planned}`: existing allocation/validation/length patterns. Explicit builtin memory discovery currently recognizes only `Len | Resizable`; account for new calls even when annotations are `Never`.
- `crates/compiler/src/codegen/abi.rs`: centralized descriptor constants and signatures; `values.rs::emit_array` demonstrates a single descriptor-plus-payload allocation.
- `crates/compiler/src/codegen/layout.rs::Layout::with_exports`, `function.rs::BodyEmitter`, `validation.rs::ValidatedProgram::lower`, and `mod.rs::assemble_inner`: planned signatures/indices, checked body caching, and assembly.
- **Proposed optional seam:** private `crates/compiler/src/codegen/string.rs` for narrowly scoped M5 helper discovery/planning/emission. Establish it within this vertical slice if useful; no mandatory separate prefactor or generic helper framework. Do not unconditionally grow `Helper::ALL` and change absent-M5 output.
- **Proposed tests:** `crates/compiler/tests/string_operations_frontend.rs`, `string_operations_lowering.rs`, and `string_operations_runtime.rs`.

Representative existing tests:

- `crates/compiler/src/codegen/memory.rs::tests::helper_indices_and_signatures_are_fixed`.
- `crates/compiler/src/codegen/memory_tests/layout.rs::static_strings_are_utf8_byte_descriptors_interned_in_deterministic_preorder`.
- `crates/compiler/src/codegen/memory_tests/validation.rs::len_checks_arity_and_rejects_strings_in_source_and_typed_ast`.
- `crates/compiler/src/codegen/memory_tests/runtime.rs::node_string_utf8_empty_nul_interning_and_distinct_descriptor_content_equality`.
- `crates/compiler/tests/resizable_arrays_frontend.rs::constructor_uses_ordinary_identifier_resolution_and_lexical_shadowing` and `constructor_checks_call_structure_before_argument_expressions`.

### Focused commands

Run from the repository root, offline, after creating the proposed test targets:

```sh
cargo test --offline -p was-compiler --test string_operations_frontend
cargo test --offline -p was-compiler --test string_operations_lowering
cargo test --offline -p was-compiler --test string_operations_runtime -- --include-ignored
cargo test --offline -p was-compiler --lib codegen::memory
cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown
git --no-pager diff --check
```

Keep Node-dependent runtime tests ignored by default. Use `tests/support/process.rs` with an explicit deadline (the existing resizable runtime harness uses 30 seconds), concurrent pipe draining, and bounded guest workloads; Cargo's `--include-ignored` is not a timeout or guest fuel. Run each Cargo command through a finite outer timeout (for example 120 seconds; report timeout rather than treating it as success). Node.js 24+, the installed Wasm target, and cached dependencies are prerequisites; do not fetch/install silently. Format only edited Rust files with `rustfmt --edition 2024 --config skip_children=true` followed by their actual paths. Parent owns the complete adapter/browser rebuild gates, integrated validation, measurements, and publication.

### Direction, open questions, and execution routing

No implementation prerequisite or standalone refactor blocker is identified. Keep the implementation as a complete vertical slice and leave slicing/replacement/join absent until their own work. The helper seam, helper ordering among newly introduced operations, and empty-string representation are implementation choices constrained by unchanged absent-M5 bytes and the accepted validation/reuse policy; they are not permission to reopen semantics.

Suggested model: `openai-subscribed/gpt-6.1-sol`; requested effort: **high, provisional**. The model catalog does not expose effort support, so verify availability rather than assuming the setting is supported. Rationale: this slice establishes checked lowering, memory/helper integration, and behavior-preserving boundaries used by later slices. Escalate to the parent if the model/effort is unavailable, a true contract ambiguity appears, or a proposed solution needs an ABI change, dependency, whitelist expansion, broader reuse, allocator/lifetime redesign, or changed absent-M5 bytes. Do not make those changes silently.
