## What to build

Extract the reusable M3 allocation/release emission behind a private explicit-index interface while keeping the existing experiment behavior-identical and test-only. This is the first approved M4 prefactor, not production activation. Existing private allocation/release workloads must still run unchanged; ordinary compilation and M2 must remain unaffected.

## Acceptance criteria

- [ ] Separate reusable class/record/free-list emission from the M3 experimental assembly/transport adapter. Pass planned helper/global/backing-allocator indices explicitly rather than deriving them inside reusable emission.
- [ ] Keep the extraction behind `cfg(test)` initially. Preserve the frozen private exports/signatures, function/global ordering, observations, identity renewal, exact-class LIFO reuse, request checks, requested extents, and failure ordering.
- [ ] Keep experimental configuration, collision checks, observations and instrumentation in the private adapter. Do not introduce M4 metadata, production options, a generic allocator framework or dependencies.
- [ ] Preserve deterministic M3 emitted behavior and ordinary bytes/indices/bodies; retain M2 noninterference. Add focused boundary tests where the extraction creates a new interface; do not regenerate fixtures to conceal drift.
- [ ] Run decoded/validator and ignored runtime suites; record actual results and unavailable prerequisites.

## Implementation handoff

- **Main files (existing):** `crates/compiler/src/codegen/allocator.rs`: `Emitter::{classify, acquire, find, release}`, `append`, `compile_variant`, `Sections` own the current emission/late assembly. Extract only the reusable implementation. `crates/compiler/src/codegen/mod.rs::assemble_inner` must retain ordinary behavior. A private `allocator/core.rs` is a **proposed new file**, not an existing module or mandatory architecture.
- **Read first:** `AGENTS.md`; `docs/agents/domain.md`; `GLOSSARY.md`; `docs/m3-private-allocator.md`; `docs/m3-allocation-infrastructure.md`; memory roadmap §8; `docs/adr/0009-opt-in-resizable-array-storage.md`. M4 uses the existing array types, eight-byte descriptors and separate authority; this ticket does not enable consumers.
- **Test anchors:** `allocator/tests.rs::appended_management_preserves_existing_indices_bodies_and_ordinary_bytes`, `management_uses_one_backing_call_and_registry_authority_before_header_access`; `release_tests.rs::decoded_release_guards_exact_provenance_identity_and_state_before_pool_writes`; `failure_order_tests.rs::decoded_acquire_validates_request_and_identity_before_reading_or_removing_free_head`; `runtime_tests.rs::live_blocks_reuse_with_new_identity_and_preserve_other_storage` (all under `crates/compiler/src/codegen/`).
- **Validation (repository root):** `cargo test --offline -p was-compiler --lib codegen::allocator -- --include-ignored`; `cargo test --offline -p was-compiler --lib codegen::region -- --include-ignored`; `cargo test --offline -p was-compiler --test abi`; `cargo test --offline`; `git --no-pager diff --check`. Node 24+ and cached dependencies are prerequisites; do not fetch/install. Format only edited Rust files with the guide's command.
- **Direction/open questions:** preserve M3's 0..=2^31-byte checked request domain, power-of-two classes and exact payload/identity release authority. Internal multivalue allocation transport is not a reason to widen the language's single-result ABI abstraction. Choose a small private index interface; if extraction requires changing frozen behavior, stop and replan. No unresolved language decision gates this prefactor.
- **Suggested model:** `zed.dev/gpt-5.3-codex`.
- **Thinking effort:** medium, provisional; catalog availability is verified but supported effort controls are not exposed.
- **Rationale:** bounded behavior-preserving extraction with strong independent decoded/runtime checks; no pricing claim is made.
- **Escalation trigger:** a required change to M3 transport, ordinary bytes or M2 behavior, or unexplained failure of the focused invariants. Resolve missing tools as prerequisites rather than escalating model cost.
