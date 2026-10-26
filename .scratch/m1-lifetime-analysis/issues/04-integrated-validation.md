## What to build

Close M1 with a reproducible contract-to-test/evidence record and fresh offline native/current-source browser validation. Clearly distinguish implemented classification from future M2 storage selection and reclamation.

Source of truth: `docs/was_compiler_memory_and_optimization_roadmap.md` §5 confirmed decisions. Ticket 4 of the approved four-ticket plan, after stable implementations of tickets 1–3.

## Acceptance criteria

- [ ] Record the implemented subset, private entry/test locations, outcome/reason semantics, owner boundaries, explicit test-only invocation and absence of production/emission consumers.
- [ ] Map positive, conservative, trap, forged-tree, deterministic and per-preset before/after byte checks to actual tests and results; no extrapolation from default Cargo success.
- [ ] Run the complete offline fast/full sequence below on stable sources; record exact commands, outcomes, tool versions and blockers. Browser tests run only after rebuilding pkg from current source. Include ignored runtimes and adapter parity evidence.
- [ ] Update roadmap M1 status and `AGENTS.md` for the implemented architecture/workflow without claiming M2, reclamation, managed-heap support or memory/performance improvements.
- [ ] Preserve existing adapter contracts, fixtures, presets, raw-host retention rights and M0 evidence. M0 success is not M1 evidence.
- [ ] Missing prerequisites or unexplained failures remain explicit pending gates. Never install/fetch tools, weaken assertions or declare M1 fully validated on partial evidence.

## Implementation handoff

- **Main files:** proposed new `docs/m1-lifetime-analysis.md` owns implemented contract, test map and evidence. `docs/was_compiler_memory_and_optimization_roadmap.md` §5 gets the factual status/evidence link; `AGENTS.md` gains the actual private-analysis boundary. Inspect actual blocker-delivered paths rather than assuming proposed filenames survived. Do not edit `docs/validation.md` merely to relax established gates.
- **Read first:** tickets 1–3 outputs and focused evidence; `AGENTS.md`; `GLOSSARY.md`; roadmap confirmed M1 decisions; `docs/validation.md`; `docs/m0-memory-baseline.md`; `docs/memory-abi.md` revision 1; ADRs 0004/0006/0007 in `docs/adr/`.
- **Test anchors:** private lifetime tests from blockers; `crates/compiler/tests/typed_arrays_lowering.rs`; `crates/cli/tests/typed_arrays_adapters.rs`: `cli_typed_array_hosts_selected_exports_and_contextual_source_match_core`, `cli_typed_array_diagnostics_match_core_without_touching_output`, `cli_typed_arrays_execute_with_raw_host_layouts`; `crates/browser/tests/typed_arrays_adapters.rs`: `rebuilt_browser_typed_array_hosts_bytes_diagnostics_and_execution_match_core`; `crates/compiler/tests/typed_arrays_runtime.rs`: retained/reentry/trap coverage. `tests/browser.cjs` is run through the existing browser test workflow. Do not expose the private classifier to adapters for new parity checks.
- **Exact validation, repository root, in order:**

```sh
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown
cargo test --offline -p was-compiler --test workspace_contract
wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline
cargo test --offline -p was-compiler-browser --tests -- --include-ignored
cargo test --offline -- --include-ignored
cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings
git --no-pager diff --check
```

  Use bounded execution, restart evidence on source changes, and review final diff/status. Node.js 24+, cached dependencies, wasm32 target and compatible installed wasm-pack/wasm-bindgen tooling are prerequisites. Alternatively, after the required browser rebuild, the documented workspace-wide `cargo test --offline --workspace -- --include-ignored` can cover all packages. Record which route actually ran. No new benchmark/performance gate is required.
- **Direction and open questions:** only actual results determine completion. Keep planned versus passed gates explicit; identify unrelated failures without repairing unrelated code. Documentation links/paths must resolve. Do not modify/close a parent tracker issue or start M2.
- **Suggested model:** `zed.dev/claude-sonnet-4-6`.
- **Thinking effort:** medium, provisional; model availability is verified, configurable effort levels are not.
- **Rationale:** bounded execution of an established workflow and factual evidence writing need less novel design reasoning than the proof implementation. No verified pricing comparison is asserted.
- **Escalation trigger:** unexplained reproducible validation failure or evidence contradicting the agreed contract needs focused diagnosis/replanning; missing tools require resolving prerequisites, not a stronger model. Never infer a pass from historical output.
