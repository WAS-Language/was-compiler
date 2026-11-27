"""Archive driver for the Linux-only M4/M5 A/A experiment. Run from repo root."""
import argparse
from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time


def snapshot(cpu):
    paths = [Path('/proc/loadavg'), Path('/proc/stat'), Path('/proc/pressure/cpu')]
    paths += sorted(Path('/sys/class/hwmon').glob('hwmon*/temp*_input'))
    paths += [Path(f'/sys/devices/system/cpu/cpu{cpu}/cpufreq') / name for name in
              ('scaling_driver', 'scaling_governor', 'scaling_cur_freq', 'scaling_min_freq', 'scaling_max_freq')]
    return {str(path): path.read_text().strip() for path in paths if path.is_file()}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('preflight', type=Path, help='parent of m4-preflight and m5-preflight')
    parser.add_argument('output', type=Path, help='fresh output directory')
    parser.add_argument('cpu', type=int)
    args = parser.parse_args()
    args.output.mkdir()
    m4 = Path('target/m4-resizable').resolve()
    m5 = args.preflight.resolve() / 'm5-preflight'
    preflight = json.loads((m5 / 'environment.json').read_text())
    assert preflight['complete']
    bench = Path(next(item['argv'][0] for item in preflight['commands'] if item['label'] == 'native-timing'))
    assert digest(bench) == preflight['executableHashes'][str(bench)]
    commands = {
        'm4-runtime': ['node', 'tests/support/m4_resizable_measure.cjs', 'timing', str(m4)],
        'm5-native': [str(bench), '--record-samples'],
        'm5-runtime': ['node', 'tests/support/m5_string_timing.cjs', 'timing', str(m5 / 'runtime.wasm')],
    }
    inputs = [m4 / 'ordinary.wasm', m4 / 'observed.wasm', m5 / 'runtime.wasm', bench,
              Path(commands['m4-runtime'][1]), Path(commands['m5-runtime'][1])]
    hashes = {str(path): digest(path) for path in inputs}
    assert digest(m5 / 'runtime.wasm') == preflight['artifacts']['runtime.wasm']['sha256']
    m4_preflight = json.loads((args.preflight / 'm4-preflight/environment.json').read_text())
    assert m4_preflight['stableSourceHashesAfterRecording']
    assert all(digest(path) == m4_preflight['artifacts'][path.name]['sha256'] for path in inputs[:2])
    env = {key: value for key, value in os.environ.items() if not key.startswith('NODE_')}
    meta = {'platform': platform.platform(), 'cpu': args.cpu, 'versions': {}, 'sha256': hashes, 'runs': []}
    for cmd in [('node', '-p', 'JSON.stringify(process.versions)'), ('rustc', '--version'),
                ('cargo', '--version'), ('python', '--version'), ('git', '--no-pager', 'rev-parse', 'HEAD')]:
        meta['versions'][cmd[0]] = subprocess.check_output(cmd, text=True, timeout=10, env=env).strip()
    os.sched_setaffinity(0, {args.cpu})
    raw = []
    for session in (1, 2, 3):
        if session > 1:
            time.sleep(30)
        order = list(commands) if session % 2 else list(reversed(commands))
        labels = ('a1', 'a2') if session % 2 else ('a2', 'a1')
        for kind in order:
            for label in labels:
                print(session, kind, label, flush=True)
                command = commands[kind]
                if kind == 'm5-native':
                    corpus = args.output / f'native-{session}-{label}'
                    corpus.mkdir()
                    command = [*command, str(corpus)]
                before = snapshot(args.cpu)
                start = time.monotonic()
                stamp = datetime.now(timezone.utc).isoformat()
                result = subprocess.run(command, capture_output=True, text=True, env=env, timeout=120)
                if result.returncode:
                    raise RuntimeError(f'{kind} exited {result.returncode}: {result.stderr}')
                meta['runs'].append({'session': session, 'kind': kind, 'label': label, 'utc': stamp,
                                     'argv': command, 'boundSeconds': 120, 'seconds': time.monotonic() - start,
                                     'before': before, 'after': snapshot(args.cpu)})
                raw.append({'session': session, 'kind': kind, 'label': label, 'stdout': result.stdout,
                            'stderr': result.stderr})
    assert hashes == {str(path): digest(path) for path in inputs}, 'timing inputs changed'
    (args.output / 'environment.json').write_text(json.dumps(meta, indent=2) + '\n')
    (args.output / 'samples.json.gz').write_bytes(gzip.compress(json.dumps(raw).encode(), mtime=0))


if __name__ == '__main__':
    main()
