# Language examples

Run commands from the repository root. Each new example below has a no-argument
`main` returning a number, so it works with the Node runner without marshaling:

```sh
cargo run --offline -- examples/placeholders.was -o target/placeholders.wasm --export main
node runtime/run.cjs target/placeholders.wasm
```

This prints `25`. Substitute another filename from the table to try its features.
Use `--export main` especially for range examples: range parameters/results are
internal-only and cannot be exported to the host. Node.js 24+ and cached Cargo
dependencies are required; no npm install is needed.

## Feature tour

| Example | Main features | Expected result |
| --- | --- | --- |
| [operators.was](operators.was) | Numeric compounds, fractional arithmetic, remainder, comparisons, boolean short-circuiting, strings, Unicode, comments, NaN/infinity/signed zero | `9` |
| [typed-arrays.was](typed-arrays.was) | `number[]`, `bool[]`, `string[]`, contextual empty arrays, indexing, mutation, aliases, content equality, `len`, array iteration | `12` |
| [control-flow.was](control-flow.was) | `while`, descending `for`, statement/value `loop`, `break`, `continue`, block/if values, shadowing | `25` |
| [range-membership.was](range-membership.was) | Inclusive/exclusive stepped ranges, `in`, internal range parameters/results | `9` |
| [shared-captures.was](shared-captures.was) | Typed local lambdas, shared mutable captures, earlier callable dependencies, read-only iteration captures | `20` |
| [placeholders.was](placeholders.was) | Anonymous `_` parameters, local/collection constraints, local pipeline stages, immediate placeholder and typed-lambda stages | `25` |
| [pipeline-effects.was](pipeline-effects.was) | Defaulted stages, named arguments, captured local stages, a final discarded `void` stage, bare return | `15` |

Placeholder types are determined within the callable's own region, not from its
call sites. Each `_` is a distinct input. Local callables are directly invoked;
they are not first-class values that can be passed or returned. Arrays have fixed
length and shared mutable storage; strings are immutable UTF-8 values.

## Existing examples

| Example | Focus | Running notes |
| --- | --- | --- |
| [array-sum.was](array-sum.was) | Typed locals, array mutation, forward function calls, `while` | Returns `65` |
| [insertion-sort.was](insertion-sort.was) | Nested loops, short-circuit bounds guard, array parameters/results and shared mutation | Returns `1` |
| [fib.was](fib.was) | Recursive functions with explicit result annotations | `main` takes a number: `node runtime/run.cjs target/fib.wasm --args '[10]'` returns `55` |
| [named-args.was](named-args.was) | Reordered named arguments | Returns `20` |
| [inferred-defaults.was](inferred-defaults.was) | Forward result inference, defaults reading earlier formals, named/default calls | Returns `584` |
| [blocks.was](blocks.was) | Block and if expressions, value loops | Export only `main`; returns `10` |
| [callbacks.was](callbacks.was) | A directly invoked local callable capturing an outer binding | Returns `20`; not a host callback registration |
| [ranges.was](ranges.was) | Inclusive range iteration into an array | Returns `11` |
| [match.was](match.was) | Ordered literal, comparison, interval and wildcard match arms | Returns the string `"huge number"`; the raw runner prints its descriptor handle, so decoding requires explicit host marshaling |
| [pipelines.was](pipelines.was) | Named pipeline arguments, range iteration, preconditions | Returns `120` |
| [preconditions.was](preconditions.was) | A failing `requires` clause | Intentionally traps because `main` calls `div(10, 0)` |
| [sum.was](sum.was) | Existing invalid-source example | Does not compile: `d` is undefined |
| [host-calls.was](host-calls.was), [host-provider.cjs](host-provider.cjs) | Explicit host imports and string/array marshaling | Requires host registration and a provider; see [runtime integration](../docs/runtime-integration.md) |
| [browser/](browser/) | Browser compiler playground | See the [browser guide](../docs/browser.md) for rebuilding and serving it |

The language also supports match statements and boolean/string match domains,
mutual recursion, and nested captures. See the [language support summary](../README.md#current-compilation-support)
and [limitations](../docs/limitations.md) for the full supported scope. Proposed
features such as resizable arrays, first-class functions and garbage collection
are not demonstrated as implemented features.
