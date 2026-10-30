# M5 private string-construction observations (#78)

## Activation and isolation

`crates/compiler/src/codegen/string_observation_tests.rs` is private `cfg(test)`
fixture machinery. It compiles through `ValidatedProgram` and ordinary assembly,
then rewrites only the requested observed module. Integration dependencies cannot
activate it. Production compilation, options, adapters, imports, helper order and
exports are unchanged. No callbacks, allocator controls or guest allocations are
introduced by observation. Host-readable globals are mutable Wasm globals, not
read-only capabilities; the fixture leaves the cursor untouched and changes one
counter only in its explicit overflow test.

The tracer measures implemented concat, not arbitrary string builtins. It also
counts all successful ordinary bump allocations, without assigning every caller a
storage category. A bounded mixed-array fixture separately decodes storage and
relocation; this distinction is important for later accumulation/join evidence.
This is an observation foundation, not final M5 construction measurements,
performance acceptance, or disposition of M4's pending performance signal.

## Event definitions

All counters start at zero and are exported only by the observed variant. For
request `q`, reservation is `max(8, align_up(q, 8))`, computed by the existing
allocator with widened arithmetic. A successful allocating concat of byte lengths
`a` and `b` requests one combined block of `8 + a + b` bytes.

| Fixture export | Increment site / meaning |
| --- | --- |
| `__m5_allocations` | One after the bump helper commits its cursor, before returning; includes every caller |
| `__m5_requested_bytes` | Original unsigned request at that success site |
| `__m5_reserved_bytes` | Actual aligned/minimum reservation at that success site |
| `__m5_string_payload_allocations` | One after concat's allocation returns, at its result `local.tee`; a subset of physical allocation events, not an additional allocation |
| `__m5_string_payload_bytes` | Concat's widened `a + b` at that same site; logical payload attribution |
| `__m5_string_descriptor_bytes` | Eight at that same site; logical descriptor attribution |
| `__m5_string_copied_bytes` | One immediately after each successfully executed payload `i32.store8` in concat; directly counted guest stores, not predicted result length |
| `__m5_cursor` | Existing bump cursor, interpreted unsigned; a snapshot, not a counter |

One successful concat contributes **one physical allocation**, one logical string
payload and one logical descriptor. Descriptor count equals payload-allocation
count for this combined-block implementation. Descriptor and payload requested
bytes partition the physical request: never add them to its physical totals as
extra storage. Padding is reserved minus requested bytes. For multiple results,
sum per-event reservations; rounding aggregate payload bytes loses per-block
padding. No mandatory physical count is imposed on future implementations.

Static inputs and initial layout precede runtime events. Host encoding/decoding,
engine storage and JS allocations are not guest copying. Other guest allocations
contribute to total counters but never acquire concat attribution. A residual is
**unattributed bump allocation**, not automatically array growth or runtime
overhead. Measuring other string operations requires separately reviewed sites.

### Failure and arithmetic

Failed allocator attempts increment none of these success counters. A trap before
allocation produces no result event. Earlier argument allocations and completed
calls remain counted. Empty-operand reuse produces no result allocation or copy,
without waiving argument effects or required validation. Host result validation
can trap before evaluation of a subsequent operand; concat's own checks happen
after its operands complete.

Successful acquisition is counted even if initialization or an enclosing call
later traps. Stores already completed remain counted; there is no rollback.
Reentrant interval deltas would include nested events and must not be summed with
overlapping outer deltas. The tracer does not export management controls or count
failed attempts. Paired capped-memory fixtures force real allocation failure
without machine exhaustion: both modules get the same initial-page maximum.

Counter arithmetic is modular i64 addition, with no new overflow traps. JS reads
BigInt; unsigned interpretation uses `BigInt.asUintN(64, value)`. Exact totals
require a separately established bound below `2^64`; wrapped residues alone do
not prove such a bound. The runtime tracer performs 15 doublings from two bytes
(final payload 65,536 bytes), fixed small setup/array operations and a separate
13-success capped sequence, so each ordinary cumulative counter is below one
million. A deliberate signed-boundary test verifies nontrapping modular addition;
its seeded snapshot is not measurement evidence. Process deadlines are neither
guest fuel nor a proof of counter bounds.

## Mixed array attribution

The paired runtime fixture constructs `resizable([])` of strings and appends
`"λ"`, `"猫!"`, and an empty fragment. It independently decodes revision-1
public descriptors and private M4 authority fields, comparing both instances.

- Construction: three physical bump allocations, 96 requested/reserved bytes:
  eight source-literal descriptor bytes, 32 allocator-record plus eight payload
  capacity bytes, and eight stable descriptor plus 40 authority bytes.
- First two appends fit eight payload bytes and allocate nothing.
- Third append replaces the payload: one 48-byte bump allocation, partitioned
  into a 32-byte allocator record and 16 bytes of capacity. The stable descriptor
  stays put; both old and new payload capacities remain physically retained.
- Completed storage after that sequence: 24 array-capacity bytes, 64 allocator
  record bytes, eight stable descriptor bytes, 40 authority bytes and eight source
  descriptor bytes = 144 physical bytes. The string counters remain unchanged.
- One observed relocation implies eight copied **handle bytes** from two prior
  four-byte entries. This is algorithm-derived relocation accounting, not a direct
  copy-store counter and emphatically not eight copied string-payload bytes.

The common allocation counters and this independent layout accounting distinguish
string payloads/descriptors, array capacity/relocation and runtime metadata in the
bounded mixed tracer. This is not automatic category coverage for arbitrary
programs. Broader M4 storage reconstruction and its inferred-copy definitions
remain in [the M4 evidence](benchmarks/m4-resizable-arrays-2026-10-29/README.md).
No new M4 allocator ownership or instrumentation hook is introduced here.

## Future construction boundary and unavailable observations

For one accumulation workload followed by one join, `n` counts fragment
**occurrences, including empty entries**. `B` includes fragment bytes by occurrence
and every output separator occurrence. Report preexisting input storage and
fragment-production costs separately from accumulation/join overhead. Repeated
joins with separately retained outputs are separate constructions. Include retained
array capacities, metadata, descriptors, new output payloads and per-block padding;
distinguish handle relocation from string copying.

Transient intra-operation old-plus-new residency and completed-call retained
storage are separate metrics. **Transient category peaks are not available from
this tracer.** Its cursor is monotonic consumed bump space, not active payload
residency. Neither pages nor completed-call M4 snapshots establish transient active
peaks. There is no generic array-copy counter, allocation-attempt counter, engine
RSS counter or join-store instrumentation yet. These absences are not zero values.
Later construction evidence must add the required observations explicitly rather
than extrapolate from this concat tracer.

A controlled nonoptimized copy-based concat baseline belongs to later construction
evidence; this contract does not require all optimized concat implementations to
remain quadratic, guarantee a speedup, or claim constant retained memory.

## Validation

The four new tests validate ordinary/observed Wasm, deterministic bytes, ordinary
nonactivation and every observation-export collision. Independent decoding removes
only the expected counter instruction sequences, checks their predecessor sites,
and recovers original bodies **including locals**, existing globals and unaffected
sections. It covers both import-free modules and imported/M4 modules, keeping
function indices distinct from code-body indices. Bounded Node execution compares
whole guest memories, independent UTF-8 content, aliases, exact event deltas,
reuse, host effects, caught-trap retention, actual growth, capped allocation
failure, mixed array accounting and counter overflow.

Run sequentially from the root with cached tools/dependencies and 600-second
external Cargo bounds:

```sh
cargo test --offline -p was-compiler --lib codegen::string_observation_tests -- --include-ignored --test-threads=1
cargo test --offline -p was-compiler --lib codegen::resizable::measurement_tests -- --skip record_m4 --test-threads=1
cargo test --offline -p was-compiler --test memory_benchmarks
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown
```

The Node case is ignored by default and uses `tests/support/process.rs` with a
60-second direct-child bound. Its modules live under `target/m5-string-observations`.
A default Cargo pass does not establish ignored runtime coverage. Browser rebuild
is unnecessary for exclusively unit-test machinery; no fresh browser evidence is
claimed.

### Recorded result — 2026-10-30

All six commands above passed (each Cargo call externally bounded at 600 seconds).
The focused M5 command executed all four tests, none ignored, including Node; the
M4 compatibility filter executed three tests. `memory_benchmarks` passed its one
default test and left four manual/Node tests ignored. The final default suite
passed, retaining its existing ignored-runtime policy. Clippy and the core Wasm
build passed. `git diff --check` also passed.

Two-axis review found no hard standards violations and one spec finding: the
initial isolation proof omitted exact export tuples. That finding was fixed by
independently checking all original ordered tuples and every added export's kind
and index for both fixtures; the spec reviewer confirmed resolution. A low-priority
suggestion to replace the small positional counter expectations with named objects
is deferred. Focused tests and Clippy were rerun after the fix, then the final
full default suite passed. No timing or broader construction acceptance is claimed.

Validation ran in a working tree containing concurrent, unrelated string-join and
design-document edits. Those changes were preserved and are not part of the #78
commit; the full-suite result describes that working tree, not an isolated clean
checkout of the commit.
