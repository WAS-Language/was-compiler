## What to build

Add a private, deterministic, test-only allocation/copy observation seam, starting with a vertical tracer bullet through implemented `string_concat`. The concat/byte-length foundation is #73; its native blocking edge is configured on this issue.

The seam must distinguish string-payload allocations and payload bytes copied from string descriptors, array allocation/growth and handle relocation, and other runtime overhead. It must support the later fragment-accumulation/join evidence without adding production telemetry, a public compiler option, instrumentation imports, or allocator-management exports. Establish event definitions and ordinary/observed isolation before extending workload breadth.

M5 strings are immutable UTF-8 values using the existing revision-1 descriptor and retained-handle contract. Concat evaluates arguments once, left-to-right, then structurally validates strings in argument order before result-size checks/allocation. Empty-operand concat returns the other operand under the initial policy; required validation/effects cannot be skipped. New results copy to their final payload with checked memory32 sizing; published strings are not reclaimed or enrolled in M4 payload recycling. Trusted hosts remain responsible for valid immutable UTF-8; structural validation is not a mandatory full UTF-8 scan.

## Acceptance criteria

- [ ] Exercise a nonempty allocating concat and empty-operand reuse, with independent content, alias-preservation, event and copied-byte expectations. Define successful allocation events, requested/reserved bytes, logical descriptor versus payload attribution, and behavior when a call traps. A combined descriptor/payload allocation must not be double-counted as physical storage.
- [ ] Deterministic counters distinguish string-payload allocations/copies, descriptor allocations/storage, array allocation/growth and handle relocation, and other runtime overhead. Do not label inferred relocation bytes as directly counted string copies or count JS decoding as guest copying.
- [ ] Instrumentation remains private `cfg(test)` fixture machinery. Ordinary compilation, adapters, imports, exports, bytes and helper ordering are unaffected; integration dependencies cannot activate it. No production telemetry, callbacks, management controls or public switch.
- [ ] Validate observed and ordinary Wasm; independently decode instrumentation sites and prove that removing only the intended additions recovers original bodies/locals and unaffected sections. Test deterministic bytes, export collision handling and nonactivation in ordinary modules.
- [ ] Execute paired ordinary/observed bounded fixtures against independent values/effect expectations, including growth and caught-trap retention where supported. Observation itself must not allocate guest storage or change the counted operation's callback/trap order.
- [ ] Publish definitions sufficient for later O(n + B) evidence: fragment occurrences include empty entries; B includes fragment bytes by occurrence and output separator occurrences. Separate preexisting input storage and fragment-production costs from accumulation/join overhead; distinguish array-handle relocation from string-payload copying.
- [ ] Report transient intra-operation storage peaks separately from completed-call retained storage. Existing completed-call M4 snapshots alone cannot satisfy a transient-peak claim. Explicitly identify any observation not yet available rather than infer it from pages.
- [ ] No mandatory physical allocation count, guaranteed speedup, constant-retained-memory claim, or requirement that optimized concat remain quadratic. A controlled nonoptimized copy-based concat baseline belongs to construction evidence, not a constraint on all implementations.

## Implementation handoff

### Read first

- `AGENTS.md`; `GLOSSARY.md`; `docs/agents/domain.md`.
- `docs/adr/0010-immutable-string-operations-and-fragment-construction.md`; `docs/was_compiler_memory_and_optimization_roadmap.md` §9, especially instrumentation and round-4 accounting. These design documents may still be uncommitted; essential rules are embedded above.
- `docs/benchmarks.md`; `docs/validation.md`.
- `docs/benchmarks/m4-resizable-arrays-2026-10-29/README.md` observation definitions/isolation; `docs/m3-private-allocator.md` before changing shared allocator emission; `docs/m4-release-readiness.md`.

### Main verified anchors

- `crates/compiler/src/codegen/resizable_measurement_tests.rs`: `modules`, `observe`, `increment`, `observations_are_private_and_ordinary_compilation_is_unchanged`, `counters_only_add_exact_increments_after_live_registry_guards`, `observation_export_collisions_are_private_to_the_requested_variant`.
- `crates/compiler/src/codegen/resizable.rs`: `#[cfg(test)] mod measurement_tests` ownership.
- `tests/support/m4_resizable_measure.cjs`: `accountingSession`, `inspect`, `operation`; current string-array slot accounting excludes referenced string payloads. `inferredCopiedLiveBytes` excludes constructor copies; peaks are completed-call only.
- `crates/compiler/tests/memory_benchmarks.rs`: marker/capacity separation. Markers measure aligned bump consumption; pages measure capacity, neither supplies categorized copying.
- `tests/support/process.rs`: `output`, `output_with_stdin` bounded subprocesses.

### Direction and open questions

Prefer the existing checked-backend/private-fixture pattern, not another IR or production instrumentation architecture. New M5 observation files, counters, fixture helpers and test names are **proposed**, not existing targets. Decide exact event placement, counter overflow/domain policy, allocation-failure accounting and transient-peak transport after inspecting #73's actual helpers. Existing M4 fixture indexing excludes imports; do not silently reuse that assumption for host-effect tests. Escalate if coverage needs a production switch, changes ordinary bytes, widens allocator ownership or cannot observe required categories independently.

### Exact validation and bounds

Run from repository root, offline, sequentially; give each Cargo command an external 600-second bound. Existing nonrecording observation compatibility check:

```sh
cargo test --offline -p was-compiler --lib codegen::resizable::measurement_tests -- --skip record_m4 --test-threads=1
cargo test --offline -p was-compiler --test memory_benchmarks
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown
git --no-pager diff --check
```

Add and record exact focused commands for the newly implemented M5 tests; no M5 target is asserted here. Use `tests/support/process.rs` for finite Node children with an explicit deadline; deadlines are not guest fuel or process-tree cancellation. Cached tools/dependencies only; report missing prerequisites. Do not claim these existing checks alone validate new counters.

### Suggested execution

Model: `openai-subscribed/gpt-6.1-sol`; effort: **high**, provisional planning recommendation, not an exposed catalog capability. Rationale: subtle attribution, stack-neutral instrumentation, failure order and independent isolation proof. Escalate ambiguous accounting or any production-boundary change before implementation. This ticket neither disposes of M4's pending performance signal nor authorizes M4 release acceptance.
