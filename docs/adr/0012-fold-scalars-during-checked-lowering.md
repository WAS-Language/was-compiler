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
interview. This does not authorize implementation or establish release acceptance.
