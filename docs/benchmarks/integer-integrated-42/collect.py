"""Collect #42 evidence using the unchanged #39 matched recorder.

Only its historical destination assertion is redirected. The raw recorder's #39
narrative is superseded by README.md; timing, corpus, parsing and gates are intact.
Run from the repository root. Existing evidence is never overwritten.
"""
import importlib.util
from pathlib import Path
import sys
import traceback

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
RECORDER = ROOT / "docs/benchmarks/integer-add-sub-39/record_matched.py"
OUTPUT = HERE / "collection-1"


def main():
    spec = importlib.util.spec_from_file_location("matched_39", RECORDER)
    recorder = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(recorder)
    require_original = recorder.baseline.require
    argument = OUTPUT.relative_to(ROOT).as_posix()
    redirects = []

    def require(condition, message):
        if message == "Output must be a NEW child of docs/benchmarks/integer-add-sub-39":
            require_original(Path.cwd().resolve() == ROOT, "Run from repository root")
            require_original(OUTPUT.parent == HERE and OUTPUT.name == "collection-1",
                             "Only the owned #42 collection-1 destination is allowed")
            require_original(not OUTPUT.exists(), "Collection must not already exist")
            redirects.append({"original_condition": condition, "original_message": message,
                              "allowed_destination": argument})
            return
        require_original(condition, message)

    metadata_path = HERE / "launcher.json"
    require_original(not metadata_path.exists(), "Launcher metadata must be new")
    metadata = {
        "utc_started": recorder.baseline.utc(),
        "recorder": RECORDER.relative_to(ROOT).as_posix(),
        "recorder_sha256": recorder.baseline.hashlib.sha256(RECORDER.read_bytes()).hexdigest(),
        "launcher_sha256": recorder.baseline.hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "output": argument,
        "adaptation": "Redirect only the #39 output-location assertion; historical recorder unchanged",
        "original_argv": sys.argv[:],
        "isolation": "No agent-launched concurrent builds, tests or benchmarks during this collection; OS/background load, thermals and JIT tiering uncontrolled",
        "redirects": redirects,
        "completed": False,
    }
    try:
        for label, command in (
            ("matched-self-test", [sys.executable, str(RECORDER), "--self-test"]),
            ("baseline-self-test", [sys.executable, "tests/support/record_integer_baseline.py", "--self-test"]),
        ):
            recorder.baseline.record(HERE, label, command,
                                     dict(recorder.baseline.os.environ, CARGO_NET_OFFLINE="true"))
        recorder.baseline.require = require
        recorder.main(argument)
        metadata["completed"] = True
    except BaseException:
        metadata["error"] = traceback.format_exc()
        raise
    finally:
        metadata["utc_finished"] = recorder.baseline.utc()
        recorder.baseline.write_json(metadata_path, metadata)


if __name__ == "__main__":
    main()
