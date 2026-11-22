"""Collect #88 with prebuilt executables, or render its report from raw logs.

Run from the repository root; see README.md for commands and timing boundaries.
"""

import importlib.util
import os
from pathlib import Path
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location(
    "matched", HERE.parent / "integer-add-sub-39/record_matched.py"
)
matched = importlib.util.module_from_spec(spec)
spec.loader.exec_module(matched)
base = matched.baseline
LEVELS = matched.LEVELS
ROUNDS = ((1, LEVELS), (2, LEVELS[::-1]))


def collect(directory, test, bench):
    executables = [Path(p).resolve(strict=True) for p in (test, bench)]
    out = base.create_output(base.ROOT, directory)
    env = dict(os.environ, CARGO_NET_OFFLINE="true", INTEGER_BASELINE_LEVEL="None")
    base.write_json(out / "environment.json", {
        "argv": sys.argv, "python": sys.version,
        "cpu_affinity": sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
        "environment": {key: env.get(key) for key in (
            "CARGO_NET_OFFLINE", "NODE_OPTIONS", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER"
        )},
    })

    def snapshot():
        return {**matched.hashes(), "collection": {
            str(p.relative_to(base.ROOT)): base.hashlib.sha256(p.read_bytes()).hexdigest()
            for p in (Path(__file__).resolve(), *executables)
        }}

    before = snapshot()
    base.write_json(out / "inputs.json", before)
    for name, command in (
        ("git-head", ["git", "--no-pager", "rev-parse", "HEAD"]),
        ("git-status", ["git", "--no-pager", "--no-optional-locks", "status", "--short"]),
        ("rustc", ["rustc", "-Vv"]),
        ("cargo", ["cargo", "--version"]),
    ):
        base.record(out, name, command, env)
    test_run = [str(executables[0]), "--exact", "--nocapture", "--test-threads=1", "--ignored"]
    base.record(out, "verify", [*test_run, "execute_integer_workloads"], env)
    guest(out, "verify", "verify", base.LEVELS)
    native_run = [str(executables[1]), "--bench", "--sample-count", "100", "--sample-size", "1",
                  "--timer", "os", "--color", "never"]
    for i, order in ROUNDS:
        for stage, command in (("compiler", native_run),
                               ("guest", [*test_run, "record_integer_guest_timing"])):
            for level in order:
                base.record(out, f"{stage}-{i}-{level}", command,
                            dict(env, INTEGER_BASELINE_LEVEL=level))
    after = snapshot()
    base.require(before == after, "Source, harness or executable changed during measurement")
    base.write_json(out / "stability.json", {"inputs_unchanged": True})


def text(out, name):
    return (out / f"{name}.txt").read_text(encoding="utf-8")


def guest(out, name, mode, levels):
    return base.parse_records(text(out, name), mode, levels)


def report(out):
    verify, machine = guest(out, "verify", "verify", base.LEVELS)
    runs = {}
    for i, order in ROUNDS:
        for level in order:
            records, runtime = guest(out, f"guest-{i}-{level}", "timing", (level,))
            base.require({**runtime, "utc": machine["utc"], "mode": machine["mode"]} == machine,
                         "Machine/runtime changed")
            native = text(out, f"compiler-{i}-{level}")
            base.require(native.startswith(f"level={level};"), f"Wrong compiler preset: {level}")
            compiler, sizes = matched.compiler_records(native)
            for name in base.NAMES:
                row = verify[level, name]
                base.require(sizes[name] == [row["sourceBytes"], row["moduleBytes"]],
                             f"Compiler/guest size mismatch: {level}/{name}")
            runs[i, level] = (compiler, records)
    matched.identities(verify, [(level, rows) for (_, level), (_, rows) in runs.items()])
    print("# Per-workload measurements (#88)\n")
    print("Each timing cell lists repetitions 1 / 2; each pair is None → Basic (percent change).")
    print("Compiler medians are µs/compilation; guest medians are ns/main call.")
    print("See the raw logs for Divan min/max/mean and all guest sample values.")
    for title, names in (("M7-focused", [n for n in base.NAMES if n != "loop_local"]),
                         ("M6/M7 composition", ["loop_local"])):
        print(f"\n## {title}\n")
        print("| Workload | Wasm bytes (delta) | Source | Typed | Guest |")
        print("| --- | ---: | ---: | ---: | ---: |")
        for name in names:
            n, b = (verify[level, name]["moduleBytes"] for level in LEVELS)
            cells = [name, f"{n} → {b} ({b - n:+})"]
            for stage in ("source_compile", "typed_compile", "guest"):
                if stage == "guest" and name == "trap_order":
                    cells.append("verify-only")
                    continue
                pairs = []
                for i, _ in ROUNDS:
                    a, b = (runs[i, level][1][level, name]["guestTiming"]["median"]
                            if stage == "guest" else runs[i, level][0][stage, name]["median_us"]
                            for level in LEVELS)
                    pairs.append(f"{a:.6g} → {b:.6g} ({100 * (b / a - 1):+.2f}%)")
                cells.append(" / ".join(pairs))
            print("| " + " | ".join(cells) + " |")


if __name__ == "__main__":
    if len(sys.argv) == 5 and sys.argv[1] == "collect":
        collect(*sys.argv[2:])
    elif len(sys.argv) == 3 and sys.argv[1] == "report":
        report(Path(sys.argv[2]))
    else:
        sys.exit("usage: record.py collect NEW_DIR TEST BENCH | record.py report RUN_DIR")
