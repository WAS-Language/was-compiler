# Fold scalars during checked lowering

M8 folds eligible scalar expressions during checked lowering, after the existing
checks have visited the original subtree in diagnostic order. Rewriting the
public typed tree before lowering can erase errors in forged nodes, including
unreachable or runtime-skipped children; validating or compiling the original
program in a separate pass would duplicate work.

Private folding facts guide emission only. Keep layout and representation
planning on the original tree, even where feeding folded values back could expose
more optimization opportunities. See the [M8 roadmap](../was_compiler_memory_and_optimization_roadmap.md#m8-interview-decisions-round-1-confirmed)
for scope and acceptance gates. The user confirmed the design and closed the
interview. Design confirmation did not authorize implementation or establish release acceptance.

## Implemented slice: #92

WP12's correctness exit was verified against [#49](../wp12-release-validation-2026-10-25.md)
and the user separately authorized #92. Enabled presets fold closed numeric
negation, addition, subtraction, and multiplication when all intermediate
binary64 results are finite. Comparisons, equality, and boolean negation are
outside this slice. Excluded parents and overflow paths retain their operators;
eligible children can still fold.

Checked emission carries finite values from literals and supported operators in
postorder. After the original result check succeeds, replace that instruction
suffix with a constant in the selected representation, before any required
widening. Only the private byte buffer changes, not the original typed tree,
local slots, helper discovery, or allocation plans. A private policy isolates
M8 in tests with M6 and M7 held fixed; `None` and direct codegen disable folding.

## Implemented slice: #93

The same pass now folds numeric ordering, same-type numeric or boolean equality
and inequality, and boolean negation. Finite numeric facts use binary64 semantics
(including equal signed zeros); boolean facts emit canonical `i32` values.
Nonfinite intermediates block ancestor folds, not independent eligible children.
Binding reads, short circuit, division, remainder, calls, strings, collections,
ranges, and control flow do not supply folding facts. Apply the same whitelist
in contracts, normalized defaults, and local callables without rewriting their
surrounding guards, evaluation order, or planning.

No broader WP13 authorization or M8 release decision follows.
