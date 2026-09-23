# Option values and database nulls

## Summary

Application values use Rust `Option<T>`, and application predicates return
`bool`. Database nullability belongs to the storage mapping and driver
interfaces. Lowering preserves application results while database operations
retain their native semantics. This document defines the target contract;
implementation follows in separate PRs.

## Motivation

Given an optional nickname, `eq("hello")` and `ne("hello")` should partition
the records. SQL null propagation can exclude an absent nickname from both
results. [#188] reports this problem for comparisons between nullable
expressions; rewriting comparisons with literal `None` alone cannot solve it.

## User-facing API

Use `Option<T>` when a field may be absent. For a `nickname: Option<String>`
field, pass either an option or a present value:

```rust
User::filter(User::fields().nickname().eq("hello"));
User::filter(User::fields().nickname().ne("hello"));
User::filter(User::fields().nickname().eq(None::<String>));
```

The first query matches `Some("hello")`; the second matches every other
nickname, including `None`. The third is equivalent to `is_none()`.
A required value, path, or expression supplied to an optional comparison
implicitly lifts into `Some`. Elision never unwraps an option or omits a
predicate.

Membership uses the same equality:

```rust
let choices = [None, Some("hello".to_owned())];
let selected = User::fields().nickname().in_list(choices);
User::filter(selected.clone());
User::filter(selected.not());
```

These queries partition the records into nicknames contained in `choices`
and those excluded from it. `in_query` compares against the returned
candidate values; required candidates lift into `Some` for an optional
subject. Optional relation paths also expose `is_none()` and `is_some()`.

Before this contract, callers may explicitly include absence with
`nickname.is_none().or(nickname.ne("hello"))`. Under this contract,
`nickname.ne("hello")` suffices. To select only present, different values,
use `nickname.is_some().and(nickname.ne("hello"))`.

## Behavior

Equality follows Rust's `Option` semantics:

| Left | Right | `eq` | `ne` |
|---|---|---|---|
| `None` | `None` | `true` | `false` |
| `None` | `Some(b)` | `false` | `true` |
| `Some(a)` | `None` | `false` | `true` |
| `Some(a)` | `Some(b)` | `a == b` | `a != b` |

Scalar payloads use Rust equality. Tuples and embedded values compare their
corresponding application members. Relations compare model identity using
their declared reference keys, including non-primary references.

Membership is `candidates.iter().any(|candidate| subject == candidate)`.
Duplicates do not affect it, and an empty candidate set yields `false`.
Subquery limits and offsets determine the candidate set before membership.

`ne(a, b)` is `not(eq(a, b))`. Predicates compose with ordinary boolean
`and`, `or`, and `not`; the contract applies in filters, projections,
relation predicates, conditional writes, and client evaluation. A variant
guard belongs inside the complete predicate being negated.

Where ordered comparisons are supported, `None < Some(_)` and present
payloads use Rust's partial ordering. Ascending sorting places absence first;
descending sorting places it last. Cursor pagination follows the same order.
Optional string predicates return `false` for `None`; present values retain
the operation's contract, including native `LIKE` and `ILIKE` matching rules.

Assigning `None` clears an optional field. Omitting an update assignment
leaves it unchanged; omitting a create field allows its default to apply.
Selecting `Option<T>` and calling `.first()` returns `Option<Option<T>>`:
`None` means no row, `Some(None)` means a row with an absent field, and
`Some(Some(value))` means a row with a present field.

## Edge cases

Presence levels must remain distinct: `Some(None) != None`, and a present
embedded value with absent members differs from an absent embed. Stored
nested options require a lossless mapping; reject unsupported nesting at
model compilation, including through aliases and transparent wrappers.
Nested query results remain valid. Unloaded `Deferred` state is separate
from loaded absence, and malformed stored data produces a decoding error.

Rust equality distinguishes string case and trailing spaces; NaN is unequal
to itself. A supported storage conversion must preserve these values and
comparisons or return an unsupported-feature error. It must not silently
turn a present value into absence.

Equality of absent values does not create a relationship. Relation lookup
requires a present reference key, including every part of a composite key.
JSON `null` inside a present JSON value is also distinct from field absence.

## Driver integration

Drivers receive lowered database operations and preserve the target's
native operator semantics, including null propagation. Application equality
must not redefine database equality used for joins or other native operations.
No new driver operation or capability is required by this contract; concrete
interface additions, if needed, belong to the implementation proposals.

Lowering accounts for presence before choosing database comparisons and
membership operators. Native null-safe operators are suitable only when
their payload comparison also agrees with the application contract.
Rewrites must preserve evaluation counts for volatile operands.

SQL nulls and omitted DynamoDB attributes can encode absence; DynamoDB's
explicit `NULL` attributes also decode as absence. Presence predicates agree
with decoding. Embedded mappings may need additional presence information.
Drivers may reject unsupported query forms, but successful execution must
preserve the application contract.

## Alternatives considered

Retaining SQL three-valued logic would require optional boolean results and
backend-specific reasoning in application predicates. Rewriting only literal
`None` misses field comparisons and membership. Treating unknown as false
only at the final filter still gives incorrect negation and projections.

## Open questions

- **Deferrable:** a lossless storage mapping for nested options. Rejecting
  unsupported stored nesting defines the behavior until one exists.
- **Deferrable:** opaque public database-expression adapters. Existing
  low-level adapters remain an explicit database boundary.

## Out of scope

Executing arbitrary user-written `PartialEq` implementations, emulating
native pattern matching, and changing database uniqueness, grouping,
aggregates, conflict detection, or native JSON operators each require
separate contracts.

[#188]: https://github.com/tokio-rs/toasty/issues/188
