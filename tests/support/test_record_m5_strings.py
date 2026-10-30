"""Recorder seam: evidence must be fresh, complete and tied to stable inputs."""
import copy
import json
import tempfile
import unittest
from pathlib import Path

import record_m5_strings as recorder


class RecorderTests(unittest.TestCase):
    def test_fresh_directory_never_overwrites_even_empty_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "recording"
            recorder.fresh_directory(path)
            with self.assertRaises(FileExistsError):
                recorder.fresh_directory(path)
            (path / "sentinel").write_text("preserve", encoding="utf-8")
            with self.assertRaises(FileExistsError):
                recorder.fresh_directory(path)
            self.assertEqual((path / "sentinel").read_text(), "preserve")

    def test_native_samples_require_full_unique_ordered_positive_records(self):
        lines = []
        for boundary, case, repetition, sample in recorder.native_schedule():
            lines.append(f"NATIVE_SAMPLE\t{boundary}\t{case}\t{repetition}\t{sample}\t10\t12345")
        raw = "\n".join(lines)
        records = recorder.parse_native(raw)
        self.assertEqual(len(records), 630)
        self.assertEqual(records[0]["elapsedNs"], 12345)
        for invalid in ["", "\n".join(lines[:-1]), raw + "\n" + lines[0],
                        raw.replace("12345", "0", 1), "\n".join(reversed(lines))]:
            with self.subTest(invalid=invalid[:80]), self.assertRaises(ValueError):
                recorder.parse_native(invalid)

    def test_build_report_selects_target_executable_not_stale_default(self):
        with tempfile.TemporaryDirectory() as temporary:
            target = Path(temporary)
            stale = target / "release" / "was-compiler.exe"
            stale.parent.mkdir()
            stale.write_bytes(b"stale compiler")
            built = target / "x86_64-pc-windows-msvc" / "release" / "was-compiler.exe"
            report = json.dumps({"reason": "compiler-artifact", "target": {
                "name": "was-compiler", "kind": ["bin"]}, "executable": str(built)})
            self.assertEqual(recorder.cargo_executable(report, "was-compiler", "bin"), built)
            for invalid in ["", report + "\n" + report,
                            report.replace('"bin"', '"lib"')]:
                with self.assertRaises(ValueError):
                    recorder.cargo_executable(invalid, "was-compiler", "bin")
            self.assertEqual(stale.read_bytes(), b"stale compiler")

    def test_stability_detects_changed_removed_and_added_inputs(self):
        recorder.require_stable({"a": "hash"}, {"a": "hash"})
        for after in [{"a": "changed"}, {}, {"a": "hash", "b": "new"}]:
            with self.assertRaises(RuntimeError):
                recorder.require_stable({"a": "hash"}, after)

    def test_runtime_records_require_all_cases_and_samples(self):
        names = [f"case_{i}" for i in range(123)]
        samples = [{"case": name, "repetition": repetition, "sample": sample,
                    "iterations": 8, "elapsedNs": 12345}
                   for repetition in range(1, 4)
                   for name in (names if repetition % 2 else list(reversed(names)))
                   for sample in range(1, 6)]
        complete = {"mode": "timing", "checkedCases": 123,
                    "cases": [{"name": name} for name in names], "samples": samples}
        recorder.validate_runtime(complete)
        invalid_records = [{}, {"checkedCases": 0, "samples": []},
                           {"checkedCases": 123, "cases": [], "samples": []}]
        for key, value in [("samples", samples[:-1]), ("samples", samples + samples[:1]),
                           ("samples", list(reversed(samples)))]:
            invalid_records.append({**complete, key: value})
        zero = copy.deepcopy(complete)
        zero["samples"][0]["elapsedNs"] = 0
        invalid_records.append(zero)
        for data in invalid_records:
            with self.assertRaises(ValueError):
                recorder.validate_runtime(data)


if __name__ == "__main__":
    unittest.main()
