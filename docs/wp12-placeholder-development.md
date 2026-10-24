# WP12 design interview and implementation handoff

## Status and scope

The user accepted interview decisions Q1–Q25 and confirmed the complete shared
understanding in Q26. The design interview is complete; implementation remains
unauthorized. WP11's release satisfies the predecessor gate, not authorization.
No WP12 implementation or release validation is claimed.

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

Distinguish out-of-region placeholders, ambiguous parameters, conflicting
constraints, invalid stage targets/arity, and forbidden callable-value uses.
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

Concrete Rust variant names/messages and public AST migration must be specified
against the implementation before coding; this interview fixes diagnostic
categories, locations and ordering, not uninspected Rust API names.

## Ordered delivery and release evidence (Q20–Q21)

After separate implementation authorization:

1. Known lexical callable pipeline stages.
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
No release success is claimed by this document. The completed design has final
shared-understanding confirmation; implementation and landing each still need
their own explicit authorization.
