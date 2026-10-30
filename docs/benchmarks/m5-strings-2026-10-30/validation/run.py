import json
from pathlib import Path
import subprocess
import time

output = Path("target/m5-80-validation")
commands = [
    ("default-tests", ["cargo", "test", "--offline"]),
    ("clippy", ["cargo", "clippy", "--offline", "--all-targets", "--", "-D", "warnings"]),
    ("existing-benchmark-smoke", ["cargo", "bench", "--offline", "-p", "was-compiler", "--bench", "compiler", "--", "--test"]),
    ("m5-divan", ["cargo", "bench", "--offline", "-p", "was-compiler", "--bench", "m5_strings", "--", "--sample-count", "100", "--sample-size", "1", "--timer", "os", "--color", "never"]),
]
records = []
for name, command in commands:
    print("Running:", " ".join(command), flush=True)
    start = time.monotonic()
    result = subprocess.run(command, capture_output=True, timeout=600)
    (output / f"{name}.stdout.log").write_bytes(result.stdout)
    (output / f"{name}.stderr.log").write_bytes(result.stderr)
    records.append(dict(name=name, argv=command, timeoutSeconds=600, exitCode=result.returncode, durationSeconds=time.monotonic()-start))
    (output / "commands.json").write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    print(name, result.returncode, round(records[-1]["durationSeconds"], 2), flush=True)
    if result.returncode:
        print(result.stdout.decode(errors="replace"))
        print(result.stderr.decode(errors="replace"))
        result.check_returncode()
