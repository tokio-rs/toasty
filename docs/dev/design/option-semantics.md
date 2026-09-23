# Option values and database nulls

## Summary

Toasty defines application expressions in terms of Rust values.
`Option<T>` contains `None` or `Some(T)`, and comparisons and membership produce
`bool`. Database `NULL` belongs to the storage and driver interfaces. Lowering
translates application operations into database expressions that preserve
their results. The [implementation record](../roadmap/option-semantics.md)
describes the API changes and regression coverage.

## Motivation

Given an optional nickname, `.eq("hello")` and `.ne("hello")` should partition
the records. An ordinary SQL inequality can exclude a record with no nickname from
both results. Similarly, an inclusion list containing `None` should match a
record whose value is `None`.

[#188] reports comparisons between optional values inheriting SQL's
three-valued logic. [#797] reports nested options losing information, and
[#1246] reports missing presence predicates on optional relations. These are
related problems: application optionality needs a contract independent of its
storage representation.

## User-facing API

### Compare optional values

Declare an optional value with `Option<T>`. Use `eq`, `ne`, `is_some`, and
`is_none` on its field path or expression:

```rust
#[derive(toasty::Model)]
struct User {
    #[key]
    id: uuid::Uuid,
    nickname: Option<String>,
}

User::filter(User::fields().nickname().eq(Some("hello".to_owned())));
User::filter(User::fields().nickname().eq(None::<String>));
User::filter(User::fields().nickname().is_none());
```

The last two queries select the same records. A comparison produces
`Expr<bool>`, including when either operand is optional.

When an operand expects `Option<T>`, a value of `T` is shorthand for `Some(T)`:

```rust
User::filter(User::fields().nickname().eq("hello"));
User::filter(User::fields().nickname().ne("hello"));
```

The first query selects users whose nickname is `Some("hello")`. The second
selects every other user, including users with no nickname. Elision only adds
`Some`; it never unwraps an option, supplies a default, or drops a filter.
Passing `None` to a generated `filter_by_*` method means comparing with `None`.

The same coercion applies to compatible required paths and expressions used
as the right operand of an optional comparison, and to values within lists. Callers can provide an explicit
type when a bare `None` is ambiguous.

### Test membership

Membership uses the same equality as `eq`:

```rust
let choices = [Some("hello".to_owned()), None];
let selected = User::fields().nickname().in_list(choices);
User::filter(selected.clone());
User::filter(selected.not());
```

The first query selects `Some("hello")` and `None`. The second selects all
other nicknames. `in_query` uses the same rule for the values returned by its
subquery. The rule also applies to tuple membership and supported collection
operations such as `contains`.

When the subject is `Option<T>`, a subquery returning `List<T>` implicitly
lifts each returned value to `Some(T)`. A subquery returning `List<Option<T>>`
retains its explicit absent values.

### Compose and return predicates

`not`, `and`, and `or` operate on ordinary booleans. If `p` is an application
predicate, `p.or(p.not())` is true for every record for which `p` is defined.
Selecting a comparison returns `bool`:

```rust
let matches: Vec<bool> = User::all()
    .select(User::fields().nickname().eq("hello"))
    .exec(&mut db)
    .await?;
```

Users with no nickname contribute `false`, rather than an absent result or a
decoding error.

### Optional relations and loading

An optional single relation uses `Option<User>` or `Deferred<Option<User>>`.
Its path exposes `is_some` and `is_none`. An optional `has_one` is `None` when
there is no matching related record; it does not require a nullable column on
the owning model.

Comparisons of model references use Toasty's model identity. `Some(user)`
equals another reference to that user, and `None` equals only `None`.
Foreign-key resolution honors the relation's declared `references` fields,
including composite and non-primary references. It must not substitute the
wrong key when converting a supplied model into a relation operand.

`Deferred` describes loading, independently of whether a value exists. An
unloaded field and a loaded `None` remain distinguishable. A database predicate
on a deferred field compares its stored value; it does not test whether a
previously loaded Rust object has included that field.

### Before and after

Code that spells out the absent-value case can simplify its predicate:

```rust
User::filter(
    User::fields().nickname().is_none()
        .or(User::fields().nickname().ne("hello")),
);
```

The equivalent expression is:

```rust
User::filter(User::fields().nickname().ne("hello"));
```

Code that intentionally selects only present, different values makes that
condition explicit:

```rust
User::filter(
    User::fields().nickname().is_some()
        .and(User::fields().nickname().ne("hello")),
);
```

## Behavior

### Equality

For a supported value comparison:

| Left | Right | `eq` | `ne` |
|---|---|---|---|
| `None` | `None` | `true` | `false` |
| `None` | `Some(b)` | `false` | `true` |
| `Some(a)` | `None` | `false` | `true` |
| `Some(a)` | `Some(b)` | `a == b` | `a != b` |

Scalar payload comparisons follow Rust's equality for the supported type.
For example, string equality distinguishes `"Hello"` and `"hello"`. Model
references use the identity rule above; they do not invoke an arbitrary
user-written `PartialEq` implementation on loaded model structs.

Tuples and embedded values compare their corresponding application values,
including any optional fields. An optional embed's presence is independent
of its fields: `Some(Metadata { note: None }) != None`.

`ne(a, b)` equals `not(eq(a, b))`. An implementation may not assume `a == a`
for every type: floating-point NaN is an exception under Rust `PartialEq`.
Storage conversions must preserve the supported comparison or reject it with
an unsupported-feature error.

### Membership and boolean logic

`in_list(x, xs)` is `xs.iter().any(|y| x == y)`. An empty list yields `false`;
duplicates do not change the result. Negation is its boolean complement.
The list's length, parameter binding, or conversion into a subquery cannot
change this behavior.

The same rules hold in filters, projections, conditional mutations, relation
filters, included-relation filters, and client-side evaluation. `any` over an
empty relation is `false`; `all` over an empty relation is `true`. Optional
values inside their predicates follow this contract.

Variant-specific paths require their selected variant. That requirement is
part of the complete boolean predicate. Negating that predicate complements
its result, including the variant check. A missing variant is distinct from
an existing variant whose optional field contains `None`.

### Other operations on optional values

Ordered comparisons follow Rust's `Option` ordering when the payload supports
the comparison: `None < Some(_)`, with payload ordering between two `Some`
values. `between` uses the corresponding `ge` and `le` comparisons.
Ascending application ordering places `None` before present values;
descending ordering reverses that. Pagination must use the same ordering and
preserve records across the boundary between `None` and `Some`.

String predicates on an optional string lift a predicate on its present
value: `None` produces `false`. `starts_with` retains its case-sensitive
prefix semantics. `like`, `ilike`, and their escaped forms retain the native
operator's matching rules on present strings. In particular, `ilike` still
requires native `ILIKE` support and is not emulated on other backends.
Negating the resulting application boolean includes `None`.

An operation that can produce an absent value exposes `Option<T>` in its
result type. A database expression returning `NULL` cannot be presented as an
ordinary application `T` without a defined conversion. Unsupported
operations report an error; they do not silently propagate a third boolean
state.

### Writes and returned values

Setting an optional field to `None` clears the value. Omitting a field in an
update leaves it unchanged. Omitting a field in a create applies its declared
default, or the existing implicit `None` default for an optional field.
Explicit `None` does not request the default.

Query cardinality is separate from field optionality. Selecting an
`Option<String>` field with `.first()` returns:

| Query result | Rust value |
|---|---|
| No record | `None` |
| A record with an absent field | `Some(None)` |
| A record with a present field | `Some(Some(value))` |

Decoding malformed stored data is an error, rather than another spelling of
`None`.

## Edge cases

Nested options contain distinct states. `None`, `Some(None)`, and
`Some(Some(value))` cannot share a lossy representation. Until stored
`Option<Option<T>>` fields have a lossless mapping, model compilation rejects
them, including nesting through type aliases or transparent wrappers.
This restriction does not apply to query cardinality or deferred loading,
which already require independent presence information.

For `Option<Json<T>>`, outer `None` represents an absent field.
`Json<Option<T>>` can contain a present JSON payload whose serialized form
is JSON `null`. Neither JSON syntax nor a driver's representation introduces
SQL three-valued logic into application predicates. The JSON wrapper's own
comparison support remains a separate concern.

Optional embedded records, enums, and document fields need enough presence
information to distinguish absent containers from present containers with
absent members. Inactive enum storage columns do not themselves represent
application `None` values.

Equality of two absent relation values must not create a relationship between
two unrelated records. Relation joins and foreign-key resolution require a
present key; comparison of optional values and lookup of a related record
are different operations. A partly absent composite foreign key cannot be
treated as a valid present key.

Database uniqueness, grouping, and native conflict detection retain their
database contracts. `None == None` in a query does not imply that a nullable
unique index allows only one absent value.

## Driver integration

Drivers receive database operations after application optionality has been
lowered. Ordinary database `=`, `<>`, `IN`, `NOT`, and null checks retain
their native semantics. Drivers must not reinterpret every database `=`
as application equality.

For scalar values represented by nullable SQL expressions `a` and `b`,
application equality can use a native null-safe comparison:

| Backend | Candidate database expression |
|---|---|
| PostgreSQL | `a IS NOT DISTINCT FROM b` |
| MySQL / MariaDB | `a <=> b` |
| SQLite | `a IS b` |

These are valid choices only when their present-value comparison matches the
application type's equality. Collation, type coercion, custom storage types,
and floating-point behavior need separate validation.

A portable SQL expression for stable operands is:

```sql
(a IS NULL AND b IS NULL)
OR (a IS NOT NULL AND b IS NOT NULL AND a = b)
```

The result is always a boolean for valid scalar encodings. Inequality negates
that complete expression. A fallback must not evaluate a volatile operand
multiple times; bind or materialize it once.

Membership in a dynamic subquery can use `EXISTS` with application equality
lowered inside the correlation. Negated membership can use `NOT EXISTS`.
Direct `IN`, array `ANY`, and `ALL` rewrites are valid only when they preserve
the application result, including absent subjects and list elements.

For an optional string predicate, lowering can use
`CASE WHEN value IS NULL THEN FALSE ELSE value LIKE pattern END`.
This preserves the native matcher's behavior for present values and supplies
the application's boolean result for `None`.

DynamoDB's canonical absent scalar representation is an omitted attribute.
An implementation must account for attribute presence when lowering
application equality and membership. Explicit DynamoDB `NULL` attributes
written by external clients are accepted as absence. Presence predicates test
both attribute existence and the DynamoDB value type, matching decoding.

No new `Operation` variant is required. Drivers that preserve floating-point
NaN advertise `native_float_nan`. PostgreSQL enables it; comparison lowering
excludes NaN payloads from equality and ordered comparisons. SQLite rejects NaN
bindings because SQLite otherwise converts them to null. MySQL, MariaDB, and
DynamoDB reject NaN storage. This keeps `Some(NaN)` from becoming `None`. If native null-safe
comparison support is exposed in the database AST, capability negotiation
needs a default fallback for existing drivers. Backends may reject an
unsupported query form, but successful execution must preserve the contract.
Client evaluation of a lowered database expression must match that target's
semantics, including three-valued logic where applicable.

## Alternatives considered

**Keep SQL semantics in the application API.** This requires optional boolean
results and SQL-specific reasoning about equality and negation. It conflicts
with the requested Rust value semantics.

**Rewrite only comparisons with literal `None`.** This handles `eq(None)`
but misses field-to-field comparisons, `ne(value)`, membership, and returned
booleans.

**Change every database comparison to null-safe equality.** Database
expressions also implement joins, storage decoding, and native operators.
Changing their meaning globally loses the application/database boundary.

**Treat unknown as false only at the final filter.** SQL already does this.
It does not make negation or boolean projection correct.

## Open questions

- **Deferrable:** support stored nested options with a lossless encoding.
  Compilation rejects these fields until such a mapping exists.
- **Deferrable:** provide an opaque public database-expression API and opaque
  cursors. Existing untyped adapters and `cast_unchecked()` remain an explicit
  boundary for driver and AST integration.

## Out of scope

- Emulating native pattern matching across databases; `LIKE` and `ILIKE`
  keep their current backend contracts on present values.
- Executing arbitrary Rust `PartialEq` or `PartialOrd` implementations
  inside the database.
- Making database unique constraints, aggregates, or JSON document operators
  behave like Rust collections; each needs its own operation contract.

[#188]: https://github.com/tokio-rs/toasty/issues/188
[#797]: https://github.com/tokio-rs/toasty/issues/797
[#1246]: https://github.com/tokio-rs/toasty/issues/1246
