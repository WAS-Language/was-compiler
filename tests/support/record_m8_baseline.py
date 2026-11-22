"""Build and record the #91 pre-folding baseline offline from the repo root."""

import argparse
import csv
from datetime import datetime, timezone
import hashlib
from itertools import product
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
HOST = Path("tests/support/m8_scalar_timing.cjs")
WORKLOADS = (
    "fraction_closed", "fraction_open", "wide_closed", "wide_open", "bool_closed",
    "bool_open", "negative_zero", "subnormal_tie", "nonfinite_root", "division_root",
    "integer_local", "range_direct", "array_growth",
)
GUEST_WORKLOADS = tuple(name for name in WORKLOADS if name not in {
    "negative_zero", "subnormal_tie", "nonfinite_root",
})
PRESETS = ("None", "Basic", "Standard", "Aggressive")
ORDERS = ("forward", "reverse")


def utc_now():
    return datetime.now(timezone.utc).isoformat()


def write_json(path, data):
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def input_hashes(node):
    inputs = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / HOST, Path(__file__).resolve()]
    inputs.extend(ROOT.glob("crates/*/Cargo.toml"))
    inputs.extend(ROOT / name for name in (
        ".cargo/config", ".cargo/config.toml", "rust-toolchain", "rust-toolchain.toml",
        "crates/compiler/build.rs",
    ) if (ROOT / name).is_file())
    for tree in ("crates/compiler/src", "crates/compiler/benches", "tests/support"):
        inputs.extend(path for path in (ROOT / tree).rglob("*")
                      if path.is_file() and path.suffix in {".rs", ".was", ".cjs", ".py", ".toml"})
    return {"sources": {path.relative_to(ROOT).as_posix(): sha256(path)
                        for path in sorted(set(inputs))},
            "node": {"path": str(node), "sha256": sha256(node)}}


def machine():
    cpuinfo = Path("/proc/cpuinfo")
    model = next((line.partition(":")[2].strip()
                  for line in cpuinfo.read_text().splitlines()
                  if line.startswith("model name")), None) if cpuinfo.is_file() else None
    power_paths = sorted(Path("/sys/devices/system/cpu/cpufreq").glob("policy*/scaling_governor"))
    power_paths.append(Path("/sys/firmware/acpi/platform_profile"))
    return {
        "os": platform.platform(), "architecture": platform.machine(),
        "cpu": model or platform.processor() or None, "logical_cpus": os.cpu_count(),
        "cpu_affinity": sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
        "power_settings": {str(path): path.read_text().strip() for path in power_paths if path.is_file()},
        "conditions": "Background activity and thermal state are not controlled by this recorder.",
    }


def csv_rows(path, header):
    fields = header.split(",")
    with path.open(newline="", encoding="utf-8") as stream:
        reader = csv.reader(stream, strict=True)
        if next(reader, None) != fields:
            raise ValueError(f"{path}: expected header {header}")
        for row in reader:
            if len(row) != len(fields):
                raise ValueError(f"{path}:{reader.line_num}: wrong field count")
            yield row


def checked_rows(path, header, key_columns, expected):
    rows = {}
    for row in csv_rows(path, header):
        key = tuple(row[:key_columns])
        if key not in expected or key in rows:
            raise ValueError(f"{path}: unexpected or duplicate row {key}")
        rows[key] = row[key_columns:]
    if rows.keys() != expected:
        raise ValueError(f"{path}: missing rows {sorted(expected - rows.keys())}")
    return rows


def artifacts(directory):
    sizes = checked_rows(directory / "sizes.csv", "workload,preset,source_bytes,module_bytes",
                         2, set(product(WORKLOADS, PRESETS)))
    manifest = {}
    for name in WORKLOADS:
        source = f"{name}.was"
        modules = [f"{name}-{preset}.wasm" for preset in PRESETS]
        for filename in (source, *modules):
            path = directory / filename
            manifest[filename] = {"bytes": path.stat().st_size, "sha256": sha256(path)}
        for preset, module in zip(PRESETS, modules):
            actual = [manifest[source]["bytes"], manifest[module]["bytes"]]
            if min(actual) <= 0 or actual != list(map(int, sizes[name, preset])):
                raise ValueError(f"{directory}: size mismatch for {name}/{preset}")
        if len({manifest[module]["sha256"] for module in modules[1:]}) != 1:
            raise ValueError(f"{directory}: enabled presets differ for {name}")
    return manifest


def growth(directory):
    path = directory / "growth.csv"
    expected = set(product(WORKLOADS, PRESETS, ("0", "1", "2048", "6144", "8192")))
    rows = checked_rows(path, "workload,preset,calls,memory_bytes", 3, expected)
    observed = {}
    for key, (value,) in rows.items():
        size = None if value == "none" else int(value)
        if size is not None and (size < 0 or size % 65536):
            raise ValueError(f"{path}: invalid memory byte length for {key}: {value}")
        observed[key] = size
    return observed


def timing(path, schedule, calls_by_boundary, sample_count):
    header = "workload,preset,boundary,sample,calls,elapsed_ns"
    rows = iter(csv_rows(path, header))
    for key in schedule:
        calls = calls_by_boundary[key[2]]
        values = []
        for sample in range(1, sample_count + 1):
            expected = (*key, str(sample), str(calls))
            row = next(rows, None)
            if row is None or tuple(row[:-1]) != expected:
                raise ValueError(f"{path}: expected row {expected}, got {row}")
            elapsed = int(row[-1])
            if elapsed <= 0:
                raise ValueError(f"{path}: nonpositive elapsed ns for {expected}")
            values.append(elapsed / calls)
        yield (*key, sample_count, calls, statistics.median(values), min(values), max(values))
    if next(rows, None) is not None:
        raise ValueError(f"{path}: unexpected trailing measurements")


def timing_summary(directory, order):
    native_calls = {"source": 128, "typed": 128}
    guest_calls = {"instantiation": 1, "first_call": 1, "warmed": 4096}
    native = list(product(WORKLOADS, PRESETS[:2], native_calls))
    guest_cases = list(product(GUEST_WORKLOADS, PRESETS[:2]))
    if order == "reverse":
        native.reverse()
        guest_cases.reverse()
    guest = [(*case, boundary) for boundary in guest_calls for case in guest_cases]
    yield from timing(directory / "native.csv", native, native_calls, 7)
    yield from timing(directory / "guest.csv", guest, guest_calls, 20)


def collect(output, node):
    env = dict(os.environ, CARGO_NET_OFFLINE="true", RUSTUP_AUTO_INSTALL="0")
    provenance = {
        "argv": sys.argv, "cwd": str(ROOT), "started_utc": utc_now(), "python": sys.version,
        "python_executable": sys.executable, "machine_before": machine(),
        "environment": {key: value for key, value in env.items()
                        if key.startswith(("CARGO_PROFILE_", "CARGO_BUILD_")) or key in {
                            "CARGO_HOME", "CARGO_TARGET_DIR", "CARGO_NET_OFFLINE",
                            "CARGO_ENCODED_RUSTFLAGS", "RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER",
                            "RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN", "RUSTUP_AUTO_INSTALL",
                            "CC", "CXX", "CFLAGS", "LDFLAGS", "NODE_OPTIONS", "NODE_PATH", "PATH",
                        }},
        "commands": [], "complete": False,
    }

    def save():
        write_json(output / "environment.json", provenance)

    def run(label, argv, deadline):
        entry = {"label": label, "argv": list(map(str, argv)), "timeout_seconds": deadline,
                 "started_utc": utc_now()}
        provenance["commands"].append(entry)
        save()
        stdout = output / f"{label}.stdout.log"
        stderr = output / f"{label}.stderr.log"
        try:
            with stdout.open("wb") as out, stderr.open("wb") as err:
                result = subprocess.run(entry["argv"], cwd=ROOT, env=env, stdout=out, stderr=err,
                                        stdin=subprocess.DEVNULL, timeout=deadline, check=False)
            entry["exit_code"] = result.returncode
            if result.returncode:
                raise ValueError(f"{label} exited {result.returncode}; see {stderr}")
        except subprocess.TimeoutExpired as error:
            entry["timed_out"] = True
            raise ValueError(f"{label} exceeded {deadline}s; see {stdout} and {stderr}") from error
        finally:
            entry["finished_utc"] = utc_now()
            save()
        return stdout

    def git_snapshot(phase):
        git = ["git", "--no-pager", "--no-optional-locks"]
        return {label: sha256(run(f"{phase}-{label}", git + args, 30)) for label, args in (
            ("git-head", ["rev-parse", "HEAD"]),
            ("git-status", ["status", "--short", "--untracked-files=all"]),
            ("git-diff-unstaged", ["diff", "--no-ext-diff", "--no-textconv", "--binary"]),
            ("git-diff-staged", ["diff", "--cached", "--no-ext-diff", "--no-textconv", "--binary"]),
        )}

    try:
        before = input_hashes(node)
        write_json(output / "inputs-prebuild.json", before)
        git_before = git_snapshot("before")
        run("rustc", ["rustc", "-vV"], 30)
        run("cargo", ["cargo", "--version"], 30)
        versions = json.loads(run("node", [node, "-p", "JSON.stringify(process.versions)"], 30)
                              .read_text(encoding="utf-8"))
        if int(versions["node"].split(".")[0]) < 24:
            raise ValueError("Node.js 24 or newer is required")

        build_log = run("bench-build", ["cargo", "bench", "--offline", "-p", "was-compiler",
                                      "--bench", "m8_scalars", "--no-run", "--message-format=json",
                                      "--target-dir", output / "build"], 600)
        records = map(json.loads, build_log.read_text(encoding="utf-8").splitlines())
        artifacts_built = [record for record in records if record.get("reason") == "compiler-artifact"
                           and record["target"]["name"] == "m8_scalars" and record.get("executable")]
        if len(artifacts_built) != 1 or artifacts_built[0]["fresh"]:
            raise ValueError("expected exactly one newly built m8_scalars executable")
        artifact = artifacts_built[0]
        bench = Path(artifact["executable"]).resolve(strict=True)
        write_json(output / "cargo-artifact.json", artifact)
        if input_hashes(node) != before:
            raise ValueError("Measured inputs changed during bench build")
        before["bench"] = {"path": str(bench), "sha256": sha256(bench)}
        write_json(output / "inputs-before.json", before)
        run("bench-smoke", [bench, "--test"], 120)

        sessions = {}
        summary = []
        for order in ORDERS:
            directory = output / order
            run(f"{order}-prepare", [bench, "--prepare", directory], 120)
            manifest = artifacts(directory)
            write_json(directory / "artifacts.json", manifest)
            run(f"{order}-verify", [node, HOST, directory, "verify"], 120)
            observations = growth(directory)
            if order == "reverse" and sessions["forward"] != (manifest, observations):
                raise ValueError("Forward and reverse artifact identities, sizes or growth differ")
            sessions[order] = manifest, observations
            run(f"{order}-native", [bench, "--record-samples", directory, order], 600)
            run(f"{order}-guest", [node, HOST, directory, order], 120)
            summary.extend((order, *row) for row in timing_summary(directory, order))

        after = input_hashes(node)
        after["bench"] = {"path": str(bench), "sha256": sha256(bench)}
        write_json(output / "inputs-after.json", after)
        git_after = git_snapshot("after")
        if before != after or any(git_before[key] != git_after[key] for key in git_before
                                 if key != "git-status"):
            raise ValueError("Measured inputs, bench executable or tracked Git state changed; discard timing")
        for order, recorded in sessions.items():
            if (artifacts(output / order), growth(output / order)) != recorded:
                raise ValueError(f"{order}: prepared artifacts or oracle growth changed during collection")

        with (output / "summary.csv").open("x", newline="", encoding="utf-8") as stream:
            writer = csv.writer(stream)
            writer.writerow(("session", "workload", "preset", "boundary", "samples", "calls_per_sample",
                             "median_ns_per_operation", "min_ns_per_operation", "max_ns_per_operation"))
            writer.writerows(summary)
        provenance["complete"] = True
    finally:
        provenance.update(finished_utc=utc_now(), machine_after=machine())
        save()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new output directory (prefer target/)")

    args = parser.parse_args()
    if Path.cwd().resolve() != ROOT:
        parser.error("run from the repository root")
    try:

        node = shutil.which("node")
        if not node or not (ROOT / HOST).is_file():
            raise ValueError(f"Node.js and {HOST} are required")
        output = args.output.resolve()
        output.mkdir(parents=True, exist_ok=False)
        collect(output, Path(node).resolve(strict=True))
    except (OSError, ValueError, csv.Error) as error:
        parser.exit(1, f"M8 baseline: {error}\n")


if __name__ == "__main__":
    main()
