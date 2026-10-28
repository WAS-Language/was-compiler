# M3 invalid-release evidence (#60)

This test-only slice extends [the private allocator contract](m3-private-allocator.md)
with adversarial release evidence. It changes neither the allocator implementation
nor ordinary compilation, public eight-byte descriptors, M2 or adapter behavior.
The owned test seam is
`crates/compiler/src/codegen/allocator/release_tests.rs`.

## Evidence boundaries

- Three physical registry records exercise exact lookup at tail, middle and head,
  with both active and released states. Each position rejects zero, incorrect
  signed/high-bit and another live allocation's identity, double release and
  obsolete authority after reacquisition of the same address. Successful recovery
  renews identities without adding physical records.
- Unknown addresses exercise null/low boundaries, signed-i32/high-bit boundaries,
  the current memory end, record addresses and aligned/unaligned payload interiors.
  Unknown lookup must terminate and return zero for every block observation.
- Valid static string descriptors, ordinary guest-created arrays and host-created
  arrays with separate descriptor/payload allocations remain usable after each
  rejected release. Structural validity supplies no management rights.
- A plausible complete record immediately before an unregistered host address and
  another forged record wholly inside initialized live payload must not grant
  release authority. Only usable payload bytes are modified; real allocator
  metadata is never corrupted to construct these inputs.
- Every rejected release is caught as `WebAssembly.RuntimeError` in the same
  instance. Before any recovery mutation, nonmutating observations compare the
  cursor, pages, population, last identity, all 29 pool heads, every registered
  block's state/identity/capacity/request and every management-record byte.
- After **each** trap, independent expected live contents are checked and all known
  free blocks are acquired simultaneously, without assuming a reuse order.
  Acquisitions must come from the expected free set, renew identities exactly,
  preserve cursor/pages/population, and yield disjoint active rounded extents.
  Pool traversal independently rejects cycles, duplicates and unexpected entries.
  Acquired bytes are initialized before reading; released payloads are not read.
- Every fixture passes `wasmparser::Validator`. A separate decoded test checks
  conditional provenance and identity/active-state traps before the first release
  write, registry-only exact matching/null termination, authoritative class loads,
  and read-only lookup/head helpers before the final pool-head mutation.

There are three new tests: one decoded contract and two bounded Node workloads.
Node tests remain ignored by default; the focused command below must select three
tests, with none ignored. Execution uses Node 24+ with BigInt identities and the
shared bounded subprocess helper. Existing core behavior already satisfies this
contract; no allocator fix is needed. The first decoded run exposed a test-fixture
assumption (an empty import section is emitted), corrected to check its zero count.

## Reproduction and validation

Run from the repository root, with cached dependencies and Node 24+:

```sh
cargo check --offline --all-targets
cargo test --offline -p was-compiler --lib codegen::allocator::release_tests -- --include-ignored
cargo test --offline -p was-compiler --lib codegen::allocator -- --include-ignored
cargo test --offline -p was-compiler --lib codegen::region -- --include-ignored
```

On 2026-10-28, the check passed; the focused release command passed **3 tests**,
the allocator command passed **23 tests**, and the M2 region command passed
**23 tests**, with none failed or ignored. The allocator total includes concurrent
#59 size-class/growth tests outside this slice's owned files; it is not an isolated
#60-only count.

After the review cleanup, the focused three tests and all-target check passed
again. Broader validation also passed:

```sh
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown
wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline
node tests/browser.cjs pkg
cargo test --offline --workspace -- --include-ignored
cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings
git --no-pager diff --check
```

The browser package was freshly rebuilt before the browser smoke and full ignored
workspace run. The full suite ran on the combined working tree, including the
concurrent #59 test additions; those additions are not owned by #60.

Independent Standards and Spec reviews used starting commit
`bd989922413502eb4aaebe43ddce576b7c031e54`, scoped to this document, the new
release-test module and its module declaration. Standards found zero hard breaches
and one low-priority duplicated bookkeeping loop; a local helper now removes that
duplication, and the reviewer confirmed it resolved. Spec found zero missing,
incorrect or out-of-scope requirements. No outstanding findings remain on either
axis. The concurrent #59 changes were excluded from these reviews.

## Limits

Private release authorization does **not** make stale payload reads safe and does
not protect against a malicious host corrupting exported memory. No host-root
protocol, language `free`, GC/RC, production activation or ordinary-array
reclamation is added. These tests establish invalid-release preservation in the
bounded workloads, not constant-time search, general fragmentation resistance,
unlimited-trap memory bounds or the remaining M3 integrated acceptance gates.
