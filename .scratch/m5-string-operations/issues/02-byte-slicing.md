## What to build

Implement the M5 strict byte-slicing vertical slice through the existing compiler pipeline:

```text
string_slice_bytes(source: string, start: number, end: number) -> string
```

Integrate with the concat/byte-length foundation delivered by the first tracer bullet. This is a fixed-arity, exact-type, positional-only named builtin, not a method, string indexing extension, or general postfix operation.

The authoritative design is in the **local checkout**: `docs/adr/0010-immutable-string-operations-and-fragment-construction.md` and confirmed M5 roadmap §9 in `docs/was_compiler_memory_and_optimization_roadmap.md`. Those design documents are currently uncommitted; do not assume remote links resolve or treat design confirmation as release evidence. Essential accepted semantics follow.

Slicing uses strict half-open UTF-8 byte offsets `[start, end)`. Both numbers must be finite integers with `0 <= start <= end <= byte_length`; accept negative zero as zero and never clamp. Both endpoints must be Unicode-scalar boundaries. An empty slice inside a scalar still traps. Scalar/grapheme indexing and normalization are out of scope.

Evaluate source, start, and end exactly once, left-to-right, before operation-specific checks. Then structurally validate the source, check operation constraints, check result size, and allocate as needed. Existing function-entry and host-return validation remains unchanged; argument traps stop later evaluation, and completed effects/allocations are not rolled back. Descriptor validation remains structural, not a mandatory full UTF-8 scan: valid immutable UTF-8 is a trusted-host obligation.

Initial storage policy: a full-range slice returns the validated source; zero-byte results may reuse a shared empty string. Proper nonempty slices use a new descriptor and copied payload, preferably in one allocation—not a view into source storage. Strings, aliases, interned literals, and strings in arrays remain immutable. No general reclamation or broader reuse is authorized.

## Acceptance criteria

- [ ] Implement the exact signature in frontend and checked typed-input compilation; source byte lengths remain distinct from character counts. Do not add coercions, optional arguments, string `+`, or `len(string)`.
- [ ] Reserve `string_slice_bytes` for global user functions and host-language registration, while keeping it an ordinary lexer identifier. Lexical shadows resolve normally; a noncallable shadow errors without builtin fallback. Arbitrary nonempty host module/field names stay unrestricted.
- [ ] Reject named arguments and direct builtin pipeline stages without changing existing user-function or lexical-callable pipeline rules. Calls inside supported placeholder stages use existing region-local constraints.
- [ ] Keep slicing outside defaults/contracts' pure-expression whitelist, including skipped/unreachable source and forged typed input.
- [ ] Preserve once-only source/start/end evaluation before builtin checks, existing entry/host-return validation, and completed effects after traps. Test that invalid bounds do not suppress later argument effects.
- [ ] Check finite/integral numeric domains, ordering, range, and both Unicode-scalar boundaries; accept `-0`. Cover NaN, infinities, fractions, negatives, reversed ranges, values above byte length, empty input, endpoint equal to length, and empty slices inside scalars.
- [ ] Boundary checks never load beyond the payload for endpoints equal to byte length; do not introduce full UTF-8 scans or promise recovery from host-invalid UTF-8.
- [ ] Validate before reuse. Full range reuses source; proper nonempty slices copy. Keep aliases/interned literals unchanged and source/result handles valid across growth, reentry, later allocations, and caught traps.
- [ ] Use widened checked size arithmetic and existing memory32/allocator bounds before result allocation. Add no arbitrary guest string-size cap, implicit host import, M4 reclaimable string storage, or published-string release.
- [ ] Preserve `Never` compatibility and independently check every child, including skipped/unreachable/unexported code. Reject forged arity, operand types, and result categories through checked emission rather than trusting annotations.
- [ ] Discover required memory/helpers structurally even with `Never` or forged annotations and nested helper bodies. Preserve deterministic planning and unchanged absent-M5 bytes/helper ordering, except deliberate name reservation errors.
- [ ] Preserve semantic/diagnostic parity across all presets, enabled-preset byte identity, and adapter parity within each preset; do not require O0–O3 byte identity.
- [ ] Extend proposed frontend, forged-input/independently decoded-Wasm, and bounded runtime suites below. Validate emitted modules with `wasmparser::Validator` in addition to independent checks. Include multibyte and NUL text, live stack prefixes, malformed descriptors, effects/traps, aliases, and retained handles.
- [ ] Update implemented-scope documentation without claiming full M5 completion. Report actual focused results; parent owns integrated adapter/browser/release validation and measurement work.

## Implementation handoff

### Read first

- `AGENTS.md`, `docs/agents/domain.md`, and `GLOSSARY.md`.
- Local ADR 0010 and confirmed roadmap §9 named above.
- `docs/memory-abi.md`, `docs/limitations.md`, and the delivered concat/byte-length implementation and tests.
- `docs/integer-specialization.md` before any representation/preset changes; avoid such changes in this slice.

### Main files and symbols

Existing paths/symbols below are verified; names marked **proposed** are not promises about the foundation's final design.

- `crates/compiler/src/host.rs::HostFunctions::register` and `semantic.rs::SemanticAnalyzer::{analyze, analyze_call}`: reserve/register a fixed `(String, Number, Number) -> String` builtin and retain ordinary shadowing/flow behavior.
- `crates/compiler/src/lexer.rs::Lexer::identifier`, `parser.rs::Parser::{parse_primary, parse_call_arguments}`: existing ordinary call syntax is sufficient; do not introduce a keyword or new expression variant.
- `crates/compiler/src/semantic/call_binding.rs::{analyze_bound_call, analyze_pipeline}`: existing non-user named/direct-stage rejection.
- `crates/compiler/src/semantic/placeholders.rs::Region::expression`: use fixed parameter constraints, not caller-driven inference or the polymorphic array-builtin branches.
- `crates/compiler/src/semantic/pure_expression.rs::check_pure_expression` and `codegen/pure.rs::check`: retain Len-only builtin admission.
- `crates/compiler/src/typed_ast.rs::{BuiltinFunction, TypedExprKind::Call}`; `semantic/inference.rs::LexicalWalker::expr` and `codegen/walker.rs::walk_expression` already traverse ordinary call children.
- `crates/compiler/src/codegen/call.rs::{emit, predicted_result}`, `expression.rs::emit_as`, and the foundation's helper dispatch: checked arity/types/results and left-to-right flow.
- `crates/compiler/src/codegen/memory.rs::{MemoryVisitor::enter_expression, Helper::ValidateString, emit_alloc, emit_typed_address}` and `abi.rs` descriptor constants. `emit_typed_address` is only a numeric-check example: array indices require `< length`, while slice endpoints permit `== length`. It cannot be reused unchanged.
- `crates/compiler/src/codegen/layout.rs::Layout::with_exports`, `validation.rs::ValidatedProgram::lower`, and `mod.rs::assemble_inner`: preserve planned helper indices and checked-body assembly.
- **Proposed:** extend private `crates/compiler/src/codegen/string.rs` if the foundation creates it; otherwise extend the actual delivered narrow helper seam. Do not force a separate refactor.
- **Proposed tests:** extend `crates/compiler/tests/string_operations_frontend.rs`, `string_operations_lowering.rs`, and `string_operations_runtime.rs` from the foundation.

Representative existing tests:

- `crates/compiler/src/codegen/memory_tests/validation.rs::memory_expressions_validate_with_live_stack_prefixes_and_nested_calls_at_all_levels`.
- `crates/compiler/src/codegen/memory_tests/runtime.rs::node_invalid_exported_and_host_descriptors_trap_before_effects`.
- `crates/compiler/src/codegen/memory_tests/runtime.rs::node_string_utf8_empty_nul_interning_and_distinct_descriptor_content_equality`.
- `crates/compiler/tests/resizable_arrays_lowering.rs::pop_remove_remain_forbidden_in_defaults_contracts_skipped_and_unreachable_nodes`.

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

Retain Node runtime tests' default ignored policy. Use `tests/support/process.rs` with explicit subprocess deadlines (30 seconds is the existing resizable harness precedent), concurrent pipe draining, and bounded guest workloads. `--include-ignored` is not a timeout or fuel. Apply a finite outer tool timeout to each Cargo command (for example 120 seconds), reporting timeouts explicitly. Node.js 24+, installed Wasm target, and cached dependencies are prerequisites; do not silently install/fetch. Format only edited Rust files with `rustfmt --edition 2024 --config skip_children=true` and their actual paths. Parent owns complete adapter/browser rebuild gates, integrated validation, measurements, and publication.

### Direction, open questions, and execution routing

Build on the delivered foundation rather than inventing a parallel allocator/helper system. No independent prefactor is required. Unicode boundary checks may inspect continuation-byte markers under the valid-UTF-8 host contract; explicitly handle zero and end-of-string before memory loads. Numeric checks must precede integer conversion. Internal helper organization is open; strict byte semantics, validation before reuse, and copy policy are not.

Suggested model: `zed.dev/claude-sonnet-4-6`; requested effort: **high, provisional**. The catalog does not expose effort support; verify rather than assuming availability. Rationale: a bounded vertical slice with established foundation, but subtle UTF-8 endpoint, numeric, trap-order, and forged-tree requirements. Escalate to the parent if the model/effort is unavailable, the foundation seam is incompatible, or a solution needs an ABI/whitelist change, broader reuse, full UTF-8 validation, a dependency, or changed absent-M5 bytes. Do not silently alter accepted semantics.
