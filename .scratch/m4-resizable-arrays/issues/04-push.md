## What to build

Implement `values.push(item)` end-to-end for number, bool and string arrays using the proven relocation path. Push appends an exactly typed element to the captured array's post-argument contents and returns void, without changing existing fixed-storage arrays' rights.

## Acceptance criteria

- [ ] Identifier-only positional arity-one syntax uses the established method grammar; no chaining, named/default arguments or broader method framework.
- [ ] Capture and validate receiver authority before item effects. Fixed receivers trap first. Evaluate item once with receiver-derived scalar expectation; revalidate current state, validate the new element and sizing before mutation. Receiver rebinding never redirects the operation.
- [ ] Cover both spare-capacity and replacement paths; initialize the new slot before publishing length. Replacement reuses the common acquire/copy/publish/release path, with no scan of unconsumed existing bool/string elements. Failed allocation preserves the array state after argument effects.
- [ ] Nested argument mutations/alias writes and host reentry are observed: the outer append uses current length after those effects. Ordinary/all-five-compound pending writes remain safe across push-triggered relocation and old-buffer reuse.
- [ ] Type/placeholder constraints stay exact and region-local; argument Never/flow and skipped/unreachable checks remain correct. Push remains forbidden in defaults/contracts including forged typed input. No new direct method pipeline stages/discard permissions.
- [ ] Iteration visits no appended indices beyond its captured initial length. Test alias/capture visibility, all three types, bool/string consumed checks, numeric special values, valid Wasm, independent decoded ordering and optimization-preset parity.

## Implementation handoff

- **Main files:** method forms and shared storage helpers delivered by the reserve blocker. `crates/compiler/src/semantic.rs` and `semantic/placeholders.rs::{Region, Relationship}`: exact receiver-to-element typing; `semantic/pure_expression.rs`/`codegen/pure.rs`: whitelist boundary. `codegen/array.rs` supplies existing typed widths/load/store/consumption. `codegen/control_flow.rs::emit_array_for` already retains handle/initial count and checked loads, so preserve its model rather than rewrite it. Update exhaustive raw/typed traversal where required.
- **Read first:** `AGENTS.md`, `GLOSSARY.md`, roadmap §8, ADR 0009, `docs/typed-arrays.md`, `docs/memory-abi.md`; constructor and reserve/pending-write blocker outputs; current placeholder contracts before inference edits. Capacity is private, full extent acquired, minimum eight bytes, and no shrinking/GC/host ownership adoption is authorized.
- **Test anchors:** M4 `resizable_arrays_frontend`, `resizable_arrays_lowering`, `resizable_arrays_runtime` created by predecessors; `crates/compiler/tests/array_for_loops.rs::decoded_array_for_has_exact_scratch_unsigned_count_checked_load_and_latch`; `typed_arrays_runtime.rs::boolean_layout_lazy_consumption_replacement_and_iteration`; existing decoded indexed-compound tests under `codegen/tests/compound_assignment.rs`.
- **Validation:** `cargo test --offline -p was-compiler --test resizable_arrays_frontend --test resizable_arrays_lowering`; `cargo test --offline -p was-compiler --test resizable_arrays_runtime -- --include-ignored`; `cargo test --offline -p was-compiler --test array_for_loops --test typed_arrays_runtime -- --include-ignored`; `cargo test --offline -p was-compiler --test closures_frontend placeholder`; `cargo test --offline`; `git --no-pager diff --check`. Node cases remain ignored by default and use bounded subprocesses. Do not claim ignored coverage from default Cargo results.
- **Direction/open questions:** share replacement and authority logic; do not duplicate reserve's allocator algorithm. Pop/remove can develop from the same blocker independently, but use one writer for overlapping shared files and agree interfaces before parallel work. No unresolved language choices; no implementation authorization beyond this slice.
- **Suggested model:** `zed.dev/gpt-5.3-codex`.
- **Thinking effort:** high, provisional; effort controls were not exposed by the catalog.
- **Rationale:** bounded operation extension on established relocation and method interfaces, with nontrivial nested effects.
- **Escalation trigger:** nested mutations expose an ambiguous publication/receiver contract or pending-write corruption; replan or use `openai-subscribed/gpt-6.1-sol` after isolating the failing invariant. Missing runtime tools remain prerequisites.
