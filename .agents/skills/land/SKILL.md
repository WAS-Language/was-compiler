---
name: land
description: >-
  Commit the user's reviewed changes on the current branch using the repository's
  Conventional Commit style. Invoke only when the user explicitly requests landing
  or commits through Land Changes, not for review, preparation, passing checks,
  pull requests, or skill installation. This skill commits locally only; it does
  not switch branches, pull, merge, push, or open a pull request.
disable-model-invocation: true
metadata:
  delta-action: land
---

# Commit reviewed changes on the current branch

Use this skill only after an explicit user request to land/commit the changes.
That request authorizes the local commit described here; do not ask for the same
permission again. This workflow ends at a local commit on the current branch.
It never pushes, merges, opens a pull request, or switches branches. A separate
explicit request to open a pull request belongs to the existing `pr` skill.

## Preflight

1. Read applicable `AGENTS.md`/`AGENT.md` instructions and inspect the current
   branch, status, staged diff, and unstaged diff.
2. Review the complete intended change. Do not assume every existing working-tree
   change belongs to this request. Preserve unrelated edits and untracked files;
   stage only reviewed files/hunks that belong in this commit.
3. Do not switch branches, pull, rebase, merge, or push. If the current branch
   has diverged or is behind a remote, this workflow still only creates the
   requested local commit; report that it remains local.
4. If there are unresolved index conflicts, resolve them only when the intended
   result is clear from the requested changes and surrounding code. Otherwise,
   stop and ask the user. Preserve unrelated staged content and any in-progress
   operation; do not abort or finalize a merge/rebase that is outside the request.
5. If there are no intended changes to commit, report that and do not create an
   empty commit.

## Verify and commit

1. Select validation in proportion to the actual diff. Follow repository
   instructions for relevant tests and checks. For this repository:
   - Documentation-only changes need `git --no-pager diff --check`, review of
     the diff/status, and checks of changed Markdown links/paths; code tests
     are normally unnecessary.
   - Rust changes require focused tests first and the documented applicable
     offline checks. The repository's broader commands include
     `cargo test --offline`, `cargo clippy --offline --all-targets -- -D warnings`,
     and `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown`.
     Browser or ignored runtime coverage is required when applicable; consult
     `AGENTS.md` for exact commands and prerequisites. Do not install tools or
     fetch dependencies. Report unavailable prerequisites and failed checks;
     never claim unrun checks passed.
2. Re-review the exact staged diff and run `git --no-pager diff --cached --check`.
   Confirm only intended files/hunks are staged. Do not use blanket staging such
   as `git add -A` when unrelated changes may exist.
3. Create a concise Conventional Commit message in the form
   `<type>(optional-scope): <imperative summary>`, using an appropriate type such
   as `docs`, `feat`, `fix`, `test`, `refactor`, or `chore`. Recent repository
   history uses prefix forms such as `docs:`, `feat(codegen):`, and
   `refactor:`. Do not claim the repository documents a stricter convention.
4. Commit only the reviewed staged changes on the current branch. Use a
   non-interactive Git invocation; do not launch an editor or interactive rebase.
5. Verify the resulting commit with `git log -1 --oneline` and inspect
   `git status --short --branch`. Report the commit hash and whether remaining
   changes are intentionally unstaged or unrelated.

If a required check fails, the intended diff is unclear, or preserving unrelated
work cannot be assured, stop without committing and explain the blocker. A
successful result is a local commit only. Do not treat it as pushed, merged, or
available in the original checkout. Handle a separately requested pull request
through the existing `pr` skill, not by extending this workflow.
