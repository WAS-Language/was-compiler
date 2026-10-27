# M2 reusable temporary region prototype (#54)

Implemented as an explicitly test-invoked experiment. Ordinary compilation,
numeric presets, CLI and browser adapters retain revision-1 storage behavior.
There is no public option, feature, AST field or new dependency. Integrated M2
hardening, measurements and release validation remain follow-on #55–#57 work;
this prototype is not public activation or a performance claim.

## Selection and checked lowering

`crates/compiler/src/codegen/region.rs::compile` is private under `cfg(test)`.
It uses existing layout, `ValidatedProgram` body checking/caching and assembly.
`lifetime::temporary_literals` exposes borrowed positive M1 facts only; M1 remains
classification, not storage selection. Eligibility is exactly the
[M1 invocation-wide closed subset](m1-lifetime-analysis.md#closed-positive-subset),
including unsupported and unreachable syntax. Checked lowering still validates
all bodies and children independently of those facts.

Each selected function reserves every candidate's individually aligned footprint:
number length N uses `8 + 8*N`, bool uses `align8(8 + N)`, and empty arrays use 8.
Checked summation includes unreachable suffixes. `Selection::CapacityExceeded`
is a deterministic private whole-function baseline fallback, not a compilation
error. No management header is stored in linear memory: management overhead is
one private mutable i32 global per selected function and one i32 local per
invocation. Global zero remains the ordinary allocator cursor. No selected
functions means no emitted region machinery.

## Ownership and completion

The first executed literal lazily takes and clears its function's cache slot,
or provisions the full block through the existing checked allocator. Ordinary
allocations cannot use its interior. Each literal has a fixed distinct offset;
its descriptor is initialized before element evaluation, and existing checked
lowering evaluates/stores elements once, left-to-right. Reuse reinitializes all
executed arrays. Reserved unreachable slices do not execute their initializers.

Explicit scalar/bare returns and void fallthrough retire the block only on normal
completion, after evaluating a scalar return. No executed literal means no
acquisition or retirement. Retirement preserves any scalar already on the operand
stack. There is no heap rewind, cross-function pooling, intra-invocation reuse,
relocation or growth of an acquired region.

A trap skips retirement and leaves the cache empty: the entire owned block is
quarantined for the instance lifetime, preserving completed writes. Later calls
provision another block. Repeated traps can exhaust memory; bounded reuse applies
to successful calls, not trapping workloads. Failed provisioning traps without
per-literal fallback and preserves the allocator cursor and existing storage.
Front-loaded reservation/growth/failure and whole-block retention after traps are
accepted experimental resource differences.

Experimental temporary addresses discovered through exported raw memory do not
acquire revision-1 instance-lifetime retention rights after normal retirement.
Ordinary allocations and legitimately published handles retain those rights.
Descriptor checks and host view-refresh obligations are unchanged.

## Test map

Library unit tests keep activation inside `cfg(test)`; integration dependencies
cannot activate it. Node tests remain ignored by default and use bounded process
support with finite guest workloads. Both numeric policies run runtime cases.

- `region::selected_function_has_private_cache_and_checked_array_stores`:
  Wasm validation, independent cache-global and typed-store decoding.
- `region::tests`: independent aligned sizes and offsets, unreachable capacity,
  acquisition/clear/allocation/store/return order, no heap rewind, deterministic
  bytes, enabled-preset parity, export selection, exclusions, no-machinery byte
  identity and independent forged-tree diagnostics. Capacity overflow uses
  synthetic footprints through the actual storage-selection transition, avoiding
  a multi-GiB AST allocation.
- `region::runtime_tests`: number/bool/empty arrays and bool alignment boundaries,
  simultaneous literals, changing parameter initialization, reads/replacements,
  all five numeric compounds, early/bare returns and void fallthrough, no executed
  literal, successful reuse without region-driven heap growth, trap quarantine
  during reads and initialization, completed writes and ordinary retained handles.
  A capped one-page module checks front-loaded provisioning failure without
  cursor corruption or per-literal fallback.

## Validation

Run the focused prototype including its ignored Node cases:

```sh
cargo test --offline -p was-compiler --lib codegen::region -- --include-ignored
```

Also preserve M1, memory and typed-array regressions and the repository's
[offline fast/full validation workflow](validation.md#finite-direct-commands).
Rebuild browser bindings before current-source browser or full ignored validation.
Broader adversarial isolation and measurement claims belong to #55/#56, with
integrated M2 evidence in #57.

### Implementation validation — 2026-10-27

All commands below completed successfully offline. The initial decoded test failed
against baseline emission (one global instead of the required two), then passed
with the private storage path. Focused tests were rerun after the review fix.

| Command | Result |
| --- | --- |
| `cargo check --offline -p was-compiler --lib` | Passed; production core builds without activation |
| `cargo test --offline -p was-compiler --lib codegen::region -- --include-ignored` | 8 passed, none failed/ignored; includes 3 bounded Node cases |
| `cargo test --offline -p was-compiler --lib lifetime -- --include-ignored` | Passed, including Node regressions |
| `cargo test --offline -p was-compiler --lib codegen::memory_tests -- --include-ignored` | 21 passed, none failed/ignored |
| `cargo test --offline -p was-compiler --test typed_arrays_lowering` | 2 passed |
| `cargo test --offline` | Passed; default ignored policy preserved |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed |
| `cargo build --offline -p was-compiler --lib --target wasm32-unknown-unknown` | Passed |
| `cargo test --offline -p was-compiler --test workspace_contract` | 3 passed |
| `wasm-pack build crates/browser --target web --out-dir ../../pkg --out-name was_compiler --mode no-install --no-opt --offline` | Passed; current-source package rebuilt |
| `node tests/browser.cjs pkg` | 25 groups passed |
| `cargo test --offline --workspace -- --include-ignored` | Passed; all packages and ignored runtimes selected after browser rebuild |
| `cargo clippy --offline -p was-compiler-browser --target wasm32-unknown-unknown --lib -- -D warnings` | Passed |

The browser build printed its usual wasm-bindgen tooling-resolution message and
optional missing package metadata notice; no separate installer was invoked.
Compatibility fixtures were not regenerated. These checks validate #54's working
prototype and ordinary-path noninterference, not the follow-on M2 exit gates.

Two independent review axes examined the implementation against starting commit
`3c237d637a674c41cc4a224aa7002129846a4d50`, excluding the pre-existing roadmap edit.
Standards found one duplicated alignment-policy constant; it was replaced by
`abi::ALLOCATION_ALIGNMENT`. Spec reported zero findings. No public activation,
proof widening or revision-1 ownership change is introduced.
