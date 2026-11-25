# Keep Binaryen opt-in at the adapter boundary

M9 applies Binaryen to Wasm from the existing backend. Use an in-process
`wasm-opt` binding behind the native CLI's opt-in `binaryen` Cargo feature and
load `binaryen.js` on demand in the browser playground's worker. This avoids a
runtime executable dependency without imposing native build or browser load
costs on the portable core and ordinary builds. The generated `BrowserCompiler`
API remains unchanged.

Availability is not activation. Require an explicit `--binaryen` request or
browser equivalent, sharing the existing optimization level rather than adding a
second selector. In an active path, levels 1–3 select matching compiler presets
and Binaryen profiles; level 0 bypasses Binaryen. Omission of the level still
selects 1, and omission of the request preserves the existing byte baselines.
Treat reported optimization and output-validation failures as errors rather than
silently publishing the original module.

Pin both adapters to the same upstream Binaryen version and equivalent settings.
Native can land first, but M9 requires both adapters and cross-environment
semantic and ABI validation. Test optimized-byte parity explicitly from identical
inputs; resolve or explicitly accept explained differences before release. See
[the M9 roadmap](../was_compiler_memory_and_optimization_roadmap.md#13-milestone-m9--optional-binaryen-integration)
for the confirmed design, ordered handoff, and release evidence. The design
interview is closed; confirmation does not authorize dependency acquisition or
implementation.
