# Bound M6 to direct range-loop scalar replacement

The M6 design interview on 2026-11-16 selected direct range-loop scalar replacement
as the only required optimization for M6 completion, rather than requiring closure
specialization, string fusion, temporary-array reuse or compact layouts. This
bounds the proof surface without introducing source syntax, ownership annotations,
public descriptor changes, host release obligations or general heap reset.

An individually authorized optimization may change internal allocation counts,
subsequent allocation addresses, memory growth and allocation-exhaustion outcomes:
requiring identical resource behavior would defeat allocation elimination. Operand
effects, validation order, ordinary invalid-input traps, published-value lifetimes
and alias behavior remain protected. This permission does not authorize deferred
optimizations, particularly string fusion, without their own contract decisions.

This establishes the intended narrow exception to ADR 0001's deferral of range
scalar replacement to WP13; other range values retain their allocated representation.
Round 2 limits selection to `For` iterables rooted directly in a typed `Range`,
including grouping parentheses and effectful/divergent operands, also inside local
callables. Other range-valued shapes retain allocated lowering. Preserve complete
safe-integer range semantics, operand evaluation before ordered numeric validation,
static checks of every child, enclosing transfer targets and per-yield captures.

All enabled optimization presets select this optimization through a distinct
internal policy. `None` remains default and byte-compatible, as do direct codegen
entry points. Programs without eligible direct loops retain bytes; enabled presets
match each other and adapters match within each preset. Existing memory exports,
helper discovery and helper ordering remain even when unused: this deliberately
trades infrastructure/size opportunities for a bounded implementation and stable
module surface. No new public flag is introduced.

Round 3 makes deterministic range-record elimination the required allocation
benefit, not a promised runtime speedup or smaller module. Independent decoded
checks and private observations distinguish eliminated records from operand,
capture and body allocations. Correctness requires independent expected results,
differential execution, forged-input/diagnostic checks and negative controls,
including effects, transfers, captures, recursion, reentry and traps.

Measure ordinary non-instrumented runtime, compilation cost and module bytes with
repeated samples and controls. Material regressions or unexplained timing
variability require explicit disposition, not an invented threshold or favorable-run
selection. Final-source offline native/portable/browser/full ignored-workspace and
strict Clippy gates, adapter parity and a contract-to-test map are required.
Integrated acceptance also requires explicit disposition of M4/M5's pending
performance signals; it does not approve either milestone implicitly. `None`
remains default even after acceptance.

The user confirmed all three rounds and the final shared understanding on
2026-11-16, closing the design interview. Design confirmation does not itself
authorize implementation or establish release acceptance. Relevant range and
optimization contracts and ADR 0001's deferral must identify the narrow exception
when implementation is authorized, without rewriting historical release evidence.
