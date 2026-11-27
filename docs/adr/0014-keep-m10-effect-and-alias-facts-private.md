# Keep M10 effect and alias facts out of lowering

M10 will prove and test a wider local-alias case in a private analysis of
source-analyzed typed programs, rather than widening M1's facts for M2 or having
checked lowering consume the new facts. This separates evidence that a proof is
sound from permission to change storage or emitted code; a temporary candidate
can still trap and confers no reclamation rights.

Keep array object identity separate from payload storage overlap: revision-1 raw
descriptors can name overlapping bytes, and a resizable array can move its payload
without becoming a different object. Treat unknown overlap conservatively rather
than using distinct handles as evidence of disjointness. An unknown host callback
can observe guest memory or reenter without receiving a newly published handle.

The [M10 roadmap](../was_compiler_memory_and_optimization_roadmap.md#14-milestone-m10--advanced-effect-and-alias-analysis)
records the confirmed proof subset and exit gates. This decision closes the design
interview; it does not authorize implementation or activation.
