"""Append-only integration reconciliation; never rerun the immutable original audit.

From repository root: run with `snapshot`, update README, then run with `audit`.
All JSON/log destinations are create-new. No build, test or timing is performed.
"""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
spec = importlib.util.spec_from_file_location("matched", ROOT / "docs/benchmarks/integer-add-sub-39/record_matched.py")
matched = importlib.util.module_from_spec(spec)
spec.loader.exec_module(matched)
base = matched.baseline


def load(path):
    return json.loads(path.read_text(encoding="utf-8"))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def original_changes():
    manifest = load(HERE / "artifact-hashes.json")
    return [{"path": name, "original_sha256": row["sha256"],
        "current_sha256": digest(HERE / name) if (HERE / name).is_file() else None}
        for name, row in manifest.items()
        if not (HERE / name).is_file() or digest(HERE / name) != row["sha256"]]


def snapshot():
    current = matched.hashes()
    references = (
        "collection-1/hashes-before.json", "collection-1/hashes-after.json",
        "expressions-2/hashes-before.json", "expressions-2/hashes-after.json",
        "inspection/hashes-before.json", "inspection/hashes-after.json",
        "final-source-state.json",
    )
    comparisons = []
    for name in references:
        reference = load(HERE / name)
        if name == "final-source-state.json":
            reference = reference["hashes"]
        # Supplement-only hashes are a separate group, not production/measurement.
        reference = {group: reference[group] for group in current}
        changes = matched.changed(reference, current)
        comparisons.append({"manifest": name, "manifest_sha256": digest(HERE / name),
            "matches_current": not changes, "changed_inputs": changes})
    timed = load(HERE / references[0])
    details = [{"path": name, "timed_sha256": timed["production"][name],
        "current_sha256": current["production"][name],
        "byte_identical": timed["production"][name] == current["production"][name]}
        for name in ("crates/compiler/src/codegen/expression.rs", "crates/compiler/src/codegen/representation.rs")]
    base.write_json(HERE / "final-integration-source-state.json", {
        "utc": base.utc(), "hashes": current, "comparisons": comparisons,
        "production_files_checked": len(current["production"]),
        "measurement_files_checked": len(current["measurement"]),
        "reported_rustfmt_files": details,
        "changed_inputs_vs_timed": matched.changed(timed, current),
        "original_artifact_manifest_sha256": digest(HERE / "artifact-hashes.json"),
        "original_artifact_changes_before_readme_update": original_changes(),
        "integration_status_user_reported_not_rerun": {
            "global_docs": "Integrator updating; outside production/measurement manifest groups",
            "workspace_attempt": "Stopped at preexisting fixed-byte enabled-negation oracle in tests/block_values.rs",
            "test_update": "Integrator reports enabled bytes updated for #41; O0 fixed oracle unchanged",
            "full_workspace_rerun": "Pending at handoff; not validated by this task"},
        "interpretation": "Compare actual bytes, not the reported rustfmt action. Identical hashes mean no observable post-timing source change; not a new measurement of final sources.",
        "timings_rerun": False,
    })
    print(json.dumps({"comparisons": comparisons, "reported_rustfmt_files": details,
        "original_artifact_changes": original_changes()}, indent=2))


def audit():
    state = load(HERE / "final-integration-source-state.json")
    base.require(matched.hashes() == state["hashes"], "Inputs changed after integration snapshot")
    base.require(all(row["matches_current"] for row in state["comparisons"]), "Differences need explicit reconciliation")
    base.require(not state["original_artifact_changes_before_readme_update"], "Original evidence already differed")
    base.require(digest(HERE / "artifact-hashes.json") == state["original_artifact_manifest_sha256"], "Original manifest changed")
    changes = original_changes()
    base.require([row["path"] for row in changes] == ["README.md"], "Only README reconciliation may change original artifacts")
    base.record(HERE, "final-integration-diff-check", ["git", "--no-pager", "diff", "--check"], dict(base.os.environ))
    base.record(HERE, "final-integration-status", ["git", "--no-pager", "--no-optional-locks", "status", "--short", "--untracked-files=all"], dict(base.os.environ))
    broken, whitespace = [], []
    for name in ("README.md", "results.md"):
        text = (HERE / name).read_text(encoding="utf-8")
        for link in re.findall(r"\]\(([^)]+)\)", text):
            if link in ("final-integration-audit.json", "final-integration-artifact-hashes.json"):
                continue
            if not link.startswith(("http:", "https:", "#")) and not (HERE / link.split('#')[0]).exists():
                broken.append(name + ":" + link)
        whitespace += [f"{name}:{n}" for n, line in enumerate(text.splitlines(), 1) if line.rstrip() != line]
    base.require(not broken and not whitespace, "Broken links/trailing whitespace")
    base.require(matched.hashes() == state["hashes"], "Inputs changed during integration audit")
    base.write_json(HERE / "final-integration-audit.json", {
        "utc": base.utc(), "passed": True, "scope": "Append-only source/artifact reconciliation; no compiler validation or timing rerun",
        "source_snapshot": "final-integration-source-state.json",
        "all_seven_source_snapshots_match_current": True,
        "original_artifact_manifest_unchanged": True,
        "original_artifact_changes": changes,
        "original_audit_and_raw_evidence_unchanged": True,
        "broken_links": broken, "trailing_whitespace": whitespace,
        "full_workspace_rerun_status": "Pending per user report; not executed here"})
    artifacts = {p.relative_to(HERE).as_posix(): {"bytes": p.stat().st_size, "sha256": digest(p)}
        for p in sorted(HERE.rglob('*')) if p.is_file() and p.name != "final-integration-artifact-hashes.json"}
    base.write_json(HERE / "final-integration-artifact-hashes.json", artifacts)
    print(f"Integration audit passed: {len(artifacts)} artifacts; original evidence unchanged except requested README reconciliation")


if __name__ == "__main__":
    base.require(Path.cwd().resolve() == ROOT, "Run from repository root")
    base.require(sys.argv[1:] in (["snapshot"], ["audit"]), __doc__)
    snapshot() if sys.argv[1] == "snapshot" else audit()
