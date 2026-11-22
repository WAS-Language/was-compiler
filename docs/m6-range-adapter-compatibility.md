# M6 range-loop adapter compatibility (#85)

This is adapter evidence for [direct range-loop scalar replacement](adr/0011-bounded-m6-range-scalar-replacement.md), not integrated M6 acceptance. No production code, ABI, flags, or APIs changed. M4 and M5 performance decisions remain pending.

## Coverage

| Contract | Evidence |
| --- | --- |
| Per-preset parity | [CLI](../crates/cli/tests/m6_range_adapters.rs) and [rebuilt browser](../crates/browser/tests/m6_range_adapters.rs) test Wasm and WAT against the core for `None`, `Basic`, `Standard`, and `Aggressive`. The shared matrix checks the unset default against explicit `None` and direct codegen, requires enabled-preset byte identity, and validates every core module. |
| Eligible and fallback shapes | The matrix covers grouped direct ranges, explicit steps and local callable loops, binding and call fallback, a program with only an indirect loop, and a range-free control. The last two have no integer rewrites and retain identical bytes at every level. The direct-only module stops calling the range helpers without removing or reordering their bodies; the local callable also stops calling both range helpers. Selected/no user exports preserve function bodies. |
| Diagnostics and host boundaries | Both adapters compare full rendered parse and semantic diagnostics (including UTF-8/CRLF input and an unreachable invalid step), the private range export error, and rejected range host parameters/results with the core and the adapter-specific guidance. |
| Bounded execution | The [shared Node oracle](../tests/support/m6_adapter_runtime.cjs) executes actual adapter output, not the core reference. It checks literal expected results and effect traces for empty and nonempty loops, safe-integer edges, once-only operand order, local captures, abrupt return, invalid-input traps and recovery. It also checks keyword module/field names, the obligation to supply an unused import, exact exported names/kinds, and raw `memory`/`__alloc` behavior, including with no user exports. |

## Validation on 2026-11-22

Executable sources were unchanged from `ea0a971`. Cargo 1.100.0, Node v26.13.1, and wasm-pack 0.15.1 were available.

- `cargo check --offline --workspace --test m6_range_adapters` and `cargo clippy --offline --workspace --test m6_range_adapters -- -D warnings` passed; both Node harnesses passed `node --check`.
- `cargo test --offline -p was-compiler-cli --test m6_range_adapters -- --include-ignored` passed all three tests, including bounded Node execution of the 20 CLI modules.
- `cargo test --offline --workspace` passed: 1,368 passed, 0 failed, 345 ignored across 95 targets. This does not count browser execution.
- The offline `wasm-pack` command below compiled the Rust Wasm target but could not find a local `wasm-bindgen` CLI matching the lockfile's `0.2.129`. The cached `0.2.130` binary is not a matching prerequisite. No tools were fetched, and no rebuilt-browser, browser smoke, or full ignored workspace result is claimed.

With Node 24+, cached Rust dependencies, `wasm32-unknown-unknown`, wasm-pack, and the matching `wasm-bindgen` CLI available offline, run from the repository root:

```sh
wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline &&
cargo test --offline -p was-compiler-browser --test m6_range_adapters -- --ignored
```

The browser integration and the CLI guest-execution test remain ignored by default. See [#84](m6-range-allocation-evidence.md) for the independent allocation evidence; this adapter suite does not measure allocation or timing.
