# M1 lifetime analysis — first subset (#50)

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

Outcomes distinguish temporary candidates, an identified direct returned-handle
escape path, and unproven results with typed reasons. Direct returns are identified
structurally even in unreachable syntax; this is a possible escape path, not a
claim that execution reaches the return. Aliases and indirect escape propagation
are not proven. A host call is an observation boundary, not evidence that the host
actually retained any particular allocation.

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
this first slice does not claim completion of those gates or the M1 milestone.
