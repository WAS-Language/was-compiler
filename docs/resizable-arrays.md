# Resizable array construction — #65

## Scope and status

[Issue #65](https://github.com/LuanRoger/was-compiler/issues/65) implements only the
explicit fresh-copy constructor `resizable(source)` for `number[]`, `bool[]` and
`string[]`. It preserves existing array types and the public eight-byte descriptor.
Push/pop/reserve/remove, payload relocation, shrinking, displaced-payload reclamation
and the future pending-write protocol are not implemented by this slice.
[Memory roadmap §8](was_compiler_memory_and_optimization_roadmap.md#8-milestone-m4--resizable-typed-arrays)
retains the full milestone design; [ADR 0009](adr/0009-opt-in-resizable-array-storage.md)
records the design trade-offs, not release evidence.

This document describes the current constructor implementation and its focused
validation below, not an integrated M4 release. Historical #37 typed-array and
#63 M3 release records retain their original scope.

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

`codegen/resizable.rs` structurally discovers constructor uses (including nested
callables and skipped/unreachable nodes), plans helper/type/global indices before
checked body lowering, and emits constructors in number/bool/string order for the
required types. Only the required M3 core is promoted to production-internal
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
authority records, payloads and unreachable arrays remain retained. There is no
relocation path yet, so existing saved-address indexed writes remain unchanged;
the future post-RHS revalidation protocol must accompany the first relocation slice.

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
No historical record substitutes for those runs on final #65 sources.

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
