# Implementation handoff

Use this guide when preparing a spec or ticket for implementation in a fresh
agent context. Make the work bounded and navigable before increasing model cost.

## Implementation map

Inspect the relevant implementation and tests before writing the handoff. Record:

- **Main files:** repository-relative paths, relevant symbols, and what each file
  owns or needs changed. Distinguish existing files from proposed new files.
- **Read first:** applicable agent instructions, authoritative contracts/ADRs, and
  prerequisite decisions or blocker outputs. Point to the relevant sections.
- **Test anchors:** existing test paths and representative cases to extend, plus
  exact validation commands and the observable outcomes they must establish.
- **Direction:** settled interface decisions, invariants, evaluation/failure order,
  and boundaries the implementation must preserve. Separate open questions from
  decisions; identify which questions block implementation.

Keep the map focused on this unit of work, not a repository inventory. Verify
existing paths and symbols against the current checkout; label proposed locations
and unverified assumptions explicitly. Paths are navigation aids, not a requirement
to retain the current structure: the implementer rechecks them before editing.
Use symbols or section headings rather than fragile line numbers.

## Model recommendation

Include these fields in every spec and every ticket:

- **Suggested model:** a concrete model identifier, with provider when needed.
- **Thinking effort:** an effort level supported by that model, or “not
  configurable” when it exposes none.
- **Rationale:** why this is the lowest-cost adequate choice for the bounded work,
  considering ambiguity, cross-module reasoning, correctness risk, and validation.
- **Escalation trigger:** a concrete condition requiring replanning or a stronger
  model/effort, such as a contract conflict or a reproducible failure that remains
  unexplained after the focused checks. Missing prerequisites call for resolving
  the prerequisite, not automatically increasing model cost.

Use the available model catalog and supported effort levels when accessible;
inspect current pricing only when making price comparisons. If availability,
effort support, or pricing cannot be verified, label that uncertainty and make the
recommendation provisional rather than inventing identifiers or cost claims.
Recommendations are handoff metadata, not permission to change persistent settings
or launch an agent.

Prefer a lower-cost model with low or medium supported effort for explicit,
testable changes with established patterns. First improve missing context, resolve
design questions, or split oversized work. Recommend a stronger model or higher
effort when substantial ambiguity, novel architecture, or correctness reasoning
remains, and name that difficulty. Spec and ticket recommendations can differ:
design difficulty does not make every implementation slice equally difficult.

## Completion gate

The handoff is ready when a fresh agent can locate every main implementation/test
entry point, distinguish settled decisions from blockers, and run the named checks
without reconstructing the conversation. Every ticket carries its own scoped map
and recommendation; parent links supply authoritative detail, not missing essentials.
