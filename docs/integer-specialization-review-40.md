# Issue #40 implementation review — 2026-10-20

## Scope and method

Issue #40, **Specialize proven never-reassigned numeric locals**, was reviewed
against starting commit `dc619db46e9dac3694adb7cc5855203a18233426`.
The starting working tree was clean. Because the implementation skill requests
review before committing, the review used
`git --no-pager diff dc619db46e9dac3694adb7cc5855203a18233426` plus the new
`crates/compiler/tests/integer_locals.rs` and
`docs/benchmarks/integer-locals-40/` files, rather than an empty committed-range
diff. Standards and Spec ran in separate parallel sub-agents. Both subsequently
reviewed the inference/pipeline expectation fixes and browser stdin transport fix.
The reviews were static; execution evidence came from the integration owner.

## Standards

**Zero hard violations.** The review found the implementation preserves separate
planning policies, checked lowering, per-body proof ownership, baseline entry
points, and captured/synthetic binding exclusions. It found no actionable
correctness defect through static inspection. Sources included `AGENTS.md`, the
integer proof contract, ADR 0007, callable ownership, benchmark and validation
rules, and the skill's code-smell baseline.

One **low-priority judgment call — possible Duplicated Code** remains: eight
adapter scenarios repeat names and source strings in `crates/cli/tests/cli.rs`
and `crates/browser/tests/integer_specialization_adapters.rs`. A focused shared
fixture could prevent future drift, with adapter execution/assertions and decoders
remaining local. This is not a documented-standard breach or blocker. It is
intentionally deferred to keep this implementation focused; no generic test
framework was introduced.

The final follow-up found no new standards issues: the inference assertions
preserve baseline storage and slot checks, pipeline assertions distinguish ordinary
locals from synthetic bindings, and browser stdin transport retains bounded
subprocess execution and parity assertions.

## Spec

**Zero findings.** The review found the implementation matches issue #40:

- Write discovery precedes initializer proofs and includes skipped/unreachable
  children. Parameter and creator-promoted IDs are excluded; helper owners remain
  separate. Only ordinary numeric lets receive local proofs.
- Eligible slots change type without shifting indices. Existing checked identifier
  resolution remains active, and unspecialized consumers widen reads to `f64`.
- Comparisons/negation remain floating-point. No public annotation, ABI change,
  branch/contract/caller refinement, constant folding or broader optimization was
  introduced.
- Tests cover independent decoding/validation, sparse and forged IDs, exclusions,
  bounded runtime behavior, and matching-preset CLI/rebuilt-browser byte/WAT parity.
- Measurements distinguish cumulative benefit from incremental #40 size changes,
  preserve conversion counts, and report regressions without release claims.

The final follow-up confirmed the inference and pipeline expectation changes retain
ABI, baseline, slot, temporary and synthetic-storage assertions. Moving browser
payloads to stdin preserves default/reset behavior, byte/WAT parity, enabled-preset
identity, execution expectations, repeated compilation and the deadline.

## Validation and measurement reconciliation

[Final correctness validation](integer-specialization.md#correctness-validation-2026-10-20)
records **1,276 tests across 81 targets, zero failed/ignored**, 25 browser smoke
groups, workspace checking, native/browser Clippy, portable core build and a
current-source browser rebuild. The final raw log and three earlier failed-run
logs are linked there. Earlier failures were obsolete storage expectations and a
Windows command-line-length limit in the new browser test; they were fixed and
the complete suite rerun successfully.

No production or measured-harness files changed after collection. The integration
owner compared every entry in the recorded `hashes-after.json` manifest against
final file bytes: no differences. Changes after timing affected integration tests
and documentation only. The matched evidence retains its original status/diff
snapshot and does not claim those later files were present during collection.

[Measurements](benchmarks/integer-locals-40/README.md) show cumulative module-size
benefit but only one byte net savings versus historical #39, with mixed timing and
material regression signals. Correctness and review completion do not accept
regressions, authorize release/default changes, or implement remaining #38 scope.

**Summary:** Standards: 0 hard findings, 1 optional low-priority duplication
suggestion deferred. Spec: 0 findings. Release/regression acceptance remains pending.
