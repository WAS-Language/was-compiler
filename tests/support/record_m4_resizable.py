"""Record #71 M4 evidence sequentially on stable sources using cached tools only."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def run(command):
    return subprocess.run(command, cwd=ROOT, check=True, capture_output=True,
                          timeout=30).stdout.decode("utf-8", errors="replace").strip()


def source_hashes():
    paths = sorted((ROOT / "crates/compiler/src").rglob("*.rs"))
    paths += [ROOT / name for name in [
        "Cargo.toml", "Cargo.lock", "crates/compiler/Cargo.toml",
        "tests/support/process.rs", "tests/support/m4_resizable_corpus.was",
        "tests/support/m4_resizable_measure.cjs", "tests/support/record_m4_resizable.py",
    ]]
    return {p.relative_to(ROOT).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in paths}


def main():
    output = ROOT / (sys.argv[1] if len(sys.argv) == 2 else "target/m4-recording")
    output.mkdir(parents=True, exist_ok=True)
    if any(output.glob("*.json")) or any(output.glob("*.log")):
        raise SystemExit("Refusing to overwrite existing evidence; choose a fresh directory")
    before = source_hashes()
    provenance = {
        "recordedUtc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "revision": run(["git", "--no-pager", "rev-parse", "HEAD"]),
        "dirtyStatus": run(["git", "--no-optional-locks", "status", "--short", "--untracked-files=all"]),
        "versions": {tool: run([tool, "--version"]) for tool in ["rustc", "cargo", "node", "python"]},
        "environment": {key: os.environ.get(key) for key in ["NODE_OPTIONS", "RUSTFLAGS", "CARGO_PROFILE_RELEASE_OPT_LEVEL"]},
        "sourceHashes": before,
        "commands": [],
    }
    commands = [
        ("runtime-correctness", ["cargo", "test", "--offline", "-p", "was-compiler",
                                 "--test", "resizable_arrays_runtime", "--", "--include-ignored"]),
        ("m3-correctness", ["cargo", "test", "--offline", "-p", "was-compiler", "--lib",
                           "codegen::allocator::measurement_tests", "--", "--include-ignored", "--test-threads=1"]),
        ("isolation", ["cargo", "test", "--offline", "-p", "was-compiler", "--lib",
                       "codegen::resizable::measurement_tests", "--", "--skip", "record_m4", "--test-threads=1"]),
    ]
    for mode, name in [("accounting", "accounting"), ("timing", "guest_timing")]:
        commands.append((mode, [
            "cargo", "test", "--offline", "--release", "-p", "was-compiler", "--lib",
            f"codegen::resizable::measurement_tests::record_m4_{name}",
            "--", "--ignored", "--exact", "--nocapture", "--test-threads=1",
        ]))
    for mode, command in commands:
        print("Running:", " ".join(command), flush=True)
        try:
            result = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=600)
        except subprocess.TimeoutExpired as error:
            (output / f"{mode}.log").write_bytes((error.stdout or b"") + (error.stderr or b""))
            provenance["commands"].append({"argv": command, "timeoutSeconds": 600})
            (output / "environment.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
            raise
        (output / f"{mode}.log").write_bytes(result.stdout + result.stderr)
        provenance["commands"].append({"argv": command, "exitCode": result.returncode})
        (output / "environment.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
        if result.returncode:
            sys.stderr.buffer.write(result.stdout + result.stderr)
            result.check_returncode()
        if mode in ["accounting", "timing"]:
            record = ROOT / f"target/m4-resizable/{mode}.json"
            data = json.loads(record.read_bytes())
            log = result.stdout.decode("utf-8", errors="replace")
            if "1 passed; 0 failed" not in log:
                raise RuntimeError("Exact recording command did not execute one passing test")
            assert data["mode"] == mode
            shutil.copyfile(record, output / f"{mode}.json")
    for name in ["ordinary.wasm", "observed.wasm"]:
        path = ROOT / "target/m4-resizable" / name
        provenance.setdefault("artifacts", {})[name] = {
            "bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }
    shutil.copyfile(ROOT / "tests/support/m4_resizable_corpus.was", output / "workload.was")
    accounting = json.loads((output / "accounting.json").read_bytes())
    timing = json.loads((output / "timing.json").read_bytes())
    assert accounting["modules"] == timing["modules"]
    assert accounting["source"] == timing["source"]
    assert accounting["harness"] == timing["harness"]
    assert before == source_hashes(), "Sources changed during measurement; discard recording"
    provenance["stableSourceHashesAfterRecording"] = True
    (output / "environment.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
    print("Recorded:", output, flush=True)


if __name__ == "__main__":
    main()
