"""Bounded post-collection evidence audit. Does not run compiler validation suites."""
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


def main():
    expected = load(HERE / "collection-1/hashes-before.json")
    final = matched.hashes()
    changed = matched.changed(expected, final)
    base.write_json(HERE / "final-source-state.json", {"utc": base.utc(), "hashes": final,
        "matches_main_collection": not changed, "changed_inputs": changed})
    base.require(not changed, "Production/historical measurement hashes changed")
    # Scripts added after supplement collection are postprocessing, not changed
    # measurement inputs; compare every entry actually present in its manifest.
    supplement = load(HERE / "expressions-2/hashes-after.json")["supplement"]
    script_changes = [name for name, digest in supplement.items()
        if hashlib.sha256((HERE / name).read_bytes()).hexdigest() != digest]
    base.require(not script_changes, "Supplement measured files changed: " + repr(script_changes))
    env = dict(base.os.environ, CARGO_NET_OFFLINE="true")
    base.record(HERE, "final-diff-check", ["git", "--no-pager", "diff", "--check"], env)
    base.record(HERE, "final-status", ["git", "--no-pager", "--no-optional-locks", "status", "--short", "--untracked-files=all"], env)
    commands, failures = [], []
    for path in sorted(HERE.rglob("*.json")):
        value = load(path)
        if isinstance(value, dict) and "command" in value and "exit_code" in value:
            name = path.relative_to(HERE).as_posix()
            commands.append(name)
            base.require((path.parent / value["output"]).is_file(), "Missing raw command output: " + name)
            if value["exit_code"] != 0 or value["timed_out"]:
                failures.append({"metadata": name, "exit_code": value["exit_code"], "timed_out": value["timed_out"]})
    base.require([r["metadata"] for r in failures] == ["expressions-1/emit.json"], "Unexpected command failures")
    for directory in ("collection-1", "expressions-2"):
        stable = load(HERE / directory / "stability.json")
        base.require(stable["commands_completed"] and stable["inputs_unchanged"] and not stable["changed_inputs"], "Incomplete/unstable success record")
    main_summary = load(HERE / "collection-1/summary.json")
    supplement_summary = load(HERE / "expressions-2/summary.json")
    fields = ("node", "v8", "platform", "arch", "osRelease", "cpu", "logicalCpus", "totalMemoryBytes", "nodeFlags")
    machines = main_summary["machines"] + [supplement_summary["machine"]]
    for path in (HERE / "expressions-2").glob("guest-*.txt"):
        records = [json.loads(line) for line in path.read_text().splitlines() if line.startswith('{')]
        machines += [r for r in records if "mode" in r]
    base.require(all(all(m[k] == machines[0][k] for k in fields) for m in machines), "Machine/runtime mismatch")
    broken_links, whitespace = [], []
    for name in ("README.md", "results.md"):
        path = HERE / name
        text = path.read_text(encoding="utf-8")
        for link in re.findall(r"\]\(([^)]+)\)", text):
            if link not in ("audit.json", "artifact-hashes.json") and not link.startswith(("http:", "https:", "#")) and not (HERE / link.split('#')[0]).exists():
                broken_links.append(name + ":" + link)
        whitespace += [f"{name}:{n}" for n, line in enumerate(text.splitlines(), 1) if line.rstrip() != line]
    base.require(not broken_links and not whitespace, "Broken links/whitespace")
    base.write_json(HERE / "audit.json", {"utc": base.utc(), "passed": True,
        "production_and_historical_measurement_inputs_unchanged": True,
        "supplement_measured_inputs_unchanged": True, "postprocessing_files_added_after_timing": True,
        "command_records_checked": len(commands), "preserved_expected_failures": failures,
        "machine_runtime_and_node_flags_match": True, "broken_links": broken_links,
        "trailing_whitespace": whitespace, "scope": "Evidence audit, not full correctness/browser/release validation"})
    artifacts = {p.relative_to(HERE).as_posix(): {"bytes": p.stat().st_size,
        "sha256": hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted(HERE.rglob('*'))
        if p.is_file() and p.name != "artifact-hashes.json"}
    base.write_json(HERE / "artifact-hashes.json", artifacts)
    print(f"Audit passed: {len(commands)} command records, {len(artifacts)} artifacts; preserved one expected failed attempt; production unchanged")


if __name__ == "__main__":
    main()
