# WP12 integrated release validation (2026-10-25)

This record covers the integrated validation/documentation scope of issue #49.
It does not authorize compiler changes, release acceptance, merging, or landing.

## Source and provenance

- Production source revision: `da3b6a0c78aa9f1d051bee14ac994b5e661bd327`
  (`feat(pipeline): add immediate unary stages (#48)`).
- Validation ran against this checked-out revision; no production source files
  were changed during validation. Generated browser bindings were rebuilt from
  current workspace sources before browser tests.
- Environment: Rust `cargo 1.98.0`, Node.js `v24.12.0`, wasm-pack `0.15.0`,
  installed `wasm32-unknown-unknown` target. Commands used Cargo offline mode.

## Results

All documented integrated commands completed successfully:

| Gate | Result |
| --- | --- |
| `cargo test --offline` | Passed. The first bounded attempt timed out after 120 seconds while tests were progressing; the retry completed successfully. |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed. |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed. |
| `cargo test --offline -p was-compiler --test workspace_contract` | Passed: 3 tests. |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed; current-source bindings rebuilt. wasm-pack reported only optional missing Cargo metadata (`description`, `repository`, `license`). |
| `node tests/browser.cjs pkg` | Passed: all 25 smoke groups. |
| `cargo test --offline -p was-compiler-browser --tests -- --include-ignored` | Passed: 13 tests across 10 targets, including browser pipeline/closure parity. |
| `cargo test --offline --workspace -- --include-ignored` | Passed: 1,362 tests across 81 targets; 0 failed and 0 ignored. |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed. |
| `git diff --check` (changed documentation paths) | Passed. Repository-wide `git diff --check` also reports a pre-existing extra blank line at EOF in the user-modified `src-python/TValue.py`; that file was not changed. |

The integrated runs exercise compiler source and forged typed input seams,
checked lowering and decoded assertions, Wasm validation, runtime effects, CLI
parity, optimization-level behavior, and current-source rebuilt-browser parity,
including ignored runtime/browser cases. The specific WP12-focused pipeline and
closure targets were included in the full workspace run. The available test
records establish that the agreed gates passed on this source revision; they do
not imply permission to land or change the release decision independently of the
issue's explicit authorization boundary.

## Scope and disposition

No compiler defect requiring an implementation change was found during these
validation gates. The WP12 implementation remains the completed #44–#48
milestones; this issue adds integrated evidence only. Optimizer work, callable
ABI changes, reclamation, and unrelated cleanup were not performed. This evidence
record does not itself provide landing authorization; that remains a separate
explicit decision.
