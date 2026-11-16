# Preserve immutable strings and construct text from fragments

Confirmed during the M5 design interview on 2026-10-30. This records the accepted
design, not implemented support, implementation authorization or release evidence.
The [M5 roadmap contract](../was_compiler_memory_and_optimization_roadmap.md#9-milestone-m5--string-operations-and-efficient-construction)
defines operation semantics, initial storage policy and acceptance criteria.

Keep strings immutable and preserve the revision-1 descriptor and host retention
contract. Expose transformations through named builtins with explicit byte lengths
and byte-offset slicing; reject slice endpoints inside Unicode scalars. This avoids
mutable aliases of interned text and ambiguity between bytes, scalars and graphemes,
at the cost of requiring callers to supply valid byte boundaries. Scalar/grapheme
indexing and normalization are deferred. Structural descriptor checks do not become
full UTF-8 scans: valid immutable UTF-8 remains a trusted-host obligation.

Use existing string arrays to accumulate fragments and one final `string_join` to
materialize text, instead of introducing a mutable builder type or automatic
concatenation fusion. This reuses existing array typing and resize authority without
creating another ownership or ABI protocol. Fragments and published results retain
their existing lifetimes; efficient construction is not general reclamation or a
constant-retained-memory guarantee. Fusion remains deferred to M6.

Distinguish the initial copy/reuse policy from lasting representation-independent
acceptance criteria. Broader reuse must preserve observable semantics and lifetime
safety, including host-visible handles, allocation failure, memory growth and trap
order. Validate construction with categorized deterministic test-only allocation and
copying counters, plus separate ordinary-instance timing. Require O(n + B) aggregate
storage and copying for fragment accumulation plus join, not mandatory physical
allocation counts, inefficient optimized concatenation or a guaranteed speedup.
