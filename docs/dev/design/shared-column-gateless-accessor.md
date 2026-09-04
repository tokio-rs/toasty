# Shared-column gateless accessor

## Summary

Variant fields that declare the same `#[shared(name)]` are stored in one nullable column, but today they can only be queried one variant at a time. This adds one accessor per shared ident on the enum's fields struct, named after the ident, that reads the shared column across all variants with no discriminant gate, in filters and `order_by`.

## Motivation

The `character_creature` scenario stores both variants' names in one `creature_name` column, under different Rust field names:

```rust
enum Creature {
    Human { full_name: String, profession: String },
    Animal { nickname: String, species: String },
}
```

Every accessor is variant-rooted and gated, so a cross-variant query must OR one predicate per variant, naming each variant's Rust field:

```rust
// Before: one branch per variant.
Character::fields().creature().human().full_name().eq("Bob")
    .or(Character::fields().creature().animal().nickname().eq("Bob"))
```

That grows with variant count, forces callers to know each variant's field name, and has no single-column form for `order_by`. The gated filter stays for the single-variant case; the cross-variant case gets one path:

```rust
// After: one read of the shared column.
Character::fields().creature().name().eq("Bob")
```

## User-facing API

Call the shared ident on the enum's fields struct, never after a variant:

```rust
// Any creature named "Bob", regardless of variant.
Character::all().filter(Character::fields().creature().name().eq("Bob"));

// Sort by the shared column.
Character::all().order_by(Character::fields().creature().name().asc());
```

Variant-gated access stays unchanged for the single-variant case:

```rust
// Only humans named "Bob" (cf. shared_column_variant_gated_filter).
Character::all().filter(
    Character::fields().creature().human().full_name().eq("Bob")
);
```

The gateless accessor is read-only: it builds filter and `order_by` expressions only, with no create or update setter. If the ident collides with anything else on the enum's fields struct (variant accessors, `is_*` guards, `eq`/`ne`/`in_list`), the macro emits a compile error. Same-variant double-share, disagreeing types, and disagreeing column overrides stay compile errors as today.

## Behavior

The shared read lowers to a plain column comparison (`col = v`) and `ORDER BY col`, with no gate. Variant-gated access keeps its gate for every variant field — including fields after a variant's first, which the engine previously rejected outright (a gated read that must distribute across a unit sibling still panics; see `shared_column_gated_order_by_unit_sibling`). Rows whose variant does not declare the ident hold `NULL`: `NULL` matches neither `eq` nor `ne`; use `is_none()` / `is_some()` for `NULL` checks. Uniqueness follows the column: only enum-level `#[unique(name)]` applies; field-level `#[unique]` on a sharing member stays rejected. Updates are unchanged: switching variants is full replacement, with no patch through the gateless path.

## Edge cases

Non-declaring variants read `NULL`; equality never matches them and ordering follows the backend's `NULL` placement. A `NULL` discriminant row reads `NULL` for every shared field. Unit variants never participate.

## Driver integration

None. SQL drivers receive an ordinary column reference; no new `Operation`, no new syntax, existing indexes apply. Lowering resolves the shared read to a column before drivers see it, so out-of-tree drivers need no changes.

## Alternatives considered

Status quo OR-of-gated-predicates (plus `CASE` for sort). Correct for filters, as the existing cross-variant test shows, but duplicates one branch per variant, defeats index use, and rebuilds ordering from gated leaves.

Sentinel index in the projection for the shared slot. Same engine work with an extra constant threaded through core, macros, and engine; harder to read, no smaller.

Encoding the shared read's trailing step as the representative's plain flattened field index. It collides with a variant-gated read's record position, so lowering cannot tell them apart; offsetting the shared step past every per-variant record position keeps the two encodings disjoint.

## Open questions

None.

## Out of scope

* Nested parents (`struct -> struct -> enum`, `enum -> enum`) — same shape, separate lowering and test models; why: keeps this review to the `character_creature` shape.
* Variant switching via patch — full replacement only; why: patch is within-variant by design.
* Tuple-variant column naming — covered by `enums-and-embedded-structs.md` §1; why: separate naming problem.
