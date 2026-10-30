# M5 deterministic fragment construction (#79)

## Result and scope

One empty resizable `string[]`, followed by `n` pushes of preexisting immutable
fragments and one final `string_join`, has **O(n + B) aggregate guest storage and
copying** in this bounded family. `n` counts occurrences, including empty entries;
`B` is fragment UTF-8 bytes by occurrence plus all output separator occurrences.
Independent decoded lowering and executed byte-store counters establish direct
final materialization, without intermediate concatenated strings or a second
allocated collection inside join. This is a storage/copy-volume result, **not an
O(n + B) running-time claim**: M4 registry searches still perform work.

This changes private `cfg(test)` fixture machinery only. Production compilation,
helper ordering, bytes, options, host integration and ownership are unchanged.
There is no builder type, fusion, telemetry, general reclamation, speedup promise,
constant-retained-memory claim or M4 release disposition. [M4's pending decision](../../m4-release-readiness.md)
remains unchanged. [#80's ordinary runtime/native compilation evidence](../m5-strings-2026-10-30/README.md)
is separately owned: its sampling infrastructure exists, but final repeated timing
results were not yet published when this record was written. Timing is not this
proof; integrated M5 acceptance remains #81's responsibility and needs both.

## Approved seams and implementation

The user approved private observed compilation and bounded paired execution.

- [`string_observation_tests.rs`](../../../crates/compiler/src/codegen/string_observation_tests.rs)
  extends [#78's tracer](../../m5-string-observations.md) to join. The same aggregate
  string counters increment after a successful result allocation (`local.tee 4`,
  widened payload length local 14) and after each completed payload byte store.
  Concat retains its existing local-9 attribution and empty-operand fast paths.
- [`resizable_construction_observations.rs`](../../../crates/compiler/src/codegen/resizable_construction_observations.rs)
  layers array observations over that tracer, as a private child of `resizable`.
  It targets checked planned indices, not guessed body offsets. Only explicitly
  requested string-element fixtures can activate it.
- [`string_construction_tests.rs`](../../../crates/compiler/src/codegen/string_construction_tests.rs)
  validates the corpus, ordinary compilation parity, determinism, valid Wasm and
  observation-export isolation, then invokes the bounded Node accounting harness.
- [`m5_construction.cjs`](../../../tests/support/m5_construction.cjs) supplies JS
  UTF-8/content oracles and independent public/private layout decoding. Every
  paired call compares complete guest memories, returned handles and cursor growth.
  Views and movable payload addresses are reacquired after calls.

The join observation test initially failed because the concat-only tracer tried
to resolve an absent concat helper; optional planned-helper selection and join
sites made it green. The array test initially failed on missing observation globals,
then passed with the new transport. The complete accounting corpus was run after
wiring and passed; no execution-level red is claimed for every later corpus case.

### Observation definitions

Counters start at zero, use nontrapping modular i64 arithmetic, and are read as
unsigned `BigInt`. The finite fresh-instance workloads bound every cumulative
counter below 67,108,864, well below 2^64. No wrapped/seeded sample is evidence.
Existing allocator counters count successful bump events, original requested bytes
and aligned reservations; failed attempts do not increment them.

| Private export | Meaning |
| --- | --- |
| `__m5_string_payload_allocations` | Successful allocating concat/join result events; a subset of physical allocations, not additional storage |
| `__m5_string_payload_bytes` | New logical string payload bytes at those events |
| `__m5_string_descriptor_bytes` | Logical descriptor bytes at those same events |
| `__m5_string_copied_bytes` | Completed concat/join payload byte stores |
| `__m5_array_acquired_bytes` | Full successful allocator payload capacities, including reused acquisitions |
| `__m5_array_released_bytes` | Full capacities after successfully committed releases |
| `__m5_array_active_bytes` | Acquired minus released capacity, including an unreleased acquisition after a later trap |
| `__m5_array_peak_active_bytes` | High-water active capacity, updated inside successful acquisition **before** reserve releases the old buffer |
| `__m5_array_copied_bytes` | Actual reserve byte stores plus four bytes per consumed constructor handle store; **not string-payload copying** |
| `__m5_array_relocations` | Successful reserve acquisitions, excluding calls that fit existing capacity |
| `__m5_allocator_registry` | Existing registry root, for independent physical-record accounting, not an allocator control |

Independent instruction decoding strips only the expected instrumentation and
recovers original bodies **including locals**, existing global values and ordered
export tuples, and all unaffected sections. Import-free and imported/M4 fixtures
check function indices versus body indices. Collision tests preserve ordinary
compilation. Join has 12 auxiliary i32 locals and one i64 local, independent of
fragment count. Its complete helper-call list contains only validators and one
static allocation call. Validation/sizing precedes allocation; both byte-copy loops
write the final payload through destination local 5. There is no concat call or
auxiliary collection allocation inside join.

The separate paired array tracer exercises fresh and reused acquisition, nonempty
constructor copying, reserve no-op, actual relocation, invalid requests, capped
allocation failure and counter wrap semantics. Its peak observes the actual
old-plus-new acquisition interval, not a completed-call estimate.

## Workloads and independent bounds

The [corpus snapshot](run1/workload.was) matches
[`m5_construction_corpus.was`](../../../tests/support/m5_construction_corpus.was).
Sizes are **0, 1, 2, 3, 4, 5, 8, 9, 16, 17, 32, 33, 64, 65, 129, 257, 1025**,
covering both sides of multiple geometric string-handle growth boundaries.
Six patterns/separators per size plus the initial two-fragment tracer produce
**103 construction rows**: ASCII, multibyte `λ猫`, duplicate/empty occurrences,
all-empty fragments, empty separator, `界` separator and mixed `a`, empty, `b`
with `|`. Each row checks exact counts and independent linear inequalities, not
just one favorable sample or page counts.

For current four-byte slots, let `C = max(8, next_power_of_two(4n))`. Starting from
an empty resizable array, acquired distinct capacities form `8, 16, ..., C`.
Their geometric sum is `2C - 8`; handle relocation copies `C - 8` bytes.
Since `C < 8n` for n >= 2, copying is less than `8n` handle bytes.
The singleton has C = 8 and copies zero handle bytes, so the same copying bound
holds for every n > 0. Each physical
capacity also has one 32-byte allocator record. There are an eight-byte source
literal descriptor and a 48-byte stable descriptor/authority object. This gives:

```text
retained reserved bytes = 8 + 48 + 32 * physical_record_count
                        + active_capacity + free_capacity
                        + result_reservation
result_reservation      = align8(8 + B) when current join materializes, else 0
```

For n >= 2, the harness independently requires total reserved storage to be at
most `256(n + 1) + B + 8`, and measured handle plus string copying at most `8n + B`.
Exact current-policy accounting is intentionally stricter than the language's
representation-independent acceptance: singleton reuse and a different valid
empty representation must not become permanent physical-allocation mandates.

**Controlled concat baseline:** default language optimization **None**, ordinary
copy-based concat lowering, one repeated nonempty fixed-size fragment of `f` bytes
and an initially empty accumulator. The first authorized empty reuse remains
active. For n >= 1, cumulative string copying is independently
`f * (n(n + 1)/2 - 1)`; n = 0 copies zero. Reservations are summed per event,
not by rounding a combined payload total. Two fragment widths (1 and 5) at all
17 sizes give **34 baselines**. This neither disables fast paths indiscriminately
nor requires future optimized implementations to remain quadratic.

Representative one-byte, empty-separator rows (bytes):

| n | B / join string copying | Array handle copying | Construction reserved | Concat string copying | Concat reserved |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 3 | 3 | 8 | 160 | 5 | 32 |
| 17 | 17 | 120 | 496 | 152 | 336 |
| 129 | 129 | 1,016 | 2,496 | 8,384 | 9,856 |
| 1,025 | 1,025 | 8,184 | 17,824 | 525,824 | 537,600 |

The small-size overhead crossover is visible; no timing or universal benefit is
inferred. Fixed-array correctness separately covers `[]`, `[""]`, `["", ""]`,
`["a", "", "b"]` and duplicate multibyte fragments, each with three separators.
Further cases verify both argument effects once/in order, separator-induced
mutation/relocation and binding rebinding, validation before singleton/empty reuse,
retained inputs/arrays/results across real memory growth and a caught trap.

## Storage attribution and temporal boundaries

Each construction uses fresh paired instances. Six scalar literals are preexisting
inputs: their descriptor/payload byte-range **union** is reported once, separately
from runtime storage; other static corpus literals and alignment are represented
by the initial layout boundary, not repeatedly charged by occurrence. Fragment
production costs zero in these accumulation rows. A separate produced-fragment
interval counts a six-byte concat result, then accumulation and join separately.

Registry traversal counts each distinct physical record/capacity once, including
released storage that remains physically retained. It checks disjoint extents,
active/free classifications, current payload, public/private lengths and acquisition
identity. Workload extents partition the cursor delta exactly, including descriptor,
authority, records, full capacities, new strings and allocation padding. Logical
string descriptor/payload counters partition their physical requests and are never
added again to reserved totals. No unexplained residual is labelled array overhead.

**Temporal metrics are separate:** completed-call active capacity is `C`; the
observed transient active capacity is `C + C/2` after at least one growth (otherwise
8). For 1,025 entries these are **8,192 versus 12,288 bytes**. Physical bump storage
retains free old capacities, so its monotonic high-water value is the completed
construction's total reserved value; this is not active-capacity residency or
engine RSS. Peaks of different categories need not be simultaneous and are not
summed into an invented active-total peak. Engine heap/RSS and general liveness
are unavailable. Separately retained second joins are recorded as separate
constructions with their own allocation/copy deltas; the first result remains valid.
No constant retained bound, general release, reclamation or page plateau follows.

## Reproduction and evidence

Run sequentially from the repository root with cached Rust dependencies, Node 24+
and Python 3. Choose a fresh directory (even an existing empty directory is refused):

```sh
python tests/support/record_m5_construction.py target/m5-construction-79-run1
python tests/support/record_m5_construction.py target/m5-construction-79-run2
```

The [recorder](../../../tests/support/record_m5_construction.py) runs these exact
nonzero test groups, in order, each with a **600-second Cargo child bound**:

```sh
cargo test --offline -p was-compiler --lib codegen::string_observation_tests -- --include-ignored --test-threads=1
cargo test --offline -p was-compiler --lib codegen::resizable::construction_observations -- --include-ignored --test-threads=1
cargo test --offline -p was-compiler --lib codegen::string_construction_tests -- --include-ignored --test-threads=1
```

They execute **6 + 3 + 2 tests**, none failed/ignored, including Node cases. The
construction Node child has a **120-second** deadline through the shared process
helper; the earlier concat/array tracers retain their own shorter bounds. Deadlines
are not guest fuel or process-tree cancellation. Default Cargo skips runtime cases.
No timing is collected here; native Cargo release optimization is not being
confused with the language's default None policy.

Two sequential recordings passed. Complete [run1 accounting](run1/accounting.json)
and [run2 accounting](run2/accounting.json) are identical, as are executable source
hashes and generated modules: ordinary **4,966 bytes**, observed **5,580 bytes**.
[Run1 provenance](run1/environment.json) and [run2 provenance](run2/environment.json)
record source SHA-256 hashes, revision/dirty state, tools, CPU/OS/Node/V8, selected
environment overrides, exact argv/bounds and successful stable-source checks.
Each directory retains full test logs and the corpus. Local attributes preserve
recorded bytes across Git newline normalization. This is dirty-working-tree
evidence, not evidence from an isolated clean checkout: concurrent #80 files and
preexisting design edits were preserved and are not enrolled in #79's commit.

## Validation and review

Before recording: `cargo check --offline --all-targets` passed. Both focused
recordings passed. Strict native Clippy passed. The unmodified M4 recorder also
completed successfully at `target/m4-reproduction-79`, executing 38 runtime tests,
5 historical M3 measurement tests, 3 M4 isolation tests and its two exact release
recordings. This establishes execution compatibility, not a new M4 performance
disposition or isolated M5 timing evidence. Ordinary string runtime tests passed
all 27 cases with `--include-ignored`.

The final `cargo test --offline` passed **1,359 tests**, with zero failures and
322 intentionally ignored cases across 80 result targets. This includes concurrent
#80 tests in the working tree; it is not a clean-checkout result. The explicit
`cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown`
passed. Browser rebuilding is not required for exclusively unit-test machinery;
no new current-source browser result is claimed by this ticket.

Two-axis review against task-start revision `3e91f5c6b074f1f02e6c905a6d402c07b22c8697`
was scoped to #79 paths, excluding staged concurrent #80 work. Standards found no
hard breaches, one accounting-explanation error (`C < 8n` did not hold for n = 1)
which was corrected above, and one optional suggestion to name the small array
tracer's positional expected-counter tuples, deferred. Spec found no actionable
findings. Both reviewers independently reconciled accounting and provenance; they
did not run builds. Raw evidence bytes remain unmodified: local attributes allow
CRLF and terminal log blank lines without masking source-code whitespace.
