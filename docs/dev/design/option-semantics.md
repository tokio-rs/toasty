# Option values and database nulls

## Summary

Application values use `Option<T>`, and application predicates return `bool`.
Toasty follows Rust's rules for `Some` and `None`. When both values are
present, their comparison follows the target database's rules. Database
nullability belongs to the storage mapping and driver interfaces. This
document defines the target contract; implementation follows in separate PRs.

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

The first query matches present nicknames the database considers equal to
`"hello"`. Under a case-insensitive MySQL collation, this can include
`Some("HELLO")`. The second query matches every other nickname, including
`None`. The third is equivalent to `is_none()`.
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

Option equality checks presence first:

| Left | Right | `eq` | `ne` |
|---|---|---|---|
| `None` | `None` | `true` | `false` |
| `None` | `Some(b)` | `false` | `true` |
| `Some(a)` | `None` | `false` | `true` |
| `Some(a)` | `Some(b)` | `value_eq(a, b)` | `!value_eq(a, b)` |

Apply the presence rules at every Option layer. For present scalar values,
`value_eq` means the target database's equality for the mapped values,
including their type and collation. Adding `Option` to a type changes how
absence works and preserves how its present values compare.

For example, string equality can ignore case or trailing spaces under some
MySQL and MariaDB collations. PostgreSQL considers floating-point NaN equal
to itself, so `Some(NaN)` equals `Some(NaN)` there. These differences remain
part of the database's behavior.

Tuples and embedded values compare their corresponding application members,
applying the same presence rules to optional members. Present scalar members
use database equality. Relations compare model identity using their declared
reference keys, including non-primary references.

`in_list` and `in_query` succeed when at least one candidate compares equal
under these rules. An absent candidate matches an absent subject. Duplicates
do not affect membership, and an empty candidate set yields `false`.
Subquery limits and offsets determine the candidate set before membership.
Native collection and JSON operators retain their own comparison contracts.

`ne(a, b)` is `not(eq(a, b))`. Predicates compose with ordinary boolean
`and`, `or`, and `not`; the contract applies in filters, projections,
relation predicates, conditional writes, and client evaluation. A variant
guard belongs inside the complete predicate being negated.

Where ordered comparisons are supported, `None < Some(_)`. Present values
use the database's ordering. Ascending sorting places absence first;
descending sorting places it last. Cursor pagination uses the same ordering
and the database's equality for tied sort values, with a unique tie-breaker.
For example, PostgreSQL's NaN values compare equal and sort after other float
values. Pagination must use those rules too. A pagination form without a
deterministic order and compatible cursor comparisons returns an
`unsupported_feature` error.

Optional string predicates return `false` for `None`; present values retain
the operation's contract, including native `LIKE` and `ILIKE` matching rules.

Assigning `None` clears an optional field. Omitting an update assignment
leaves it unchanged; omitting a create field allows its default to apply.
Field presence and row presence are separate. Selecting an optional nickname
and calling `.first()` returns `Option<Option<String>>`:

- `None`: no user was found.
- `Some(None)`: a user was found, but they have no nickname.
- `Some(Some("Sam"))`: a user was found, and their nickname is `"Sam"`.

These states remain distinct. Likewise, an absent address differs from a
present address whose optional fields are all absent. For `Deferred`, a value
that has not been loaded differs from one that was loaded and found absent.

## Edge cases

Stored nested options require a mapping that keeps every presence state.
Reject unsupported nesting at model compilation, including through aliases
and transparent wrappers. Nested query results remain valid.

A supported storage conversion must not silently turn a present value into
absence. A driver may reject a value it cannot store. Invalid stored data
produces a decoding error instead of `None`.

Equality of absent values does not create a relationship. Relation lookup
requires a present reference key, including every part of a composite key.
JSON `null` inside a present JSON value is also distinct from field absence.

## Driver integration

Drivers receive lowered database operations and preserve the target's
native operator semantics, including null propagation. Application equality
must not redefine database equality used for joins or other native operations.
No new driver operation or capability is required by this contract; concrete
interface additions, if needed, belong to the implementation proposals.

Lowering adds presence handling around the database's value comparisons and
membership operators. Native null-safe operators are suitable when they
preserve both the presence rules and the value comparison. Presence handling
must preserve the existing collation and float comparison rules.

Optimizations and client-side query evaluation must preserve the same
database comparison results. A comparison may be evaluated in Rust only
when doing so agrees with the target database; otherwise it must stay in the
database or use an equivalent evaluation for that backend. This also applies
to comparisons used for projections, membership, and relation merging.
Rewrites preserve evaluation counts for volatile operands.

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
