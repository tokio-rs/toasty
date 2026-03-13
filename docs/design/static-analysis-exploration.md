# Static Analysis and Formal Verification for Toasty

## Problem

Toasty's query engine compiles user queries through multiple phases (simplify →
lower → plan → execute). Each phase relies on invariants that are currently
enforced only through `assert!`, `debug_assert!`, and `unwrap()` calls —
runtime checks that catch violations late, produce poor diagnostics, and are
absent in release builds (`debug_assert!`). Some invariants are not checked at
all.

Examples of invariants that exist today only as implicit assumptions:

- **Variable slot lifecycle**: Each `VarId` is stored exactly once and loaded
  exactly `num_uses` times before being freed (`engine/exec/var.rs`)
- **Type-value consistency**: A `stmt::Value::I64(x)` is only stored in a slot
  typed `stmt::Type::I64` (`engine/exec/var.rs:70-74`)
- **Dependency graph acyclicity**: After cycle-breaking, the MIR node graph is
  a DAG (`engine/plan.rs`)
- **OnceCell single-write**: `load_data_columns` is set exactly once during
  planning (`engine/hir.rs:82`)
- **Expression well-formedness**: Free-evaluated expressions contain only `Arg`,
  `Value`, and operators — no `Reference` or field-access nodes (`engine/eval.rs:73-95`)

The goal is to move some of these invariants from runtime assertions into
static verification that runs at compile time or as a separate verification
step, catching violations before they reach production.

## Tool Landscape

Four Rust verification tools exist at different maturity levels. Each takes a
different approach and covers different kinds of properties.

### Kani (Model Checking)

[Kani](https://github.com/model-checking/kani) is a bit-precise model checker
maintained by AWS. It translates Rust to CBMC (C Bounded Model Checker) and
exhaustively explores all possible inputs within bounded domains.

**How it works**: You write proof harnesses (similar to test harnesses) using
`#[kani::proof]`. Kani replaces concrete inputs with symbolic values via
`kani::any()` and explores all possible execution paths. It also supports
function contracts (`#[kani::requires(...)]`, `#[kani::ensures(...)]`) for
modular verification.

```rust
#[cfg(kani)]
#[kani::proof]
fn check_type_value_consistency() {
    let ty: stmt::Type = kani::any();
    let value: stmt::Value = kani::any();

    // If is_a succeeds, the type and value must agree
    if value.is_a(&ty) {
        match (&value, &ty) {
            (stmt::Value::I64(_), stmt::Type::I64) => {}
            (stmt::Value::String(_), stmt::Type::String) => {}
            // ... exhaustive matching
            _ => unreachable!("is_a accepted incompatible pair"),
        }
    }
}
```

**What it checks**: Panics, arithmetic overflow, out-of-bounds access, custom
assertions, function contracts. All checks are exhaustive within the bounded
domain.

**Maturity**: Production-quality. Monthly releases throughout 2025 (v0.59–v0.66).
Used to verify parts of the Rust standard library. Supports stable Rust patterns
(requires nightly toolchain internally but works with stable-compatible code).

**Limitations**: Bounded verification — loops and recursion need bounds.
Complex data structures (like Toasty's `stmt::Expr` enum tree) can cause state
explosion. No concurrency support. Verification time grows with input domain
size.

**Fits Toasty for**: Verifying type-value consistency (`is_a`), schema
verification invariants, expression well-formedness checks, integer conversion
safety in `Type::cast`.

### Flux (Refinement Types)

[Flux](https://github.com/flux-rs/flux) adds refinement types to Rust. You
annotate types with logical predicates (e.g., `i32{v: v > 0}`) and Flux
verifies them statically using an SMT solver.

**How it works**: Flux is a Rust compiler plugin. You add refinement annotations
to function signatures and type definitions. Flux infers loop invariants
automatically in many cases (liquid type inference).

```rust
#[flux::sig(fn(slots: &mut Vec<Option<Entry>>, var: VarId{v: v < slots.len()}) -> Entry)]
fn load_slot(slots: &mut Vec<Option<Entry>>, var: VarId) -> Entry {
    slots[var.0].take().unwrap()
}
```

**What it checks**: Index bounds, non-zero/positivity constraints, size
relationships between containers, numeric range invariants. Checks are fully
static — no runtime cost.

**Maturity**: Research tool with active development. Used to verify process
isolation in Tock OS (a production microcontroller OS). Generic refinement types
added in 2025. Requires nightly Rust.

**Limitations**: Only works on safe Rust. Cannot express properties about enum
variants or complex data structure shapes. Limited trait support. The annotation
surface area is smaller than Kani's — you can only express properties that fit
the refinement type framework (decidable arithmetic/set predicates).

**Fits Toasty for**: Index bounds safety on `VarId` slot access, ensuring
`Vec` lengths match between `tys` and `slots` in `VarStore`, numeric range
invariants on schema IDs.

### Prusti (Deductive Verification)

[Prusti](https://github.com/viperproject/prusti-dev) is a deductive verifier
from ETH Zurich. It translates Rust to the Viper intermediate verification
language and uses SMT solvers to prove pre/postconditions and loop invariants.

**How it works**: You add `#[requires(...)]` and `#[ensures(...)]` attributes
to functions. Prusti verifies each function modularly — it checks that the
postcondition holds assuming the precondition, using Rust's ownership model to
avoid separation logic.

```rust
#[requires(self.slots.len() == self.tys.len())]
#[ensures(self.slots.len() == self.tys.len())]
fn store(&mut self, var: VarId, count: usize, rows: Rows) {
    // ...
}
```

**What it checks**: Absence of panics, integer overflow, pre/postconditions,
loop invariants, data structure invariants.

**Maturity**: Research prototype. Actively developed with academic funding
(ETH Zurich, Amazon, Meta). Targets safe Rust only. A 2023 case study found
practical limitations with mutable indexing patterns. Has a VS Code extension.

**Limitations**: Prototype status — covers a subset of Rust. Mutable indexing
requires workarounds. No unsafe Rust support. Higher annotation burden than
Flux for equivalent properties.

**Fits Toasty for**: Function-level contracts on schema verification methods,
invariants on the `Verify` struct methods.

### Creusot (Deductive Verification)

[Creusot](https://github.com/creusot-rs/creusot) is a deductive verifier from
Inria. It translates safe Rust to Why3 and uses prophecies to reason about
mutation through references.

**How it works**: Similar to Prusti (pre/postconditions, loop invariants) but
uses a prophecy-based model for mutable references. Generates Why3 verification
conditions discharged by SMT solvers.

**Maturity**: Less mature than Prusti. Used to verify CreuSAT (a SAT solver),
which demonstrates it can handle non-trivial programs. Active research in 2025
(new Coma intermediate language). Requires nightly Rust.

**Fits Toasty for**: Similar to Prusti. The prophecy model could handle
Toasty's `Cell`-based mutation patterns better than Prusti, but this is
speculative.

### Miri (UB Detection)

[Miri](https://github.com/rust-lang/miri) is an interpreter for Rust's MIR
that tracks pointer provenance, type validity, and aliasing rules at runtime.
It runs your existing test suite under interpretation with no code changes.

**What it detects**: Out-of-bounds access, use-after-free, unaligned reads,
aliasing violations (Stacked Borrows model), data races, invalid type
invariants (e.g., `bool` that isn't 0 or 1).

**Maturity**: Part of the official Rust toolchain. Published at POPL 2026.
Integrated into Rust standard library CI. Install with
`rustup component add miri`, run with `cargo +nightly miri test`.

**Limitations**: 10-100x slower than normal execution. Only finds bugs on
executed code paths (not exhaustive). Toasty's core has no `unsafe` code, so
Miri's value here is moderate — it still catches issues in dependencies and
driver crates.

### cargo-careful (Extra Runtime Checks)

[cargo-careful](https://github.com/RalfJung/cargo-careful) rebuilds the
standard library with debug assertions enabled. It catches things that debug
mode alone misses: `NonNull::new_unchecked` on null, `unreachable_unchecked`
on reachable paths, collection internal consistency violations.

**Maturity**: Stable, maintained by Ralf Jung. Install with
`cargo install cargo-careful`, run with `cargo +nightly careful test`.

**Effort**: Zero annotation. Faster than Miri.

### Bolero (Unified Testing Frontend)

[Bolero](https://github.com/camshaft/bolero) provides a single harness
interface that dispatches to multiple backends: random testing (like proptest),
coverage-guided fuzzing (libfuzzer, AFL), or Kani model checking.

```rust
#[test]
fn check_type_roundtrip() {
    bolero::check!().with_type::<SimpleValue>().for_each(|value| {
        let ty = value.infer_ty();
        assert!(value.is_a(&ty));
    });
}
```

Write the harness once, then run it as a random test in CI, under libfuzzer
for deeper coverage, or under Kani for bounded proofs. The `proptest` crate
(already a workspace dependency) provides similar random-testing capabilities
without the multi-backend support.

**Maturity**: Actively maintained. Works on stable Rust for random testing;
nightly for fuzzing and Kani backends.

## Toasty Invariants and Tool Fit

The table below maps each invariant to the tool best suited to verify it.

| Invariant | Location | Current Check | Best Tool | Why |
|---|---|---|---|---|
| Type-value consistency (`is_a`) | `exec/var.rs:70-74` | `assert!` | Kani | Enumerate type/value pairs exhaustively |
| VarId slot bounds | `exec/var.rs:46-47` | panic on OOB | Flux | Refinement type on VarId constrains range |
| `slots.len() == tys.len()` | `exec/var.rs` | none | Flux | Length-indexed refinement types |
| Expression well-formedness | `eval.rs:73-95` | `verify_expr` returns bool | Kani | Explore all Expr shapes, prove no Reference leaks |
| Filter is boolean | `verify.rs:129-143` | `panic!` | Kani | Enumerate Expr variants in filter position |
| Schema IDs populated | `schema/verify.rs:48-87` | `debug_assert!` | Kani | Prove no placeholder IDs survive schema building |
| Index scoping order | `schema/verify.rs:89-107` | `panic!` | Kani | Prove local-then-nonlocal ordering |
| DAG after cycle-breaking | `plan.rs` | none | Kani | Build symbolic graphs, verify no cycles |
| num_uses matches load count | `mir/node.rs:34` | none | Prusti/Creusot | Requires per-function contracts with counting |
| Integer cast safety | `stmt/ty.rs:280-340` | `todo!` on unknown pair | Kani | Explore all (Value, Type) pairs for cast |
| Auto-increment field type | `schema/verify.rs:165-202` | `Err(...)` return | Kani | Verify all auto-increment columns are numeric |

## Recommendation

Adopt tools in order of effort-to-value ratio:

### Phase 1: Zero-effort runtime checks

Add `cargo +nightly miri test` and `cargo +nightly careful test` to CI. These
require no code changes and catch UB, aliasing violations, and standard library
misuse in existing tests.

### Phase 2: Kani proof harnesses

**Kani is the best fit for Toasty's invariants.** It is the most mature
verification tool, requires the least annotation overhead, and covers the
widest range of properties.

Kani proof harnesses live alongside tests — they use `#[cfg(kani)]` so they
have zero impact on normal builds. The workflow is:

1. Install: `cargo install --locked kani-verifier && cargo kani setup`
2. Write a `#[kani::proof]` harness next to the code it verifies
3. Run: `cargo kani --harness <name>`

Start with these three harnesses (highest value, lowest effort):

1. **`is_a` consistency** — verify that `Value::is_a(&Type)` agrees with the
   structural match between value variants and type variants
2. **`Type::cast` completeness** — verify that casting between supported type
   pairs does not panic
3. **Schema verification coverage** — verify that `verify_ids_populated` catches
   all placeholder IDs

### Phase 3: Property-based testing

Use `proptest` (already a workspace dependency) or `bolero` to write strategies
for schema types and AST nodes. Properties to test:

- Simplify(expr) preserves semantics on concrete inputs
- Lower → Plan never panics for valid HIR input
- Schema serialization roundtrips losslessly

### Phase 4: Refinement types (experimental)

After building experience with Kani, consider adding Flux refinement types to
`VarStore` for index bounds safety — this gives static (compile-time)
guarantees rather than Kani's bounded verification.

## Kani Integration Plan

### File Organization

Place proof harnesses in the same file as the code they verify, gated behind
`#[cfg(kani)]`:

```rust
// Bottom of stmt/ty.rs
#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    fn is_subtype_of_is_reflexive() {
        // For simple types, is_subtype_of(self) is always true
        let ty: Type = kani::any();
        kani::assume(!matches!(ty, Type::List(_) | Type::Record(_) | Type::Union(_)));
        assert!(ty.is_subtype_of(&ty));
    }
}
```

### CI Integration

Add a `cargo kani` step to CI that runs all proof harnesses. Kani harnesses are
independent of database drivers — they verify internal logic only.

### Dependency

Add `kani-verifier` as a dev-dependency (or install it in CI only). The
`#[cfg(kani)]` gate means Kani is never compiled into normal builds.
