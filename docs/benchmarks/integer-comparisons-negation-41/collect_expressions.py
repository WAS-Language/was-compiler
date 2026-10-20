"""Expression-only supplement: unchanged Divan closures and Node timing protocol.

Run after collect.py, sequentially, from repository root. Outputs must be new.
Uses only cached dependencies; its standalone Cargo workspace is evidence-only.
"""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
spec = importlib.util.spec_from_file_location("matched", ROOT / "docs/benchmarks/integer-add-sub-39/record_matched.py")
matched = importlib.util.module_from_spec(spec)
spec.loader.exec_module(matched)
base = matched.baseline
NAMES = ("expr_compare_1", "expr_negate_1", "expr_compare_32", "expr_negate_32", "expr_zero", "expr_min")


def hashes():
    result = matched.hashes()
    result["supplement"] = {p.name: base.hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(HERE.iterdir()) if p.suffix in (".rs", ".cjs", ".py") or p.name in ("Cargo.toml", "Cargo.lock")}
    return result


def main():
    base.require(Path.cwd().resolve() == ROOT, "Run from repository root")
    out = HERE / "expressions-2"
    out.mkdir(exist_ok=False)
    env = dict(os.environ, CARGO_NET_OFFLINE="true")
    manifest = str(HERE / "Cargo.toml")
    target = str(ROOT / "target/integer-expressions-41")
    cargo = ["cargo"]
    flags = ["--offline", "--manifest-path", manifest, "--target-dir", target]
    # Freeze the offline dependency resolution before taking supplement hashes.
    base.record(out, "lockfile", cargo + ["generate-lockfile", "--offline", "--manifest-path", manifest], env)
    before = hashes()
    timed = json.loads((HERE / "collection-1/hashes-after.json").read_text())
    base.require(all(before[k] == v for k, v in timed.items()), "Inputs changed since main collection")
    base.write_json(out / "hashes-before.json", before)
    base.write_json(out / "environment.json", {"utc": base.utc(), "command": sys.argv,
        "python": sys.version, "cwd": str(ROOT), "target_dir": target,
        "note": "Other owner may run concurrent builds/tests; not an idle-machine timing experiment.",
        "order": "build/emit/verify; compiler None/Basic, guest None/Basic; compiler Basic/None, guest Basic/None"})
    completed = False
    def run(name, command, level="None"):
        base.require(hashes() == before, "Inputs changed before " + name)
        base.write_json(out / ("inputs-" + name + "-before.json"), hashes())
        try:
            base.record(out, name, command, dict(env, INTEGER_BASELINE_LEVEL=level))
        finally:
            after = hashes()
            base.write_json(out / ("inputs-" + name + "-after.json"), after)
            base.require(after == before, "Inputs changed during " + name)
    try:
        run("emit", cargo + ["run"] + flags + ["--bin", "emit-expressions", "--", str(out / "artifacts")])
        # Thin I/O bridge only: the copied Node verifier/timing loop reads stdin.
        payload = json.loads((out / "artifacts/payload.json").read_text())
        for mode, selected in [("verify", payload)] + [
            (f"guest-{i}-{level}", [r for r in payload if r[1] == level])
            for i, order in ((1, ("None", "Basic")), (2, ("Basic", "None"))) for level in order]:
            base.write_json(out / (mode + "-payload.json"), selected)
        bridge = str(HERE / "node_bridge.py")
        run("verify", [sys.executable, bridge, "verify", str(out / "verify-payload.json")])
        for i, order in ((1, ("None", "Basic")), (2, ("Basic", "None"))):
            for level in order:
                run(f"compiler-{i}-{level}", cargo + ["bench"] + flags + ["--bench", "expressions", "--",
                    "--sample-count", "100", "--sample-size", "1", "--timer", "os", "--color", "never"], level)
            for level in order:
                name = f"guest-{i}-{level}"
                run(name, [sys.executable, bridge, "timing", str(out / (name + "-payload.json"))], level)
        base.NAMES = NAMES
        verify, machine = base.parse_records((out / "verify.txt").read_text(), "verify", base.LEVELS)
        guests, compilers = {}, {}
        for i in (1, 2):
            for level in matched.LEVELS:
                guests[i, level], _ = base.parse_records((out / f"guest-{i}-{level}.txt").read_text(), "timing", (level,))
                compilers[i, level] = matched.compiler_records((out / f"compiler-{i}-{level}.txt").read_text(encoding="utf-8"))
        matched.identities(verify, [(level, rows) for (_, level), rows in guests.items()])
        rows = []
        for name in NAMES:
            row = {"name": name, "levels": {}}
            for level in matched.LEVELS:
                identity = verify[level, name]
                for i in (1, 2):
                    base.require(compilers[i, level][1][name] == [identity["sourceBytes"], identity["moduleBytes"]], "Compiler/guest size mismatch")
                row["levels"][level] = {**{k: identity[k] for k in base.IDENTITY},
                    "guest_medians_ns": [guests[i, level][level, name]["guestTiming"]["median"] for i in (1, 2)],
                    "compiler": {stage: [compilers[i, level][0][stage, name] for i in (1, 2)]
                        for stage in ("source_compile", "typed_compile")}}
            rows.append(row)
        base.write_json(out / "summary.json", {"scope": "Expression-only supplement, not historical corpus", "verify_records": 24, "machine": machine, "workloads": rows})
        completed = True
    finally:
        after = hashes()
        base.write_json(out / "hashes-after.json", after)
        base.write_json(out / "stability.json", {"utc_finished": base.utc(), "commands_completed": completed,
            "inputs_unchanged": before == after, "changed_inputs": matched.changed(before, after)})
    base.require(completed and before == after, "Incomplete/unstable supplement")


if __name__ == "__main__":
    main()
