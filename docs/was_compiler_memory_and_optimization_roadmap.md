# WAS Compiler — Memory, Numeric Representation, and Optimization Roadmap

**Status:** Proposed technical plan; M0 implemented and validated (later milestones remain proposals)

**Updated:** October 26, 2026

**Version:** 2.0 — Goose-informed memory strategy  
**Repository:** https://github.com/LuanRoger/was-compiler  
**Scope:** Stabilize runtime values and memory management; introduce conservative escape/lifetime analysis and temporary regions; add safe reclamation for shared heap objects and resizable arrays; preserve practical immutable string operations; introduce `i32`/`f64` specialization and an optional Binaryen pipeline.  
**Explicitly deferred:** Multi-file modules, package management, additional execution targets, the Component Model, and a native-code backend.

> **Evidence boundary.** The “Current state” section summarizes the repository's public `main` branch and the previously conducted architecture review. Implementation proposals, hypothetical examples, and acceptance criteria are not claims about existing functionality. Paths and behavior should be revalidated against the branch used when each milestone begins. No tests were executed during the original plan preparation; subsequent M0 implementation and validation evidence is linked in §4.

---

## 1. Executive summary

The WAS compiler already has a useful foundation: a Rust frontend and typed representation, a `wasm-encoder` backend, Core WebAssembly output, typed arrays, immutable UTF-8 strings, ranges, local closures with shared captures, and a documented memory ABI. The key limitation is not the correctness of basic linear-memory allocation; it is **object lifetime**. The current monotonic allocator grows memory but does not reclaim dead objects or obsolete buffers.

The revised progression is:

1. **Freeze semantics and establish memory regression tests (M0).** Make aliases, host ownership, reentrancy, traps and ABI invariants explicit.
2. **Introduce conservative escape and lifetime analysis (M1).** Classify storage as scalar, static, temporary-region, or shared/escaping heap without adding source-language ownership syntax.
3. **Prototype reclaimable temporary regions (M2).** Reuse storage when all allocations in a region are proven dead; begin with narrowly proven local cases.
4. **Add managed-heap allocation/reuse infrastructure (M3).** Retain the checked bump allocator as a fallback; do not conflate allocation reuse with determining whether an object is dead.
5. **Implement resizable arrays (M4) and practical immutable string operations (M5).** Preserve aliasing, descriptor identity and host ABI guarantees.
6. **Refine lifetime and storage representations (M6).** Optimize nonescaping closure environments, ranges and temporary objects; selectively consider inline layouts.
7. **Specialize numeric operations (M7).** Preserve source `number` semantics and public `f64` boundaries when choosing `i32` internally.
8. **Add language-aware optimization (M8) and optional Binaryen integration (M9).** Measure and validate every stage against unoptimized Wasm.

Continuous regression tests and benchmarks gate all milestones.

**Main recommendation:** Adopt a **hybrid region + managed-heap architecture** inspired by Goose, not Goose’s data-stack layout verbatim. Regions may reclaim provably temporary allocations cheaply; arrays or objects with uncertain/shared lifetimes still need safe heap allocation and an explicit reclamation policy. Do not equate `free` with automatic memory management.

---

## 2. Verified baseline and existing constraints

| Area | Current design / evidence | Consequence |
|---|---|---|
| WebAssembly representation | Core Wasm, `memory32`, `wasm-encoder` | The runtime operates on integer memory offsets and linear memory. |
| Memory provisioning | Monotonic bump allocation; 8-byte allocation alignment; guarded `memory.grow` | Allocation is simple, but memory is not recovered during an instance's lifetime. |
| Allocation safety | Allocation arithmetic uses widened `i64` intermediates and checks growth/limits before committing the heap pointer | Preserve these invariants if adding a free-list allocator. |
| Public string/array descriptor | 8 bytes: `data_ptr: u32` at `+0`, `length: u32` at `+4` | Public consumers and validation code assume this layout. |
| `number[]` payload | Eight bytes per element (`f64`) | Must not silently change because an optimizer discovers integral values. |
| `bool[]` payload | One byte per element | Typed allocation and validation must remain consistent. |
| `string[]` payload | Four-byte string handles per element | String lifetimes interact with arrays and host-visible handles. |
| Strings | UTF-8, immutable; literal interning in the static data region | In-place edits of interned objects would be unsafe without a different representation. |
| Arrays | Mutable elements; fixed logical length; reference/handle-sharing on assignment and calls | Resizing must update all aliases consistently. |
| Ranges | 32-byte private records containing progression metadata | Many direct `for` loops could eventually avoid record allocation. |
| Closures | Locally callable helpers; shared by-reference captures; 8-byte capture cells and environment slots | Correct lifetime and reentrancy are essential; current allocations are monotonic. |
| `number` ABI | `f64` for function parameters/results and numeric-array elements | Internal `i32` specialization must insert conversions at boundaries and preserve observable semantics. |
| Host integration | Node and browser adapters use documented descriptor handles | ABI changes require coordinated updates and regression tests. |

### Relevant implementation entry points

- [`crates/compiler/src/codegen/memory.rs`](https://github.com/LuanRoger/was-compiler/blob/main/crates/compiler/src/codegen/memory.rs) — memory planning, static string interning, allocator, validators, equality helpers.
- [`crates/compiler/src/codegen/abi.rs`](https://github.com/LuanRoger/was-compiler/blob/main/crates/compiler/src/codegen/abi.rs) — descriptor offsets, payload widths, range records, capture-cell sizing, Wasm type mapping.
- [`crates/compiler/src/codegen/values.rs`](https://github.com/LuanRoger/was-compiler/blob/main/crates/compiler/src/codegen/values.rs) — allocation and lowering of value constructors.
- [`docs/memory-abi.md`](https://github.com/LuanRoger/was-compiler/blob/main/docs/memory-abi.md) — public contract and host integration guidance.
- [`README.md`](https://github.com/LuanRoger/was-compiler/blob/main/README.md) — language implementation status and validation milestones.

### Baseline memory model

```text
Static data / interned literals
           |
           v
[ reserved ][ descriptors and static bytes ][ heap allocations ... ][ available space ]
                                                 ^
                                                 monotonic heap cursor

Every heap allocation moves the cursor forward.
Dead arrays, ranges, closure environments, and superseded buffers do not automatically return space.
```

**Strength worth retaining:** allocations are explicit in generated Wasm and the compiler already separates runtime helpers from most source-language semantics.

---

## 3. Design principles and invariants

1. **Preserve source semantics before optimizing representation.** A backend optimization may change the implementation, not the results or specified traps.
2. **Keep host-visible handles stable.** Resizing an object must not invalidate aliases to its object descriptor. Payload pointers may move only if no outstanding contract promises their stability.
3. **Document ownership at each boundary.** Distinguish compiler-owned data, host-allocated data, static interned data, and temporary runtime storage.
4. **Do not reclaim live aliases.** In particular, arrays, string handles in arrays, closure captures, and exported values may remain reachable after an expression or function returns.
5. **Avoid in-place mutation of shared immutable data.** Literals and aliased immutable strings must not change unexpectedly.
6. **Use conservative fallbacks.** Unknown numeric ranges use `f64`; unknown object lifetime uses safe nonreclamation until a sound reclamation strategy exists.
7. **Make failures observable and testable.** Define invalid indexing, capacity overflow, allocation failure, partial modification, and mutation-during-iteration behavior.
8. **Keep Core Wasm portable.** Neither Binaryen nor a new allocator should become a required runtime import.
9. **Prefer incremental changes.** Establish tests and invariants, then evolve the existing backend without a mandatory large IR rewrite.
10. **Region reset requires proof of aggregate death.** A lexical scope boundary alone never authorizes rewinding a global heap cursor. Escaping values and interleaved heap allocations must be isolated.
11. **Separate stable handles from movable payloads.** `memory.grow` preserves Wasm linear-memory offsets, but an array resize may move its payload.
12. **Keep analysis conservative.** When aliasing, host effects, or reentrancy is unknown, use the established heap path, not optimistic region reclamation.

---

## 4. Milestone M0 — Semantics and regression baseline

**Goal:** Define the behavioral contract before changing the representation.

### M0 interview decisions (2026-10-26)

The user confirmed the consolidated scope and authorized M0 implementation on
2026-10-26. M0 is implemented with [recorded coverage, measurements and passing
validation](m0-memory-baseline.md). These decisions do not authorize implementation
of later milestones; future-feature constraints below remain proposals.

- **M0 scope:** documentation, focused regression tests and test-only measurement
  support. Preserve current language behavior, emitted ABI, allocation policy and
  adapters. No escape analysis, reclamation, capacity fields or numeric-policy
  changes. Future resize/string-operation contracts remain explicitly proposed.
- **Collection direction:** M4 proposes resizing the existing `number[]`, `bool[]`
  and `string[]` types, not adding a separate growable-list type. Resizing through
  one alias would change the same collection observed by all aliases. Existing
  arrays remain fixed-length until that feature is implemented. Future operation
  syntax remains deferred; the proposed iteration interaction is defined below.
- **Raw-host compatibility:** preserve existing instance-lifetime storage and raw
  handle-retention rights without retroactive root-registration, release or
  nonretention obligations. A compliant host's retained handle remains valid
  after a guest trap and during later calls. Future reclamation must respect this
  contract or use an explicitly separate opt-in ownership protocol; return or
  trap alone cannot invalidate retained handles.
- **Validity is not ownership:** descriptor bounds/alignment checks do not prove
  allocation provenance or authorize freeing, relocation or resizing. Compliant
  raw descriptors remain accepted unchanged. Explicit Node encoding copies into
  guest storage, but current descriptors contain no ownership metadata. A future
  management protocol must establish management rights separately; M0 adds none.
- **Failure and effects:** a failed allocation does not advance the allocator
  cursor or corrupt existing descriptors. Completed writes, host effects and
  earlier successful allocations survive traps; there is no operation-wide
  rollback. Future resize replacement-allocation failure must leave the old
  descriptor/storage valid, without rolling back prior argument effects.
- **Proposed string operations:** require immutable result contents without
  modifying existing strings, not fresh allocation or distinct physical identity
  on every invocation. Unchanged/already available results may reuse storage.
  Allocation and failure rules remain operation-specific future decisions.
- **Measurement boundary:** use existing test-only measurements, with initial/final
  pages on fresh unmarked runs and aligned bump consumption on separate marked
  runs, correcting marker overhead. Pages are capacity, not live bytes. General
  allocation counts and peak live bytes remain unavailable unless independently
  measured; never infer them from page growth. No new production counters,
  exports or allocator behavior are authorized.
- **Proposed resize/iteration semantics:** capture the handle and initial length
  once; read current elements by checked index, without an element snapshot or
  cached payload pointer. Appends beyond the initial length are not visited. If
  shrinking invalidates a subsequent index, that read traps rather than ending
  iteration early. Any future shifting removal affects subsequent indexed reads;
  there is no exactly-once guarantee for original elements.
- **Proposed pointer contract:** descriptor handles remain stable across resize,
  but payload addresses need not. Hosts must reacquire payload addresses after
  calls/reentrant callbacks that may resize, separately from refreshing JavaScript
  views after memory growth. M0 adds no pinning API and does not weaken current
  fixed-storage lifetime guarantees. The original proposal required future lowering
  to preserve pre-RHS checked assignment places across relocation. M4's confirmed
  opt-in design explicitly revises this proposal for resizable arrays: validate
  before the RHS, then revalidate the captured handle/index and resolve current
  storage before the final store (see §8). Existing fixed-storage writes retain
  their saved checked-address semantics.
- **Contract revision:** identify the existing memory/ownership contract as
  revision 1 in a dated compatibility section of `memory-abi.md`. This is a
  documentation revision, not a new binary ABI, version export, descriptor tag or
  runtime negotiation mechanism. Keep future features outside the implemented
  revision's guarantees.
- **Acceptance evidence:** map contracts to existing tests and add focused coverage
  only for gaps, including retained `bool[]` and `string[]` through reentry, caught
  traps and actual growth. Require fresh offline fast/full validation, ignored
  runtime cases and current-source rebuilt-browser evidence, plus separate
  page/consumption records with workload/tool-version provenance. Missing tools
  leave gates pending; historical results and default Cargo success alone do not
  close M0. Preserve compatibility fixtures; proposals for unimplemented features
  do not require executable feature tests in M0.

### Contract summary (existing guarantees versus future proposals)

| Topic | Proposed decision | Reason |
|---|---|---|
| Array assignment | Reference semantics: `b = a` refers to the same array object | Matches current behavior. |
| Array growth | Proposed: descriptor/handle stable; backing payload may move | Lets aliases observe resize; not implemented by M0. |
| Array iteration | Initial length and live checked reads; proposed shrinking traps at an invalid next index | Appends do not extend iteration; no element snapshot. |
| Array bounds | Checked access; invalid indices trap | Matches the existing checked model. |
| String assignment | Immutable value/handle semantics | Prevent accidental mutation of shared literals. |
| String edits | Proposed immutable results; fresh allocation is not mandatory | Keeps shared strings safe without requiring allocation for unchanged results. |
| Host-originated arrays | Preserve raw acceptance; validity does not establish management rights | Future resize/reclamation needs a separately specified management protocol. |
| Raw memory addresses | Proposed: reacquire payload addresses after possible resize; no pinning API in M0 | Distinct from JavaScript view refresh after growth. |
| Allocation failure | Failed allocation leaves cursor unchanged; completed effects remain | Preserve live descriptors without promising transactional rollback. |
| Reentrancy | Recursive and host-reentrant invocations must not share mutable scratch unsafely | Necessary for closure correctness. |

### Deliverables

- A short **memory/ownership contract revision 1**, updating `docs/memory-abi.md`
  with a dated compatibility section and no binary ABI or runtime-version changes.
- A contract-to-test coverage map for existing strings, typed arrays, captures,
  exports, traps and memory growth; reuse current suites and add focused tests for
  uncovered guarantees, including integrated bool/string-array retention scenarios.
- Test-only measurements of memory pages and aligned bump consumption on separate
  runs using existing support; explicitly mark allocation counts and peak live
  bytes unavailable unless independently measured. Do not add production telemetry.
- An explicit policy for compatibility with existing Node/browser marshaling.

**Exit gate:** Existing tests pass without weakening assertions or regenerating
compatibility fixtures; uncovered implemented guarantees receive focused tests.
Complete the offline fast/full validation in [validation.md](validation.md), with
ignored runtime cases and a current-source browser rebuild, and record separate
page/consumption measurements with workload/tool-version provenance. Missing
prerequisites leave gates pending. No language, binary ABI, allocator or adapter
behavior changes are permitted in M0; future-feature proposals remain labeled and
are not claims of tested functionality. The 2026-10-26 implementation meets this
M0 gate; see the [evidence record](m0-memory-baseline.md#validation-2026-10-26).

---

## 5. Milestone M1 — Conservative escape and lifetime analysis

**Goal:** Identify provably temporary objects before changing the allocator. The analysis must remain conservative, with the current heap as a sound fallback.

**M1 status — 2026-10-26:** #50–#52 implement private `cfg(test)` classification
and noninterference tests; #53's fresh offline fast/full gates passed, including
all ignored native runtimes and current-source rebuilt-browser tests. The
[implemented contract, test map and evidence](m1-lifetime-analysis.md) record
1,392 native plus 13 browser tests passed, none failed/ignored in the full runs.
Only direct function-body let-bound array literals are inventoried; the positive
set is the closed number/bool-array subset below. There is no production or
emission consumer. Classification selects no storage and changes no bytes, ABI,
presets, allocator or host retention rights. M2, reclamation and managed-heap
support remain unimplemented and unauthorized; no memory/performance improvement
is claimed. Broader design examples and future proof obligations below are not
implemented M1 interfaces.

### M1 interview decisions (2026-10-26; confirmed)

The user confirmed the consolidated M1 contract on 2026-10-26, completing the
interview. The decisions below define the agreed scope subsequently implemented
under #50–#53. They do not authorize reclamation in M2.

- **Analysis only:** internal classification and classification tests, without
  changes to allocation, emitted Wasm bytes, ABI, language, numeric presets or
  adapter behavior. Require identical bytes with and without the analysis.
- **Separate concepts:** distinguish allocation origin, escape-analysis results
  and future storage decisions. A temporary candidate still uses the existing
  monotonic heap. The illustrative `StorageClass` below is not an approved M1
  interface, and `ManagedHeap` does not describe a new implemented allocator.
- **Initial positive slice:** local `number[]` and `bool[]` allocations consumed
  within straight-line code in their allocating function, without branches,
  loops, calls, captures or returned handles. String arrays, ranges, capture cells
  and environments are outside the initial positive set. Aliases and other
  interactions require later proof decisions; excluded cases receive conservative
  classification tests rather than speculative positive proofs.
- **Host observation barrier:** a host call during a candidate's lifetime blocks
  the initial positive proof even without handle arguments/results. Exported
  memory and possible reentry preclude equating absent handle arguments with
  absent observation. Add no host purity, nonretention or nonreentrancy promises.
- **Invocation-wide proof boundary:** require the entire allocating function body
  to satisfy the supported subset, rather than finding eligible fragments or
  computing last uses. Calls, branches, loops and callable declarations block
  the initial positive proof even when apparently unreachable. Candidate status
  bounds lifetime by the invocation; it selects no early reclamation point.
- **Single-binding origins:** positive candidates are array literals bound
  directly to one ordinary `let`, without copying their handle to another binding
  or reassigning that binding. Reads, `len` and checked element writes are allowed
  in principle; exact supported expression forms remain to be settled. Local
  aliases and handle reassignment are unproven, not necessarily escaping.
- **Distinct outcomes:** separate temporary candidates, identified escape paths
  (including returned handles and passage to potentially retaining hosts), and
  unproven cases outside the supported analysis. Record conservative reasons;
  identified escape and unproven cases both retain current storage. These are
  analysis results, not source-language types or proof of actual host retention.
- **Private borrowed metadata:** keep facts internal and associated with the
  borrowed typed tree. Add no public AST fields, allocation/expression IDs or
  ownership annotations. Exact metadata representation remains a technical design
  detail. Preserve independent backend validation of mutable/forged typed input;
  caller-supplied annotations cannot authorize temporary storage.
- **Closed positive-operation set:** admit scalar literals, scalar parameters and
  ordinary number/bool locals; directly let-bound number/bool array literals;
  scalar arithmetic, comparisons and unary operators; element reads, builtin
  `len`, checked element assignments and numeric element compounds; and scalar
  or bare returns. Element expressions and indices must themselves satisfy the
  supported subset. Block expressions, if/match, loops, short-circuit operators,
  calls other than builtin `len`, and other handle uses remain outside the initial
  positive proof. Unknown forms default to unproven, never implicit acceptance.
  Test each admitted operation; widening this set requires an explicit decision.
- **Traps do not imply escape or reclamation permission:** candidate proofs need
  not establish trap freedom or normal completion. Preserve completed effects and
  earlier allocations after traps. M2 must separately establish safe reclamation
  across traps, reentry and suspended invocations; candidate status alone is not
  sufficient proof for cleanup or cursor restoration.
- **Explicit analysis, no production consumer:** exercise the internal component
  explicitly in tests, without enabling it by default in compilation or tying it
  to optimization presets. Code emission does not consume its classifications.
  Verify identical emitted bytes for the same borrowed tree before and after
  analysis, separately within each optimization preset. Storage-selection
  integration is deferred to M2.
- **M1 acceptance:** positive coverage for both array types and every supported
  operation; conservative cases for direct/indirect returns, aliases, reassignment,
  captures, internal calls, recursion, hosts, reentry and control flow; trap and
  forged-typed-tree cases without weakening backend checks; deterministic facts,
  checkable reasons and per-preset byte identity. Require fresh offline fast/full
  validation from `validation.md`, including ignored runtime cases and a
  current-source browser rebuild. Missing prerequisites leave gates pending.
  No page-count, allocation-consumption or execution-time improvement is required:
  M1 changes no storage. These were planned gates at interview time; #53's actual
    completed results are linked in the M1 status above.

### Core design

Track object origins and propagation through local bindings, calls, returns, arrays, closures, and host boundaries. Do not presume that a value is nonescaping simply because its local variable is out of scope. A by-reference closure capture is an alias; a returned array and an object shared with a host must be conservatively classified as escaping or externally rooted.

```rust
// Illustrative compiler-internal analysis result, not a source-language type.
enum StorageClass {
    WasmLocal,       // numeric/bool scalars or scalar-replaced aggregates
    StaticData,      // literals/interned immutable values
    TemporaryRegion, // proven aggregate lifetime
    ManagedHeap,     // escaping, shared or lifetime-unknown values
}
```

```mermaid
flowchart TD
  A[Typed AST and call effects] --> B[Escape and alias analysis]
  B --> C{Storage classification}
  C -->|scalar| L[Wasm locals]
  C -->|static| S[Static data]
  C -->|proven temporary| R[Region candidate]
  C -->|escapes or unknown| H[Managed heap fallback]
```

### Conservative first slice

- Begin with straight-line, local arrays or temporary records fully consumed by the allocating function, with no host callbacks, escaping closures, stored aliases, or return handles.
- Model unknown host functions as potentially retaining passed handles unless the host ABI explicitly promises otherwise.
- Handle returns, global-like roots, nested captures, recursion and reentrancy before broadening the candidate set.
- Keep escape facts separate from codegen; encode decisions in metadata that optimization passes can query.
- If an analysis cannot prove safety, allocate on the heap as before.

### Proof obligations

1. Every region-resident object and every transitive reference to it becomes unreachable before region reclamation.
2. No aliased object remains reachable from a live array, captured binding, returned value or host-visible handle.
3. Nested/reentrant calls never reclaim a frame or region that an outer or suspended call still uses.
4. Compilation with analysis disabled preserves identical source-level behavior; analysis only changes storage representation.

**Exit gate:** Classification tests cover direct returns, indirect aliasing, closures, recursion, host calls and traps; at least one nonescaping temporary is identified, while adversarial escaping examples conservatively remain heap-allocated.

---

## 6. Milestone M2 — Temporary region allocation prototype

**Goal:** Reclaim groups of proven-dead allocations using allocation-region checkpoints, while preserving ordinary heap allocations and host-visible handles.

### M2 interview decisions (2026-10-26; confirmed)

The user confirmed the consolidated contract and four-ticket implementation
breakdown on 2026-10-26, completing the interview and approving ticket publication.
At confirmation, M2 was unimplemented; the confirmation itself performed no
implementation or public activation. The private prototype is now implemented
through #54–#56, with [integrated validation under #57](m2-temporary-regions.md)
passed on 2026-10-27. Public activation remains unauthorized. The illustrative
region model and experiments below remain subject to these narrower experimental
boundaries.

Approved implementation tickets (all initially `ready-for-agent`):

1. [#54 — Prototype reusable temporary region allocations](https://github.com/WAS-Language/was-compiler/issues/54): no blockers.
2. [#55 — Establish adversarial region isolation and failure guarantees](https://github.com/WAS-Language/was-compiler/issues/55): blocked by #54.
3. [#56 — Measure successful reuse and trap quarantine](https://github.com/WAS-Language/was-compiler/issues/56): blocked by #54.
4. [#57 — Validate and document the integrated region prototype](https://github.com/WAS-Language/was-compiler/issues/57): blocked by #55 and #56.

Dependencies are native GitHub blocking links, not authorization for concurrent
compiler planning or edits to shared files. No source parent issue was supplied.

- **Private experimental path:** M2 is explicitly test-invoked, without a public
  compiler option, CLI flag or browser setting. Existing compilation paths,
  direct codegen entry points, numeric presets and adapters retain their behavior
  and emitted bytes. Public activation requires a separate decision.
- **Unchanged eligibility:** consume exactly M1's existing invocation-wide closed
  positive subset for direct function-body let-bound number/bool-array literals.
  Do not expand the proof to aliases, calls, branches, loops, captures, handle
  reassignment or string arrays, including unreachable syntax. Repeated external
  invocations can exercise reuse; excluded recursive/reentrant callers can test
  isolation without becoming eligible. Independent checked lowering remains
  authoritative for mutable/forged typed input.
- **Separate experimental storage contract:** do not claim revision-1 raw-memory
  compatibility for experimental temporaries. Discovering their addresses through
  exported memory does not grant instance-long retention rights after successful
  retirement; trapped blocks follow the quarantine policy below. Address reuse,
  allocator-address observations, growth and exhaustion may differ under the
  experimental contract. Ordinary
  heap allocations and legitimately published handles retain existing rights;
  descriptor checks are not weakened. Revision 1 remains unchanged for existing
  paths. Temporary retirement and trap handling follow the decisions below.
- **Exclusive backing blocks:** each eligible invocation obtains an exclusively
  owned reusable block, isolated from ordinary heap storage. The existing checked
  bump allocator may provision backing blocks, but ordinary allocations never use
  their reserved interiors. Retirement makes a block available to later eligible
  invocations; it never rewinds the global heap cursor. General heap free lists,
  live-payload movement and M3 reclamation are outside M2.
- **Invocation-wide retirement:** temporary storage remains valid throughout the
  allocating invocation. It becomes reusable only on normal completion, after
  computing any scalar return value. Simultaneously live literals have distinct
  storage; there is no last-use reclamation or intra-invocation reuse.
- **Trap quarantine:** a block acquired by an invocation that traps remains
  unavailable for reuse for the rest of the instance's lifetime. Ordinary return
  cleanup is not trap cleanup. Later calls use another available block or provision
  new backing storage, without host-assisted cleanup or release of suspended
  callers' live regions. Completed temporary writes remain intact after the trap.
  Repeated traps may consume additional storage until allocation fails; bounded
  reuse is a successful-invocation guarantee, not a trap-heavy-workload guarantee.
  Trap recovery requires a separate design decision.
- **Fixed capacity, lazy acquisition:** size a fixed-capacity block from the
  eligible function's statically known literal requirements, acquiring it only
  at the first executed temporary allocation. Do not grow the region or relocate
  live payloads during the invocation. Each literal reserves its own distinct
  slice and initializes its descriptor before evaluating its elements once,
  left-to-right; reserving capacity does not execute later initializers. Full
  reservation may fail or grow memory earlier than baseline allocation, including
  before an element access that would otherwise trap. An early trap quarantines
  the entire acquired block, not just the executed allocation prefix. These are
  accepted experimental differences, not baseline allocation-failure parity.
- **Conservative sizing:** include every candidate literal in the structural
  inventory, including unreachable suffixes after returns, and sum individually
  aligned footprints with checked arithmetic. Account separately for any block
  management overhead. No executed literal means no acquired block. Unreachable
  nodes remain independently backend-checked; tighter reservation sizing is
  deferred. Unreachable literals may enlarge an acquired block.
- **Unrepresentable-total fallback:** if the complete reservation cannot be
  represented within supported memory32 allocation limits, select baseline
  storage for the entire function. Record a deterministic private fallback reason;
  do not introduce a compilation error or partial region allocation for that
  function. Candidate classification alone does not require region storage.
- **Per-function reuse:** each eligible function has one private reusable-block
  slot, empty while an invocation owns its block. Take the cached block at the
  first executed literal, or provision one if absent. Normal completion returns
  the block; a trap leaves it quarantined and absent from the slot. Eligible
  functions cannot call or reenter themselves; ineligible callers may recurse or
  reenter around them. Retained reusable capacity is per exercised eligible
  function. Cross-function pooling, size classes and eviction are deferred.
- **Runtime provisioning failure:** a representable reservation that cannot be
  provisioned traps immediately, without retrying ordinary per-literal allocation.
  Failed provisioning preserves the ordinary allocator cursor and existing
  storage. Successful acquisition followed by a trap quarantines the block.
  Compile-time sizing fallback and runtime provisioning failure are distinct.
- **One checked pipeline:** use a private `cfg(test)` compilation seam through
  existing layout, checked body lowering and module assembly, not another emitter
  or validator. Classification/storage metadata privately borrow the typed tree;
  add no public AST fields, allocation IDs, options or dependencies. Experimental
  consumers and supporting machinery stay out of production builds. Emit region
  machinery only for explicitly experimental modules that select region storage.
  Facts never authorize skipping validation or trusting forged annotations.
- **Correctness gates:** test both array types, empty arrays, alignment boundaries,
  distinct simultaneously live literals, full initialization on reuse, reads,
  writes, every numeric element compound and early returns. Cover repeated normal
  reuse, traps during initialization/access, repeated quarantine, success after
  traps and no acquisition for entirely unreachable literals. Check ordinary heap
  allocations and retained handles around eligible callees, recursive callers,
  host reentry and caught inner traps; suspended live storage must not be reused.
  Cover checked sizing, whole-function fallback, capped-memory provisioning failure
  without allocator corruption and accepted front-loaded failure behavior. Require
  forged-tree rejection, Wasm validation, independent decoded assertions and
  bounded Node execution with independent expected results.
- **Noninterference and integrated gates:** ordinary-path bytes and diagnostics
  remain unchanged within each numeric preset. Experimental semantics match
  except for agreed storage/resource differences; enabled presets retain byte
  parity. Preserve fixtures and revision-1 retention tests. Require fresh offline
  fast/full gates from `validation.md`, including ignored runtimes and rebuilt
  browser checks. Adapter checks prove ordinary-path noninterference, not public
  prototype activation. Missing prerequisites leave gates pending.
- **Measured exit gate:** demonstrate bounded region backing consumption across
  repeated successful invocations. After warm-up, successful calls reuse backing
  storage without region-driven heap growth; interleaved eligible functions retain
  at most their own reusable block each, apart from quarantined blocks. Trapping
  calls visibly accumulate quarantined storage; the matched baseline continues
  allocating for the successful workload. Record reserved pages, provisioned,
  reused and quarantined backing bytes, allocation counts, module size and bounded
  guest timing with explicit definitions and test-only instrumentation. Measure
  capacity separately from allocator markers and timing separately from
  instrumented runs, following `benchmarks.md`. No speedup is required; speedup
  claims require repeated matched measurements. Unavailable measurements are not
  inferred. This gate authorizes no release, default or public activation change.

### Region model

```text
function/frame enters
  -> obtain a frame-specific region or mark
  -> allocate proven-nonescaping objects in that region
  -> perform computation
  -> all region roots have been proved dead
  -> restore the region cursor / retire region as a unit
```

```mermaid
flowchart TD
  A[Request allocation] --> B{Analysis result}
  B -->|proven temporary| C[Temporary region allocator]
  B -->|shared, escaping or unknown| D[Managed heap]
  C --> E[Checked linear memory provisioning]
  D --> E
  C --> F[Bulk reclaim when all region objects are dead]
```

**Crucial safety invariant:** A `mark/reset` of the global bump cursor is *not* safe when longer-lived allocations occurred after the mark. Use disjoint allocation regions, a frame-local arena backed by an independent reusable block, or another design that isolates region lifetime from heap allocations. Never blindly reset the global heap pointer at function return.

### WebAssembly-specific constraints

- Core `memory32` has one linear address space with growth at the end. It does not provide Goose's independently growable virtual-address-space data stacks.
- `memory.grow` does not relocate existing Wasm memory offsets; JavaScript cached views may need refreshing. This is different from `realloc`, which **may** relocate an array payload.
- The first region can be bounded or backed by a managed heap block; any growing region must have an explicit overflow/reallocation story.
- Do not expose region addresses to the host or allow their handles to persist after the region is reclaimed.
- Traps must not cause later execution/reentry to reuse still-live memory incorrectly. Validate both successful and trapping paths.

### Experiments

```text
fn local_sum() -> number { let xs = [1, 2, 3]; return xs[0] + xs[1] + xs[2]; }
fn escapes() -> number[] { return [1, 2, 3]; }
fn aliases(xs: number[]) { xs.push(42); }
```

The first example is eligible only after proof of nonescape; the second must remain live after return; the third must preserve caller-visible aliasing. These examples illustrate desired classification, not already-implemented array APIs.

Measure reserved pages, region bytes reused, allocation count, execution time, and semantic equivalence across repeated and recursive calls. Add instrumentation in debug/test builds only.

**Exit gate:** A tight loop creating nonescaping temporaries demonstrates bounded region-reserved memory (subject to high-watermark behavior), while escaping and host-visible objects are never freed by region resets. Negative tests cover aliases, recursive calls, callbacks and traps.

---

## 7. Milestone M3 — Allocation infrastructure and reclamation policy

**Goal:** Add reusable heap allocation for objects and buffers that are not proven region-local, without prematurely claiming a complete memory manager. The region allocator from M2 and this heap allocator serve different lifetimes.

### M3 interview decisions (2026-10-27; confirmed)

The consolidated scope and six-ticket implementation breakdown are confirmed.
This records the agreed private prototype contract. [#58's first private allocation,
release and reuse cycle](m3-private-allocator.md) and #59–#62's coverage/measurement
slices are implemented behind `cfg(test)`. [#63's integrated contract-to-test and
validation record](m3-allocation-infrastructure.md) passed on 2026-10-28 against
final sources, including private runtimes, ordinary/M2 noninterference, portable
builds and freshly rebuilt browser compilation. [Sequential final-source measurements](benchmarks/m3-allocation-infrastructure-2026-10-28/integrated-63/README.md)
reproduce backing/metadata plateaus and linear search costs. This accepts the
characterized private milestone, not production activation or downstream consumers.

- **Initial delivery:** a private experimental allocation/release infrastructure,
  exercised by internal test workloads through the existing checked backend.
  Ordinary compilation remains unchanged; no public option or adapter activation
  is introduced. A production consumer requires separate explicit approval.
  Evidence must distinguish bounded reuse in the tested workload from general
  memory reduction in current language programs.
- **Reclamation authority:** management rights and proof that no usable references
  remain are separate obligations. M3 introduces neither GC nor reference counting,
  and neither binding overwrite nor scope/function exit alone proves death.
  Shared, published or unknown-provenance allocations without an explicit proof
  retain the ordinary monotonic/nonreclaiming fallback.
- **M2 isolation:** preserve M2's existing per-function caches, normal-return reuse
  and permanent trap quarantine. Do not migrate M2 to the new allocator, pool its
  blocks across functions or recover quarantined blocks as part of M3.
- **Compatibility:** retain the public eight-byte descriptors and revision-1 raw
  host obligations. Private management metadata cannot retroactively establish
  ownership of accepted descriptors, including separately allocated payloads.
  No host root registration, handle release or adapter ownership transfer is
  required. Optional managed-host ownership remains M11 work; capacity/resizing
  decisions remain M4 work.

#### Confirmed operation and evidence boundaries

- **Minimal operations:** the first delivery implements private allocation and
  release only. `realloc_owned`, content-preserving relocation and production
  resizing consumers are deferred. A future failed reallocation must preserve the
  original live allocation, but realignment/copy/growth/shrink policy is not part
  of this first gate.
- **Allocation identity:** an address, size and occupied flag alone do not authorize
  release. Private release authority must distinguish successive allocations at
  the same address and reject obsolete authority after reuse (the ABA case).
  This does not change public handles or make stale memory reads safe. The limits
  below settle identity representation and exhaustion behavior.
- **Emitted-runtime evidence:** internal workloads must exercise helpers actually
  emitted and executed in Wasm, with module validation and independent decoded
  assertions. A Rust model may complement but cannot replace this evidence.
  Test-only scenarios must not add language-level `free`, public management APIs
  or management exports to ordinary modules. Cover simultaneous live blocks,
  content preservation, explicit release, reuse and memory growth.
- **Trap policy:** traps authorize no automatic release, next-call sweep or recovery
  of still-active allocations. Completed releases remain completed; allocations
  active at a trap remain unavailable for reuse, with no recovery promise in this
  delivery. Bounded-growth claims apply to successful explicit-release cycles,
  not unlimited traps that leave active allocations behind.

#### Confirmed reuse, validation and measurement policy

- **Size classes:** round requests to power-of-two capacities, with an eight-byte
  minimum. Reuse only within the requested class; an empty class obtains fresh
  storage through the existing checked bump allocator. No splitting, larger-class
  borrowing or coalescing is included. The request-domain decision below settles
  the upper limit and oversized-request policy. Accept internal fragmentation and
  free capacity stranded in other classes; do not claim general fragmentation resistance.
- **Invalid release:** obsolete identity, double release or unknown address must
  trap before modifying free lists or valid allocation metadata. Registered metadata, not caller-supplied size/class, is authoritative.
  Exercise these failures through the private harness, without claiming protection
  against arbitrary memory corruption by a malicious host.
- **Identity lifetime:** use an instance-wide monotonic identity renewed on every
  acquisition, including reuse. Never wrap or recycle identities. Exhaustion
  fails before removing a free block or reserving fresh storage. The bounded-metadata
  decision below fixes the identity representation as u64.
- **Separate reuse from space efficiency:** with fixed size classes and a fixed
  simultaneous-live-block peak, successful allocate/release cycles must stop
  consuming fresh storage after warmup. Measure requested bytes, reserved capacity,
  metadata, retained free storage and memory pages separately. Page stability
  alone is insufficient: observe bump/storage consumption as well. Mixed-class
  workloads must verify contents of still-live blocks. No page return, shrinking
  `memory.size` or memory bound under an increasing demand peak is promised.

#### Confirmed limits and acquisition consistency

- **Request domain:** the experimental allocator has 29 power-of-two payload
  capacity classes, from eight bytes through 2^31 bytes. Reject larger requests,
  negative inputs and out-of-domain values before mutation, without truncation.
  A supported class does not guarantee allocation success: metadata, existing
  reservations and memory limits still apply. This private limit changes neither
  the source language nor the ordinary bump allocator.
- **Alignment and zero size:** guarantee fixed eight-byte alignment instead of
  exposing a generic alignment parameter. A zero-byte request acquires a distinct
  live allocation in the minimum class, with its own identity and explicit release
  obligation. Larger alignments are outside this delivery.
- **Bounded metadata:** each physical block has a reusable management record,
  separate from its usable payload. Renew an internal u64 identity on acquisition
  without allocating a permanent history record. Metadata storage must stabilize
  with the physical block population; compare release authority against the current
  identity rather than retaining every obsolete identity.
- **Acquisition failure:** invalid requests, arithmetic overflow, identity
  exhaustion and allocation failure preserve live blocks and free-block
  availability. Check size and identity availability before mutation. Reserve
  fresh payload and metadata together through a single checked backing allocation,
  then publish only a fully initialized block. Do not promise rollback of earlier
  effects or shrinking of already grown memory.

#### Confirmed release authority, initialization and acceptance gates

- **Release authority:** use the private pair of payload address and u64 allocation
  identity. First locate an exact payload-address match in a private linked
  registry of physical blocks, then validate current identity and active state.
  Reject interior, unknown, static and host-provided addresses lacking such a
  record; never infer provenance by interpreting bytes before an untrusted
  address as an allocation header. Derive size/class from the record rather than
  requiring caller-supplied values. Linear release validation is accepted for
  this prototype: O(number of registered blocks), to be measured, not advertised
  as constant time.
- **Initialization:** reused payload is not guaranteed to be zeroed. A private
  consumer must initialize every byte it will read or publish. Requested extent
  and rounded capacity are distinct; spare capacity does not enlarge the
  consumer's requested extent. Tests initialize before reading and check that
  writes within that extent preserve other live blocks. No isolation from a host
  with exported-memory access is claimed. Production initialization proofs remain
  part of separately approved consumer work.
- **Post-trap adversarial evidence:** catch invalid-release/acquisition traps in
  the harness and continue in the same instance. Cover double release, stale
  authority after address reuse, unknown/interior addresses, invalid requests,
  identity exhaustion and failed acquisition with live and free blocks present.
  Verify live contents and free-block availability after failure, without issuing
  duplicate active allocations. Use deterministic private test mechanisms for
  identity exhaustion and allocation failure, not enormous iteration counts or
  exhaustion of actual host resources.
- **Acceptance:** deliver a correct, characterized private prototype, not production
  activation or a required speedup. Require validated Wasm, independent decoded
  layout/instruction checks, executed reuse/growth/failure/content scenarios,
  storage-and-metadata plateaus under stabilized demand, and fragmentation/search
  cost measurements without an arbitrary speed threshold. Verify ordinary
  compilation and M2 contract preservation. Run native/browser evidence appropriate
  to affected paths, rebuilding bindings when needed; unexecuted ignored tests
  are not evidence. No result implicitly authorizes ordinary-array or M4 integration.

All five question rounds and the consolidated six-ticket breakdown are confirmed.
Implementation must remain within the approved private scope; ordinary-array/M4
integration and production activation require separate explicit approval.

| Ticket | Deliverable | Blocked by |
| --- | --- | --- |
| [#58](https://github.com/WAS-Language/was-compiler/issues/58) | Safe private allocation/release/reuse cycle | None |
| [#59](https://github.com/WAS-Language/was-compiler/issues/59) | Size classes, multiple blocks and growth | #58 |
| [#60](https://github.com/WAS-Language/was-compiler/issues/60) | Same-instance invalid-release evidence | #58 |
| [#61](https://github.com/WAS-Language/was-compiler/issues/61) | Acquisition failures and trap retention | #58 |
| [#62](https://github.com/WAS-Language/was-compiler/issues/62) | Storage/metadata plateaus, fragmentation and search costs | #59 |
| [#63](https://github.com/WAS-Language/was-compiler/issues/63) | Integrated correctness and noninterference gates | #60, #61, #62 |

These are logical blocking edges, not permission for concurrent edits to shared
compiler planning/emission. Core changes retain a single owner; measurements and
integrated gates run sequentially on stable sources.

### Proposed heap allocator layering

```mermaid
flowchart TD
  A[Request allocation] --> B{Suitable reusable block?}
  B -->|yes| C[Size-class free list / reusable pool]
  B -->|no| D[Existing checked bump allocator]
  D --> E[memory.grow if necessary]
  C --> F[Return aligned storage]
  E --> F
  G[Safe release of proven-dead allocation] --> H[Validate ownership and allocation class]
  H --> C
```

Conceptual private operations for the confirmed first delivery (not Rust or Wasm
signature specifications):

```text
acquire(requested_size) -> (payload_address, allocation_identity)
release(payload_address, allocation_identity) -> void
```

Alignment is fixed at eight bytes; registered metadata determines the size class.
These are test-only runtime implementation operations, not public WAS language
functions or ordinary-module management exports. `realloc_owned` is deferred to
separately approved follow-up work.

### Key engineering requirements

- Track sufficient allocation metadata to distinguish dynamically owned buffers from static or host-owned regions.
- Preserve alignment required by `f64` and descriptor storage.
- Keep widened arithmetic and overflow checks from the existing allocator.
- Ensure a failed `realloc` leaves the original live allocation intact.
- Avoid reusing a block until all references to that block have been retired under a sound ownership rule.
- Validate behavior after `memory.grow`, especially in JavaScript where cached typed-array views may need refreshing.
- Document whether fragmentation and coalescing are supported; an initial size-class allocator may be intentionally limited.

### Deferred host-ownership protocol (post-M9)

M3 must preserve the revision-1 raw-host contract: host-retained handles remain valid for the instance lifetime, including across traps and subsequent calls. Reusable heap blocks may contain **only allocations with established management rights and proven dead lifetimes**. Valid descriptor bounds alone do not establish ownership. Do not require existing Node/browser consumers to register roots, release raw handles or opt into a new contract. An optional managed-host protocol belongs to M11; until then, unknown, host-visible or potentially retained objects remain nonreclaimable.

An allocator free list is permissible without an advanced alias/effects engine, provided release sites have local, explicit proofs. Advanced interprocedural effects and aliasing are deferred to M10. Treat conservative nonreclamation as correct behavior, not an incomplete free operation.

### Crucial limitation

A free list answers **how to reuse released memory**, not **when memory can be released**. Region classification from M1/M2 handles only proven temporaries; arbitrary shared heap values still require a separate liveness/ownership policy. The second question needs a language-level or compiler-proven lifetime rule. Do not free an array descriptor merely because a local binding is overwritten; another alias or the host may still reference it.

### Exit gate

- Dedicated emitted-Wasm workloads repeatedly acquire and release privately owned
  buffers and demonstrate storage-and-metadata plateaus for fixed classes and
  fixed simultaneous-live demand, with live-content checks and separate accounting
  for requested bytes, capacity, metadata, retained free storage and pages.
- Exercise invalid releases and failed acquisitions, then verify preserved state
  in the same instance. Cover stale authority after reuse, double release,
  unknown/interior addresses, request bounds, identity exhaustion and memory growth.
- Host-owned/static values cannot enter the free lists merely by passing descriptor
  bounds checks. Private release identity does not make stale payload reads safe;
  supported language operations cannot invoke this experimental management seam.
- Preserve ordinary compilation and M2 behavior; record validation and measured
  limitations without claiming production activation, general bounded memory,
  constant-time release, or a complete memory manager.

These private exit gates passed under [#63's final-source evidence](m3-allocation-infrastructure.md).
Fixed-demand storage and metadata stabilize, but free capacity can remain stranded
across classes, release search is linear, private machinery adds module bytes and
wall-clock samples vary on unchanged modules. M2 migration, ordinary-array/M4
consumers, resizing/reallocation, host ownership changes and public activation
remain separately authorized work; no default or release decision is implied.

---

## 8. Milestone M4 — Resizable typed arrays

**Goal:** Add collection operations without changing the current alias model.

**Final integrated status (#72, 2026-10-30):** [All eight execution gates passed](m4-release-readiness.md),
including 1,602 full-workspace tests with none failed/ignored, freshly rebuilt
browser coverage and two sequential measurement recordings. The
[contract-to-test map](m4-contract-test-map.md) records actual coverage and limits.
Accounting and module bytes reproduce #71 exactly; material timing variability
leaves **integrated release acceptance pending explicit performance-signal disposition**.
No new default, optimization or general reclamation is authorized.

**Current slices (#65/#66/#67/#68):** `resizable(source)` creates a fresh, resize-authorized
copy of `number[]`, `bool[]` or `string[]` with the existing public descriptor.
#66 adds only `identifier.reserve(additional)` and relocation-safe indexed writes
for all three types: reserve preserves length/handle, may relocate, and releases
only displaced owned payloads after publication. Fixed writes retain their original
saved address; opt-in writes revalidate captured handle/index after the RHS and
preserve compound old values. No public capacity/authority query is added.
Only the required allocation core is production-internal; its M3 test adapter
remains `cfg(test)`. Ordinary literals/host encoding stay fixed-storage and retain
host lifetime rights; opt-in hosts must reacquire payload pointer/length borrows
after possible relocation, separately from refreshing views after memory growth.
Programs without M4 retain existing bytes/helper order. See [the slice contract](resizable-arrays.md)
for ordering, numeric bounds, raw-copy consumption rules, helper planning and node
migration. [#66 final validation and review passed](resizable-arrays.md#issue-66-validation);
that historical slice does not substitute for #72's integrated record above.
#67 adds identifier-only `push(item)` for all three array types: exactly one
positional receiver-derived scalar, Void/Never result and exact region-local
placeholder inference. It preserves purity rejection and pipeline/discard rules.
Authority precedes once-only item effects, then current authority/storage is
revalidated; receiver rebinding cannot redirect the append. Push consumes only the
new bool/string item; shared reserve growth raw-copies old slots without scans.
Typed-slot initialization
precedes public/private length publication, with no callbacks or expected failures
after reserve succeeds. Pending writes remain safe; iteration retains its initial
length.
#68 adds contextual identifier-only `pop()` (zero arguments) and `remove(index)`
(one positional number), with exact scalar/Never results and exact region-local
`ArrayElement` placeholder relations. Both are forbidden in pure defaults/contracts.
Authority precedes argument effects; remove checks the captured receiver's current
index after once-only argument evaluation. Empty pop traps; remove uses normal
index rules, including `-0`. Shared `emit_take` validates only the returned bool/string
before any mutation and raw-shifts following slot bytes without consuming them.
Shared public/private length publication retains payload, capacity, acquisition
identity and referenced strings. Pending writes target the current captured index
after shrink/shift, or trap, preserving pre-RHS compound old values. Initial-count
iteration reads current elements; shrinking can trap a later read.
Historical #65–#67 evidence retains its original scope; see
[#67 validation/review](resizable-arrays.md#issue-67-validation) and
[#68 validation/review evidence](resizable-arrays.md#issue-68-validation).
#68's focused/default/build/browser checks and two-axis review retain their
historical scope, including its incomplete ignored-workspace run. #69 supplied
dedicated adapter/raw-host cases; #70 supplied failure/recovery checks; #71 supplied
measurements. #72 now records completed integrated gates independently. Capacity
shrinking and general reclamation remain unimplemented.
The interview decisions and original acceptance criteria below retain their
full-milestone scope.

### M4 interview decisions (2026-10-28; confirmed)

The user confirmed the consolidated design below on 2026-10-28, closing the interview.
These decisions define the intended milestone, not implementation authorization or
release evidence.

- **Production target:** implement source-language resizing across the compiler,
  CLI, browser adapter and Node marshaling for all three existing array types.
  Develop bounded private slices before explicit integrated release gates. M3's
  completed private gates do not authorize production consumers; general reclamation,
  M2 migration and a host ownership protocol are not implicitly included.
- **Public layout:** preserve the eight-byte pointer/length descriptor and stable
  array handles. Capacity and resize authority require separate private metadata.
  Structural validity alone grants no management rights. This is a layout decision,
  not a promise of unchanged bytes for M4-using programs. The opt-in payload-lifetime
  contract is specified below.
- **Operation scope:** include push, pop, reserve and order-preserving remove for
  number, bool and string arrays, alongside existing len. Reserve changes capacity,
  not length. Syntax, results and failure/evaluation ordering are specified below.
  Exclude insertion, slicing/views, capacity shrinking, nested arrays and a separate
  collection type.
- **Iteration:** capture the handle and initial length once, then read current
  elements through checked indices without snapshots or cached payload addresses.
  Appended indices beyond the initial length are not visited; shrinking that
  invalidates a later read traps. Shifting removal affects subsequent indexed reads,
  with no exactly-once guarantee for original elements.

- **Opt-in movability:** introduce explicit creation of resize-authorized arrays
  using the existing array types, not a separate collection type. Ordinary literals
  and existing host-created arrays retain fixed-storage guarantees; resizing them
  traps rather than implicitly adopting their storage. Aliases preserve the same
  collection identity and authority.
- **Indexed writes (replaces the initial pinning selection):** fixed-storage arrays
  retain their saved pre-RHS checked-address contract. For explicitly created
  resizable arrays, capture the array object and index once and validate the
  destination before the RHS. Numeric compounds also retain the old numeric value.
  After evaluating the RHS once, revalidate that captured array/index and resolve
  its current payload address before storing. Shrink-invalidated indices trap after
  completed RHS effects; shifting removal makes the store target the current element
  at that index, not the original element. Reassignment of the receiver binding does
  not change the captured array. This explicitly amends M0's proposed future place
  guarantee only for opt-in resizable arrays, not existing fixed-storage arrays.
  Runtime pinning was rejected because ordinary Wasm traps can abandon pins without
  cleanup, indistinguishably from live outer pins suspended during host reentry.
  No host invocation wrapper, cleanup obligation or permanent resize quarantine is
  introduced.
- **Operation results:** push and reserve return void; pop and order-preserving
  remove return the removed element. Empty pop and invalid remove indices trap.
  Reserve requests at least length plus additional elements, never reduces capacity,
  and accepts only finite integral nonnegative numbers with checked sizing before
  mutation. No coercions, optional/sentinel results or fluent returns are introduced.

- **Construction:** `resizable(source)` makes a fresh resize-authorized copy of
  any supported array, including another resizable array. Source aliases are
  unaffected; string-array copies may share immutable string values. Existing
  contextual literal typing applies, including the unconstrained empty-array
  number fallback. No in-place adoption or separate collection type is introduced.
- **Surface:** use built-in push/pop/reserve/remove methods on identifier receivers,
  with positional arguments only; keep len and add the resizable constructor.
  Capture the receiver object before argument evaluation; rebinding the receiver
  during arguments does not redirect the operation. No general methods, callable
  properties, named/default method arguments or postfix chaining are introduced.
- **Host boundary:** resizable handles may be retained, but payload pointer and
  length must be reacquired after calls or reentrant callbacks that may resize.
  This is separate from refreshing views after memory growth. Existing fixed-storage
  guarantees remain unchanged. Node encoding continues to create fixed-storage
  arrays; decoding copies current logical elements. Hosts obtain resizable arrays
  through guest construction, not descriptor fabrication or ownership registration.

- **Storage integration:** promote only the required M3 allocation core into an
  internally planned production path for explicitly resizable arrays. Acquire/copy
  replacement payloads, publish, then release displaced owned payloads. Retain
  descriptors and authority records for the instance lifetime; do not release
  referenced strings or reclaim unreachable arrays. Ordinary allocation and M2
  behavior remain unchanged. Private management exports, observations and test
  configuration do not become production APIs.
- **Capacity:** use existing exact power-of-two byte classes, minimum eight bytes,
  including empty arrays. Element capacity is byte capacity divided by element width.
  Acquire the entire capacity extent, not merely live length or unauthorized rounding
  slack. Retain the checked 2^31-byte request ceiling, without promising allocation
  success. Growth temporarily retains old and new buffers; pop/remove do not shrink
  capacity. Spare slots are unreadable until initialized and made live.
- **Method order:** capture and validate the receiver, including resize authority,
  before argument effects. Evaluate arguments once left-to-right, then validate
  current state and mutate without callbacks during copy/publication. Fixed-storage
  receivers trap even for reserve(0). Push appends to post-argument contents;
  reserve uses post-argument length; remove checks against post-argument length.
  Completed argument effects survive subsequent failure.
- **Naming and grammar:** resizable follows len's ordinary-identifier built-in
  resolution, reserves the global function/host-language name and permits normal
  lexical shadowing. Existing global uses require migration. Method names remain
  contextual and otherwise legal identifiers. Accept exactly identifier.method(...),
  not parenthesized/call-result receivers or chaining; preserve ranges and rejection
  of .5 and 1. Direct built-in pipeline stages are not added.
- **Typing and purity:** resizable preserves its source array type and forwards
  expected array context into its source; unconstrained empty arrays still default
  to number[]. Methods derive element types from receivers. Placeholder constraints
  remain exact and region-local with reachable evidence; resizable(_) alone remains
  ambiguous and callers supply no refinement. Reject the constructor and all four
  methods in defaults/contracts, including skipped operands; retain independent
  forged-tree backend checks.

- **Consumed validation:** construction validates all copied logical elements;
  push validates its new element; pop/remove validate their returned element before
  mutation. Internal growth/reserve copies and shifting unreturned elements do not
  scan otherwise unconsumed bool bytes or string handles. Existing len, equality,
  indexing and iteration consumption rules remain unchanged.
- **Failure:** preserve published logical state on operation failure, without
  invocation-wide rollback. Perform sizing, element and other expected-to-fail
  checks before publication; replacement-allocation failure leaves the old array
  usable. No user code runs between final copy, publication and old-buffer release.
  Completed argument effects and successful earlier allocations remain; failed
  construction may retain unpublished allocations, without sweeping or trap recovery.
- **Authority checks:** require exact handle registry membership and consistency
  with private metadata before management mutation, including element kind,
  payload, logical length/capacity and current acquisition authority. Fabricated
  valid descriptors remain fixed-storage. Arbitrary host corruption of private
  metadata remains unsupported; this is not a security boundary.
- **Acceptance:** require frontend, forged-tree, independent decoded lowering and
  Wasm validation evidence; all element types, alias/capture/recursion/reentry,
  post-RHS indexed-write revalidation, iteration mutation, lazy consumption,
  fixed-storage rejection and deterministic failure/recovery coverage. Preserve
  existing-program compatibility fixtures, preset parity and CLI bytes; rebuild
  browser output and explicitly run ignored runtimes with full offline integrated
  gates. Historical M3/default-suite success does not establish M4 acceptance.
- **Measurements:** measure append/reserve/remove workloads, geometric growth and
  reuse across arrays. Distinguish length, capacity, active/free payload storage,
  allocator records, authority metadata, bump consumption and pages. Separate fresh
  uninstrumented timing from instrumented observations. Explain fragmentation and
  retained object/failed-construction storage without claiming generally bounded
  memory or inferring storage plateaus from pages alone.

- **Numeric boundaries:** remove uses existing checked index rules: finite,
  integral, nonnegative and in bounds, accepting negative zero as zero. Reserve
  accepts negative zero as zero and rejects other negative, fractional and nonfinite
  values. Check length addition and byte sizing before narrowing, including the
  allocator-domain ceiling. Capacity is private; no capacity query or exact reserve
  capacity guarantee is added.
- **Stage boundaries:** malformed syntax is a parser error; wrong receiver types,
  arity and element types are semantic errors; absent resize authority traps at
  runtime even when fixed provenance is statically apparent. No ownership type or
  static resize-effect system is added. Preserve Never compatibility, flow, spans,
  skipped/unreachable checking and independent malformed typed-input rejection.
- **Isolation:** programs without M4 construction/operations retain existing bytes
  and gain no new allocator/authority machinery. Discover M4 structurally, including
  nested callables and skipped/unreachable nodes; plan indices before checked body
  lowering and emit deterministically. Ordinary literals stay fixed-storage even
  in M4 modules; storage semantics do not depend on numeric presets.
- **Architecture record:** [ADR 0009](adr/0009-opt-in-resizable-array-storage.md)
  records the descriptor, authority, pending-write and reclamation trade-offs.

<a id="selected-representation-65-construction-and-66-reserve-implemented"></a>

### Selected representation (#65 construction, #66 reserve and #67 push implemented)

```text
Stable array handle
        |
        v
+--------------------------+
| data_ptr : u32   (+0)    |
| length   : u32   (+4)    |
+--------------------------+
        |
        v
[ element 0 ][ element 1 ] ... [ capacity - 1 ]

Private authority metadata, keyed by stable handle:
element kind, capacity, current owned payload and acquisition identity
```

The interview selected the existing eight-byte public descriptor with separate
private capacity/authority metadata over a sixteen-byte ABI revision. This preserves
public layout, but opt-in payload relocation has its own explicit host and indexed-write
contracts above. #65 implements the descriptor/private-authority construction
shape shown here; [the ABI document](memory-abi.md#private-resizable-array-authority-65)
records its current private layout. #66 implements reserve relocation and release
of displaced owned payloads, not general reclamation. #67 push reuses that reserve
path and preserves the same layout and authority boundary.

### Compatibility constraints for deferred ownership work

Preserve the existing eight-byte public descriptor and store capacity/authority
privately. Only explicit construction establishes movable storage. Structurally
valid host-originated or otherwise unproven descriptors remain fixed-storage;
methods trap before argument effects without invalidating them. Raw handles remain
stable. Existing fixed-storage lifetime rights remain intact, while opt-in payload
addresses must be reacquired after possible resize. This is distinct from JavaScript
view refresh and does not introduce M11's deferred host ownership protocol.

<a id="growth-algorithm-proposed-not-implemented"></a>

### Growth algorithm (#67 push implemented; release evidence pending)

1. Capture and validate the receiver and resize authority before item effects.
2. Evaluate the item once, then revalidate the receiver's current authority/storage
   and validate the newly consumed bool/string value. Do not scan old elements.
3. Call shared `reserve(handle, 1)` for checked length/byte sizing and, if needed,
   full-capacity acquisition, raw live-slot copy, replacement publication and release
   of the displaced owned payload. This does not change logical length.
4. Initialize the typed new slot at the post-item logical end, then publish increased
   public/private lengths. No callbacks or expected-to-fail checks follow reserve
   success. Stable handles and post-RHS revalidation preserve pending writes; hosts
   must reacquire opt-in payload borrows after possible relocation.

#66 implements reserve's replacement path without changing logical length: capture
and validate authority before the argument (fixed reserve(0) traps before effects),
then revalidate post-argument state and check finite integral nonnegative additional
(`-0` accepted), widened addition and byte sizing within the 2^31-byte ceiling.
Acquire the full selected capacity and raw-copy live slots without scanning
unconsumed bool/string values; publish before releasing the old owned buffer.
Constructor/`Reserve`/`Push` discovery and per-element authority helpers are planned before
checked lowering. A failed allocation preserves the post-argument published array,
not prior effects or allocator history. Implemented pop/remove validate their returned
element before mutation and retain capacity.

### API status (implemented versus proposed)

```text
resizable(source)
values.push(item)
values.pop()
values.reserve(additional)
values.remove(index)
len(values)
```

The interview selected identifier-receiver method syntax and the explicit copy
constructor `resizable(source)`. Push/reserve return void; pop/remove return an element,
with empty pop trapping. The constructor is implemented by #65, reserve by #66,
push by #67 and pop/remove by #68; `len` already exists. Capacity shrinking remains
unimplemented. [#72's final-source evidence](m4-release-readiness.md) supplies
completed correctness/portability/adapter gates and sequential measurements;
historical slice records alone do not close M4. Integrated release acceptance
remains pending explicit disposition of the material timing signal.

### Specific tests

- Aliases observe `push`, `pop`, and `remove` consistently.
- `number[]`, `bool[]`, and `string[]` use widths 8, 1, and 4 bytes respectively.
- Reallocation preserves element values and descriptor identity.
- Nested calls, captures and host interactions do not retain stale payload pointers.
- Mutation while iterating follows the documented iteration policy.
- Capacity overflow, invalid indices and allocation failures trap without descriptor corruption.
- Repeated growth is not quadratic for ordinary append-heavy workloads.

**Exit gate:** All three array types support the agreed resizing operations; aliasing and host marshaling pass tests; growth performance and retained memory are measured.

---

## 9. Milestone M5 — String operations and efficient construction

**Goal:** Make strings practically editable at the language level without undermining immutable value semantics.

### Implemented first slice — #73–#76 (2026-10-30)

`string_byte_len`, `string_concat`, `string_slice_bytes` and `string_replace_all`
are implemented. [#76's contract-to-test record](string-operations.md#issue-76-first-slice-adapter-validation--2026-10-30)
covers core/CLI/freshly rebuilt browser bytes, WAT and diagnostics per preset,
plus bounded effects, validation-before-reuse, immutable aliases, retained handles,
caught traps, reentry and actual growth. This is first-slice compatibility evidence,
not final M5 acceptance. Existing `string[]`/resizable arrays can hold fragments;
`string_join`, efficient-construction instrumentation/measurements and the final
milestone decision remain pending. There is no new ownership/marshaling API,
full UTF-8 scan, reclamation or fusion. The design record below retains its
proposal/confirmation scope; it is not evidence that the second slice is delivered.

### M5 interview decisions (2026-10-30; confirmed)

The following boundaries are confirmed design decisions, not implemented support.
Rounds 1–4 and the final shared-understanding confirmation settle the contracts
below and close the design interview. Implementation and tracker planning remain
separate work, not authorized by this design confirmation. See
[ADR 0010](adr/0010-immutable-string-operations-and-fragment-construction.md)
for the architectural trade-offs.

- **Immutable values:** Transformations leave aliases, interned literals and
  strings stored in arrays unchanged. Reassignment changes only the binding.
  Results may reuse existing strings; fresh handle identity is not guaranteed.
  No mutable-string type or general string reclamation is authorized.
- **Two required slices:** First implement immutable concatenation, literal
  replacement and slicing; then explicit buffered construction. Each slice needs
  its own acceptance gate, and both are required for M5 completion. Measuring
  quadratic retention from repeated immutable concatenation is not sufficient
  to satisfy efficient construction. Automatic concatenation fusion stays in M6.
- **Byte positions:** M5 uses UTF-8 byte offsets and byte lengths, exposed through
  explicitly byte-named APIs. Slices must end and begin at Unicode-scalar
  boundaries; splitting a code point traps. Scalar/grapheme indexing and
  normalization remain deferred. Confirmed bounds are specified below.
- **Builtin surface:** Immutable transformations start as named builtin functions,
  not string `+`, identifier-only methods or general postfix chaining. Calls are
  positional-only and excluded from direct builtin pipeline stages.
- **Trusted-host boundary:** Preserve structural descriptor validation and the
  host obligation to provide valid, immutable UTF-8. Add operation-specific checks,
  not mandatory full UTF-8 scans for every string operation. Valid-output guarantees
  assume inputs supplied under the existing ABI contract.

#### Confirmed immutable API and construction direction (round 2)

```text
string_concat(left: string, right: string) -> string
string_replace_all(source: string, needle: string, replacement: string) -> string
string_slice_bytes(source: string, start: number, end: number) -> string
string_byte_len(source: string) -> number
```

- Fixed arity and exact types; no conversions, optional arguments, string `+`, or
  `len(string)` overload. Byte length does not count scalars or graphemes.
- Replacement uses exact, case-sensitive UTF-8 matching, left-to-right and
  nonoverlapping, without rescanning inserted text. Empty needles trap. There
  are no regexes, normalization, locale rules or replacement-template syntax.
  Replacing `aa` with `b` in `aaa` yields `ba`.
- Slicing uses strict half-open `[start, end)` bounds. Both numbers must be finite
  integers with `0 <= start <= end <= byte_length`, and both positions must be
  Unicode-scalar boundaries. Accept negative zero as zero; never clamp. An empty
  slice at a position inside a code point still traps.
- Evaluate arguments once, left-to-right, before operation-specific checks;
  then structurally validate string operands in argument order, check operation
  constraints, check result size and allocate as necessary. Argument evaluation
  traps stop later evaluation; existing function-entry and host-return validation
  remains unchanged. Fast paths cannot skip argument effects or validation of an
  otherwise unused operand. Completed effects and allocations are not rolled back.
- The second slice uses `string[]` fragments plus one-shot join, not a new mutable
  builder type. Existing resizable arrays support fragment accumulation. This
  avoids repeatedly copying accumulated text but retains fragments and array
  storage under existing lifetime rules; it promises no constant retained-memory
  bound. Join's confirmed signature and observation rules follow below.
- New builtin names are reserved for global user functions and host-language
  registration, but lexical bindings may shadow them, following `resizable`.
  A shadowing noncallable produces the ordinary call error, without builtin
  fallback. Arbitrary nonempty host module/field names remain unrestricted.
- All new M5 builtins, including byte length, remain outside the default/contract
  pure-expression whitelist. Rejection covers skipped/unreachable source and
  forged typed input; M5 does not authorize a whitelist expansion.

#### Join, initial storage policy and validation gates (round 3)

```text
string_join(parts: string[], separator: string) -> string
```

- Both arguments are required. Accept fixed or resizable string arrays without
  requiring resize authority. Insert separators only between entries, including
  empty entries: `[]` and `[""]` yield empty text; `["", ""]` with separator `,`
  yields `,`; `["a", "", "b"]` yields `a,,b`. Apply the other M5 builtin rules.
- Evaluate parts then separator before inspecting the captured array's current
  contents. Separator evaluation may mutate/resize that array through aliases;
  join sees the resulting state. Binding reassignment does not redirect it.
  Validate separator and complete array storage before processing entries, then
  validate all consumed string handles in index order. Even zero/one-entry joins
  validate the separator. After argument evaluation there are no callbacks.
  Use a sizing/validation pass followed by copying without mutating inputs.
- **Initial M5 implementation policy, not permanent language restrictions:** Empty-operand
  concat returns the other operand; no-match or content-identical replacement
  returns the source; full-range slicing returns the source; a one-entry join
  returns its validated entry. Zero-byte results may reuse a shared empty string.
  Other results use a new descriptor and copied payload, including proper
  nonempty slices. Required validation always precedes reuse. Later implementations
  may broaden reuse only after establishing equivalence for host-observable handles,
  allocation failures, memory growth, validation/trap order and lifetime safety,
  not merely equal contents. This authorizes no M5 concatenation fusion.
- Preserve memory32 ABI and allocator bounds with widened checked result-size
  arithmetic. Unrepresentable sizes trap before result allocation. Add no arbitrary
  guest string-size cap, implicit host import, release of published strings, or
  enrollment of string results in M4 reclaimable payloads. Prefer one allocation
  for a new descriptor plus payload. Do not retain per-match/per-fragment
  intermediate result strings; join uses constant auxiliary Wasm locals rather
  than allocating a second collection.
- Each slice requires frontend, forged-typed-input, independent decoded-Wasm and
  bounded runtime coverage. Final completion also requires current-source CLI/
  browser parity and the full documented offline validation gates. Cover aliases,
  interned literals, malformed descriptors, UTF-8 boundaries, numeric edge cases,
  argument effects/trap order, retained handles after traps, memory growth/reentry
  and optimization-preset parity. Programs without new operations preserve bytes
  and helper ordering except deliberate new-name reservation errors.
- Fragment construction uses implemented M4 behavior but does not dispose of
  M4's pending performance acceptance. Correctness, measurement evidence and
  integrated release decisions remain separate.

#### Efficient string construction — acceptance criteria (user-specified)

Efficient string construction must be validated through deterministic allocation
and copying instrumentation, complemented by independent runtime and
compilation-time benchmarks.

The primary comparison must cover repeated string concatenation versus fragment
accumulation followed by a single final `join`.

For fixed-size fragments, the validation suite must establish that:

1. **Baseline concatenation:** A non-optimized, copy-based repeated-concatenation
   baseline exhibits the expected cumulative copying cost. This behavior is not
   required of optimized implementations.
2. **Linear construction:** Fragment accumulation followed by `join` exhibits
   O(n + B) aggregate storage and copying growth, where n is the fragment count
   and B is the total byte length of fragments and separators.
3. **No intermediate strings:** When final materialization is required, `join`
   writes each fragment and separator occurrence directly into the final result
   payload, without constructing intermediate concatenated strings.
4. **Storage reuse:** The implementation may reuse an existing immutable string
   representation when doing so preserves observable semantics and lifetime safety.
5. **Correctness:** Tests cover empty fragments, empty arrays, empty and nonempty
   separators, multibyte UTF-8, and array-growth boundaries.
6. **Instrumentation:** Deterministic, test-only counters distinguish string
   payload allocations and copies from array growth, descriptor allocations, and
   other runtime overhead.
7. **Performance measurements:** Runtime and compilation-time costs are measured
   separately using ordinary, non-instrumented instances. Results must report
   methodology and environment without assuming a guaranteed speedup.

These criteria define the expected algorithmic and allocation behavior, not
mandatory physical allocation counts or an implementation-specific storage layout.

#### Accounting boundary and replacement complexity (round 4)

- The O(n + B) claim covers one fragment-accumulation workload through one final
  join. Count fragment occurrences, including empty ones, in n; count fragment
  bytes by occurrence plus all output separator occurrences in B.
- Attribute retained array-growth allocations, descriptors and newly allocated
  string payloads to the workload. Count array-handle relocation separately from
  string-payload copying. Report preexisting input storage separately rather than
  charging it repeatedly as new allocation; report fragment-production costs
  separately from accumulation/join overhead. Repeated joins producing separately
  retained outputs are separate constructions.
- Report transient storage peaks separately from completed-call retained storage.
- Linear-construction acceptance does not promise linear-time substring search
  for replacement. Initial replacement may use straightforward bounded-memory
  search; include adversarial repetitive sources/needles in runtime measurements.
  Checked result sizing and direct final-payload construction remain mandatory.

### Agreed staged surface (not yet implemented)

| Slice | Functionality | Acceptance |
|---|---|---|
| First | `string_concat`, `string_replace_all`, `string_slice_bytes`, `string_byte_len` | Immutable transformations, strict byte boundaries, checked lowering and observable-order tests. |
| Second | `string_join` over fixed/resizable `string[]` fragments | Direct final materialization and deterministic O(n + B) construction evidence. |
| Deferred | Scalar/grapheme operations, normalization, automatic fusion | Separately designed/authorized work; fusion stays in M6. |

Proposed usage (not current language support):

```text
let message = "hello";
let original = message;
message = string_replace_all(message, "hello", "world");
// original still denotes "hello".

let parts: string[] = resizable([]);
parts.push(message);
parts.push("!");
let result = string_join(parts, "");
```

**Exit gate:** Both slices pass their focused and integrated correctness gates;
aliases/interned strings remain unchanged; valid UTF-8 inputs produce valid UTF-8
results; efficient construction meets the representation-independent acceptance
criteria above. Measurements report environment/methodology without promising a
speedup. M5 completion does not imply M4 performance acceptance.

---

## 10. Milestone M6 — Escape and lifetime improvements

**Goal:** Refine M1/M2 escape proofs and reduce allocations through **conservative, locally provable** scalar replacement, compact layouts and closure/range specialization. Full interprocedural alias/effect proofs and SSA-based dataflow transformations are deferred until M10–M13. This is not the first introduction of lifetime analysis.

| Target | Current concern | Candidate optimization | Soundness precondition |
|---|---|---|---|
| Direct range loop | A range value may require a 32-byte record | Scalar-replace `start`, `end`, `step`, inclusivity into Wasm locals | Range never needs object identity or an exported handle. |
| Nonescaping closure | Capture cells/environments allocate monotonically | Store proven nonescaping state in an invocation-scoped region or optimize away the environment | All uses and recursive/reentrant lifetimes are accounted for. |
| Immutable temporary string | Intermediate objects accumulate | Fuse concatenations or use a builder internally | Observable allocation/evaluation semantics remain unchanged. |
| Temporary array buffer | Superseded storage consumes heap | Reuse buffer under controlled ownership | No stale pointer or host reference remains. |

### M6 interview decisions (2026-11-16; confirmed)

The user confirmed the complete shared understanding after rounds 1–3 on
2026-11-16, closing the design interview. These decisions define the agreed design;
implementation authorization and release acceptance remain separate.

- **Required first scope:** Direct range-loop scalar replacement is the only
  required optimization for M6 completion. Closure specialization, string fusion,
  temporary-array reuse and compact layouts remain deferred candidates, not
  required slices.
- **Resource observability:** An individually authorized optimization may change
  internal allocation counts, subsequent allocation addresses, memory growth and
  allocation-exhaustion outcomes. It must preserve operand effects, validation
  order, ordinary invalid-input traps, published-value lifetimes and alias behavior.
  This is not blanket permission for the other candidate optimizations. In
  particular, string fusion needs its own contract reconciliation before approval.
- **Release dependencies:** M6 design may proceed now, but integrated M6 release
  acceptance requires explicit disposition of M4/M5's pending performance signals.
  M6 work does not silently approve either milestone.
- **Internal-only boundary:** No new source syntax, ownership annotations, public
  descriptor layout, host release obligations or general heap reset. Unsupported
  cases retain existing lowering and lifetime guarantees.

Round 2 confirms eligibility, activation and semantic preservation:

- **Direct-root selection:** Only a `For` iterable whose root typed expression is
  `Range` qualifies, including inclusive/exclusive bounds, default/explicit step
  and grouping parentheses. Identifiers, calls, block wrappers, conditionals,
  value loops and pipelines returning ranges retain allocated lowering. No alias
  analysis, constant propagation or inferred-last-use selection is introduced.
- **Effectful operands:** Calls, allocations, transfers and divergence within
  operands do not exclude a direct range. Loops inside local callable bodies also
  qualify. Evaluate operands once, left-to-right; only after normal completion of
  all operands perform existing ordered numeric validation. Abrupt completion
  suppresses later runtime effects, not static checks of later operands or bodies.
  Preserve enclosing transfer targets and keep the new loop/binding inactive
  during iterable evaluation. Iteration-capture allocation remains per yield.
- **Activation:** All three enabled presets select the optimization through a
  distinct internal policy, not through the integer proof. `None` stays default;
  it and direct codegen entry points retain baseline lowering and bytes. No new
  public flag is added. Require enabled-preset byte identity, adapter parity within
  each preset and existing bytes for programs with no eligible direct range loop.
- **Infrastructure:** Retain existing memory exports, helper discovery and helper
  ordering even when selected loops no longer use their range helpers. This slice
  eliminates runtime range-record allocation, not unused module infrastructure.
  Smaller modules are not promised; measure actual size and compilation costs.
- **Full range semantics:** Preserve the complete safe-integer domain
  ±(2^53−1), exact `i64` iteration state and existing numeric validation rules.
  Reject nonfinite, fractional and out-of-domain inputs and both zero steps;
  accept negative-zero endpoints and yield canonical positive zero. Test endpoints
  before conversion; an advanced value beyond the safe domain exhausts normally
  instead of introducing a new trap. Preserve shared checked validation logic
  where practical. Other operand allocations, captures and arbitrary-handle
  validation are unchanged.

Round 3 confirms proof and completion gates:

- **Deterministic allocation benefit:** Each executed eligible construction has
  zero range-record allocations versus one 32-byte record in the baseline.
  Noncompleting operands produce no range record in either path. Operand, capture
  and body allocations remain separate; an allocation-free loop is not promised.
  Use independent decoded-Wasm assertions and narrowly scoped test-only allocation
  observations, with no production telemetry or public inspection API.
- **Correctness evidence:** Use independent expected results plus differential
  baseline/enabled execution. Cover full-domain endpoints, step signs, inclusivity,
  empty ranges, signed zero, invalid inputs and safe-domain-crossing advancement;
  operand effects/order, abrupt transfers, nested loops, break/continue/return;
  iteration captures, recursion, host reentry and caught traps preserving completed
  effects and live outer state. Cover forged annotations, operands, bindings and
  transfer targets, including skipped/unreachable children and unexported/local
  callable bodies. Negative controls retain allocated indirect-range lowering.
  Assess authorized resource differences separately. Require Wasm validation and
  independent instruction/layout checks, preserving diagnostic variants, spans
  and first-error ordering.
- **Performance evidence and disposition:** Deterministic record elimination is
  the required allocation benefit; no runtime speedup or module-size reduction is
  required. Separately measure ordinary non-instrumented runtime, compilation cost
  and module bytes, including repeated eligible-loop invocations, small/larger trip
  counts and ineligible controls. Report methodology and repeated samples without
  invented percentage thresholds or favorable-run selection. Material regressions
  or unexplained timing variability require explicit disposition before acceptance.
- **Integrated completion:** On final executable sources, pass documented offline
  native tests, strict native Clippy, portable-core build, workspace-boundary checks,
  freshly rebuilt browser checks, full ignored-workspace execution and strict browser
  Clippy. Require adapter parity and a contract-to-test map. Acceptance additionally
  requires performance disposition above and explicit disposition of M4/M5 pending
  signals. Focused success alone is not integrated acceptance; `None` stays default
  after acceptance. Update relevant range/optimization contracts and ADR 0001's
  deferral with the narrow M6 exception, preserving historical release evidence and
  explicit deferred-candidate status.

See [ADR 0011](adr/0011-bounded-m6-range-scalar-replacement.md) for the scope and
resource-observability trade-off. All three interview rounds and the final shared
understanding are confirmed. No implementation is authorized by these interview
records alone.

Issue #82 authorizes and implements the direct-loop slice described in Round 2;
see the [private range contract](memory-abi.md#private-range-record). Other
candidates remain deferred. [#87's final-source release record](m6-release-readiness.md)
and [contract-to-test map](m6-contract-test-map.md) document the verified native
and allocation evidence, blocked gates, and pending performance dispositions.
Round 3's integrated release decision is pending.

### Direct-loop compatibility evidence (#83)

- [`for_loops.rs`](../crates/compiler/tests/for_loops.rs) validates and executes
  baseline and all enabled presets against literal results and host-effect traces.
  Nested direct ranges exercise enclosing break/continue targets from start,
  end, and step and suppress later operands after Never. Enabled presets emit
  identical bytes for these runtime fixtures. A range loop in a local callable exercises
  transitive dependencies, recursive and reentrant calls, a caught guest bounds
  trap after publication, growth while the outer loop is suspended, and retained
  array identity and completed writes after resumption.
- [`closures_runtime.rs`](../crates/compiler/tests/closures_runtime.rs) independently
  checks distinct, retained per-yield cells. The existing allocation and decoded
  controls in `for_loops.rs` and [`for_lowering.rs`](../crates/compiler/tests/for_lowering.rs)
  isolate the optional 32-byte range record. Compatibility checks compare handle
  identity within each instance, not absolute addresses or page counts across
  presets. Resource exhaustion parity is not required and was not tested here.
- [`for_frontend.rs`](../crates/compiler/tests/for_frontend.rs) and `for_lowering.rs`
  check later operands and bodies after an abrupt operand, including a source-level
  read-only capture write and forged transfers to the not-yet-active loop. The
  lowering suite also decodes the correct enclosing branch depths in all presets.
  No production change was needed.

`cargo check --offline --workspace --all-targets` passed. `cargo test --offline
--workspace` passed (1,365 passed, 342 ignored); `cargo test --offline -p was-compiler
-p was-compiler-cli -- --include-ignored` passed (1,687 passed, none ignored).
Strict Clippy passed for the three changed test targets. Workspace strict Clippy
was blocked by an existing `needless_range_loop` in `named_defaults.rs`; the
offline browser rebuild was blocked by a missing local `wasm-bindgen` CLI, so the
full ignored workspace suite was not run. This is focused compatibility evidence,
not integrated M6 release acceptance.

### Direct-loop allocation evidence (#84)

[Repeated deterministic accounting](m6-range-allocation-evidence.md) combines
ordinary Wasm decoding with a private test-only observer. It confirms one 32-byte
baseline record versus zero with enabled presets per completed eligible direct
construction, including empty ranges. Indirect controls still allocate, and
operand, capture and body allocations remain separately attributable. This is not
ordinary timing evidence or integrated M6 release acceptance.

### Important safety rule

Do **not** implement a general “reset heap at function return” strategy. The temporary-region path from M2 must remain isolated from longer-lived heap allocations. Function calls may return handles, mutate shared arrays, call the host, or create closure environments. A blanket reset can invalidate live data. Escape analysis or a typed ownership discipline must justify every region reset.

### Optional Goose-inspired compact layouts

Explore inline storage only when aliasing and host contracts make it observationally equivalent. For example, compact immutable `string[]` representations may reduce indirection, but random access, UTF-8 boundaries, resizing, and public descriptor layout can erase those benefits. Treat these as **measured, opt-in internal representations**, not a general replacement for the existing handle-based ABI.

**Scope gate:** Prefer straight-line or directly proven cases. If a candidate requires whole-program effects, joins, dominance, memory SSA or general alias resolution, record it as deferred rather than building a throwaway framework here.

**Exit gate:** Each optimization has a negative test that deliberately exercises aliases, reentrancy, returns and traps; optimized and unoptimized modules agree on results.

---

## 11. Milestone M7 — Safe internal `i32` / `f64` specialization

**Goal:** Lower operations to `i32` only where the existing `number` semantics can be preserved exactly.

### Representation policy

- Keep **one source-language type**: `number`.
- Preserve the public `number -> f64` function ABI for the initial release.
- Preserve `number[]` payload elements as `f64` for now.
- Permit selected **internal locals, loop counters and operations** to use `i32` after proof of equivalence.
- Insert conversions on boundaries where an `i32` temporary feeds an `f64` consumer.
- Fall back to `f64` when the proof is incomplete.

```mermaid
flowchart TD
  A[Typed `number` expression] --> B[Abstract interpretation / range analysis]
  B --> C{Integer semantics provably equivalent?}
  C -->|yes| D[Use internal i32 operations]
  C -->|no or unknown| E[Use f64 operations]
  D --> F[Convert at f64 ABI / payload boundary]
  F --> G[WebAssembly codegen]
  E --> G
```

### Cases requiring particular caution

| Operation or property | Risk |
|---|---|
| Addition/subtraction/multiplication | `i32` overflow wraps; `f64` does not wrap at 32 bits. |
| Division | `f64` can yield fractions, infinities or NaN; integer division behaves differently and may trap. |
| Remainder | Source `%` follows the language's `f64` behavior, which may differ from integer remainder on edge cases. |
| Negative zero | `-0` may be observable through arithmetic or formatting even though it compares equal to `+0`. |
| NaN and infinities | Cannot be represented as ordinary `i32` values. |
| Comparison/branching | Correct only when inputs and conversions preserve comparison semantics. |
| Loop ranges | Range implementation may internally use integer progression already; this does not justify changing all `number` values. |
| Calls and arrays | Maintain `f64` at established ABI boundaries until explicitly revised. |

### M7 interview decisions (round 1; confirmed)

- M7's implementation scope is the existing [#38–#42 subset](integer-specialization.md):
  exact signed-`i32` literals, proven addition and subtraction, eligible
  never-reassigned ordinary locals, comparisons, and restricted negation.
  Bounded induction variables, mutable locals, and broader proofs are deferred.
  If performance requires follow-up, improve the existing pass before widening it.
- Keep `number` as the single binary64 language type. As in
  [ADR 0007](adr/0007-proven-integer-specialization.md), function signatures
  (including internal calls), array elements, capture cells, and range records
  keep their existing representations. Select signed `i32` only with a static
  proof of equivalent behavior; otherwise retain `f64`. No runtime guards or
  ABI migration are in scope. M6 direct range-loop scalar replacement is separate
  and does not narrow the range progression to `i32`.
- Acceptance does not change activation: `None` stays the default and byte
  baseline; `Basic`, `Standard`, and `Aggressive` keep the same numeric policy.
  No new flag or preset split is in scope.

### M7 interview decisions (round 2; confirmed)

- Repeat matched measurements on current sources before deciding the
  [#42 trade-offs](integer-specialization.md#repeated-measurements-and-decision).
  Reuse the historical corpus for continuity, reporting compilation cost,
  module bytes, and guest time. Distinguish M7-only results from M6/M7
  composition. Reproducible size savings can qualify without a guest speedup,
  but material regressions require explicit disposition, not a post hoc threshold.
- Require current-source numeric differential execution and independent decoded
  checks, focused M6/M7 composition coverage, and the existing offline release
  gates, including a fresh browser build and parity checks and the full workspace
  run with ignored tests enabled. A blocked gate is unverified; historical passes
  do not substitute for it.
- Decide M7 separately from M4, M5, and M6. Shared validation blockers still count,
  but M7 acceptance does not resolve other milestones' pending decisions or
  authorize M8.

### M7 interview decisions (round 3; confirmed)

- Preserve all 27 historical cases. Report the 26 without an M6-eligible loop
  as the M7-focused set and report `loop_local` separately as M6/M7 composition.
  Enabled presets still select the M6 policy, even when there is no eligible
  loop; compilation timing is not an isolated cost measurement of the M7 pass.
  Do not add a compiler switch solely to isolate the measurement.
- Keep the first follow-up to tests, measurements, and documentation. Do not
  change production optimization during evidence collection. If fresh results
  show material regressions, keep M7 experimental until the user explicitly
  accepts the opt-in trade-off or authorizes a bounded improvement and another
  matched run.

The user confirmed the shared understanding after all three rounds, closing
M7's design interview. The evidence follow-up is not yet authorized, and
M7 release acceptance remains pending.

### Validation method

Run the same program both with and without specialization, comparing:

- Numeric results (including bit-level distinctions when relevant).
- Traps and short-circuit behavior.
- Mutation effects and evaluation order.
- Boundary cases for `i32`, IEEE-754, safe integers, `-0`, infinities and NaN.
- Wasm emitted types, instruction counts and benchmark results.

**Exit gate:** The documented proof subset emits `i32`-based Wasm without
semantic regression; unproven operations and storage retain `f64`. Current-source
correctness gates must pass and repeated matched measurements must establish a
size or guest-time benefit with explicit disposition of material regressions.
Until then, the subset remains experimental.

---

## 12. Milestone M8 — Language-aware optimization passes

**Goal:** Implement optimizations that benefit from information only the WAS compiler knows.

### M8 interview decisions (round 1; confirmed)

- M8 targets one bounded constant-folding pass for closed scalar expressions.
  Constant propagation, dead-code elimination, control-flow simplification,
  broader numeric representation selection, and allocation elimination are
  deferred. M6 direct range-loop scalar replacement and M7 integer specialization
  are composition cases, not new M8 deliverables.
- This interview does not create an exception to the [WP13 entry gate](was_language_extensions_plan.md#wp13--benchmark-gated-optimization-separate-authorization).
  Implementation requires WP12's verified correctness exit and separate explicit
  authorization. A confirmed design is not implementation or release approval.
- M6 and M7's pending performance decisions block M8 release acceptance, not
  design or separately authorized implementation. Require current-source
  composition evidence and explicit disposition of material regressions; an
  explicit decision to keep either optimization experimental can satisfy the
  sequencing gate without shipping it or changing defaults.
- Preserve semantic and backend diagnostics for malformed source and forged
  typed input, including unreachable and runtime-skipped children. Optimization
  must not remove or replace any check that can produce a diagnostic.

### M8 interview decisions (round 2; confirmed)

- Fold only trees built from number and bool literals and the following operators:
  numeric negation, addition, subtraction, multiplication, and ordering comparisons;
  same-type number or bool equality/inequality; and boolean negation. Use binary64
  semantics at every numeric operation, including rounding, subnormals, and signed
  zero. Require finite numeric results at every step; decline a parent fold if an
  intermediate result is nonfinite. Division, remainder, short-circuit operators,
  calls, binding reads, strings, collections, ranges, and control flow are not
  foldable roots. Their eligible children may still fold independently.
- Use private folding facts in the existing checked lowering path. Emit a folded
  constant only after checking the original subtree in diagnostic order. Do not
  rewrite the public typed AST, add a second validator or IR, or compile a baseline
  module as a preliminary check.
- `None` remains default and byte-compatible, and direct `codegen::compile` is
  unchanged. `Basic`, `Standard`, and `Aggressive` share one folding policy and
  must emit identical bytes. Require adapter byte parity within each preset. A
  private test toggle isolates M8 while holding M6 and M7 fixed; there is no new
  public flag or preset split.

### M8 interview decisions (round 3; confirmed)

- Apply the same whitelist at existing checked expression sites, including
  contracts, normalized defaults, local callables, and eligible children of
  excluded constructs. Folding must not remove a contract guard, move evaluation
  across control flow, or change argument, effect, or trap order.
- Replace only the selected subtree's emitted instructions. Keep the representation
  selected for that site and keep M6/M7 facts, local planning, helper discovery,
  and module layout based on the original tree. If no fold applies, bytes must
  match with M8 disabled and M6/M7 held fixed.
- On final sources, require independent expected results, decoded Wasm, and
  isolated M8-on/off comparisons. Cover signed zero, subnormal rounding,
  overflow fallback, forged errors in skipped and unreachable code, effect
  and trap order, and M6/M7 composition. Run the existing offline release
  gates, including a fresh browser build, adapter byte parity, and the full
  workspace with ignored tests enabled. An unavailable gate stays unverified.
- Use reproducible, matched measurements of compilation cost, module bytes,
  and guest time on folding workloads and no-fold controls, separating M8
  from M6/M7. M8 stays experimental without a demonstrated size or guest-time
  benefit and explicit disposition of material regressions. Instruction-count
  reductions alone are insufficient; do not set a threshold after seeing results.
- [ADR 0012](adr/0012-fold-scalars-during-checked-lowering.md) records the
  checked-lowering boundary and why M8 does not rewrite the typed tree.

The user confirmed the shared understanding after all three rounds, closing the
M8 design interview. The separately authorized [#92 arithmetic slice](adr/0012-fold-scalars-during-checked-lowering.md#implemented-slice-92)
is implemented after verification of WP12's correctness exit. The rest of the
whitelist and M8 release acceptance are beyond this slice.

### Original candidate list (not the M8 completion checklist)

1. Constant folding with exact source semantics and trap/evaluation-order preservation.
2. Constant propagation for bindings without conflicting writes or captures.
3. Reachability-aware dead code elimination that preserves required static diagnostics.
4. Simplification of conditionals and loops where side effects and contracts are preserved.
5. Range scalar replacement for direct consumption by `for`.
6. Local numeric representation selection backed by dataflow/range analysis.
7. Targeted allocation elimination for provably nonescaping objects, informed by M1 escape facts and M2 region profiling.

### Selected pipeline and pass contract

```text
original TypedProgram -> existing layout and representation planning
                      -> checked lowering with optional scalar folding
                      -> Wasm section assembly
```

M8 does not add guest allocation, reorder effects, or remove static checks.
The pass is independently toggleable in tests and must preserve behavior and
first-diagnostic order. M8 acceptance requires the correctness, composition,
and measurement evidence above. Binaryen and any future control-flow IR remain
separate decisions.

---

## 13. Milestone M9 — Optional Binaryen integration

**Goal:** Reuse a mature WebAssembly optimizer without making the language dependent on a particular host runtime.

### Independence from future SSA

M9 consumes valid Core Wasm emitted by the **existing Typed AST backend**. Do not make M9 depend on M10 effects/aliasing, M11 host ownership or M12 SSA IR. Preserve before/after Binaryen artifacts and differential test cases as a baseline for the later IR migration.

### Proposed integration

```text
WAS source
  -> WAS frontend / semantic passes
  -> wasm-encoder
  -> valid module.wasm
  -> [optional: wasm-opt -O2 / -O3 / -Os / -Oz]
  -> optimized module.wasm
```

### Implementation approach

- Start with **invoking the external `wasm-opt` CLI** from the native compiler CLI as an optional post-processing step.
- Keep the compiler library capable of emitting valid Wasm without Binaryen installed.
- Make tool discovery, version reporting, subprocess failure and fallback behavior explicit.
- Do not presume `-O4` or `-O3` always outperforms `-O2`: benchmark runtime and binary size separately.
- For the browser-hosted compiler, consider a separately loaded `binaryen.js` adaptation later, not as a prerequisite for the native implementation.
- Run `wasm-tools validate` or equivalent validation as part of the test pipeline when available.
- Preserve sufficient artifacts to debug miscompilations: original Wasm, optimized Wasm, compiler settings and Binaryen version.

### CLI shape (illustrative)

```text
was-compiler input.was --opt-level 0
was-compiler input.was --opt-level 2
was-compiler input.was --opt-level 3
was-compiler input.was --opt-size
```

The exact CLI flags should follow the project's established conventions. **Binaryen is an optimizer, not the runtime and not a replacement for the project's Rust frontend.**

**Exit gate:** A reproducible build can enable/disable Binaryen; missing Binaryen behaves as documented; optimizer-on/off differential tests pass across the current supported features.

---

## 14. Milestone M10 — Advanced effect and alias analysis

**Goal:** Build a reusable, conservative model of memory, host and call effects for advanced optimization without retroactively changing M2–M9 semantics.

### Scope

- Define effects such as allocation, trap, guest-memory reads/writes, host calls, possible reentry, handle publication and retention. Distinguish local-object writes from writes observable through aliases.
- Track object identity and may-alias relationships across assignment, calls, returned handles, captures and collection elements; use unknown/top states when proofs are absent.
- Start intraprocedurally with conservative call summaries; add interprocedural summaries only when supported by soundness tests.
- Keep revision-1 host behavior conservative: unknown host calls may observe and retain handles; no purity or nonretention is inferred from signatures alone.
- Provide diagnostics explaining blocked optimizations, not just yes/no answers. Store semantic facts separately from both Typed AST mutation and Wasm emission.

**Exit gate:** Adversarial alias, host callback, capture, recursion, trap and reentry tests demonstrate conservative soundness; at least one formerly unproven optimization candidate is correctly classified without changing public ABI rights.

---

## 15. Milestone M11 — Optional managed host ownership protocol

**Goal:** Introduce an **opt-in** mechanism allowing the guest and host to coordinate management of explicitly enrolled memory objects, while preserving revision-1 raw handles unchanged.

### Scope

- Define enrollment/provenance separately from descriptor validity and structural bounds checks.
- Specify whether managed handles use explicit retain/release, handles with generation checks, or another versioned protocol; **do not assume** that one mechanism is already approved.
- Preserve existing unregistered raw host handles for the instance lifetime. Never impose retroactive root registration, release obligations or different trap-retention rules on existing clients.
- Specify reentry, duplicate release, stale handle, failed allocation and trap behavior before allowing reuse of managed buffers.
- Keep integration optional for Node/browser hosts and document interoperation between raw and managed handles.

**Exit gate:** Managed handles obey the chosen lifecycle under nested calls, growth and traps; raw revision-1 compatibility fixtures remain valid and independent of enrollment.

---

## 16. Milestone M12 — SSA-oriented intermediate representation

**Goal:** Add a compiler-owned optimization IR with explicit control flow **after** the stable Typed AST path and preserve the existing backend as a differential oracle during migration.

### Scope and staged migration

1. Define typed values, basic blocks, terminators and block parameters (or equivalent phi semantics); use explicit operations for allocations, loads, stores, traps and calls.
2. Lower a narrow subset of numeric functions from the Typed AST first. Preserve source semantics and the established `number`/`f64` boundaries.
3. Add CFG verification (block arguments and types, dominance, well-formed branches, effects) and a debugging dump format.
4. Generate valid structured Core Wasm from the IR, initially alongside the current codegen rather than replacing it.
5. Extend lowering to arrays, dynamic strings, closures, ranges, contracts and host interactions with explicit memory effects.
6. Migrate individual optimizations only after program-level equivalence is demonstrated. Retain compatibility with optional Binaryen.

**Exit gate:** The IR path matches the existing path for an agreed feature corpus across results, memory effects, host interactions, traps and ABI layout; supported optimization levels retain differential coverage. Incomplete language constructs stay on the old path until validated.

---

## 17. Milestone M13 — Advanced dataflow and allocation optimization

**Goal:** Use M10 analysis and M12 IR to implement transformations that would otherwise require an ad hoc CFG and alias framework on the Typed AST.

### Candidate passes

- Global constant propagation and value numbering where observable effects permit.
- Dominance- and loop-aware range propagation, induction-variable simplification and loop-invariant code motion with trap/order guards.
- Scalar replacement of aggregates, finer-grained allocation removal and region expansion beyond the initial invocation-wide M1/M2 subset when aliasing and lifetime proofs are sufficient.
- Dead-store elimination and targeted specialization, with memory effects explicitly accounted for.
- Numeric `i32` representation propagation through SSA joins and reassignments, preserving `-0`, overflow, NaN and public `f64` semantics.

**Exit gate:** Each transformation has adversarial correctness tests, pass-on/off differential cases, reproducible benchmarks and a conservative fallback. Require measured benefit or demonstrably better generated code before enabling broadly.

---

## 18. Milestone M14 — Profile-guided optimization and performance tooling

**Goal:** Prioritize optimizations using reproducible workload evidence and make optimization decisions inspectable without requiring a production debugging runtime.

### Scope

- Establish instrumented workloads and offline profile artifacts with explicit engine/compiler/Binaryen versions and warm-up configuration.
- Prototype profile-guided prioritization or specialization of hot functions; do not require profiling for correct or useful default compilation.
- Add optional optimization reports: applied passes, numeric representation decisions, allocation/region decisions and conservative rejection reasons.
- Compare compilation latency, guest execution time, Wasm size, capacity growth and genuine retained memory (when measurable); do not infer live bytes from pages.
- Separate test/debug counters from release builds and maintain privacy/performance boundaries for runtime profiling.

**Exit gate:** A profile is reproducible; profile/no-profile behavior is semantically equivalent; reports explain decisions without changing Wasm bytes when reporting alone is enabled.

---

## 19. Testing, benchmarks and observability

### Testing matrix

| Test family | Focus | Required variants |
|---|---|---|
| Memory safety | Bounds, alignment, overflow, growth failure | Native compiler / browser compiler where supported |
| Container semantics | `push`, `pop`, `remove`, aliasing, `string[]`, iteration mutation | Small and large capacities; shared handles |
| Host ABI | Descriptor/handle stability and marshaling | Node and browser adapters |
| Closures | Shared cells, nesting, recursion, reentrancy, traps | Before/after lifetime optimizations |
| Strings | UTF-8, literal interning, replacement, builder | Unicode edge cases, alias cases |
| Numerical equivalence | `f64` vs specialized `i32` | Boundary and randomized cases |
| Optimizer equivalence | Original vs own passes vs Binaryen; later SSA and PGO paths | All optimization levels |
| Advanced effects and host ownership (post-M9) | Aliases, reentry, publication, managed-vs-raw handles | Unknown-call fallback and opt-in managed protocol |
| IR differential validation (post-M9) | Typed AST backend vs SSA lowering and backend | CFG joins, loops, arrays, captures, traps |
| Long-running execution | Sustained allocation and reclamation | Millions of operations, repeated calls |

### Suggested benchmark dimensions

| Metric | Why it matters |
|---|---|
| Execution time | Detect optimization wins and regressions. |
| Wasm file size | Important for browser download and loading. |
| Compilation time | Important for interactive playground latency. |
| Memory pages / peak memory | Measures actual memory growth. |
| Allocations and reallocations | Helps explain memory and execution costs. |
| Bytes copied on resize | Diagnoses overly aggressive array growth or string construction. |
| Host boundary cost | Separates pure computation from marshaling overhead. |

Benchmark scenarios should include numeric loops, Fibonacci/recursion, array append-heavy workloads, repeated string construction, range loops, nested closures and host-visible array mutations. Record engine versions and warmup policy; compare like-for-like Wasm semantics, not just headline durations.

**No numerical improvement target is asserted here.** Establish an empirical baseline before agreeing on regression budgets or performance targets.

---

## 20. Sequencing and dependencies

```mermaid
flowchart TD
  M0[M0: Semantics and regression baseline] --> M1[M1: Escape and lifetime analysis]
  M1 --> M2[M2: Temporary region prototype]
  M0 --> M3[M3: Reusable managed heap]
  M3 --> M4[M4: Dynamic arrays]
  M0 --> M5[M5: Immutable string operations]
  M2 --> M6[M6: Conservative lifetime and layouts]
  M4 --> M6
  M5 --> M6
  M0 --> M7[M7: Safe i32/f64 specialization]
  M6 --> M8[M8: Basic language-aware optimizations]
  M7 --> M8
  M8 --> M9[M9: Optional Binaryen]
  M9 --> M10[M10: Advanced effects and aliases]
  M10 --> M11[M11: Optional host ownership]
  M10 --> M12[M12: SSA IR]
  M9 --> M12
  M12 --> M13[M13: Advanced optimization]
  M10 --> M13
  M13 --> M14[M14: PGO and tooling]
  M0 --> T[Continuous tests and benchmarks]
  M14 --> T
```

| Priority | Milestone | Primary outcome | Main prerequisite |
|---|---|---|---|
| P0 | M0 — ABI / semantic contracts | Baselines and documented alias behavior | Existing code |
| P0 | M1 — Escape / lifetime analysis | Conservative storage classification | M0 |
| P0 | M2 — Temporary regions | Provably safe bulk reclamation | M1 |
| P0 | M3 — Managed heap reuse | Reuse of proven-dead owned buffers, with conservative fallback | M0; coordinate with M2 |
| P0 | M4 — Dynamic arrays | Stable descriptors and safe resizing | M0, M3 core; ownership-compatible design |
| P0 | M5 — String operations | Useful immutable transformations | M0; may proceed in parallel |
| P1 | M6 — Conservative lifetime/layout optimization | Locally proven scalar replacement and compact representations | M1, M2, M4/M5 |
| P1 | M7 — `i32` / `f64` specialization | Proven-correct numeric lowering | M0; partially implemented, may proceed in parallel |
| P1 | M8 — Basic language-aware optimizer | Typed AST passes without mandatory CFG/SSA | M6/M7 |
| P1 | M9 — Binaryen | Optional Wasm postprocessing | Valid codegen; may prototype earlier |
| P2 | M10 — Advanced effects/aliases | Shared soundness facts and conservative call summaries | M9 roadmap phase (architecturally independent) |
| P2 | M11 — Host ownership protocol | Optional managed-handle lifecycle | M10; preserve revision-1 raw handles |
| P2 | M12 — SSA IR | Verified CFG + staged alternative backend | M9; M10 facts helpful but not required for numeric prototype |
| P2 | M13 — Advanced optimizer | CFG/dataflow, loops and allocation optimization | M10, M12 |
| P2 | M14 — PGO and tooling | Profile-driven choices and explainable optimizer | M13 |
| Continuous | Tests and benchmarking | Correctness and measured improvements | Every milestone |

**Pragmatic sequencing:** Finish the already-started M2 unchanged. M3–M9 must stay implementable on the current Typed AST/codegen; M3/M4 must honor revision-1 retention and obtain explicit management rights before relocating/reclaiming storage. Defer global alias analysis, optional managed-host ownership and mandatory SSA until after M9. M11 and M12 may progress independently after their own contracts are established; the arrows express a suggested rollout, not a release mandate.

---

## 21. Decisions required before implementation

These are decision checkpoints, not hidden assumptions. M5 checkpoints 8–9
are resolved in §9's confirmed interview record, and M7 checkpoint 10 is
resolved for this milestone in §11. Other checkpoints retain their
milestone-specific status.

1. **Array descriptor compatibility:** adopt a public ABI v2 (e.g., 16-byte descriptor), or preserve the public 8-byte descriptor with private capacity metadata?
2. **Reclamation model:** use proven region reclamation for temporaries; what ownership, RC, tracing, or nonreclamation fallback applies to arbitrary shared heap objects?
3. **Post-M2 region expansion:** M2 already specifies per-function exclusive reusable blocks, normal-return reuse and trap quarantine. What additional lifetimes or trap-recovery protocols would justify broadening this policy later?
4. **Escape-analysis contract:** what host annotations or ABI rules allow proving a handle is not retained? Is unknown host behavior always considered escaping?
5. **Host ownership:** may JS/Node allocate and hand off array descriptors? Can the Wasm side resize these arrays, and who frees their storage?
6. **`pop` and empty collections:** return type and error/trap behavior given the currently limited optional-value types.
7. **Iteration under mutation:** M0 confirms initial-length capture and live checked reads as the proposed M4 contract; M4 must implement/test this behavior without assuming it is already shipped.
8. **String API (M5 design resolved):** immutable transformation builtins plus fragment accumulation in `string[]` and one final `string_join`; no new mutable builder type. See §9.
9. **Unicode indexing (M5 design resolved):** explicit byte lengths/offsets with strict scalar-boundary slicing; scalar/grapheme indexing and normalization deferred. See §9.
10. **ABI numeric boundaries (M7 design resolved):** retain `f64` for all `number` signatures and numeric array elements. ABI changes are outside M7; see §11.
11. **Optimization failure behavior:** should a missing/failing Binaryen installation be an error only for explicitly requested optimization, or always fall back to unoptimized Wasm?
12. **Memory diagnostics:** M0/M2 establish test-only instrumentation boundaries; which additional counters and opt-in reports are worthwhile for M14 without shipping a debugging runtime in release builds?
13. **M10 effect granularity:** which calls, memory effects and alias classes merit precise modeling after M9 versus unknown conservative fallback?
14. **M11 managed host protocol:** what opt-in provenance and lifecycle mechanism enables reclaiming enrolled handles without changing revision-1 raw retention?
15. **M12 SSA design:** use block parameters or phi nodes, what verification and CFG-to-structured-Wasm strategy, and how long should the legacy codegen coexist?
16. **M14 profiling:** which workload format, performance counters and offline profile compatibility policy are practical for browser and native compilation?

---

## 22. Explicit non-goals

The following are deliberately not prerequisites for this cycle:

- Multi-file modules, a standard-library module resolver, or a package manager.
- WASI as a required import or target-specific I/O abstractions.
- WebAssembly Component Model and WIT interoperability.
- Native executables or a Cranelift native backend.
- A mandatory full compiler IR rewrite **before M9** (an incremental SSA IR is planned in M12).
- General interprocedural effect/alias infrastructure or host-managed-handle enrollment as a prerequisite of M2–M9 (planned for M10–M11).
- Profile-guided compilation as a prerequisite for efficient default output (optional after M13).
- A full Goose-style data-stack model relying on independently growing native virtual-memory regions.
- General-purpose garbage collection **without** a separate ownership/lifetime design.
- Full first-class function values or arbitrary closure escape if the language continues to disallow them.
- Transparent integer specialization of public `number[]` data.

---

## 23. Definition of “stable enough to optimize”

Memory and numerical representation are sufficiently stable to begin aggressive optimization when:

- Source-level aliasing and string mutation semantics are documented and tested.
- Dynamic array resizing preserves stable object identity and host-safe ABI contracts.
- No compiler-supported operation can reuse live memory or silently corrupt an existing descriptor.
- Escape analysis proves temporary-region lifetimes and defaults to the managed heap when uncertain.
- Region bulk-reset cannot invalidate an escaping alias, host-visible handle or an interleaved heap allocation.
- Long-lived workloads demonstrate understood, measured memory behavior; reclamation limits are documented.
- Closure recursion, reentry and capture mutations still behave correctly after memory changes.
- Internal numeric specialization has a differential-test harness against the `f64` baseline.
- The raw unoptimized module remains valid, independently executable Core WebAssembly.
- Binaryen is optional and optimizer-on/off equivalence is tested.

**Outcome through M9:** a small, well-defined runtime model with proven conservative optimization and optional Binaryen, without making SSA, PGO, modules, WASI or multiple output targets part of the critical path. **After M9:** progressively extend analyses, optional host ownership and compiler IR under separate compatibility and differential-test gates.

---

## References

Repository sources reviewed or used as architectural reference (public `main` branch, accessed October 17, 2026):

1. [WAS Compiler README](https://github.com/WAS-Language/was-compiler)
2. [Memory ABI documentation](https://github.com/WAS-Language/was-compiler/blob/main/docs/memory-abi.md)
3. [Memory codegen and helpers](https://github.com/WAS-Language/was-compiler/blob/main/crates/compiler/src/codegen/memory.rs)
4. [ABI constants and type mapping](https://github.com/WAS-Language/was-compiler/blob/main/crates/compiler/src/codegen/abi.rs)
5. [Value lowering](https://github.com/WAS-Language/was-compiler/blob/main/crates/compiler/src/codegen/values.rs)
6. [Binaryen project](https://github.com/WebAssembly/binaryen)
7. [Goose, Byte by Byte](https://aardappel.github.io/goose/docs/html/Goose,%20Byte%20by%20Byte.html) — inspiration for compile-time lifetime reasoning, region/data-stack techniques and representation specialization; its performance results are not assumed to transfer to WAS.
8. [Goose — Running on WebAssembly](https://aardappel.github.io/goose/docs/design/wasm.html) — design caveats for adapting native virtual-memory data stacks to WebAssembly linear memory.

This plan is an engineering proposal and should be revisited after implementation milestones or relevant changes to the source language's semantics.
