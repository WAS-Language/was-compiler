# M1 lifetime analysis — conservative subset (#50–#51)

Issue #50 implements the first private classifier, not the complete M1 milestone
or M2 reclamation. The confirmed contract is in
[roadmap §5](was_compiler_memory_and_optimization_roadmap.md#5-milestone-m1--conservative-escape-and-lifetime-analysis).

## Invocation and facts

`crates/compiler/src/lifetime.rs` is compiled only under `cfg(test)` and explicitly
invoked by its unit tests. Compilation and emission never call it. Facts borrow
the original function and literal, retaining the function-local binding identity
and static element type. Functions and direct body declarations determine report
order; hash maps are used only for binding lookup. No public AST fields, IDs,
exports, dependencies or storage-selection consumers are added.

The first slice reports **direct function-body let-bound array literals only**.
Nested origins, nonliteral initializers, capture cells and environments are not
inventoried. Their enclosing unsupported forms still block the invocation-wide
positive proof. This is intentionally not a complete allocation inventory.

Outcomes distinguish temporary candidates, identified outward handle paths, and
unproven results with typed reasons. Returns and host arguments are identified
structurally even in unreachable syntax; these are potential paths, not claims
that execution reaches them. A host call is an observation boundary, not evidence
that the host actually retained any particular allocation. Issue #51 adds the
explanation traversal below without widening #50's positive proof.

## Closed positive subset

Only single-binding `number[]` and `bool[]` literals qualify. The entire allocating
function must stay in the subset, including syntax after a return and parameter
contracts. Supported operations are scalar number/bool parameters and ordinary
locals, scalar literals, arithmetic/comparisons/unary operators, array element
reads, builtin `len`, checked element replacement, all five numeric element
compounds, and scalar/bare returns. Element initializers, indices and right-hand
sides are checked recursively. Empty arrays and potentially trapping operations
are allowed; candidate status does not imply trap freedom.

Aliases, handle reassignment, scalar writes, other handle uses/types, non-builtin
calls, normalized/lexical calls, captures/callable declarations, block expressions,
branches, match, loops and short-circuiting cannot earn a positive result. Scalar
result annotations never excuse unsupported operands. Sparse binding IDs work by
lookup; duplicate or missing bindings and inconsistent supported annotations are
unproven. This is not a replacement checked-tree API: existing backend validation
remains independent and may reject a forged tree regardless of classification.

Candidate status changes no allocation, bytes, runtime behavior, ABI or numeric
preset. It never authorizes heap rewind, early cleanup or reclamation after traps.
Existing [ownership and lifetime guarantees](memory-abi.md#ownership-and-lifetime)
remain unchanged. No performance or reclaimed-memory claim is made.

## Conservative explanations (#51)

The closed positive proof remains separate from the exhaustive structural
explanation traversal in `crates/compiler/src/lifetime/diagnostics.rs`. A failed
proof never stops explanation collection. Every fact retains a `reasons` vector:

- Structural preorder, contracts before body, statements and expression children
  in their stored order. Form-level reasons precede their children. Return/host
  publication evidence appears at the corresponding handle leaf. Repeated reasons
  are retained; this is syntax order, not a claim about executed effects.
- Normalized bindings are inspected in evaluation order, then an immediate helper
  declaration/body (when present), then the canonical call. Named/default calls,
  lexical stages, immediate stages and recursion remain call barriers. No callee
  summaries or call-result provenance are inferred.
- If absent from that vector, the first closed-proof failure is appended last.
  Duplicate owner-local declarations or malformed closure ownership suppress
  escape evidence and append an inconsistency reason. Hash iteration never
  determines report order.
- The summary `outcome` uses deterministic precedence: returned handle, potential
  host publication, then the original closed-proof result. The summary is not a
  replacement for the multiple-reason vector; a later return never hides an
  earlier host path, or vice versa.

Path evidence follows only direct literal bindings and immutable, direct let-copy
chains (including invocation-local normalized argument bindings). A preliminary
structural inventory excludes **every** written binding, even if the write is
later, skipped, unreachable or inside a capturing helper. Aliases without an
identified outward path receive `AliasCreation`, not an escape claim. Reassignment
receives `HandleReassignment`; unsupported or mutable propagation stays unproven.
This is bounded syntactic copy evidence, not general alias analysis, liveness,
branch refinement or an expanded candidate set.

Return/host publication context passes through block tails, if branches and match
arms, but not arbitrary operations, loop results or call results. Nested returns,
normalized bindings, array elements, indices and right-hand sides are still
visited. A scalar-only host call receives `HostObservation` even before an origin
or after apparent last use: the proof is invocation-wide, and exported memory
and reentry prevent a no-observation inference from absent handle arguments.

Keys are `(BindingOwner, VariableId)`, never numeric variable IDs alone. Callable
declarations block the creator proof even when unused; captures and forwarded
contexts receive `CaptureStorage`. Helper bodies are inspected but do not produce
creator origins, and helper returns alone are not returns from the allocating
invocation. Explicit original-owner captured arguments to hosts can identify
potential publication even within an uninvoked helper's syntax. This is not an
assertion that a declaration executes its body. String arrays, ranges, capture
cells/environments and unsupported handle operations remain outside the positive
set and the existing direct-body origin inventory remains unchanged.

No production consumer, public API/AST/ABI, numeric policy, allocation or emission
changes are introduced. Runtime noninterference gets separate evidence under
#52; integrated M1 acceptance and M2 authorization are not claimed by #51.

## Focused evidence

Run `cargo test --offline -p was-compiler --lib lifetime`. The private tests cover
both positive types, each arithmetic/comparison operator and numeric compound,
empty arrays, nested supported expressions, invocation-wide exclusions, typed
reasons, host observation, contracts, owner-local deterministic order, forged
annotations, and unchanged borrowed trees/validated Wasm bytes before and after
explicit analysis separately at all four presets.

Issue #50's native gates are `cargo test --offline` and
`cargo clippy --offline --all-targets -- -D warnings`. Full ignored-runtime and
current-source browser gates in the roadmap apply to integrated M1 acceptance;
these slices do not claim completion of those gates or the M1 milestone.

Issue #51 adds private source/forged-tree coverage for immutable alias returns,
non-escaping aliases, reassignment, deterministic multiple reasons, owner-local
sparse/duplicate IDs, nested/forwarded captures, unused helpers, all call forms,
pipeline/immediate stages, recursion, host observation before/after use, skipped
and unreachable host publication, control-flow/value-block children, contracts
and normalized defaults. These classification tests require no Node runtime.
