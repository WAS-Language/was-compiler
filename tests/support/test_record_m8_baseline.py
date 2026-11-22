"""Check the recorder's timing CSV boundary without collecting timings."""

from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from record_m8_baseline import timing


class TimingRecords(unittest.TestCase):
    def read_records(self, records):
        with TemporaryDirectory() as directory:
            path = Path(directory) / "native.csv"
            path.write_text("workload,preset,boundary,sample,calls,elapsed_ns\n" + records,
                            encoding="utf-8")
            return list(timing(path, [("fraction_closed", "None", "source")], {"source": 128}, 2))

    def test_normalizes_batches_before_summarizing(self):
        self.assertEqual(self.read_records("fraction_closed,None,source,1,128,256\n"
                                           "fraction_closed,None,source,2,128,512\n"),
                         [("fraction_closed", "None", "source", 2, 128, 3, 2, 4)])

    def test_rejects_incomplete_out_of_order_and_bad_samples(self):
        first = "fraction_closed,None,source,1,128,256\n"
        second = "fraction_closed,None,source,2,128,512\n"
        for invalid in (first, first + first, second + first, first + second + second,
                        first + second.replace(",128,", ",64,"),
                        first + second.replace(",512", ",0")):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                self.read_records(invalid)
