"""Record #80 ordinary M5 timing sequentially; cached tools only, no counters."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import time
import zipfile

ROOT = Path(__file__).resolve().parents[2]
FAMILIES = ["concat_empty", "concat_separated", "join_empty", "join_separated",
            "replace_no_match", "replace_expand", "replace_shrink"]
NATIVE_CASES = [f"{family}_{size}" for family in FAMILIES for size in [1, 16, 64]]


def fresh_directory(path):
    path.mkdir(parents=True, exist_ok=False)


def require_stable(before, after):
    if before != after:
        changed = sorted(key for key in before.keys() | after.keys() if before.get(key) != after.get(key))
        raise RuntimeError(f"Executable inputs changed; discard recording: {changed}")


def cargo_executable(text, name, kind):
    artifacts = [json.loads(line) for line in text.splitlines() if line.startswith("{")]
    executables = [a["executable"] for a in artifacts if a.get("reason") == "compiler-artifact"
                   and a.get("target", {}).get("name") == name
                   and kind in a.get("target", {}).get("kind", []) and a.get("executable")]
    if len(executables) != 1:
        raise ValueError(f"Expected exactly one built {kind} executable for {name}")
    return Path(executables[0])


def native_schedule():
    pairs = [(boundary, case) for boundary in ["source_compile", "typed_compile"] for case in NATIVE_CASES]
    return [(boundary, case, repetition, sample)
            for repetition in range(1, 4)
            for boundary, case in (pairs if repetition % 2 else list(reversed(pairs)))
            for sample in range(1, 6)]


def parse_native(text):
    records = []
    for line in text.splitlines():
        fields = line.split("\t")
        if len(fields) != 7 or fields[0] != "NATIVE_SAMPLE":
            raise ValueError(f"Unexpected native sample line: {line!r}")
        _, boundary, case, repetition, sample, iterations, elapsed = fields
        record = dict(boundary=boundary, case=case, repetition=int(repetition),
                      sample=int(sample), iterations=int(iterations), elapsedNs=int(elapsed))
        if record["iterations"] != 10 or record["elapsedNs"] <= 0:
            raise ValueError("Native sample must contain ten measured compilations and positive elapsed time")
        records.append(record)
    schedule = [(r["boundary"], r["case"], r["repetition"], r["sample"]) for r in records]
    if schedule != native_schedule():
        raise ValueError("Native recording is incomplete, duplicated or reordered")
    return records


def validate_runtime(data):
    if data.get("mode") != "timing" or data.get("checkedCases") != 123:
        raise ValueError("Expected 123 independently checked runtime cases")
    cases = data.get("cases", [])
    names = [case["name"] for case in cases]
    if len(names) != 123 or len(set(names)) != 123:
        raise ValueError("Incomplete or duplicated runtime corpus")
    schedule = [(name, repetition, sample) for repetition in range(1, 4)
                for name in (names if repetition % 2 else list(reversed(names))) for sample in range(1, 6)]
    samples = data.get("samples", [])
    actual = [(s["case"], s["repetition"], s["sample"]) for s in samples]
    if actual != schedule or any(s["iterations"] != 8 or s["elapsedNs"] <= 0 for s in samples):
        raise ValueError("Runtime recording is incomplete, duplicated, reordered or has invalid durations")


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_hashes():
    # Include untracked executable sources too: concurrent work invalidates a run.
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    for directory in ["crates", "tests", "runtime"]:
        paths.extend(p for p in (ROOT / directory).rglob("*") if p.is_file()
                     and p.suffix in [".rs", ".toml", ".lock", ".was", ".cjs", ".py"]
                     and "__pycache__" not in p.parts)
    return {p.relative_to(ROOT).as_posix(): sha256(p) for p in sorted(set(paths))}


def summarize(samples, native=False):
    groups = {}
    for sample in samples:
        key = (sample["boundary"] + "/" if native else "") + sample["case"]
        groups.setdefault(key, []).append(sample["elapsedNs"] / sample["iterations"])
    return {key: {"samples": len(values), "minNsPerOperation": min(values),
                  "medianNsPerOperation": statistics.median(values), "maxNsPerOperation": max(values)}
            for key, values in groups.items()}


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: python tests/support/record_m5_strings.py FRESH_DIRECTORY")
    output = (ROOT / sys.argv[1]).resolve()
    fresh_directory(output)
    native = output / "native-corpus"
    native.mkdir()
    before = source_hashes()
    provenance = {"recordedUtc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  "sourceHashes": before, "commands": [], "complete": False,
                  "platform": platform.platform(), "machine": platform.machine(),
                  "environment": {key: value for key, value in sorted(os.environ.items())
                                  if key.startswith(("CARGO_", "RUST", "NODE_", "DIVAN_"))
                                  or key in ["CC", "CXX", "CI", "PROCESSOR_IDENTIFIER", "NUMBER_OF_PROCESSORS"]}}

    def save():
        (output / "environment.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")

    def run(label, command, bound):
        print("Running:", " ".join(map(str, command)), flush=True)
        start = time.monotonic()
        entry = {"label": label, "argv": list(map(str, command)), "timeoutSeconds": bound}
        provenance["commands"].append(entry)
        save()
        try:
            result = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=bound)
        except subprocess.TimeoutExpired as error:
            (output / f"{label}.stdout.log").write_bytes(error.stdout or b"")
            (output / f"{label}.stderr.log").write_bytes(error.stderr or b"")
            entry.update(timedOut=True, durationSeconds=time.monotonic() - start)
            save()
            raise
        (output / f"{label}.stdout.log").write_bytes(result.stdout)
        (output / f"{label}.stderr.log").write_bytes(result.stderr)
        entry.update(exitCode=result.returncode, durationSeconds=time.monotonic() - start)
        save()
        if result.returncode:
            sys.stderr.buffer.write(result.stderr)
            result.check_returncode()
        return result.stdout.decode("utf-8", errors="strict").strip()

    try:
        provenance["revision"] = run("revision", ["git", "--no-pager", "rev-parse", "HEAD"], 30)
        provenance["dirtyStatus"] = run("status", ["git", "--no-optional-locks", "status", "--short", "--untracked-files=all"], 30)
        provenance["versions"] = {tool: run(f"version-{tool}", [tool, "--version"], 30)
                                  for tool in ["rustc", "cargo", "node", "python"]}
        checked = run("runtime-correctness", ["cargo", "test", "--offline", "-p", "was-compiler", "--test", "m5_measurements", "--", "--include-ignored"], 600)
        if "2 passed; 0 failed; 0 ignored" not in checked:
            raise RuntimeError("Runtime correctness command did not execute both tests")
        checked = run("native-correctness", ["cargo", "test", "--offline", "-p", "was-compiler", "--test", "m5_benchmarks", "--", "--include-ignored"], 600)
        if "5 passed; 0 failed; 0 ignored" not in checked:
            raise RuntimeError("Native correctness command did not execute all five tests")
        build = run("build-benchmark", ["cargo", "bench", "--offline", "-p", "was-compiler", "--bench", "m5_strings", "--no-run", "--message-format=json"], 600)
        bench = cargo_executable(build, "m5_strings", "bench")
        build = run("build-cli", ["cargo", "build", "--offline", "--release", "-p", "was-compiler-cli", "--message-format=json"], 600)
        # Cargo's artifact path respects target triples, target directories and profiles;
        # guessing target/release can silently execute a stale default-target compiler.
        cli = cargo_executable(build, "was-compiler", "bin")
        run("compile-runtime", [cli, "tests/support/m5_string_timing.was", "--emit", "wasm", "-o", output / "runtime.wasm"], 30)
        shutil.copyfile(ROOT / "tests/support/m5_string_timing.was", output / "runtime.was")
        smoke = run("benchmark-smoke", [bench, "--test"], 600)
        if any(sum(line.endswith(case) for line in smoke.splitlines()) != 2 for case in NATIVE_CASES):
            raise RuntimeError("Divan smoke did not execute all 42 cases")
        require_stable(before, source_hashes())
        binaries = {str(path): sha256(path) for path in [bench, cli]}
        runtime_hash = sha256(output / "runtime.wasm")
        # No builds, correctness subprocesses or competing agent measurements are
        # launched while either measured process runs. The caller must ensure idle conditions.
        raw_native = run("native-timing", [bench, "--record-samples", native], 600)
        native_samples = parse_native(raw_native)
        run("runtime-timing", ["node", "tests/support/m5_string_timing.cjs", "timing", output / "runtime.wasm"], 120)
        runtime = json.loads((output / "runtime-timing.stdout.log").read_bytes())
        validate_runtime(runtime)
        if runtime["module"]["sha256"] != runtime_hash:
            raise RuntimeError("Runtime module mismatch")
        require_stable(before, source_hashes())
        require_stable(binaries, {str(path): sha256(path) for path in [bench, cli]})
        if sha256(output / "runtime.wasm") != runtime_hash:
            raise RuntimeError("Runtime module changed during recording")
        (output / "native.json").write_text(json.dumps({"mode": "native", "numericPreset": "None",
            "timer": "std::time::Instant", "repetitions": 3, "samplesPerCase": 5,
            "iterations": 10, "warmupCompilationsPerCasePerRepetition": 2,
            "order": "forward/reverse/forward", "samples": native_samples,
            "summary": summarize(native_samples, native=True)}, indent=2) + "\n", encoding="utf-8")
        (output / "runtime.json").write_text(json.dumps(runtime, indent=2) + "\n", encoding="utf-8")
        (output / "summary.json").write_text(json.dumps({"native": summarize(native_samples, True),
            "runtime": summarize(runtime["samples"])}, indent=2) + "\n", encoding="utf-8")
        # Preserve executable inputs as a compact snapshot, independent of future checkout changes.
        with zipfile.ZipFile(output / "inputs.zip", "x", compression=zipfile.ZIP_DEFLATED) as snapshot:
            for name in before:
                snapshot.write(ROOT / name, name)
        with zipfile.ZipFile(output / "inputs.zip") as snapshot:
            require_stable(before, {name: hashlib.sha256(snapshot.read(name)).hexdigest() for name in snapshot.namelist()})
        require_stable(before, source_hashes())
        provenance.update(complete=True, stableSourceHashesAfterRecording=True, executableHashes=binaries,
                          runtimeEnvironment=runtime["environment"],
                          artifacts={p.relative_to(output).as_posix(): {"bytes": p.stat().st_size, "sha256": sha256(p)}
                                     for p in sorted(output.rglob("*")) if p.is_file() and p.name != "environment.json"})
    finally:
        save()
    print("Recorded:", output, flush=True)


if __name__ == "__main__":
    main()
