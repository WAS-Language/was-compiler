# Resizable arrays — #65 construction and #66 reserve

## Scope and status

[Issue #65](https://github.com/LuanRoger/was-compiler/issues/65) implements only the
explicit fresh-copy constructor `resizable(source)` for `number[]`, `bool[]` and
`string[]`. It preserves existing array types and the public eight-byte descriptor.
[Issue #66](https://github.com/LuanRoger/was-compiler/issues/66) adds only
`values.reserve(additional)` and relocation-safe indexed writes for all three types.
Reserve may relocate payload storage and release its displaced owned buffer, but
never changes logical length. Push, pop, remove and shrinking remain proposed;
there is no public capacity or authority query.
[Memory roadmap §8](was_compiler_memory_and_optimization_roadmap.md#8-milestone-m4--resizable-typed-arrays)
retains the full milestone design; [ADR 0009](adr/0009-opt-in-resizable-array-storage.md)
records the design trade-offs, not release evidence.

This document describes construction and narrow reserve support, not an integrated
M4 release. [#66 final validation and review passed](#issue-66-validation);
integrated M4 release acceptance remains pending. The #65 record below and
historical #37 typed-array/#63 M3 release records retain their original scope.

## Source contract and naming migration

- The builtin accepts exactly one positional array argument and returns the exact
  same array type. It evaluates the source once, validates complete outer storage
  and every copied logical element, and publishes a fresh collection with independent
  mutable slots. Source aliases are unaffected; copying another resizable array is
  still a fresh copy. Immutable string values may be shared, not mutated or released.
- `resizable` follows `len` as an ordinary identifier, not a lexer keyword. Existing
  global functions or registered host-language names `resizable` must be renamed.
  Host module/field names remain unrestricted. Normal lexical shadowing is legal:
  a scalar binding hides the builtin and is not callable; a local callable with
  that name uses ordinary callable rules.
- Direct builtin pipeline stages such as `source |> resizable` are rejected.
  A typed local callable or immediate typed stage can explicitly call the constructor;
  this adds neither first-class builtins nor a builtin pipeline exception.
- The constructor allocates and is forbidden in pure defaults/contracts, including
  skipped operands. Source checking and forged typed-input backend checks reject it
  independently.

```text
fn copy_flags(source: bool[]) -> bool[] {
    let copy = resizable(source);
    return copy;
}
fn main() -> bool {
    let source = [true];
    let copy = copy_flags(source);
    copy[0] = false;
    let empty: string[] = resizable([]);
    return source[0] && !copy[0] && len(empty) == 0;
}
```

## Reserve contract (#66)

- Only `identifier.reserve(additional)` is supported, with exactly one positional
  `number` argument and a void result (or Never when evaluation cannot complete).
  The receiver must have type `number[]`, `bool[]` or `string[]`. `reserve` is a
  contextual method name, not a reserved global/host-language name. Parenthesized,
  indexed and call-result receivers, general methods and postfix chaining are not
  introduced. Named/default method arguments and direct builtin pipeline stages
  are not supported. Reserve is forbidden in pure defaults/contracts, even in
  skipped operands, with independent forged-tree backend checks.
- Capture the receiver handle once and validate its current storage and exact resize
  authority **before** evaluating the argument. Fixed-storage receivers trap even
  for `reserve(0)`, before argument effects; static array typing does not prove
  authority. Rebinding the receiver during argument evaluation cannot redirect it.
- Evaluate the argument once, then revalidate the captured receiver's current
  authority/storage after argument effects, including nested reserve or host reentry.
  Sizing uses that post-argument logical length. Completed effects survive a later trap.
- Additional must be finite, integral and nonnegative; negative zero is accepted as
  zero. Reject fractions, other negative values, NaN and infinities at runtime.
  Check length addition and element-width byte sizing before narrowing, within the
  2^31-byte allocation request ceiling. Allocation success is not guaranteed.
- Ensure capacity for at least `len(receiver) + additional` elements, without reducing
  capacity or changing length. There is no exact-capacity guarantee or public query.
  If current capacity suffices, no replacement is needed, but both validation phases
  and argument evaluation still apply.
- Growth acquires the **full** selected power-of-two byte capacity, validates that
  acquisition, and raw-copies only live slot bytes. It does not consume or scan
  otherwise unconsumed bool bytes or string handles; later reads retain their normal
  validation. Spare capacity is not copied or made logically readable. Publish the
  replacement descriptor/private authority before releasing the displaced owned
  payload, with no callbacks during copy/publication/release. Failure before
  publication preserves the post-argument published array, not allocator history
  or prior effects. Referenced immutable strings are never released.

```text
fn main() -> number[] {
    let values = resizable([1, 2]);
    values.reserve(100);
    return values; // same handle and length, possibly a new payload
}
```

## Relocation-safe writes and host borrows (#66)

Fixed-storage indexed assignments retain the original pre-RHS checked address,
including in M4 modules. Opt-in resizable destinations instead capture the handle
and index once and validate the destination before the RHS. After evaluating the
RHS once, revalidate the captured array's authority and index and resolve its
**current** payload before storing. Numeric compounds preserve the pre-RHS old
numeric value, including for `%=`. Receiver rebinding cannot redirect the store;
RHS reserve through aliases, captures or reentry cannot leave it targeting a
released/reused buffer. If the RHS traps, no final store occurs and completed
reserve effects remain. Future shrink/remove semantics are not implemented here.

Hosts may retain opt-in array handles, but payload pointer/length borrows cannot
span a call or reentrant callback that may relocate storage: reacquire both from
the stable descriptor afterward. This is separate from recreating JavaScript views
after possible memory growth. Fixed-storage host lifetime guarantees are unchanged;
ordinary literals and host encoding do not gain resize authority.

## Rust source/typed-node migration (#66)

Public raw syntax adds `Expr::Reserve { receiver, args, span }`; typed syntax adds
`TypedExprKind::Reserve { receiver, additional }`. Update exhaustive matches and
source/typed-tree construction accordingly. This is a dedicated operation, not an
ordinary callable or first-class method. Semantic checking resolves the receiver
and numeric argument; checked lowering independently validates identifier/captured
receiver shapes, operand/result annotations, purity and skipped/unreachable nodes.
The structural walker visits receiver then additional, preserving separate planning
policies and owner-local scratch for pending writes.

## Contextual typing

Known array annotations, destinations, mapped formals and explicit named-function
results supply context through the constructor to its source, including nested
constructors and existing block/if/match/value-loop paths. For example,
`let flags: bool[] = resizable([]);` selects bool storage; unconstrained
`resizable([])` defaults to `number[]`. Context does not reinterpret a concrete
function result or flow backward from siblings, equality operands or callers.

Placeholder constraints relate source and result as the exact same supported array
type within their own region. `resizable(_)` alone is ambiguous, and a later call
or pipeline input cannot refine it. Reachable region-local evidence can select a
type, for example a known bool-array formal consuming `resizable(_)`. Existing
Never propagation, skipped/unreachable checking and first-error ordering remain
in force. See [typed-array inference](typed-arrays.md#contextual-literal-typing-and-stable-inference).

## Storage and authority boundary

The [memory ABI authority section](memory-abi.md#private-resizable-array-authority-65)
owns the current record layout: a retained 48-byte reservation contains the stable
eight-byte public descriptor and a private 40-byte authority record. The record
tracks exact handle, element kind, logical length, full byte capacity, owned payload
and acquisition identity. Descriptor validity alone never establishes authority.

`codegen/resizable.rs` structurally discovers constructors and `Reserve` nodes
(including nested callables and skipped/unreachable nodes), plans helper/type/global
indices before checked body lowering, and selects helpers deterministically in
number/bool/string order. It plans per-element authority lookup/validation helpers
for array kinds used in an M4 module, constructors for constructed kinds and reserve
helpers for reserved kinds; reserve-only discovery also activates the M4 plan.
Only the required M3 core is promoted to production-internal
`codegen/allocator_core.rs`; the M3 adapter, test configuration, instrumentation and
management exports remain `cfg(test)`. Core acquisition returns payload and identity
through private `(i64) -> (i32, i64)` transport, not a source or host ABI signature.
Authority/extent validation uses the allocator-owned registry interface, not private
allocator offsets copied into the constructor.

Capacity uses power-of-two byte classes, minimum eight bytes even when empty, with
the checked 2^31-byte request ceiling. The constructor acquires the entire selected
capacity as its requested extent; allocator rounding slack outside a request is not
usable capacity. Allocation success is not guaranteed. Every copied bool byte and
string handle receives existing consumed-value validation; strings retain their
UTF-8/immutability host obligations.

The completed descriptor/record is published only after preparation and copy, with
no intervening callbacks. A failure can retain successful unpublished allocations:
there is no trap rollback, sweeping or general reclamation. Stable descriptors,
authority records, current payloads and unreachable arrays remain retained. #66
releases only displaced owned payloads after successful replacement publication;
it introduces neither ordinary-array reclamation nor a general free API.

Ordinary array literals and host encoding remain fixed-storage even in M4 modules.
Programs without M4 acquire no new allocator/authority machinery and retain existing
bytes/helper order. Numeric presets do not choose storage semantics. No M2 migration,
public management API or host ownership-transfer protocol is introduced.

## Validation guidance (not execution evidence)

Existing focused targets cover frontend inference/naming/purity and checked/decoded
lowering. From the repository root, run offline:

```sh
cargo test --offline -p was-compiler --test resizable_arrays_frontend --test resizable_arrays_lowering
cargo test --offline -p was-compiler --test resizable_arrays_runtime -- --include-ignored
cargo test --offline -p was-compiler --test typed_arrays --test typed_arrays_lowering
cargo test --offline -p was-compiler --test closures_frontend placeholder
cargo test --offline -p was-compiler --lib codegen::allocator -- --include-ignored
cargo test --offline
git --no-pager diff --check
```

These commands are guidance, not a claim that they passed. Default Cargo success
alone does not establish executed-Wasm or browser coverage. For integrated evidence,
follow [the validation workflow](validation.md) and [root validation commands](../AGENTS.md#validation-commands),
including current-source browser rebuilding before browser/full ignored runs.
Bounded execution must cover copy identity/isolation, consumed-value traps, failure
ordering and preset parity; keep Node/browser-dependent cases ignored by default.
No historical record substitutes for those runs on final #66 sources.

## #66 focused validation — 2026-10-29

The following focused command passed on the working sources:

```sh
cargo test --offline -p was-compiler --test resizable_arrays_frontend --test resizable_arrays_lowering --test resizable_arrays_runtime -- --include-ignored
```

- Frontend: **27 passed**, zero failed/ignored.
- Forged-input/decoded lowering: **17 passed**, zero failed/ignored.
- Explicitly enabled Node runtimes: **14 passed**, zero failed/ignored. Each runtime
  test runs all four numeric presets and checks enabled-preset byte identity.

These are the focused results directly run by the documentation agent, not an
integrated M4 release. The later [final #66 record](#issue-66-validation) separately
captures the user-observed broader validation and independent review. The
constructor-only record below remains historical #65 evidence.

<a id="issue-66-validation"></a>

## #66 final validation and review — 2026-10-29

**#66 validation and review passed. Integrated M4 release acceptance remains pending.**
This record covers narrow reserve and relocation-safe writes, not push/pop/remove,
shrinking or the full M4 milestone. The #65 constructor record below is unchanged.

The user reported the following final-source validation outcomes; the documentation
agent did not rerun these broader gates. The focused 27 frontend / 17 lowering /
14 runtime results above were directly executed by the agent, with all four numeric
presets exercised by every runtime test. No aggregate suite test count is asserted.

| Final validation | Observed outcome |
| --- | --- |
| `cargo test --offline` | Passed after correcting the legacy `range_frontend` expectation from `Invalid` to `Dot`. |
| `cargo test --offline --workspace -- --include-ignored` | Passed, exit 0, on the separate repeat described below. |
| `cargo clippy --offline --all-targets -- -D warnings` | Strict native all-targets check passed. |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Strict browser Wasm-target check passed. |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Portable core build passed. |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Final offline current-source browser rebuild passed. |
| `node tests/browser.cjs pkg` | All 25 existing browser smoke groups passed against rebuilt output. |

Earlier focused lexer/parser, temporary-planning, compound-assignment and typed-array
runtime checks also passed, as did the all-targets check. Existing browser tests
provide regression evidence only: no new constructor-specific browser scenario is
claimed, and this record does not substitute for full-M4 adapter/release acceptance.

The first combined run with a five-minute bound timed out during browser adapter
tests. Repeating the full workspace/include-ignored command separately with the same
five-minute bound completed successfully with exit 0. The timeout is not counted as
a passing run; the subsequent completed run is the full-suite evidence.

Test corrections during development aligned expectations with the intended contract:

- The initial Never-tail expectation was corrected to preserve existing divergence
  semantics, not to introduce a new result rule.
- Raw relocation-copy checks accept the emitted byte-copy loop; the contract does
  not require an invented `MemoryCopy` instruction shape.
- The legacy range frontend expectation was intentionally updated from `Invalid`
  to `Dot` for the new dot token; this is an intentional token expectation change,
  not removal of malformed-source coverage.

Independent two-axis review against starting commit `6d782f0` found **zero hard
standards findings and zero spec findings**. Two low-priority suggestions were
addressed: signature accessor naming and exact forged-input error assertions.
Re-review found **zero outstanding findings on both axes**. These results close
#66 validation/review only; push/pop/remove remain proposed and integrated M4
release acceptance remains pending.

## Constructor validation and review — 2026-10-28

The final constructor sources passed:

- The three approved seams: 16 frontend tests, seven forged-input/decoded lowering
  tests, and three explicitly enabled Node runtime tests. Each runtime test executes
  all four numeric presets and checks enabled-preset byte identity. Coverage includes
  all array types, independent slots/handles, copying resizable inputs, empty copies,
  consumed-element traps, completed effects, captures, recursion, host reentry and
  memory growth. Divergent bool/string iteration metadata has a test-first regression.
- Existing typed-array frontend/lowering and placeholder regressions; private M3
  (39 tests) and M2 (23 tests), explicitly including ignored runtimes.
- `cargo test --offline`, strict all-targets Clippy, and the portable core
  `wasm32-unknown-unknown` library build.
- A current-source offline browser rebuild using the documented `wasm-pack` command,
  `node tests/browser.cjs pkg`, and `cargo test --offline --workspace -- --include-ignored`.
  These existing browser tests establish regression coverage, not a new constructor-
  specific browser scenario.
- Edited Rust formatting and `git diff --check`.

The first full default run timed out at 120 seconds; the subsequent bounded run
completed successfully. An initial new runtime expectation was corrected to preserve
existing parameter-entry outer-storage validation before body effects.

Independent standards and issue-spec reviews against starting commit `794910d`
found three standards issues (signature interning, typed indices and ABI ownership)
plus a divergent-iteration metadata bug. All were corrected and re-reviewed:
**zero outstanding findings on either axis**, with the global-plan maintainability
suggestion also addressed. No resize operations, relocation, reclamation or full-M4
release acceptance is claimed.
