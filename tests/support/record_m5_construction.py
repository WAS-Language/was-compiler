"""Record #79 deterministic construction evidence; no timing or tool installation."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command):
    return subprocess.run(command, cwd=ROOT, check=True, capture_output=True,
                          timeout=30).stdout.decode("utf-8", errors="replace").strip()


def sources():
    paths = sorted((ROOT / "crates/compiler/src").rglob("*.rs"))
    paths += [ROOT / name for name in [
        "Cargo.toml", "Cargo.lock", "crates/compiler/Cargo.toml",
        "tests/support/process.rs", "tests/support/m5_construction_corpus.was",
        "tests/support/m5_construction.cjs", "tests/support/record_m5_construction.py",
        "tests/support/m5_string_observations.cjs",
    ]]
    return {p.relative_to(ROOT).as_posix(): digest(p) for p in paths}


def main():
    if len(sys.argv) != 2:
        raise SystemExit("Usage: python tests/support/record_m5_construction.py FRESH_DIRECTORY")
    output = ROOT / sys.argv[1]
    output.mkdir(parents=True, exist_ok=False)
    before = sources()
    provenance = {
        "recordedUtc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "revision": run(["git", "--no-pager", "rev-parse", "HEAD"]),
        "dirtyStatus": run(["git", "--no-optional-locks", "status", "--short", "--untracked-files=all"]),
        "platform": platform.platform(),
        "nodeEnvironment": json.loads(run(["node", "-e", "console.log(JSON.stringify({cpu:require('node:os').cpus()[0].model,node:process.version,v8:process.versions.v8,arch:process.arch}))"])),
        "versions": {tool: run([tool, "--version"]) for tool in ["rustc", "cargo", "node", "python"]},
        "environment": {key: os.environ.get(key) for key in ["NODE_OPTIONS", "RUSTFLAGS", "CARGO_PROFILE_RELEASE_OPT_LEVEL"]},
        "sourceHashes": before,
        "commands": [],
        "mode": "deterministic-accounting-only",
        "nodeTimeoutSeconds": 120,
    }
    commands = [
        ("strings", ["cargo", "test", "--offline", "-p", "was-compiler", "--lib",
                     "codegen::string_observation_tests", "--", "--include-ignored", "--test-threads=1"]),
        ("arrays", ["cargo", "test", "--offline", "-p", "was-compiler", "--lib",
                    "codegen::resizable::construction_observations", "--", "--include-ignored", "--test-threads=1"]),
        ("accounting", ["cargo", "test", "--offline", "-p", "was-compiler", "--lib",
                        "codegen::string_construction_tests", "--", "--include-ignored", "--test-threads=1"]),
    ]
    for name, command in commands:
        print("Running:", " ".join(command), flush=True)
        entry = {"argv": command, "timeoutSeconds": 600}
        provenance["commands"].append(entry)
        try:
            result = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=600)
        except subprocess.TimeoutExpired as error:
            (output / f"{name}.log").write_bytes((error.stdout or b"") + (error.stderr or b""))
            entry["timedOut"] = True
            (output / "environment.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
            raise
        (output / f"{name}.log").write_bytes(result.stdout + result.stderr)
        entry["exitCode"] = result.returncode
        (output / "environment.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
        if result.returncode:
            sys.stderr.buffer.write(result.stdout + result.stderr)
            result.check_returncode()
        expected = {"strings": 6, "arrays": 3, "accounting": 2}[name]
        assert f"{expected} passed; 0 failed; 0 ignored" in result.stdout.decode("utf-8"), "Missing/nonzero focused test count"
    fixture = ROOT / "target/m5-construction"
    data = json.loads((fixture / "accounting.json").read_bytes())
    assert data["schema"] == "m5-construction-79-v1"
    assert len(data["rows"]) == 103 and len(data["baselines"]) == 34
    shutil.copyfile(fixture / "accounting.json", output / "accounting.json")
    shutil.copyfile(ROOT / "tests/support/m5_construction_corpus.was", output / "workload.was")
    provenance["artifacts"] = {name: {"bytes": (fixture / name).stat().st_size,
                                      "sha256": digest(fixture / name)}
                               for name in ["ordinary.wasm", "observed.wasm"]}
    assert before == sources(), "Executable inputs changed during recording; discard evidence"
    provenance["stableSourceHashesAfterRecording"] = True
    (output / "environment.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
    print("Recorded:", output, flush=True)


if __name__ == "__main__":
    main()
