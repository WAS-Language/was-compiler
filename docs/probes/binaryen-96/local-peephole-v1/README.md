# Import-preserving Binaryen profile probe

**Result:** `local-peephole-v1` passed the unchanged #96 guard in both pinned
Binaryen v116 bindings at levels 1–3. It is a candidate for adapter adoption,
not stock O1–O3 or a release decision.

The sole pass is `optimize-instructions`, a function-local peephole optimizer.
Rather than trying to disable individual cleanup passes inside a default pipeline,
Rust disables the default passes and adds `Pass::OptimizeInstructions`; JS calls
`module.runPasses(['optimize-instructions'])` instead of `module.optimize()`.
Both still set optimize level to the selected level, shrink level to zero and
features to MVP, without fast math or debug info. The optimizer doesn't need to
remove, protect, reinsert or rename imports. The trade-off is giving up the rest
of the stock pipeline, including whole-module size reductions.

| Core-emitted input | Input → output bytes at each level | Interface checked |
| --- | --- | --- |
| #96 unused import | 61 → 61 | Uncalled `host.unused` and exported `main` |
| #96 memory only | 721 → 701 | No user exports; `memory` (1..65535 pages) then `__alloc` |
| Ordered imports | 73 → 73 | Unused `z.unused` before called `a.used`, distinct signatures; exported `main` |

All 18 outputs validated as MVP without a start section and preserved the ordered,
resolved external interface. All nine Rust/JS pairs matched byte-for-byte. See
[raw results](results.txt) and [input/output modules](artifacts/). These examples
show a real size reduction, not general speed, semantic equivalence, or worker
compatibility. The strict output guard is still needed for any future adapter.

## Reproduce offline

Use the [#96 pinned dependencies, prerequisites and build command](../README.md#reproduce-offline).
From the parent `binaryen-96` directory, run with a fresh output directory:

```sh
../../../target/binaryen-96/debug/binaryen-96-probe ../../../target/binaryen-96/local-reproduced local-peephole-v1
diff -r local-peephole-v1/artifacts ../../../target/binaryen-96/local-reproduced
```

Expect exit 0; a guard rejection or cross-binding byte difference fails the candidate
probe. Omitting the profile still runs the original stock matrix (expected exit 2).
No production dependencies, downloads, or adapter changes are required.
