# Integer Enum Storage

## Summary

Integer-discriminant embedded enums may select an integer database type with
`#[column(type = ...)]`. The selection applies to flattened discriminant
columns and unit-enum collection elements without changing the enum's `i64`
application representation.

## Motivation

Integer-discriminant enums previously always used the inferred `i64` database
type. A model could not map a small enum to an existing `TINYINT` column or
avoid a wider column than its values require. The same storage decision must
remain consistent when the enum is nested, wrapped, or used in a collection.

## User-facing API

Set a default storage type on the enum:

```rust
#[derive(toasty::Embed)]
#[column(type = u8)]
enum Priority {
    #[column(variant = 10)]
    Low,
    #[column(variant = 20)]
    High,
}
```

Every flattened `Priority` field uses the requested type. The setting follows
the enum through flattened embeds and transparent field wrappers such as
`Option`, `Deferred`, `Box`, `Arc`, and `Rc`.

A field-level type overrides the enum default for one use:

```rust
#[derive(toasty::Model)]
struct Task {
    #[key]
    id: u64,

    priority: Priority,

    #[column(type = u16)]
    imported_priority: Priority,
}
```

Unit enums are scalar collection elements. The enum default applies to each
element, and a field-level override selects a different element type:

```rust
#[derive(toasty::Model)]
struct Schedule {
    #[key]
    id: u64,

    priorities: Vec<Priority>,

    #[column(type = u16)]
    imported_priorities: Vec<Priority>,
}
```

Before this change, the enum-level type was unavailable and a field-level type
worked only for direct enum fields. Collection fields always retained their
inferred element width.

## Behavior

Integer enum discriminants remain `i64` in Rust expressions and the app
schema. Toasty converts between that representation and the selected database
type when writing and reading a column.

Storage precedence is field override, then enum default, then inferred `i64`
storage. For `Vec<unit-enum>`, Toasty applies that rule to the element and
stores a list of the resulting type.

Every discriminant must fit the selected signed or unsigned width. Enum-level
errors point to the variant declaration. Field-level overrides are checked by
generated trait bounds, including overrides behind transparent wrappers and
on collections. The schema builder retains range checks for schemas not
produced by the derive macros.

## Edge cases

- Integer discriminants are explicit, non-negative `i64` values and need not
  be sequential.
- `int(N)` and `uint(N)` use their exact N-byte range, including non-standard
  widths.
- Only unit enums can be scalar collection elements. Data-carrying enums map
  to multiple columns.
- Toasty currently rejects enum embeds inside `#[document]` fields, so an
  integer storage selection does not describe a value inside JSON.
- SQL `DEFAULT` remains untyped and bypasses application-to-storage casts.

## Driver integration

This feature adds no driver capability or operation. The database schema
continues to expose both the physical `db::Type` and the closest `stmt::Type`
used at the driver boundary. `db::Type::bridge_type` applies the scalar
conversion recursively to list elements.

Drivers keep their existing type mappings. A driver without an exact unsigned
type may use its existing closest supported representation. Out-of-tree
drivers need no API changes.

## Alternatives considered

Applying the enum type only to direct fields was rejected because transparent
wrappers and nested flattened fields represent the same discriminant.
Ignoring it for `Vec<unit-enum>` was also rejected: collection elements are
the same scalar values and would otherwise silently use a wider type.

Encoding enum collections as documents was rejected because unit-enum
collections already have scalar collection semantics and native array support
where the backend provides it.

## Open questions

None.

## Out of scope

- Enum encoding inside `#[document]` fields. Document storage does not expose
  a physical discriminant column.
- Collections of data-carrying enums. Their values span multiple fields and
  are not scalar collection elements.
