"""Audit the finished evidence without building or fetching; write a final manifest."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
spec = importlib.util.spec_from_file_location(
    "matched", ROOT / "docs/benchmarks/integer-add-sub-39/record_matched.py")
matched = importlib.util.module_from_spec(spec)
spec.loader.exec_module(matched)
base = matched.baseline


def main():
    base.require(not (HERE / "audit.json").exists(), "Audit must be new")
    env = dict(os.environ, CARGO_NET_OFFLINE="true")
    base.record(HERE, "final-diff-check", ["git", "--no-pager", "diff", "--check"], env)
    base.record(HERE, "final-status", ["git", "--no-pager", "--no-optional-locks",
                                    "status", "--short", "--untracked-files=all"], env)
    collection = HERE / "collection-1"
    before = json.loads((collection / "hashes-before.json").read_text())
    after = json.loads((collection / "hashes-after.json").read_text())
    base.require(before == after == matched.hashes(), "Final inputs differ from timed inputs")
    per_command = list(collection.glob("inputs-*.json"))
    base.require(all(json.loads(p.read_text()) == before for p in per_command),
                 "Per-command input manifests differ")
    metadata = []
    for path in HERE.rglob("*.json"):
        data = json.loads(path.read_text())
        if "exit_code" not in data:
            continue
        base.require(data["exit_code"] == 0 and not data["timed_out"],
                     f"Command failure: {path.name}")
        base.require((path.parent / data["output"]).is_file(), "Missing raw output")
        metadata.append(path.relative_to(HERE).as_posix())
    inspection = json.loads((HERE / "inspection/counts.json").read_text())
    reference = json.loads((collection / "summary.json").read_text())
    reference = {r["name"]: r for r in reference["workloads"]}
    for row in inspection["rows"]:
        target = reference[row["name"]]["levels"][row["level"]]
        base.require(all(row[k] == target[k] for k in ("sourceSha256", "moduleSha256", "moduleBytes")),
                     "Inspection identity mismatch")
    text_files = [HERE / name for name in ("README.md", "collect.py", "inspect_wat.py", "audit.py")]
    for path in text_files:
        lines = path.read_text(encoding="utf-8").splitlines()
        base.require(all(line == line.rstrip() for line in lines), f"Trailing whitespace: {path.name}")
    links = re.findall(r"\]\(([^)]+)\)", (HERE / "README.md").read_text(encoding="utf-8"))
    generated = {"audit.json", "artifact-hashes.json"}
    base.require(all((HERE / link).exists() or link in generated for link in links),
                 "Broken README relative link")
    report = {
        "utc": base.utc(), "commands_checked": len(metadata), "command_metadata": sorted(metadata),
        "all_commands_succeeded_without_timeout": True,
        "per_command_input_manifests_checked": len(per_command),
        "timed_inputs_still_unchanged": True,
        "inspection_module_identities_checked": len(inspection["rows"]),
        "authored_text_whitespace_checked": [p.name for p in text_files],
        "readme_relative_links_checked": len(links),
        "git_diff_check_exit_code": 0,
        "scope": "Evidence audit only, not full compiler/browser release validation",
    }
    base.write_json(HERE / "audit.json", report)
    entries = {}
    for path in sorted(HERE.rglob("*")):
        if path.is_file() and path.name != "artifact-hashes.json":
            content = path.read_bytes()
            entries[path.relative_to(HERE).as_posix()] = {
                "bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()}
    base.write_json(HERE / "artifact-hashes.json", {"utc": base.utc(), "algorithm": "SHA-256",
        "excludes": ["artifact-hashes.json"], "files": entries})
    base.require(all((HERE / link).exists() for link in links), "Missing final linked artifact")
    print(f"Audit passed: {len(metadata)} successful command records, {len(per_command)} stable input manifests, "
          f"{len(inspection['rows'])} matching inspection modules, {len(entries)} hashed artifacts.")


if __name__ == "__main__":
    main()
