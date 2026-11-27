# M4/M5 bounded A/A performance investigation — 2026-11-27

**M4: inconclusive. M5: inconclusive.** No comparable historical *timing*
reference was available. The A/A controls show how much identical artifacts can
vary on this setup; they cannot attribute the historical signals to a code change
or establish that the milestones are free of regressions. The [official Linux
baseline](baseline.md) supersedes Windows timing for [M4](../../m4-release-readiness.md)
and [M5](../../m5-release-readiness.md) and records a matched native compiler
comparison against #80.

## Method and decision rule

The predeclared rule escalates a case only if there is a comparable reference,
the median slowdown is at least 15%, it exceeds observed A/A variability, and
it repeats in the same direction in all three sessions. This is a practical
triage threshold, not a significance test or release budget. The historical
Windows/Intel/Rust 1.98/Node 24 measurements are context, not a timing control
for this Linux/AMD/Rust 1.100/Node 26 run. Matching Wasm files do not make those
timings comparable; M5's historical native executable is also not the one timed
here. No old-versus-new code or engine change was isolated.

The three sessions reused the unmodified runtime harnesses and the M5 release
benchmark. Each boundary ran twice per session with the same artifacts, in
separate processes; session 2 reversed boundary and pair-label order. Sessions
were separated by 30 seconds. All children inherited affinity to logical CPU 0
(AMD Ryzen 7 5700X, SMT sibling CPU 8 was not isolated), and no build or other
measurement was launched alongside them. Start/end snapshots record host load,
CPU counters and pressure, frequency and available thermal readings. The AMD
`performance` governor was already active, but sampled CPU 0 frequency still
ranged from 3.59 to 4.64 GHz. Affinity is not exclusive CPU reservation; neither
frequency, temperature, background scheduler activity nor JIT tiers were fixed.
Snapshots do not identify a cause for the observed spread.

M4 timing includes the JS loop and guest crossings, not compiler time. M5
reports native source-to-Wasm, prepared-typed-to-Wasm and Node guest time
separately. Both runtime harnesses exclude engine module compilation,
instantiation and oracle/decoding from the timer. Unchanged warmups and case
orders inside each invocation retain the historical measurement boundaries.

## Evidence and investigation conclusions

| Boundary | Cases | Session 1 max A/A | Session 2 max A/A | Session 3 max A/A | Largest max/min over all six runs |
| --- | ---: | ---: | ---: | ---: | ---: |
| M4 guest runtime | 12 | 1.114× (0) | 1.085× (0) | 1.327× (2) | 1.465× |
| M5 native source compile | 21 | 1.041× (0) | 1.117× (0) | 1.178× (3) | 1.247× |
| M5 native prepared typed compile | 21 | 1.062× (0) | 1.038× (0) | 1.026× (0) | 1.062× |
| M5 guest runtime | 123 | 1.145× (0) | 1.168× (1) | 1.103× (0) | 1.180× |

Each A/A ratio is the slower of the two case medians divided by the faster;
parentheses count cases at or above 1.15×. Across all session/boundary groups,
the median case ratio was between 1.005× and 1.024×. The rightmost column uses
the six medians per case and is not a within-session noise bound. No case crossed
1.15× in all three A/A pairs; that is not an A/B regression test.

- **M4.** Both the ordinary (6,126 B) and observed (6,305 B) modules matched
  #71/#72, and the preflight accounting results exactly matched #71. Only the
  ordinary module was timed. `number/append` varied by 32.7% within one pair even
  with identical bytes. The historical residual remains unattributed.
- **M5.** The runtime module (4,258 B) and all 42 native corpus files (21 sources
  and 21 O0 modules) matched #81 in all six runs. A separate post-session
  construction run passed 6 string, 3 array and 2 accounting tests and reproduced
  the archived #81 `accounting.json` byte for byte. The largest source-compile A/A
  ratio was 1.178× (`replace_shrink_64`), while prepared-typed stayed under 1.063×.
  Runtime hit 1.168× (`repeated_empty_0`) in one pair. These data do not resolve
  the older variability, and emitted-byte identity does not validate native
  performance.

## Reproduce and inspect

Run from the repository root on Linux with cached prerequisites and separate
fresh paths. The existing recorders preflight correctness, rebuild benchmark
artifacts offline and also collect timings; their timings are **not** included
in the three A/A sessions. Use an allowed logical CPU in place of `0` if needed.
To repeat the independent M5 construction check, run the existing
`record_m5_construction.py` recorder after the timing sessions and compare
`accounting.json` with [#81's archived result](../m5-integrated-81/construction-1/accounting.json).

```sh
mkdir -p target/m4-m5-aa
python -B tests/support/record_m4_resizable.py target/m4-m5-aa/m4-preflight
python -B tests/support/record_m5_strings.py target/m4-m5-aa/m5-preflight
python -B docs/benchmarks/m4-m5-aa-2026-11-27/replay.py target/m4-m5-aa target/m4-m5-aa/sessions 0
python -B docs/benchmarks/m4-m5-aa-2026-11-27/summarize.py target/m4-m5-aa/sessions/samples.json.gz
```

Preflight passed M4's 38 runtime, 5 M3 compatibility and 3 isolation tests plus
its accounting/timing recorders; M5 passed 2 runtime and 5 native correctness
tests plus the 42-case Divan smoke. The first A/A attempt stopped at the first
M5 native invocation because the required fresh corpus directory argument was
missing. That partial attempt was excluded; the corrected replay ran all 18
processes in a new directory, with a 120-second bound each and unchanged inputs.

[Environment and commands](environment.json.gz), [raw runs](samples.json.gz),
and [all 531 paired case medians](medians.csv) are retained. There are 180 M4
runtime, 630 M5 native and 1,845 M5 runtime samples per invocation (15,930
samples total). The archive driver and summarizer are evidence tools, not CI
steps or changes to the production compiler or historical harnesses.
