"""Reconcile #42 evidence, final sources, local link paths and validation totals.

Postprocessing only: no compiler invocation or measurement. Historical files and
raw logs are read-only. audit.json is the current packaging reconciliation.
"""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BASE = "0e1b5225a6d63c18244ab8c06fc460dd2d7a0020"
TESTS = [
    "crates/compiler/tests/integer_locals.rs",
    "crates/cli/tests/cli.rs",
    "crates/browser/tests/integer_specialization_adapters.rs",
    "tests/support/integer_composition_cases.rs",
]
DOCS = ["README.md", "AGENTS.md", "docs/integer-specialization.md",
        "docs/benchmarks.md", "docs/was_language_extensions_plan.md",
        "docs/limitations.md", "docs/compilation-roadmap.md"]


def load(path):
    return json.loads(path.read_text(encoding="utf-8"))


def git(*args):
    return subprocess.check_output(["git", "--no-pager", *args], cwd=ROOT, timeout=30)


def main():
    spec = importlib.util.spec_from_file_location("matched", ROOT / "docs/benchmarks/integer-add-sub-39/record_matched.py")
    matched = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(matched)
    require = matched.baseline.require
    expected = load(HERE / "collection-1/hashes-before.json")
    current = matched.hashes()
    require(expected == current, "Production or historical measurement inputs changed")
    require(load(HERE / "collection-1/hashes-after.json") == expected, "Collection input mismatch")
    stability = load(HERE / "collection-1/stability.json")
    require(stability["commands_completed"] and stability["inputs_unchanged"] and not stability["changed_inputs"], "Incomplete collection")
    # The historical recorder's pre-run tracked diff contains these three files.
    # The untracked shared cases were not input-monitored; record their final hash
    # without pretending the original recorder captured their content.
    measured_diff = (HERE / "collection-1/git-diff-before.txt").read_bytes()
    current_test_diff = git("diff", "--binary", BASE, "--", *TESTS[:3])
    require(measured_diff == current_test_diff, "Tracked tests changed after measurement")
    commands = load(HERE / "validation-1/commands.json")
    require(commands["all_gates_passed"] and len(commands["commands"]) == 9, "Incomplete correctness gates")
    require([r["name"] for r in commands["commands"]][-4:] == [
        "browser-build", "browser-smoke", "workspace-ignored", "browser-clippy"], "Browser rebuild ordering changed")
    totals = {}
    for row in commands["commands"]:
        require(row["exit_code"] == 0 and not row["timed_out"], "Failed correctness gate")
        path = HERE / "validation-1" / row["output"]
        text = path.read_text(encoding="utf-8")
        rows = [tuple(map(int, match)) for match in re.findall(
            r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored", text)]
        if rows:
            passed, failed, ignored = map(sum, zip(*rows))
            totals[row["name"]] = dict(targets=len(rows), passed=passed, failed=failed, ignored=ignored)
    require(totals["workspace-ignored"] == dict(targets=81, passed=1296, failed=0, ignored=0), "Workspace totals differ")
    require("browser compiler: 25 tests passed" in (HERE / "validation-1/browser-smoke.txt").read_text(), "Missing browser smoke success")
    require(not git("diff", BASE, "--", "tests/fixtures"), "Baseline fixtures changed")
    git("diff", "--check")
    paths = [ROOT / name for name in DOCS] + [HERE / name for name in ("README.md", "results.md")]
    broken = []
    for path in paths:
        text = path.read_text(encoding="utf-8")
        require(all(line == line.rstrip() for line in text.splitlines()), "Trailing whitespace: " + str(path))
        for link in re.findall(r"\]\(([^)]+)\)", text):
            if link.startswith(("http:", "https:", "#", "mailto:")):
                continue
            destination = (path.parent / link.split("#")[0]).resolve()
            if destination == HERE / "audit.json":
                continue
            if not destination.exists():
                broken.append(path.relative_to(ROOT).as_posix() + ": " + link)
    require(not broken, "Broken local link paths: " + repr(broken))
    evidence = {p.relative_to(HERE).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                for p in sorted(HERE.rglob("*")) if p.is_file() and p.name != "audit.json"}
    result = {
        "utc": matched.baseline.utc(), "passed": True, "base_revision": BASE,
        "scope": "Postprocessing reconciliation, not regression/release acceptance",
        "production_and_historical_harness_match_measured_inputs": True,
        "tracked_test_diff_matches_collection": True,
        "test_source_sha256": {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in TESTS},
        "shared_cases_note": "Final hash only; the original recorder did not monitor this untracked test-only file",
        "production_and_measurement_hashes": current,
        "baseline_fixtures_unchanged": True,
        "validation_totals": totals, "browser_smoke_groups": 25,
        "all_nine_correctness_commands_passed": True, "browser_rebuilt_before_execution": True,
        "broken_local_link_paths": broken, "evidence_sha256": evidence,
        "release_and_regression_acceptance": "PENDING; keep experimental; None default",
    }
    (HERE / "audit.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print("Audit passed:", len(evidence), "evidence artifacts;", totals)


if __name__ == "__main__":
    main()
