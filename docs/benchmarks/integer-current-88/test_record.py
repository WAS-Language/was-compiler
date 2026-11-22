"""Check the #88 recorder's CLI without building or timing anything."""

from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory
import unittest

HERE = Path(__file__).resolve().parent


class ReportTests(unittest.TestCase):
    def report(self, evidence):
        return subprocess.run([sys.executable, str(HERE / "record.py"), "report", str(evidence)],
                              capture_output=True, timeout=10, check=False)

    def test_archived_report_reproduces_table(self):
        result = self.report(HERE / "run")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.decode().splitlines(),
                         (HERE / "results.md").read_text(encoding="utf-8").splitlines())

    def test_rejects_wrong_preset_header(self):
        self.reject_compiler_edit("level=None;", "level=Basic;", b"Wrong compiler preset: None")

    def test_rejects_mismatched_module_size(self):
        self.reject_compiler_edit("literal_1,86,100", "literal_1,86,101",
                                  b"Compiler/guest size mismatch: None/literal_1")

    def reject_compiler_edit(self, original, edited, error):
        with TemporaryDirectory() as temp:
            evidence = Path(temp) / "run"
            shutil.copytree(HERE / "run", evidence)
            path = evidence / "compiler-1-None.txt"
            text = path.read_text(encoding="utf-8")
            self.assertIn(original, text)
            path.write_text(text.replace(original, edited, 1), encoding="utf-8")
            result = self.report(evidence)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(error, result.stderr)


if __name__ == "__main__":
    unittest.main()
