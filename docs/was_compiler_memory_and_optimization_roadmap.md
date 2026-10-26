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
  fixed-storage lifetime guarantees. Future lowering must preserve pre-RHS checked
  assignment-place semantics: moving storage must not invalidate or silently
  retarget an outstanding checked write.
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

Possible internal operations:

```text
alloc(size, alignment) -> address
realloc_owned(old_address, old_size, new_size, alignment) -> address
release_owned(address, allocation_class) -> void
```

These are runtime implementation operations, **not necessarily public WAS language functions**.

### Key engineering requirements

- Track sufficient allocation metadata to distinguish dynamically owned buffers from static or host-owned regions.
- Preserve alignment required by `f64` and descriptor storage.
- Keep widened arithmetic and overflow checks from the existing allocator.
- Ensure a failed `realloc` leaves the original live allocation intact.
- Avoid reusing a block until all references to that block have been retired under a sound ownership rule.
- Validate behavior after `memory.grow`, especially in JavaScript where cached typed-array views may need refreshing.
- Document whether fragmentation and coalescing are supported; an initial size-class allocator may be intentionally limited.

### Crucial limitation

A free list answers **how to reuse released memory**, not **when memory can be released**. Region classification from M1/M2 handles only proven temporaries; arbitrary shared heap values still require a separate liveness/ownership policy. The second question needs a language-level or compiler-proven lifetime rule. Do not free an array descriptor merely because a local binding is overwritten; another alias or the host may still reference it.

### Exit gate

- A dedicated stress test repeatedly allocates and releases **provably owned buffers** and demonstrates bounded reserved-memory growth for that scenario.
- Wrong-size frees, stale pointers, double release, and use-after-free cannot be triggered through supported language operations.
- Host-owned/static values cannot accidentally enter the free lists.

---

## 8. Milestone M4 — Resizable typed arrays

**Goal:** Add collection operations without changing the current alias model.

### Recommended representation

```text
Stable array handle
        |
        v
+--------------------------+
| data_ptr : u32   (+0)    |
| length   : u32   (+4)    |
| capacity : u32   (+8)    |  <- Proposed private/ABI-v2 layout
| reserved/padding (+12)   |
+--------------------------+
        |
        v
[ element 0 ][ element 1 ] ... [ capacity - 1 ]
```

**ABI choice required:** Either (A) adopt a new 16-byte descriptor and version all public consumers, or (B) preserve the public 8-byte descriptor while storing capacity in separately managed metadata. Option A is simpler internally; Option B may preserve more compatibility but can be significantly harder to implement safely. Do not treat the diagram above as the final ABI until this decision is made.

### Growth algorithm

1. Evaluate the array receiver and new element exactly once in a specified order.
2. Validate the handle, type and capacity arithmetic.
3. If `length < capacity`, write the new element and increment the length.
4. Otherwise allocate a larger backing buffer (a geometric growth policy is a reasonable starting point).
5. Copy live elements using the correct element width; update the stable descriptor only after successful allocation/copy.
6. Retire the old owned buffer **only if** no supported alias can independently retain it.

### Proposed API

```text
values.push(item)
values.pop()
values.reserve(additional)
values.remove(index)
len(values)
```

The exact syntax and `pop` return behavior (particularly for empty arrays) require a separate language-design decision. These examples are target behaviors, not confirmed features.

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

### Recommended semantics

Keep `string` immutable. Operations such as concatenation and replacement return an immutable result without changing existing strings; unchanged or already available results may reuse storage rather than requiring fresh allocation. Reassignment updates a binding, not the bytes owned by the prior string value.

```text
let message = "hello";
message = message.replace("hello", "world");
```

That supports a common meaning of “change a string” without exposing shared mutable UTF-8 buffers. Literal interning remains valid.

### Implementation considerations

- Store/validate UTF-8 byte length independently of any future Unicode scalar or grapheme operations.
- Specify whether indexing is by byte, scalar value, or grapheme cluster; do not conflate them.
- Prevent a shared literal from becoming the destination of a mutable write.
- For many concatenations, introduce a builder/buffer abstraction or compiler-recognized concatenation lowering to reduce intermediate allocations.
- Decide whether string APIs become builtins first or wait for a future standard library. They need not wait for a module system.
- Ensure `string[]` stores valid handles and preserves value semantics during array resizing.

### Candidate staged surface

| Step | Candidate functionality | Rationale |
|---|---|---|
| First | Concatenation, `replace`, substring/slice | Usable immutable transformations. |
| Next | `StringBuilder` or equivalent buffered construction | Avoid allocating O(n) intermediates in repeated appends. |
| Later | Unicode-aware iteration/editing | Requires explicit indexing and normalization decisions. |

**Exit gate:** Modifying a binding's string never mutates an aliased or interned string; UTF-8 remains valid; repeated construction has measured allocation behavior.

---

## 10. Milestone M6 — Escape and lifetime improvements

**Goal:** Refine M1/M2 escape proofs and reduce allocations further through scalar replacement, compact layouts and closure/range specialization. This is the advanced optimization stage, not the first introduction of lifetime analysis.

| Target | Current concern | Candidate optimization | Soundness precondition |
|---|---|---|---|
| Direct range loop | A range value may require a 32-byte record | Scalar-replace `start`, `end`, `step`, inclusivity into Wasm locals | Range never needs object identity or an exported handle. |
| Nonescaping closure | Capture cells/environments allocate monotonically | Store proven nonescaping state in an invocation-scoped region or optimize away the environment | All uses and recursive/reentrant lifetimes are accounted for. |
| Immutable temporary string | Intermediate objects accumulate | Fuse concatenations or use a builder internally | Observable allocation/evaluation semantics remain unchanged. |
| Temporary array buffer | Superseded storage consumes heap | Reuse buffer under controlled ownership | No stale pointer or host reference remains. |

### Important safety rule

Do **not** implement a general “reset heap at function return” strategy. The temporary-region path from M2 must remain isolated from longer-lived heap allocations. Function calls may return handles, mutate shared arrays, call the host, or create closure environments. A blanket reset can invalidate live data. Escape analysis or a typed ownership discipline must justify every region reset.

### Optional Goose-inspired compact layouts

Explore inline storage only when aliasing and host contracts make it observationally equivalent. For example, compact immutable `string[]` representations may reduce indirection, but random access, UTF-8 boundaries, resizing, and public descriptor layout can erase those benefits. Treat these as **measured, opt-in internal representations**, not a general replacement for the existing handle-based ABI.

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

**Suggested first slice:** specialize a small, proven-safe integer induction variable in a bounded loop, keeping the public result as `f64`. Do not start with a generic “integer-looking literal -> i32” rewrite.

### Validation method

Run the same program both with and without specialization, comparing:

- Numeric results (including bit-level distinctions when relevant).
- Traps and short-circuit behavior.
- Mutation effects and evaluation order.
- Boundary cases for `i32`, IEEE-754, safe integers, `-0`, infinities and NaN.
- Wasm emitted types, instruction counts and benchmark results.

**Exit gate:** A documented set of proven-safe patterns produces `i32`-based Wasm without semantic regression; everything else still uses `f64`.

---

## 12. Milestone M8 — Language-aware optimization passes

**Goal:** Implement optimizations that benefit from information only the WAS compiler knows.

### Recommended starting passes

1. Constant folding with exact source semantics and trap/evaluation-order preservation.
2. Constant propagation for bindings without conflicting writes or captures.
3. Reachability-aware dead code elimination that preserves required static diagnostics.
4. Simplification of conditionals and loops where side effects and contracts are preserved.
5. Range scalar replacement for direct consumption by `for`.
6. Local numeric representation selection backed by dataflow/range analysis.
7. Targeted allocation elimination for provably nonescaping objects, informed by M1 escape facts and M2 region profiling.

### Suggested pipeline

```text
Typed AST
  -> semantic and effect information
  -> conservative optimization passes
  -> optional representation/lifetime analysis
  -> wasm-encoder
  -> optional Binaryen wasm-opt
  -> validated Core Wasm
```

Avoid prematurely introducing a large custom intermediate representation. If join points, control-flow edges and effect analysis become cumbersome in the Typed AST, consider a small explicit-control-flow IR as a **later design decision**, supported by concrete optimizer needs.

### Pass contract

Each pass should declare its prerequisites, preserved invariants, whether it can allocate, and whether it may reorder effects. Optimizations must never suppress static errors in unreachable code if that remains the language's checking policy.

**Exit gate:** Passes are independently toggleable in tests and preserve the behavior of their unoptimized inputs.

---

## 13. Milestone M9 — Optional Binaryen integration

**Goal:** Reuse a mature WebAssembly optimizer without making the language dependent on a particular host runtime.

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

## 14. Testing, benchmarks and observability

### Testing matrix

| Test family | Focus | Required variants |
|---|---|---|
| Memory safety | Bounds, alignment, overflow, growth failure | Native compiler / browser compiler where supported |
| Container semantics | `push`, `pop`, `remove`, aliasing, `string[]`, iteration mutation | Small and large capacities; shared handles |
| Host ABI | Descriptor/handle stability and marshaling | Node and browser adapters |
| Closures | Shared cells, nesting, recursion, reentrancy, traps | Before/after lifetime optimizations |
| Strings | UTF-8, literal interning, replacement, builder | Unicode edge cases, alias cases |
| Numerical equivalence | `f64` vs specialized `i32` | Boundary and randomized cases |
| Optimizer equivalence | Original vs own passes vs Binaryen | All optimization levels |
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

## 15. Sequencing and dependencies

```mermaid
flowchart TD
  M0[M0: Semantics and regression baseline] --> M1[M1: Escape and lifetime analysis]
  M1 --> M2[M2: Temporary region prototype]
  M0 --> M3[M3: Reusable managed heap]
  M3 --> M4[M4: Dynamic arrays]
  M0 --> M5[M5: Immutable string operations]
  M2 --> M6[M6: Advanced lifetime and layouts]
  M4 --> M6
  M5 --> M6
  M0 --> M7[M7: Safe i32/f64 specialization]
  M6 --> M8[M8: Language-aware optimizations]
  M7 --> M8
  M8 --> M9[M9: Optional Binaryen]
  M0 --> T[Continuous tests and benchmarks]
  M9 --> T
```

| Priority | Milestone | Primary outcome | Main prerequisite |
|---|---|---|---|
| P0 | M0 — ABI / semantic contracts | Baselines and documented alias behavior | Existing code |
| P0 | M1 — Escape / lifetime analysis | Conservative storage classification | M0 |
| P0 | M2 — Temporary regions | Provably safe bulk reclamation | M1 |
| P0 | M3 — Managed heap reuse | Reuse known-dead shared buffers | M0; coordinate with M2 |
| P0 | M4 — Dynamic arrays | Stable descriptors and safe resizing | M0, M3 core |
| P0 | M5 — String operations | Useful immutable transformations | M0; may proceed in parallel |
| P1 | M6 — Lifetime/layout optimization | Scalar replacement and compact internal representations | M1, M2, M4/M5 |
| P1 | M7 — `i32` / `f64` specialization | Proven-correct numeric lowering | M0; may proceed in parallel |
| P1 | M8 — Language-aware optimizer | Controlled semantic passes | M6/M7 for advanced passes |
| P1 | M9 — Binaryen | Optional Wasm optimizer | Valid codegen; may prototype earlier |
| Continuous | Tests and benchmarking | Correctness and measured improvements | Every milestone |

**Pragmatic sequencing:** M1 can initially target a narrow family of temporaries without blocking array functionality. M2 must be designed for safe isolation, not a blanket heap reset. M3/M4 can proceed in parallel with broader escape-analysis work once M0 is established. A basic `wasm-opt` experiment can happen at any time because it consumes valid generated Wasm.

---

## 16. Decisions required before implementation

These are intentionally **open decisions**, not hidden assumptions:

1. **Array descriptor compatibility:** adopt a public ABI v2 (e.g., 16-byte descriptor), or preserve the public 8-byte descriptor with private capacity metadata?
2. **Reclamation model:** use proven region reclamation for temporaries; what ownership, RC, tracing, or nonreclamation fallback applies to arbitrary shared heap objects?
3. **Temporary-region shape:** per-invocation independent arena, bounded scratch region, or reusable managed block? How are recursion, callbacks and trap paths handled?
4. **Escape-analysis contract:** what host annotations or ABI rules allow proving a handle is not retained? Is unknown host behavior always considered escaping?
5. **Host ownership:** may JS/Node allocate and hand off array descriptors? Can the Wasm side resize these arrays, and who frees their storage?
6. **`pop` and empty collections:** return type and error/trap behavior given the currently limited optional-value types.
7. **Iteration under mutation:** what happens if `remove` invalidates the next index? Initial-length snapshots are a recommendation, not an established dynamic-array contract.
8. **String API:** immutable `replace`/concat only, or an additional explicit mutable builder?
9. **Unicode indexing:** bytes, Unicode scalars, grapheme clusters, or a restricted first implementation?
10. **ABI numeric boundaries:** retain `f64` for all public `number` signatures and numeric arrays until a separately planned ABI version?
11. **Optimization failure behavior:** should a missing/failing Binaryen installation be an error only for explicitly requested optimization, or always fall back to unoptimized Wasm?
12. **Memory diagnostics:** how will the compiler or test harness expose heap cursor, reused bytes, and outstanding allocations without shipping a debugging runtime in release builds?

---

## 17. Explicit non-goals

The following are deliberately not prerequisites for this cycle:

- Multi-file modules, a standard-library module resolver, or a package manager.
- WASI as a required import or target-specific I/O abstractions.
- WebAssembly Component Model and WIT interoperability.
- Native executables or a Cranelift native backend.
- A mandatory full compiler IR rewrite.
- A full Goose-style data-stack model relying on independently growing native virtual-memory regions.
- General-purpose garbage collection **without** a separate ownership/lifetime design.
- Full first-class function values or arbitrary closure escape if the language continues to disallow them.
- Transparent integer specialization of public `number[]` data.

---

## 18. Definition of “stable enough to optimize”

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

**Outcome:** a small, well-defined runtime model that can support meaningful optimization and portability work later, without making modules, WASI, or multiple output targets part of the current critical path.

---

## References

Repository sources reviewed or used as architectural reference (public `main` branch, accessed October 17, 2026):

1. [WAS Compiler README](https://github.com/LuanRoger/was-compiler)
2. [Memory ABI documentation](https://github.com/LuanRoger/was-compiler/blob/main/docs/memory-abi.md)
3. [Memory codegen and helpers](https://github.com/LuanRoger/was-compiler/blob/main/crates/compiler/src/codegen/memory.rs)
4. [ABI constants and type mapping](https://github.com/LuanRoger/was-compiler/blob/main/crates/compiler/src/codegen/abi.rs)
5. [Value lowering](https://github.com/LuanRoger/was-compiler/blob/main/crates/compiler/src/codegen/values.rs)
6. [Binaryen project](https://github.com/WebAssembly/binaryen)
7. [Goose, Byte by Byte](https://aardappel.github.io/goose/docs/html/Goose,%20Byte%20by%20Byte.html) — inspiration for compile-time lifetime reasoning, region/data-stack techniques and representation specialization; its performance results are not assumed to transfer to WAS.
8. [Goose — Running on WebAssembly](https://aardappel.github.io/goose/docs/design/wasm.html) — design caveats for adapting native virtual-memory data stacks to WebAssembly linear memory.

This plan is an engineering proposal and should be revisited after implementation milestones or relevant changes to the source language's semantics.
