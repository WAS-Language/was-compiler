# M8 final-source correctness and release readiness (#95)

**Keep M8 experimental; release acceptance is pending.** The focused checks,
fresh browser execution and full ignored workspace suite passed with the local
browser dependency configuration below. Strict native Clippy failed on an
existing lint, and fresh browser execution against the committed lockfile is
unverified. The measured size benefit does not resolve the performance decisions.
`None` remains the default and byte baseline; direct codegen still does not fold.
No production code, preset activation, public option, or ABI changed in #95.

## Correctness evidence

The [checked-lowering contract](adr/0012-fold-scalars-during-checked-lowering.md)
keeps the original checks and representation. The private toggle holds both
M6 and M7 enabled; its off arm is **not** `None`.

| Contract | Executed evidence |
| --- | --- |
| Binary64 edge cases and finite-only fallback | [`folding_tests.rs`](../crates/compiler/src/codegen/folding_tests.rs#L152) validates both arms, checks decoded constant bits for negative zero, subnormal ties and rounding, and verifies that overflow keeps its operators while eligible children fold. Independent Node results distinguish signed zeros, subnormals, infinity comparison and NaN. |
| Closed scalar whitelist and excluded roots | [Decoded boolean checks](../crates/compiler/src/codegen/folding_tests.rs#L210) assert canonical constants for eligible comparisons, equality and negation, and retained nonfinite comparisons. The [no-fold test](../crates/compiler/src/codegen/folding_tests.rs#L133) requires whole-module byte identity when no fold applies. |
| Original checked diagnostics | [Forged input checks](../crates/compiler/src/codegen/folding_tests.rs#L554) expect the first invalid literal, owning function and span in short-circuit RHSs, unreachable statements and unexported bodies at every preset. The arithmetic test also checks the private off arm and malformed source diagnostic parity. |
| Effects, traps and nested sites | [Decoded and executed cases](../crates/compiler/src/codegen/folding_tests.rs#L291) cover contracts, normalized defaults, local callables and skipped RHSs in both arms. Runtime checks expect exact host effects for normal return, early contract failure, and a trapping default after both written arguments. |
| M6/M7 composition and original planning | The [decoded composition test](../crates/compiler/src/codegen/folding_tests.rs#L65) verifies unchanged non-code sections, helper bodies, calls and local types across the toggle: the direct loop still omits range helper calls, the exact local remains `i32`, and the multiplication-derived local remains `f64`. [Corpus preflight](../crates/compiler/src/compiler/m8_measurement.rs) checks isolated output, and the [ordinary corpus oracle](../crates/compiler/tests/m8_benchmarks.rs) checks preset results and growth; the no-fold M6/M7 controls are separate from the folded-workload measurements. |
| Adapter parity and default compatibility | The [shared matrix](../tests/support/m6_adapter_cases.rs) checks enabled-preset byte identity and default/direct-codegen parity. The CLI and freshly built browser compare Wasm **and** WAT to core at each preset, check their defaults against `None`, and execute the new composition case: folded range bounds, an integer local and a fractional folded operand in one function. The [independent runtime oracle](../tests/support/m6_adapter_runtime.cjs) expects result `25` and host checkpoints `[3, 4, 5]`. |

These are fixture-level byte and behavior checks, not a claim of universal
`None`/enabled byte identity, general guest speedup, or whole-loop allocation freedom.

## Offline gate results

Run on 2026-11-23 from `47135049d5877a3b7a3d297da4eab7ea87c2a512`
plus the #95 test-fixture edits and pre-existing local edits to `Cargo.lock` and
`crates/browser/Cargo.toml`. Rust/Cargo 1.100.0, Linux 7.2.15-1-omarchy x86-64,
Node 26.13.1 / V8 14.6.202.34-node.40, `wasm32-unknown-unknown`, and wasm-pack
0.15.1. All browser execution below is against a fresh build with local
`js-sys` 0.3.107 and `wasm-bindgen` 0.2.130; the matching cached CLI was put on
`PATH`. The existing local edits were not included in #95. Their local lockfile
SHA-256 was `c9380997510d008de418120f101a36c10e5ef3f31bf649fdc6297d4c5801de2f`;
the committed lockfile SHA-256 was `af7880ab794e4a61cf78a04cc10b1478084132648d39a4cfb8495d0ec0294145`.

| Command | Timeout / elapsed / exit | Result |
| --- | --- | --- |
| `cargo check --offline --workspace --all-targets` | 120 s / 2.50 s / 0 | Passed. |
| `cargo test --offline -p was-compiler --lib codegen::folding_tests -- --include-ignored` | 120 s / 1.27 s / 0 | 14 passed. |
| `cargo test --offline -p was-compiler --lib baseline_corpus_isolates_m8_with_m6_and_m7_enabled` | 120 s / 0.06 s / 0 | 1 passed. |
| `cargo test --offline -p was-compiler --test m8_benchmarks -- --include-ignored` | 120 s / 0.10 s / 0 | 2 passed. |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | 160 s / 2.96 s / 0 | Fresh browser package built with the local lockfile and matching CLI. |
| `cargo test --offline -p was-compiler-cli --test m6_range_adapters -- --include-ignored` | 120 s / 1.48 s / 0 | 3 passed. |
| `cargo test --offline -p was-compiler-browser --test m6_range_adapters -- --include-ignored` | 120 s / 0.51 s / 0 | 1 passed, including the shared six-fixture matrix. |
| `node tests/browser.cjs pkg` | 120 s / 0.29 s / 0 | 25 smoke groups passed. |
| `cargo test --offline` | 180 s / 33.93 s / 0 | 1,379 passed, 0 failed, 333 ignored across 83 result targets. |
| `cargo clippy --offline --all-targets -- -D warnings` | 120 s / 0.19 s / 101 | Failed on pre-existing `clippy::needless_range_loop` in [`named_defaults.rs:487`](../crates/compiler/tests/named_defaults.rs#L487); no suppression or unrelated fix. |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | 120 s / 0.43 s / 0 | Passed. |
| `cargo test --offline -p was-compiler --test workspace_contract` | 120 s / 0.18 s / 0 | 3 passed. |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | 120 s / 0.48 s / 0 | Passed. |
| `cargo test --offline --workspace -- --include-ignored` | 300 s / 37.19 s / 0 | 1,733 passed, 0 failed, 0 ignored across 97 result targets. |

Focused strict Clippy for both `m6_range_adapters` targets also passed. After a
review-only assertion wording change, the workspace typecheck and both focused
adapter suites passed again; the full ignored workspace run repeated with the
same counts (36.49 s, exit 0). In a
scratch copy with #95's test fixture and the **committed** browser dependencies,
the same offline browser build compiled the Rust Wasm but exited 1 after 0.45 s
(160 s limit): wasm-pack could not find the matching local `wasm-bindgen` CLI
0.2.129. Browser smoke, same-preset browser parity and the full ignored
workspace run are **unverified on that lockfile**. To clear the remaining gates,
make the committed CLI version available offline (or separately approve the
dependency update), rebuild before running browser checks and the full workspace
suite, and clear and rerun strict native Clippy separately.

## Performance disposition and release status

- **M8 ([#94](benchmarks/m8-folding-94/README.md)):** Matched forward/reverse
  sessions with M6/M7 fixed reproduce **3,441 → 2,914 Wasm bytes (−527, −15.3%)**
  across the 13 frozen modules; all eight no-fold controls are byte-identical.
  The synthetic `wide_closed` workload accounts for 472 saved bytes. No guest
  speedup is established. Its warmed median is 25.4% / 26.5% slower, with
  overlapping ranges and considerable order-dependent variation even in
  byte-identical controls. Small typed-compile increases (up to 1.8% for the
  clearest signals in this test build) do not establish a material native
  regression. **Defer acceptance** of the size-versus-timing trade-off pending
  explicit maintainer disposition or an authorized, bounded follow-up run.
- **M6 ([#87](m6-release-readiness.md#decisions-still-required)):** **Defer, not
  accepted.** Record elimination and repeatable short-loop gains do not settle
  the longer-loop slowdown, size and compilation increases, or unexplained timing
  tails. No maintainer disposition is recorded; M4/M5 decisions remain unchanged.
- **M7 ([#90](integer-specialization.md#90-m7-release-readiness-and-maintainer-disposition)):**
  **Defer, not accepted.** The 31.51% aggregate size reduction across 26 M7-focused
  modules does not settle the consistent compilation-cost increase or mixed
  guest timings. No explicit opt-in trade-off acceptance or authorization of
  bounded improvements is recorded.

Closure of #87, #90 and #94 marked evidence work complete, not release acceptance.
No new threshold was set. Resolve the outstanding correctness gates and obtain
explicit dispositions for M6, M7 and M8 before a release acceptance decision.
