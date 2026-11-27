"""Compare prebuilt #80/current benchmarks on Linux. Run from the repository root."""
import argparse
import csv
from datetime import datetime, timezone
import gzip
import io
import json
import os
from pathlib import Path
import subprocess
from statistics import median
import time

from replay import digest, snapshot
from summarize import measurements


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reference', type=Path)
    parser.add_argument('candidate', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('cpu', type=int)
    args = parser.parse_args()
    args.output.mkdir()
    paths = {'a1': args.reference.resolve(), 'a2': args.reference.resolve(), 'b': args.candidate.resolve()}
    hashes = {label: digest(path) for label, path in paths.items()}
    assert hashes['a1'] != hashes['b'], 'A/B must use distinct executables'
    os.sched_setaffinity(0, {args.cpu})
    orders = [('a1', 'b', 'a2'), ('b', 'a2', 'a1'), ('a2', 'a1', 'b')]
    runs = []
    result = io.StringIO()
    writer = csv.writer(result, lineterminator='\n')
    writer.writerow(['case', 'session', 'a1_ns', 'a2_ns', 'b_ns', 'aa_ratio', 'b_over_a', 'above_triage'])
    artifacts = None
    for session, order in enumerate(orders, 1):
        if session > 1:
            time.sleep(30)
        by_label = {}
        for label in order:
            print(session, label, flush=True)
            corpus = args.output / f'corpus-{session}-{label}'
            corpus.mkdir()
            argv = [str(paths[label]), '--record-samples', str(corpus)]
            stamp = datetime.now(timezone.utc).isoformat()
            before = snapshot(args.cpu)
            proc = subprocess.run(argv, check=True, capture_output=True, text=True, timeout=120)
            after = snapshot(args.cpu)
            files = {p.name: digest(p) for p in sorted(corpus.iterdir())}
            assert len(files) == 42
            if artifacts is None:
                artifacts = files
            assert artifacts == files, 'corpus or Wasm bytes differ between benchmark runs'
            cases = {}
            for case, rep, sample, iterations, ns in measurements('m5-native', proc.stdout):
                values = cases.setdefault(case, {})
                assert (rep, sample) not in values and iterations == 10 and ns > 0
                values[rep, sample] = ns / iterations
            assert len(cases) == 42
            assert all(samples.keys() == {(r, s) for r in range(1, 4) for s in range(1, 6)}
                       for samples in cases.values())
            by_label[label] = {case: median(samples.values()) for case, samples in cases.items()}
            runs.append(dict(session=session, label=label, argv=argv, utc=stamp, before=before,
                             after=after, stdout=proc.stdout, stderr=proc.stderr))
        assert by_label['a1'].keys() == by_label['a2'].keys() == by_label['b'].keys()
        for case in sorted(by_label['a1']):
            a1, a2, b = (by_label[label][case] for label in ('a1', 'a2', 'b'))
            aa = max(a1, a2) / min(a1, a2)
            ratio = b / median((a1, a2))
            writer.writerow([case, session, a1, a2, b, aa, ratio, ratio >= 1.15 and ratio > aa])
    assert hashes == {label: digest(path) for label, path in paths.items()}
    evidence = dict(cpu=args.cpu, timeout_seconds=120, binary_sha256=hashes, corpus_sha256=artifacts, runs=runs)
    (args.output / 'native-ab.json.gz').write_bytes(gzip.compress(json.dumps(evidence).encode(), mtime=0))
    (args.output / 'native-ab.csv').write_text(result.getvalue())


if __name__ == '__main__':
    main()
