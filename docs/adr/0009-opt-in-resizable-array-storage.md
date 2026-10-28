# Make resizable array storage explicit without changing descriptor layout

Confirmed during the M4 design interview on 2026-10-28. These are design decisions,
not implementation authorization or release evidence; the
[M4 roadmap contract](../was_compiler_memory_and_optimization_roadmap.md#8-milestone-m4--resizable-typed-arrays)
defines the operation and acceptance boundaries.

Preserve the public eight-byte pointer/length descriptor and existing array types.
Create movable storage only through an explicit fresh-copy constructor,
`resizable(source)`, with private capacity and resize-authority metadata keyed by
stable handles. Ordinary literals and existing host encoding remain fixed-storage.
A valid descriptor does not establish resize authority. This avoids a sixteen-byte
ABI migration and retroactive weakening of existing host storage-lifetime rights,
at the cost of private authority lookup and runtime rejection of fixed-storage
receivers. Hosts may retain opt-in array handles, but must reacquire payload pointers
and lengths after possible resize, separately from refreshing views after memory growth.

For opt-in arrays, indexed writes capture and validate the handle/index before the
RHS, then revalidate and resolve current storage before the final store. Numeric
compounds also retain the old numeric value. Shrinking may cause a post-RHS trap;
shifting removal makes the final write target the current element at the captured
index. Existing fixed-storage writes retain their saved-address behavior. Runtime
pins were rejected: a trap can abandon a pin without guest cleanup, while host
reentry can suspend a still-live outer pin in otherwise indistinguishable persistent
state. Neither clearing pins on entry nor retaining them indefinitely meets the
selected raw-Wasm reentry and recovery contract. This explicitly revises M0's
proposed future place guarantee only for opt-in resizable storage.

Use the necessary M3 allocation core internally to recycle displaced owned payloads
only after successful replacement publication, without exposing private management
exports or test instrumentation. Stable descriptors, authority records and
unreachable arrays remain retained; referenced immutable strings are not released.
This bounds the reclamation claim to superseded payload storage, rather than
introducing garbage collection, host ownership transfer, M2 migration or trap rollback.
