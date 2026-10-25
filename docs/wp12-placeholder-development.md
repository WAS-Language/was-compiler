# WP12 design interview and implementation handoff

## Status and scope

The user accepted interview decisions Q1–Q25 and confirmed the complete shared
understanding in Q26. WP11's release satisfied the predecessor gate. Explicit
authorization for issue #44 implemented known lexical callable pipeline stages;
integrated WP12 release validation remains pending. Issue #45 implements the
direct-constraint local-declaration milestone described below. Immediate stages,
local/collection propagation (#46), and full control-flow/nesting composition
(#47) remain deferred. This is not full WP12 release evidence.

### Implemented direct-constraint milestone (#45)

Local initializers such as `let add = (_ + _);`, `let negate = !_;`,
`let same = _ == "ready";`, and `let apply = consume(_);` synthesize ordinary
direct local callables. Each owned occurrence supplies a distinct parameter in
source order. Concrete named/host/lexical formals constrain written arguments;
named/default calls retain their existing binding rules. Callers and pipeline
inputs never choose a declaration's signature. `len(_)`, `_`, and `_ == _`
remain ambiguous.

The private `semantic/placeholders.rs` boundary collects direct numeric/logical,
scalar-equality, range-operand and resolved-formal constraints. Straight-line
blocks support existing captures, ordinary/compound writes, expression statements,
and returns. Count unreachable occurrences, but use only reachable evidence;
potentially reachable short-circuit operands contribute. Resolve names and call
eligibility before reporting collected conflicts, then ambiguity. Fully concrete
parameters enter the existing closure checker before capture analysis. Raw-node
identity binds placeholder reads directly to fresh owner-local IDs, without
manufactured source names, AST cloning or backend placeholder nodes.

`StatementKind::Let.value_span` and `Expr::Placeholder { span }` are implemented
raw-AST migrations. Explicit and synthesized helpers share source-preorder
`CallableId`s. Existing typed closure/call shapes and the memory ABI are unchanged.
Placeholder-callable value uses receive `InvalidCallableValueUse`; unchanged
explicit-lambda programs retain their historical diagnostics.

This milestone deliberately rejects unresolved complex regions with
`CannotInferPlaceholderParameter`, rather than guessing a signature. Inferred-local
relationships, collection propagation, branch/match/value-loop/pipeline expressions
inside a placeholder body, and nested callable composition inside that body await
#46/#47. Ordinary code surrounding a placeholder declaration may use these features,
including iteration captures, explicit enclosing lambdas, and known lexical
pipeline stages. Immediate stages still await their separate milestone. The
remaining sections describe the final design, not all current capabilities.

#### #45 validation and remaining gate

The implementation passed:

- The ticket's focused frontend/lowering, explicitly executed closure/pipeline
  runtime, core adapter, and CLI adapter commands.
- `cargo check --offline --workspace --all-targets`.
- `cargo test --offline` and `cargo test --offline -- --include-ignored`
  (compiler/CLI default members, including native Node execution; not browser).
- `cargo clippy --offline --all-targets -- -D warnings`.
- `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown`.
- `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings`.
- Edited-file formatting and `git diff --check`.

The current-source browser rebuild was attempted with the documented
`wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline`.
Rust compilation succeeded, but packaging stopped because local `wasm-bindgen`
tooling was unavailable (the lockfile requires version `0.2.129`). No tooling was
installed. Browser parity cases were extended and typechecked, but neither those
ignored tests nor `node tests/browser.cjs pkg` ran against stale bindings.
Rebuild and run them after supplying compatible tooling; full workspace
include-ignored/browser validation and WP12 release readiness remain pending.

Self-review against #45 and the repository standards corrected unreachable
equality evidence and conflict ordering when fallback source spans overlap.
The added regressions pass. This was a self-review, not independent agent review.

WP12 covers placeholder local declarations, known lexical callable pipeline
stages, and immediate parenthesized typed-lambda/placeholder stages. It does not
add first-class callable values, aliases, callable arguments/results, general
immediate invocation, partial application, collection intrinsics, pipeline fusion,
reclamation, or general WP13 optimization.

## Regions and binding (Q1–Q2, Q6–Q7, Q9–Q12, Q15, Q18)

- Only a local callable initializer or immediate pipeline stage owns a placeholder
  region. Each owned expression-position `_` denotes a distinct anonymous
  parameter in lexical source order, including unreachable/skipped occurrences.
- Ordinary grouping parentheses do not create nested regions. Match-pattern `_`
  remains a wildcard. `_temp` remains an ordinary identifier.
- Nested explicit lambdas are boundaries: an otherwise unowned `_` in their body
  is invalid, not an extra parameter of either lambda. Nested placeholder local
  declarations and immediate placeholder stages own their own occurrences.
- An initializer with no owned placeholder remains an ordinary value initializer,
  unless independently recognized as an explicit lambda. Nested regions do not
  trigger abstraction of their enclosing initializer.
- Placeholders are reads, not assignment targets, declaration names, or explicit
  parameter names. Writable ordinary locals may be initialized from them.
- Ordinary expression bodies, including blocks, if/match and value loops, are
  supported. Return and loop contexts belong to the synthesized callable:
  `return` returns from it, and transfers cannot target creator loops.
- Callable declarations execute none of their bodies. Shared captures retain
  original binding identity and mutability. Anonymous symbols are hygienic.
- Placeholder abstraction precedes final capture analysis and uses WP11's same
  callable ownership, metadata, cells, environments and direct helpers.

Examples:

```text
let combine = (_ + 1) * (_ - 1); // one binary number callable
let choose = if _ { _ + 1 } else { _ + 2 }; // bool, number, number
let outer = {
    let inner = _ + 1;
    inner(_)
}; // separate unary inner and outer callables
let result = {
    let inner = _ + 1;
    inner(10)
}; // number value, not an outer callable
```

Outside an owning region, `return (_ + 1)`, `consume(_ + 1)`, assignment of
`(_ + 1)`, and `[(_ + 1)]` are forbidden callable-value/placeholder uses.
Inside `let apply = consume(_ + 1);`, however, the region belongs to `apply`:
if `consume` accepts a number, its argument is a number, not a callable.
There are no special permissions for names such as `map` or `filter`.

## Public AST migration and callable ownership

The raw AST keeps source syntax and provenance; it does not introduce a callable
type or a second intermediate representation:

- Add `Expr::Placeholder { span: Span }` for an expression-position `_`. Its span
  is exactly the original UTF-8 byte range of that token. Match-pattern `_`
  remains `MatchPatternKind::Wildcard`, not an expression placeholder.
- Add `value_span: Span` to raw `StatementKind::Let`, covering exactly the
  initializer expression, including its written grouping parentheses when
  present. This retains the initializer's original source range independently
  of the wider declaration `Statement.span`.
- Replace `PipelineStage.name: String` with
  `PipelineStage.target: PipelineStageTarget`, defined exactly as:

  ```rust
  pub enum PipelineStageTarget {
      Named(String),
      Immediate { expression: Expr, expression_span: Span },
  }
  ```

  Keep `args: Vec<CallArgument>` and the full stage `span`. Existing named
  stages retain their target and argument representation. Immediate targets
  retain the original expression and its exact UTF-8 byte span (excluding the
  stage's surrounding grouping parentheses); they require an empty `args`
  vector. Only a parenthesized explicit lambda or placeholder-bearing
  expression is an eligible immediate target, not an arbitrary parenthesized
  value.
- A local initializer containing an owned placeholder remains a raw expression
  with placeholder nodes until region constraints have selected concrete
  parameter types. Semantic analysis then synthesizes the callable directly into
  the existing typed closure path before final capture analysis. An immediate
  pipeline target follows the same path at its stage's source location. Do not
  expose unresolved parameters in the public AST or add a second public
  intermediate representation.
- Every synthesized callable uses the existing `TypedClosure` declaration and
  `TypedExprKind::LexicalCall`/`TypedNormalizedCall` direct-call forms. The local
  declaration owns its helper body. For immediate stages, extend the
  existing `TypedNormalizedCall` with `immediate_closure: Option<TypedClosure>`;
  it is `Some` only for a synthesized immediate stage, and the normalized
  call's existing direct `call` invokes that helper. The `TypedClosure` is
  owned by that normalized stage and its `owner` is the lexical owner containing
  the pipeline expression. Check/cache its body through the existing closure
  lifecycle. Evaluate and validate the injected input before creating that
  stage's environment; then create the environment and make the direct helper
  call. This attaches helper ownership to the existing normalized call, without
  adding a pipeline node, another IR, or a runtime callable value.
- Program-wide `CallableId`s follow the existing source-depth-first preorder,
  including synthesized helpers, with deterministic traversal of pipeline input
  before stages and stage contents in source order. Synthesized parameter and
  binding identities are fresh, hygienic IDs in their existing owner-local
  namespaces; they must not collide with source declarations or depend on
  spellings such as `_0`. The required `TypedClosure.name` for an immediate
  stage is the non-source label `"<pipeline stage>"`. This fills the existing
  string field without creating a source identifier: angle brackets are not
  valid in source names, and this label is never used for lexical lookup or
  diagnostics. Diagnostics use the original source spans. Its `name_span` is
  the original full stage span.
- Preserve original source locations through abstraction: placeholder parameter
  identity retains its `_` token span; for local placeholder initializers the
  synthesized closure `span` and `body_span` are the new exact initializer
  `value_span`; for immediate stages they are the target's exact
  `expression_span`. Each stage retains its original full stage span. Explicit
  lambdas continue to use their existing lambda/body spans. Do not manufacture
  source locations from normalized typed calls.
- Lower pipeline injection by reusing WP10's existing checked normalized-call
  representation. The preceding pipeline input is the unique first
  `PipelineInput` binding mapped to formal 0; written bindings follow in source
  order. Local callable stages remain positional-only. Named user-function
  stages retain WP10 named/default behavior. Immediate stages invoke the
  synthesized/direct callable with the input as its sole parameter. No alternate
  pipeline IR, indirect call, or runtime callable representation is added.

These are additive raw-AST migration requirements. Existing constructors and
exhaustive consumers must be updated; typed callable ownership and backend
validation continue to use the WP11 shapes described in the
[local callable interface](local-callable-interface.md).

## Parameter inference (Q3, Q5–Q6, Q9, Q11, Q13–Q14, Q17, Q22–Q25)

Infer each parameter from exact constraints within its region, never from later
invocations, the incoming pipeline input, or a later pipeline stage. Every
successful parameter must resolve to exactly one existing concrete language type.
No coercions, numeric promotions, callable types, or public unknown/type-variable
categories are introduced.

Constraints come from existing operations, resolved formal parameter types,
annotations, ordinary binding uses/assignments, and structural branch joins.
They can propagate through inferred locals irrespective of textual order,
without relaxing scope, definite initialization, or fixed binding types.
Do not infer across callable boundaries or alter named-function signatures,
result scheduling, or existing local-callable result inference.

```text
let negate = !_;            // bool
let add = _ + _;            // number, number
let same = _ == "ready";    // string
let equal = _ == _;         // ambiguous, despite an equality relationship
let length = len(_);        // ambiguous among supported arrays
let increment = {
    let x = _;
    x += 1;
    x
}; // number
let first_plus_one = {
    let items = _;
    items[0] + 1
}; // number[]
```

No postfix indexing expansion is included: source indexing still starts from an
identifier, so `_[0]` is not new supported syntax. Array-literal fallback does not
select the type of an unknown placeholder element.

Count parameters structurally but gather inference evidence only at reachable
constraint sites under existing conservative structural completion rules. Do not
perform constant folding or value-based dead-code analysis. Potentially reachable
short-circuit operands participate. A later divergent operation does not erase
earlier reachable constraints.

An occurrence after an unconditional return still adds a parameter, but a numeric
operation solely at that unreachable occurrence cannot infer its type. After
parameter types are selected, all unreachable/skipped children are checked for
conflicts and ordinary semantic errors. Parameter inference success does not
override existing rejection of an uninferable Never-only callable result.

Establish ownership, lexical binding, resolved callees, and structurally known
reachability before solving constraints. Reachability must not depend on
speculative placeholder types or unresolved callable results. Do not add iterative
whole-program inference; constructs unresolved within these boundaries receive a
focused diagnostic.

Inference variables and relationships remain private to semantic analysis.
Fully concrete parameters enter the existing typed-callable boundary before
checked lowering; the backend does not solve types.

## Pipeline stages and effects (Q4, Q7–Q8, Q16)

Known local callable stages follow ordinary lexical resolution and positional-only
local-call rules. They gain no named/default parameters, contracts, or result
annotations. Named user-function stages retain WP10 named/default support.
Injection occupies formal 0; written arguments retain their existing order.

Immediate lambda/placeholder stages must be parenthesized, have exactly one
parameter, and have no written argument suffix:

```text
10 |> ((x: number) => x * 2); // valid
10 |> (_ * 2);               // valid
10 |> (_ + _);               // arity error
10 |> (() => 42);            // arity error
10 |> (42);                  // invalid stage, not an implicit callable
let add = _ + _;
let result = 10 |> add(5);   // valid multi-parameter lexical stage
```

General postfix invocation remains forbidden. Final declared-Void stages require
WP10 discard permission; nonfinal Void is rejected even after Never. Preserve
caller control contexts for pipeline input and written arguments, independent
callable control contexts for bodies, once-only evaluation, callee parameter
validation, and existing user-function defaults/contracts.

Evaluate the incoming input first. If it completes, create the immediate stage's
environment, then invoke its direct helper. If input traps/diverges, that stage
creates no environment and executes no body, but remains statically checked.

This ordering does not defer all capture-cell allocations: original bindings
still use WP11 eager promotion, which can occur before the stage is reached.
Environments and cells retain monotonic allocation, shared live captures,
recursion/reentry lifetime rules, and no rollback. No allocation-free sugar,
byte identity, identical allocation counts, or memory-exhaustion trap equivalence
between differently structured programs is promised.

## Diagnostics (Q19, Q24)

Use the following semantic variants and stable messages for the new diagnostic
categories. `Span` values are original UTF-8 byte offsets. The names/messages
below are the frozen public Rust/API contract for implementation; they do not
authorize implementation in this documentation ticket.

| Variant | Display message | Location |
| --- | --- | --- |
| `InvalidPlaceholder { span }` | `Placeholder is outside a placeholder region at {span:?}` | The offending `_` token. |
| `CannotInferPlaceholderParameter { span }` | `Cannot infer a concrete type for placeholder parameter at {span:?}` | The first ambiguous placeholder in parameter order. |
| `ConflictingPlaceholderConstraints { expected, found, span }` | `Conflicting placeholder constraints: expected {expected:?}, found {found:?} at {span:?}` | The earliest incompatible reachable use, using that use's source span where available. |
| `InvalidPipeline { message, span }` | Existing display form: `Invalid pipeline: {message} at {span:?}`. Use `pipeline stage target is not callable` for an ineligible target and `immediate pipeline stage must have exactly one parameter` for zero- or multi-parameter immediate callables. | The full stage span. |
| `InvalidCallableValueUse { span }` | `Callable values are not supported at {span:?}` | The forbidden value-use site. |

`InvalidPipeline` already exists and keeps its current variant/display format;
WP12 fixes the two additional `message` values above rather than adding a
parallel pipeline error type. A named target that does not resolve continues to
use the existing `UndefinedFunction`; a lexical variable used as an ordinary
call continues to use `NotCallable`. Existing `InvalidClosure` remains for
existing explicit-closure diagnostics. The new callable-value diagnostic covers
attempts to pass, return, assign, store, export or otherwise use a callable as a
value; it does not turn a valid `consume(_ + 1)` local initializer into a
callable argument.

Constraint records retain the originating use span while solving. Where the
current raw node has no narrow expression span, use the narrowest existing
source-bearing child span; only fall back to its enclosing declaration/statement
span when no more precise original location exists. Never derive offsets from a
rewritten or typed tree. The shared diagnostic renderer continues to render the
same semantic span in native and browser adapters.

Within a new placeholder region, use this deterministic boundary:

1. Establish syntax, ownership, lexical names and callable eligibility using
   existing source-order checking.
2. Gather and solve reachable constraints.
3. Report the earliest conflicting constraint in source order; otherwise report
   the first ambiguous `_` in parameter order.
4. With parameter types fixed, run ordinary body/result checking, including
   unreachable children.

A reachable conflict takes priority over an earlier unconstrained parameter.
Anchor ambiguity at its original `_`, conflict at the incompatible use where
possible, and stage arity at the stage span. Preserve UTF-8 byte spans and shared
native/browser rendering. Unchanged source retains existing error variants,
messages, spans, and first-error ordering. No general recovery is introduced.

The ordering is part of the contract: for example, an unconstrained first `_`
does not mask a later reachable type conflict; report that conflict first.
After inference succeeds, ordinary body/result diagnostics retain their existing
ordering and still inspect unreachable/skipped children. No recovery is added.

Representative acceptance/rejection contract:

| Source shape | Result |
| --- | --- |
| `let add = _ + _;` | Accept; two distinct `number` parameters. |
| `let outer = { let inner = _ + 1; inner(_) };` | Accept; nested regions have independent parameters. |
| `10 |> (_ * 2)` | Accept; one-parameter immediate stage, reusing input injection. |
| `10 |> (_ + _)` or `10 |> (() => 42)` | `InvalidPipeline` at the stage: immediate arity message above. |
| `10 |> (42)` | `InvalidPipeline` at the stage: target message above. |
| `let same = _ == _;` | `CannotInferPlaceholderParameter` at the first `_`. |
| `let bad = _ + true;` | `ConflictingPlaceholderConstraints` at the incompatible use. |
| `consume(_ + 1);` as an ordinary expression statement | `InvalidPlaceholder` at the first unowned `_`. |
| `consume((x: number) => x);` | `InvalidCallableValueUse` at the callable value-use site, not an implicit callable argument. |
| `let f = (x: number) => x; let y = 1 |> f;` | Accept; the local callable is invoked directly with the injected input. |
| `let y = 1 |> named_with_default;` | Preserve WP10 named-function stage/default behavior and diagnostics. |

The cases above supplement, rather than replace, the existing WP10/WP11
compatibility suite. Unchanged source retains its existing error variant,
message, span, and first-error ordering.

## Ordered delivery and release evidence (Q20–Q21)

Following separate implementation authorization, delivery remains ordered:

1. Known lexical callable pipeline stages (#44 implemented; release validation pending).
2. Placeholder inference and local declarations.
3. Immediate stages.
4. Integrated release validation.

Each milestone preserves the final agreed semantics. Extend existing closure and
pipeline suites at their source, forged-input, decoded-lowering, runtime and
adapter seams. Include:

- Region nesting, grouping, shadowing, hygiene, wildcard separation and forbidden
  callable-value uses; unary/binary arity and no-owned-placeholder initializers.
- Exact constraints through locals, annotations, calls, arrays and joins;
  ambiguity/conflicts; reachable versus unreachable evidence and checking.
- Independent return/loop contexts; skipped/unreachable declarations and stages.
- Forged synthesized metadata, independent direct-call/helper decoding and
  `wasmparser::Validator`, not validity alone.
- Bounded execution for input/argument/body effects, trap order, live capture
  mutation, nested callable dependencies, iteration captures and allocation order.
- Existing named/default/contract interactions, deterministic output, optimization
  semantic/diagnostic parity, enabled-preset byte identity, and adapter parity
  within each level.
- CLI and current-source rebuilt-browser tests plus explicitly executed ignored
  runtime/browser suites under the documented offline validation workflow.

Default Cargo success is not release evidence for ignored runtime/browser cases.
No full WP12 release success is claimed by this document. The completed design has
final shared-understanding confirmation. Issue #44 implementation is authorized and
committed; remaining WP12 implementation and landing each require their own explicit
authorization.
