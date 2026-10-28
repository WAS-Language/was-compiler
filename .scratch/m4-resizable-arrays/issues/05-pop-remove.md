## What to build

Implement element-valued `values.pop()` and `values.remove(index)` for number, bool and string arrays, preserving order, shared collection identity and capacity. Demonstrate actual source-level shrinking/shifting during pending writes and iteration.

## Acceptance criteria

- [ ] Pop takes no arguments and traps when empty; remove takes one numeric positional index using existing finite/integral/nonnegative/in-bounds rules (-0 is zero). Both return the exact receiver element type.
- [ ] Capture/check receiver and resize authority before argument effects. Remove evaluates index once and checks it against post-argument current length; receiver rebinding does not redirect it. Fixed-storage receivers trap without argument effects.
- [ ] Validate the returned bool/string element before any length/shift mutation. Invalid consumed values leave logical state unchanged. Shift remaining storage by the selected element width without scanning unreturned values.
- [ ] Preserve relative order, keep capacity/current acquisition, update public/private logical length consistently and retain referenced strings. Spare slots are not logical readable elements; no capacity shrinking, zeroing guarantee or automatic object reclamation.
- [ ] Source-level tests prove RHS shrink invalidating a captured write index causes a post-RHS trap with completed effects retained; removal shifting elements makes final stores target current index. Cover ordinary writes and all five numeric compounds with saved pre-RHS old values, aliases, captures, recursive/reentrant calls and caught traps.
- [ ] Iteration retains initial count and current checked reads: shrinking can trap a later yield, shifting affects later reads without exactly-once original-element promises. Preserve Never, region-local placeholder constraints, skipped/unreachable checking, pure-whitelist rejection and forged-tree checks.
- [ ] All three types receive frontend, validator, independently decoded and bounded runtime coverage, including numeric NaN/infinity/signed-zero/subnormal payload preservation as applicable and optimization-preset parity.

## Implementation handoff

- **Main files:** reuse reserve blocker's method/authority/runtime representation. `crates/compiler/src/semantic.rs`, `semantic/placeholders.rs` for exact result/argument relationships. Shared private M4 helpers plus `codegen/array.rs::{load,store,consume,width}` for consumption and movement. `codegen/place.rs` provides captured destination revalidation; `codegen/control_flow.rs::emit_array_for` provides initial-count iteration. Do not independently reimplement authority or mutation publication per method.
- **Read first:** `AGENTS.md`, `GLOSSARY.md`, roadmap §8, ADR 0009, `docs/memory-abi.md`, `docs/typed-arrays.md`, reserve/pending-write blocker outputs and applicable placeholder/capture contracts. This ticket does not require push to be completed: do not add a false dependency.
- **Test anchors:** predecessor-created M4 frontend/lowering/runtime targets; `crates/compiler/tests/typed_arrays_runtime.rs::{boolean_layout_lazy_consumption_replacement_and_iteration,string_handles_are_lazy_and_equality_compares_contents}`; `array_for_loops.rs::node_descriptor_entry_traps_even_empty_and_each_element_rechecks_descriptor`; `codegen/tests/compound_assignment.rs` saved-old and scratch-order cases.
- **Validation:** `cargo test --offline -p was-compiler --test resizable_arrays_frontend --test resizable_arrays_lowering`; `cargo test --offline -p was-compiler --test resizable_arrays_runtime -- --include-ignored`; `cargo test --offline -p was-compiler --test array_for_loops --test typed_arrays_runtime --test compound_assignment -- --include-ignored`; `cargo test --offline -p was-compiler --test closures_frontend placeholder`; `cargo test --offline`; `git --no-pager diff --check`. Preserve ignored runtime policy and bounded subprocess helper, but explicitly execute the cases for acceptance.
- **Direction/open questions:** the pre-RHS saved old numeric value remains authoritative for compounds even if the RHS changes/shifts elements; final destination uses current index for opt-in arrays only. Old fixed-storage place semantics remain unchanged. Coordinate a single writer for shared files with the push slice; shared ownership is not a semantic push dependency.
- **Suggested model:** `zed.dev/gpt-5.3-codex`.
- **Thinking effort:** high, provisional; supported effort configuration is not exposed.
- **Rationale:** bounded pair of operations sharing authority/length publication, with subtle consumption and suspended-write semantics.
- **Escalation trigger:** shrink/shift reveals a gap in established revalidation, lazy consumption or publication invariants; isolate it and replan/escalate to `openai-subscribed/gpt-6.1-sol` rather than adding pins or changing contracts.
