"""Validate the archived A/A samples and report medians in ns per operation."""
from collections import defaultdict
import csv
import gzip
import json
from pathlib import Path
from statistics import median
import sys


def measurements(kind, stdout):
    if kind == 'm5-native':
        for line in stdout.splitlines():
            if line.startswith('NATIVE_SAMPLE\t'):
                _, boundary, case, repetition, sample, iterations, ns = line.split('\t')
                yield f'{boundary}/{case}', int(repetition), int(sample), int(iterations), int(ns)
        return
    data = json.loads(stdout)
    rows = data['results']['samples'] if kind == 'm4-runtime' else data['samples']
    for row in rows:
        case = f"{row['type']}/{row['kind']}" if kind == 'm4-runtime' else row['case']
        yield case, row['repetition'], row['sample'], row['iterations'], int(row['elapsedNs'])


def main():
    if len(sys.argv) not in (2, 3) or (len(sys.argv) == 3 and sys.argv[2] != '--baseline'):
        raise SystemExit('usage: summarize.py SAMPLES.json.gz [--baseline]')
    baseline = len(sys.argv) == 3
    runs = json.loads(gzip.decompress(Path(sys.argv[1]).read_bytes()))
    medians = defaultdict(dict)
    for run in runs:
        kind, session, label = (run[key] for key in ('kind', 'session', 'label'))
        case_count, start = {'m4-runtime': (12, 0), 'm5-native': (42, 1), 'm5-runtime': (123, 1)}[kind]
        cases = defaultdict(dict)
        for case, repetition, sample, iterations, ns in measurements(kind, run['stdout']):
            coord = (repetition, sample)
            assert coord not in cases[case] and iterations > 0 and ns > 0
            cases[case][coord] = ns / iterations
        assert len(cases) == case_count
        for case, samples in cases.items():
            assert samples.keys() == {(i, j) for i in range(start, start + 3) for j in range(start, start + 5)}
            values = medians[kind, case]
            assert (session, label) not in values
            values[session, label] = median(samples.values())

    writer = csv.writer(sys.stdout, lineterminator='\n')
    if baseline:
        writer.writerow(['boundary', 'case', 'runs', 'median_ns_per_op', 'min_ns_per_op',
                         'max_ns_per_op', 'max_paired_aa_ratio'])
    else:
        writer.writerow(['boundary', 'case', 'session', 'a1_ns_per_op', 'a2_ns_per_op', 'slower_over_faster'])
    for (kind, case), values in sorted(medians.items()):
        assert values.keys() == {(session, label) for session in (1, 2, 3) for label in ('a1', 'a2')}
        if baseline:
            runs = list(values.values())
            aa = [sorted((values[session, 'a1'], values[session, 'a2'])) for session in (1, 2, 3)]
            writer.writerow([kind, case, len(runs), median(runs), min(runs), max(runs),
                             max(slow / fast for fast, slow in aa)])
            continue
        for session in (1, 2, 3):
            a1, a2 = (values[session, label] for label in ('a1', 'a2'))
            writer.writerow([kind, case, session, a1, a2, max(a1, a2) / min(a1, a2)])


if __name__ == '__main__':
    main()
