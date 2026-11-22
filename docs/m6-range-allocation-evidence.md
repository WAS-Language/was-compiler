# M6 direct-range allocation evidence (#84)

A normally completed eligible direct range construction allocates **one 32-byte
record under `None` and no record under `Basic`, `Standard`, or `Aggressive`**.
This also holds when the range yields nothing. A range stored in a binding or
returned by an internal call still allocates its record under every preset.
This is allocation evidence for [ADR 0011](adr/0011-bounded-m6-range-scalar-replacement.md),
not integrated M6 acceptance.

## What was observed

[`range_allocation_tests.rs`](../crates/compiler/src/codegen/range_allocation_tests.rs)
validates both ordinary and observed Wasm. It independently identifies the range
constructor from its decoded integer stores, checks its one call to `__alloc`
with literal argument `32`, and checks which user and local callable bodies call
the constructor. The constructor itself remains in enabled modules.

The private `cfg(test)` observer rewrites a **copy** of the ordinary fixture module.
It adds an `i64` counter increment immediately after the constructor's allocator
call succeeds and exports that counter and the existing bump cursor. It does not
instrument the ordinary allocator, add imports, or change compilation options.
Other function bodies are checked byte-for-byte, and recompiling through the public
API returns the original ordinary bytes. Production compilation and adapters
cannot select this private rewrite; neither observation export is added there.

The [WAS fixture](../tests/support/m6_range_allocations.was) uses one registered
`env.checkpoint(number) -> void` import. The [Node harness](../tests/support/m6_range_allocations.cjs)
reads but never writes the observation globals; the host does not call the allocator.
At each checkpoint and at return or caught trap, it subtracts `32 × record delta`
from the unsigned cursor delta to isolate other bump bytes. Literal phase expectations
attribute the remainder, rather than assuming any 32-byte cursor change is a range
record. Ordinary and observed instances are both checked against independent guest
results and checkpoint order. Each case runs twice in the same instance per preset
and variant, including successful calls after traps.

| Case, per invocation | Records: `None` / enabled | Other bump bytes in both |
| --- | ---: | --- |
| Direct, exclusive, default step; three yields or empty | 1 / 0 | 32 across two operand arrays |
| Local callable, inclusive, explicit step; one/three yields or empty | 1 / 0 | 48 across three operand arrays, plus 8 per promoted iteration cell, 8 per closure environment and 16 per body array, measured at separate checkpoints |
| Indirect through a binding or range-returning call; nonempty and empty | 1 / 1 | 0 |
| Return from start, end, or step operand | 0 / 0 | 16 / 32 / 48; later operands and the body do not run |
| Completed invalid operands (fractional, NaN, infinite, signed-zero step or out-of-domain step) | 0 / 0 | 32 or 48; all operands run before the trap and the body does not run |

These are successful bump reservations in a fixed, small, monotonic fixture, not
physical memory or RSS, page growth, or an allocation-exhaustion experiment. The
measurements do not imply whole-loop allocation freedom, general reclamation, or
any timing benefit. Instrumented module sizes below are provenance, not a size or
performance comparison.

## Reproduce

Run from the repository root with cached dependencies and Node on `PATH`. Run
sequentially; the test writes its eight paired artifacts to the same `target/`
directory each time. The test harness runs Node through a bounded subprocess;
the separate Node invocation below records the same deterministic JSONL without
Cargo test-output prefixes.

```sh
mkdir -p target/m6-range-evidence/run1 target/m6-range-evidence/run2
for run in run1 run2; do
    cargo test --offline -p was-compiler --lib codegen::range_allocation_tests -- --include-ignored --test-threads=1 &&
    node tests/support/m6_range_allocations.cjs target/m6-range-allocations > "target/m6-range-evidence/$run/trace.jsonl" &&
    sha256sum target/m6-range-allocations/*.wasm > "target/m6-range-evidence/$run/modules.sha256" || exit 1
done
cmp target/m6-range-evidence/run1/trace.jsonl target/m6-range-evidence/run2/trace.jsonl
cmp target/m6-range-evidence/run1/modules.sha256 target/m6-range-evidence/run2/modules.sha256
wc -l target/m6-range-evidence/run1/trace.jsonl
sha256sum target/m6-range-evidence/run1/trace.jsonl
```

### Recorded run — 2026-11-22

Linux `7.2.15-1-omarchy`, x86-64 (AMD Ryzen 7 5700X); Rust `1.100.0`
(`0ef1bc386`, `x86_64-unknown-linux-gnu`), Cargo `1.100.0` (`3acc79c0c`),
Node `v26.13.1` (V8 `14.6.202.34-node.40`). Compiler sources were at
`c04b994a8f3ddb638ae3337f32149761d0e57671` plus this issue's test-only changes;
the pre-existing uncommitted `README.md` change was not an input. SHA-256:

| Source/input | SHA-256 |
| --- | --- |
| `Cargo.lock` | `af7880ab794e4a61cf78a04cc10b1478084132648d39a4cfb8495d0ec0294145` |
| `crates/compiler/src/codegen/mod.rs` | `9ff1569345246ce965de2e6697ed516297234fd03a19d8f2f54b80f6e6f4e067` |
| `crates/compiler/src/codegen/range_allocation_tests.rs` | `3aa4cdb569e36777c212f520bccee21ad263f43f818b78c6da48d7ada3d3dee1` |
| `tests/support/m6_range_allocations.was` | `11dedf519d097bee04fa78f0cac03d8cb13df4702e683ce56eab4a9f83eb28e2` |
| `tests/support/m6_range_allocations.cjs` | `46f9ac9d13d34502cf2b781f42e67c1d05ae20e2248e20f4386e389aa58da1ec` |

Both runs passed both tests and produced identical 136-line traces (SHA-256
`0a26623288efc0b86c356b4dcd05fc28441f5f9684fe0128a71e45f7f7ecb663`) and identical
module digests. Enabled presets also matched byte-for-byte within each variant.

| Module | Bytes | SHA-256 |
| --- | ---: | --- |
| `o0-ordinary.wasm` | 2227 | `f6aaf9bcefd22b418e03adbfe25dbb8c5954164e64d2f4d7cb8e7e23e6dd30bb` |
| `o0-observed.wasm` | 2268 | `152a1914d930dc3cc7e6a0617e426974f2abd8f5c38e0b3e3f38b7cfa25b11ff` |
| `o{1,2,3}-ordinary.wasm` | 2321 | `8d74ed612c068aba2402e26a64a825c68bf3532a500a2a251d76e5a9fbc7040a` |
| `o{1,2,3}-observed.wasm` | 2362 | `1b42a8ec030238d3ceab5948726221cd296bacc294710a7c2cf92c7332b39499` |
