"""Record private M3 evidence sequentially on stable sources; never fetch tools."""
import datetime
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def run(command):
    return subprocess.run(
        command, cwd=ROOT, check=True, capture_output=True, timeout=360
    ).stdout.decode("utf-8", errors="replace").strip()


def source_hashes():
    paths = sorted((ROOT / "crates/compiler/src").rglob("*.rs"))
    paths += [ROOT / name for name in [
        "Cargo.toml", "Cargo.lock", "crates/compiler/Cargo.toml",
        "tests/support/process.rs", "tests/support/m3_allocator_measure.cjs",
        "tests/support/record_m3_allocator.py",
    ]]
    return {p.relative_to(ROOT).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in paths}


def main():
    output = ROOT / (sys.argv[1] if len(sys.argv) == 2 else "target/m3-recording")
    output.mkdir(parents=True, exist_ok=True)
    before = source_hashes()
    provenance = {
        "recordedUtc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "revision": run(["git", "--no-pager", "rev-parse", "HEAD"]),
        "dirtyStatus": run(["git", "--no-optional-locks", "status", "--short", "--untracked-files=all"]),
        "versions": {tool: run([tool, "--version"]) for tool in ["rustc", "cargo", "node"]},
        "sourceHashes": before,
        "commands": [],
    }
    commands = [("correctness", [
        "cargo", "test", "--offline", "-p", "was-compiler", "--lib",
        "codegen::allocator", "--", "--include-ignored", "--test-threads=1",
    ])]
    for mode, name in [("accounting", "accounting"), ("search", "registry_search"), ("timing", "guest_timing")]:
        commands.append((mode, [
            "cargo", "test", "--offline", "--release", "-p", "was-compiler", "--lib",
            f"codegen::allocator::measurement_tests::record_m3_{name}",
            "--", "--ignored", "--exact", "--nocapture", "--test-threads=1",
        ]))
    for mode, command in commands:
        print("Running:", " ".join(command), flush=True)
        result = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=360)
        (output / f"{mode}.log").write_bytes(result.stdout + result.stderr)
        provenance["commands"].append({"argv": command, "exitCode": result.returncode})
        if result.returncode:
            sys.stderr.buffer.write(result.stdout + result.stderr)
            result.check_returncode()
        if mode != "correctness":
            shutil.copyfile(ROOT / f"target/m3-allocator/{mode}.json", output / f"{mode}.json")
    for name in ["workload.was", "ordinary.wasm", "prototype.wasm", "instrumented.wasm"]:
        path = ROOT / "target/m3-allocator" / name
        provenance.setdefault("artifacts", {})[name] = {
            "bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()
        }
        if name.endswith(".was"):
            shutil.copyfile(path, output / name)
    assert before == source_hashes(), "Sources changed during measurement; discard recording"
    provenance["stableSourceHashesAfterRecording"] = True
    (output / "environment.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
    print("Recorded:", output, flush=True)


if __name__ == "__main__":
    main()
