"""Run the unchanged #39 recorder in the exclusively owned #41 evidence directory.

Only the historical output-location assertion is redirected. All recording,
120-second deadlines, hashes, oracle checks, identities, parsing and summaries
execute from the original recorder. Its #39 narrative remains verbatim and is
superseded by this directory's README, not silently rewritten.
"""

import importlib.util
import json
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
    original_require = recorder.baseline.require
    old_message = "Output must be a NEW child of docs/benchmarks/integer-add-sub-39"
    argument = OUTPUT.relative_to(ROOT).as_posix()
    redirects = []

    def require(condition, message):
        if message == old_message:
            original_require(Path.cwd().resolve() == ROOT, "Run from repository root")
            original_require(OUTPUT.parent == HERE and OUTPUT.name == "collection-1",
                             "Only the owned #41 collection-1 destination is allowed")
            original_require(not OUTPUT.exists(), "Collection must not already exist")
            redirects.append({"original_condition": condition, "original_message": message,
                              "allowed_destination": argument})
            return
        original_require(condition, message)

    metadata = {
        "utc_started": recorder.baseline.utc(),
        "recorder": RECORDER.relative_to(ROOT).as_posix(),
        "recorder_sha256": recorder.baseline.hashlib.sha256(RECORDER.read_bytes()).hexdigest(),
        "launcher_sha256": recorder.baseline.hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),

        "output": argument,
        "adaptation": "Redirect only the #39 output-location assertion; original recorder file unchanged",
        "original_argv": sys.argv[:],
        "redirects": redirects,
        "completed": False,
    }
    metadata_path = HERE / "launcher.json"
    original_require(not metadata_path.exists(), "Launcher metadata must be new")
    try:
        for label, command in (
            ("matched-self-test", [sys.executable, str(RECORDER), "--self-test"]),
            ("baseline-self-test", [sys.executable, "tests/support/record_integer_baseline.py", "--self-test"]),
        ):
            recorder.baseline.record(HERE, label, command,
                                     dict(recorder.baseline.os.environ, CARGO_NET_OFFLINE="true"))
        recorder.baseline.require = require
        # Record the actual launcher invocation in environment.json; do not pretend
        # the historical recorder CLI natively accepts the new destination.
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
