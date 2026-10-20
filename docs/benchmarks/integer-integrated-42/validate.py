"""Run finite, offline #42 correctness gates after measurement; retain every log.

Run from the repository root. Refuses to overwrite a previous validation record.
Stops at the first failed gate and records its exit status or timeout accurately.
"""
import datetime
import json
import os
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
COMMANDS = [
    ("check", ["cargo", "check", "--offline", "--all-targets"]),
    ("focused", ["cargo", "test", "--offline", "-p", "was-compiler", "--test", "integer_specialization", "--test", "integer_locals", "--", "--include-ignored"]),
    ("native", ["cargo", "test", "--offline"]),
    ("clippy", ["cargo", "clippy", "--offline", "--all-targets", "--", "-D", "warnings"]),
    ("core-wasm", ["cargo", "build", "--offline", "-p", "was-compiler", "--lib", "--target", "wasm32-unknown-unknown"]),
    ("browser-build", ["wasm-pack", "build", "crates/browser", "--target", "web", "--out-dir", "../../pkg", "--out-name", "was_compiler", "--mode", "no-install", "--no-opt", "--offline"]),
    ("browser-smoke", ["node", "tests/browser.cjs", "pkg"]),
    ("workspace-ignored", ["cargo", "test", "--offline", "--workspace", "--", "--include-ignored"]),
    ("browser-clippy", ["cargo", "clippy", "--offline", "-p", "was-compiler-browser", "--target", "wasm32-unknown-unknown", "--lib", "--", "-D", "warnings"]),
]


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def main():
    if Path.cwd().resolve() != ROOT:
        raise ValueError("Run from repository root")
    out = HERE / "validation-1"
    out.mkdir(exist_ok=False)
    env = dict(os.environ, CARGO_NET_OFFLINE="true")
    records = []
    passed = False
    try:
        for name, command in COMMANDS:
            record = {"name": name, "command": command, "utc_started": utc(),
                      "timeout_seconds": 600, "exit_code": None, "timed_out": False,
                      "output": name + ".txt"}
            records.append(record)
            print("COMMAND:", " ".join(command), flush=True)
            with (out / record["output"]).open("xb") as log:
                try:
                    result = subprocess.run(command, cwd=ROOT, env=env, stdout=log,
                                            stderr=subprocess.STDOUT, timeout=600, check=False)
                    record["exit_code"] = result.returncode
                except subprocess.TimeoutExpired:
                    record["timed_out"] = True
                finally:
                    record["utc_finished"] = utc()
            print(name, "exit:", record["exit_code"], "timeout:", record["timed_out"], flush=True)
            if record["exit_code"] != 0 or record["timed_out"]:
                raise RuntimeError("Failed gate: " + name + "; see retained log")
        passed = True
    finally:
        (out / "commands.json").write_text(json.dumps({"utc_finished": utc(),
            "cwd": str(ROOT), "offline": True, "all_gates_passed": passed,
            "commands": records}, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
