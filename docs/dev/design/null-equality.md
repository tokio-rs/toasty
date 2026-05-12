# Equality with `NULL`

## Summary

`.eq` and `.ne` on a nullable field become null-safe: comparing two
`Option<T>` columns counts a row where both sides are `NULL` as a match,
and comparing a nullable field to `None` is the same as
`.is_none()`.  This matches the `Option::eq` behavior that Rust callers
already assume.  A new `.sql_eq` / `.sql_ne` pair stays available for
the strict SQL three-valued-logic semantics when the caller wants them.

The simplifier and the in-memory evaluator are tightened in the same
change so SQL backends and the DynamoDB driver agree on what `NULL`
comparisons return.  Two `Expr` variants — `IsNotDistinctFrom` and
`IsDistinctFrom` — appear in the IR and serialize to
`IS NOT DISTINCT FROM` / `IS DISTINCT FROM` on PostgreSQL, `IS` / `IS
NOT` on SQLite, and `<=>` / `NOT (... <=> ...)` on MySQL.  No new
`Driver` capability is required.

## Motivation

Issue [#188] flags that `.eq` on a nullable column today follows SQL's
three-valued logic, which surprises Rust callers and silently drops
rows.  The two cases the issue raises are common in practice:

- A join keyed on a nullable column — for example, `author_email =
  editor_email` where both columns are `Option<String>` — returns no
  rows where either side is `NULL`, even when both sides are `NULL` on
  the same row.  Rust callers reading the code expect `Option::eq`
  behavior, where two `None`s compare equal.
- An expression that produces `NULL` intrinsically, such as
  `CASE WHEN random() % 2 = 0 THEN 'hello' ELSE NULL END`, contaminates
  the comparison surrounding it.  The IR has to be able to represent
  this without surprising the caller, and the simplifier must not
  silently fold the result to `false`.

Today's behavior is also internally inconsistent.  Three places handle
`NULL`:

- The lowering pass rewrites `Value::Null = expr` into `IS NULL`.  This
  is Rust-style: a literal `None` on one side of `=` is interpreted as
  a null test.
- The fold pass propagates null through binary operators in SQL
  three-valued style: `NULL <op> anything → NULL`.
- The in-memory evaluator compares with Rust `==`, so
  `Value::Null == Value::Null` returns `Bool(true)`, while
  `NULL AND x` errors out because the operand cannot be coerced to
  bool.

The user-facing API is split the same way: `.is_none()` and `.is_some()`
work as expected, but `.eq(None)` and `.eq(other_nullable_field)` hit
the path that goes through SQL `=` and behave like 3VL.  The result is
that two different ways of asking the same question — "is this column
null" or "do these two columns hold the same value" — give different
answers depending on which method the caller picked.

This design makes the API coherent for the cases Rust callers reach
for, keeps the strict SQL semantics reachable for callers who want
them, and brings the simplifier and evaluator into agreement so the
DynamoDB driver and the SQL drivers no longer diverge on null
comparisons.

[#188]: https://github.com/tokio-rs/toasty/issues/188

## User-facing API

### Comparing fields with `.eq` and `.ne`

`.eq` and `.ne` on a field handle compare the column against a value or
against another column.  When the field is non-nullable, they behave
the same as before: the database compares the values directly.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key] #[auto] id: u64,
#     name: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let alices = User::all()
    .filter(User::fields().name().eq("Alice"))
    .all(&mut db)
    .await?;
# Ok(())
# }
```

When the field is `Option<T>`, `.eq` is null-safe: it treats two
`NULL`s as equal, and a `NULL` against any concrete value as not
equal.  This matches `Option::eq` in plain Rust.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key] #[auto] id: u64,
#     author_email: Option<String>,
#     editor_email: Option<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Rows where author_email and editor_email hold the same value,
// including rows where both are NULL.
let same = Post::all()
    .filter(Post::fields().author_email().eq(Post::fields().editor_email()))
    .all(&mut db)
    .await?;
# Ok(())
# }
```

Comparing a nullable field against `None` finds the rows where the
field is `NULL`.  This is the same as calling `.is_none()`:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key] #[auto] id: u64,
#     bio: Option<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let no_bio = User::all()
    .filter(User::fields().bio().eq(None))
    .all(&mut db)
    .await?;
# Ok(())
# }
```

`.ne` is symmetric: it returns the rows where the two sides differ,
counting `(NULL, value)` and `(value, NULL)` as differing and counting
two `NULL`s as equal.

### Strict SQL equality with `.sql_eq` and `.sql_ne`

Sometimes the caller wants exact SQL semantics — for example, to write
a predicate that mirrors a SQL query word-for-word, or to interoperate
with a hand-written `IS DISTINCT FROM` clause.  `.sql_eq` and `.sql_ne`
compile to a plain `=` or `<>` and follow SQL's three-valued logic:
any comparison involving `NULL` evaluates to unknown, and the row is
dropped.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key] #[auto] id: u64,
#     author_email: Option<String>,
#     editor_email: Option<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// SQL `=`: rows where both columns hold the same non-null value.
// Rows where either column is NULL are dropped.
let same_value = Post::all()
    .filter(Post::fields().author_email().sql_eq(Post::fields().editor_email()))
    .all(&mut db)
    .await?;
# Ok(())
# }
```

`.sql_eq` against `None` evaluates to unknown for every row and returns
nothing.  This matches `WHERE col = NULL` in raw SQL and is almost
never what the caller wants.  When in doubt, prefer `.eq(None)` or
`.is_none()`.

### Existing null tests are unchanged

`.is_none()` and `.is_some()` continue to compile to `IS NULL` and
`IS NOT NULL`.  Use them when the intent is specifically a null test
and the reader of the code should see that intent at the call site.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key] #[auto] id: u64,
#     bio: Option<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let no_bio = User::all()
    .filter(User::fields().bio().is_none())
    .all(&mut db)
    .await?;
let has_bio = User::all()
    .filter(User::fields().bio().is_some())
    .all(&mut db)
    .await?;
# Ok(())
# }
```

### Before and after

Existing call sites that compare a nullable field to a value continue
to work unchanged.  Two patterns change observably:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key] #[auto] id: u64,
#     author_email: Option<String>,
#     editor_email: Option<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Before this change: returned no rows where either side was NULL.
// After this change: returns rows where both sides hold the same
// value, counting two NULLs as equal.
let _ = Post::all()
    .filter(Post::fields().author_email().eq(Post::fields().editor_email()))
    .all(&mut db)
    .await?;

// To keep the previous SQL `=` behavior explicitly, use `.sql_eq`.
let _ = Post::all()
    .filter(Post::fields().author_email().sql_eq(Post::fields().editor_email()))
    .all(&mut db)
    .await?;
# Ok(())
# }
```

The second pattern is `.eq(None)`.  It was inconsistent across paths
before this change; now it always finds the rows where the field is
`NULL`, the same as `.is_none()`.

## Behavior

### Truth tables

Toasty's filter language is three-valued at the IR level.  Every binary
comparison can produce `TRUE`, `FALSE`, or `NULL` (meaning unknown).
`WHERE` clauses keep rows whose predicate evaluates to `TRUE` and drop
rows whose predicate evaluates to `FALSE` or `NULL`.

The Option-aware operators are two-valued.  They produce `TRUE` or
`FALSE` and never `NULL`:

| Left      | Right     | `.eq` / null-safe | `.ne` / null-safe |
| --------- | --------- | ----------------- | ----------------- |
| `NULL`    | `NULL`    | `TRUE`            | `FALSE`           |
| `NULL`    | `v`       | `FALSE`           | `TRUE`            |
| `v`       | `NULL`    | `FALSE`           | `TRUE`            |
| `v1`      | `v2`      | `v1 == v2`        | `v1 != v2`        |

The strict SQL operators follow standard three-valued logic:

| Left      | Right     | `.sql_eq` | `.sql_ne` |
| --------- | --------- | --------- | --------- |
| `NULL`    | `NULL`    | `NULL`    | `NULL`    |
| `NULL`    | `v`       | `NULL`    | `NULL`    |
| `v`       | `NULL`    | `NULL`    | `NULL`    |
| `v1`      | `v2`      | `v1 = v2` | `v1 <> v2`|

Boolean composition follows three-valued logic everywhere:

| `a`     | `b`     | `a AND b` | `a OR b` |
| ------- | ------- | --------- | -------- |
| `TRUE`  | `TRUE`  | `TRUE`    | `TRUE`   |
| `TRUE`  | `FALSE` | `FALSE`   | `TRUE`   |
| `TRUE`  | `NULL`  | `NULL`    | `TRUE`   |
| `FALSE` | `FALSE` | `FALSE`   | `FALSE`  |
| `FALSE` | `NULL`  | `FALSE`   | `NULL`   |
| `NULL`  | `NULL`  | `NULL`    | `NULL`   |

`NOT TRUE` is `FALSE`, `NOT FALSE` is `TRUE`, `NOT NULL` is `NULL`.

### Ordered comparison

`.lt`, `.gt`, `.le`, `.ge` on a nullable field follow SQL three-valued
logic regardless of which operator family the caller used for
equality: any operand that is `NULL` makes the result `NULL`, and the
row is dropped.  There is no null-safe ordered comparison, because
there is no obvious total order on `Option<T>` that callers agree on.
A caller who wants `NULL`-aware ordering writes the null branch
explicitly with an `.is_none()` / `.is_some()` predicate.

### `WHERE` semantics

A predicate that evaluates to `NULL` drops the row.  This is identical
to SQL `WHERE` and matches what callers writing raw SQL queries
expect.  The Option-aware operators above never produce `NULL`, so
they never silently drop a row that the caller meant to include.

### Error mapping

No new error variants are introduced.  The in-memory evaluator no
longer errors on `NULL AND x`, `NULL OR x`, or comparing `NULL` with
an ordered operator; it returns `Value::Null` (meaning unknown)
instead, which the surrounding `WHERE` machinery handles uniformly.

## Edge cases

### `IN` with a `NULL` element

`x IN (a, NULL, b)` evaluates to `TRUE` if `x` matches any non-null
element, `NULL` if `x` matches none of the non-null elements (because
the unmatched comparison against `NULL` is unknown), and never
`FALSE`.  In a `WHERE` clause this drops rows that do not match a
non-null element, which mirrors SQL.

`x NOT IN (a, NULL, b)` is `NULL` whenever the list contains a
`NULL`, because the negation of an unknown is unknown.  This is the
standard SQL footgun; the Option-aware operators do not paper over it
because the list is not statically typed as a sequence of `Option<T>`.

A caller building a list dynamically and worried about `NULL`
contamination can filter the list before constructing the predicate or
use a join against a `VALUES` clause.

### Aggregates

Aggregates follow SQL semantics: `SUM`, `AVG`, `MIN`, `MAX`, `COUNT`
all skip `NULL` inputs, and `COUNT(*)` counts every row.  This design
does not change aggregate behavior.

### Empty `IN` list

`x IN ()` is `FALSE` and `x NOT IN ()` is `TRUE`.  The fold pass
canonicalizes these before they reach the database.  Empty-list
handling is independent of `NULL` handling.

### Expressions that produce `NULL` intrinsically

A user-written expression such as `CASE WHEN cond THEN value ELSE NULL
END` can produce `NULL` regardless of column nullability.  Comparing
two such expressions with `.eq` follows the Option-aware truth table
above only when both sides are typed as `Option<T>` in Rust.  When the
sides are dynamically typed expressions with no Rust `Option` at the
call site — for example, two `case!` expressions built through the
expression API — `.eq` falls back to SQL three-valued logic.  Callers
who need null-safe semantics on such expressions construct the
`IsNotDistinctFrom` node directly through the expression builder.

### Composite keys and record equality

Comparing a record-typed value (composite key, embedded struct)
against another record-typed value decomposes to a conjunction of
per-field comparisons.  Each per-field comparison picks its operator
from the table in [API routing](#api-routing).  A composite key with
one or more nullable components is null-safe per component, so
`compound.eq(other_compound)` matches when every component is pairwise
null-safe equal.

### Sorting and pagination

`ORDER BY` on a nullable column follows the underlying database's
null-ordering rules.  PostgreSQL puts `NULL` last in ascending order;
SQLite and MySQL put `NULL` first.  This design does not change that;
explicit null-ordering (`NULLS FIRST` / `NULLS LAST`) is the subject
of a separate roadmap item.

### Backward compatibility

Code that compares non-nullable fields is unchanged.  Code that
compares a nullable field to a `Some(v)` literal is also unchanged
(SQL `column = v` and Option-aware `column.eq(v)` agree when one side
is concretely non-null).  The two behavioral changes are:

- `nullable_field.eq(other_nullable_field)` now matches rows where
  both sides are `NULL`.
- `nullable_field.eq(None)` now finds the rows where the field is
  `NULL`, consistently across paths.

Callers who relied on the previous behavior migrate to `.sql_eq`.

## Driver integration

No new `Driver` trait method or capability flag is required.  The
existing serializer hook covers the new `Expr` variants.

### SQL serialization

The SQL backends emit dialect-native null-safe operators:

| Operator               | PostgreSQL                  | SQLite       | MySQL              |
| ---------------------- | --------------------------- | ------------ | ------------------ |
| `IsNotDistinctFrom`    | `lhs IS NOT DISTINCT FROM rhs` | `lhs IS rhs` | `lhs <=> rhs`   |
| `IsDistinctFrom`       | `lhs IS DISTINCT FROM rhs`  | `lhs IS NOT rhs` | `NOT (lhs <=> rhs)` |

Every supported backend has a native null-safe equality operator;
there is no portability shim and no need for an emulation path.

`IS NULL` / `IS NOT NULL`, `=`, and `<>` are unchanged on every
backend.

### DynamoDB

DynamoDB has no `NULL` value, only attribute presence.  Lowering to
the DynamoDB plan maps:

- `ExprIsNull(path)` → `attribute_not_exists(path)`.
- `ExprIsNotNull(path)` → `attribute_exists(path)`.
- `ExprIsNotDistinctFrom(a, null literal)` →
  `attribute_not_exists(a)`.
- `ExprIsNotDistinctFrom(a, b)` where both are paths →
  `(attribute_not_exists(a) AND attribute_not_exists(b)) OR a = b`.
- `BinaryOp::Eq(a, b)` → `a = b`.  Returns no match when either
  attribute is missing, matching SQL three-valued row-drop behavior.

The in-memory evaluator becomes three-valued in the same change, so
SQL backends and the DynamoDB driver agree on what each filter
returns even though the engines model `NULL` differently.

### Out-of-tree drivers

Drivers that delegate SQL serialization to `toasty-sql` pick up the
new operators automatically.  Drivers that produce their own
serialization receive two new `Expr` variants —
`Expr::IsNotDistinctFrom` and `Expr::IsDistinctFrom` — through the
existing visitor.  A driver that does not handle them produces a clear
compile-time exhaustiveness error and can fall back to the SQL
expansion shown in the DynamoDB section above when its backend lacks
a native null-safe operator.

## Alternatives considered

**Keep raw SQL `=` as the default and add an explicit null-safe
operator.**  Rejected because the current API already documents
`.eq(None)` as a null test and existing call sites read as if `.eq`
were Option-aware.  Defaulting to SQL semantics asks every reader of
every call site to remember three-valued logic; defaulting to
Option-aware semantics asks the smaller set of callers who actually
want raw `=` to spell it.

**Type every IR node with nullability and dispatch in the engine.**
Rejected as substantially larger work for marginal gain.  The Rust
`Option<T>` vs `T` distinction at the call site already carries the
information needed for the common case.  Expressions built through
the typed builder where Rust loses track of nullability fall back to
SQL semantics, which is the correct conservative default.  Engine-level
nullability tracking remains available as a follow-up if it is needed
for query optimization later.

**Overload `BinaryOp::Eq` with a `null_safe: bool` field.**  Rejected
because the resulting node has two distinct truth tables and folds,
threaded through every match arm.  Distinct `Expr` variants parallel
the existing `ExprIsNull` factoring, keep fold and eval rules
per-node-kind, and match Postgres's surface syntax.

**Synthesize `(a IS NULL AND b IS NULL) OR a = b` on every nullable
comparison.**  Rejected because every supported backend has a native
null-safe operator that is cheaper for the planner and clearer in
`EXPLAIN` output.  The expansion remains the fallback shape for
out-of-tree drivers without native null-safe equality.

**Add `.eq_some` / `.eq_none` as discrete methods.**  Rejected because
it splits a single comparison into two and inverts the static-type
ergonomics callers have today.  The proposed routing keeps `.eq` as
the one method callers reach for, and lets the type of the argument
do the dispatch.

## Open questions

- **Naming of `.sql_eq` and `.sql_ne`.**  Alternatives: `.eq_strict`,
  `.eq_sql`, `.eq_3vl`.  Blocking acceptance; settles before merge.
- **Surfacing `IsDistinctFrom` directly on `Path`.**  This design
  exposes `.eq` / `.ne` and the strict variants.  A `.is_distinct_from`
  method that mirrors Postgres syntax is available through the
  expression builder but not as a path-level shortcut.  Deferrable;
  adding it later is non-breaking.
- **Promoting `IS DISTINCT FROM` to the existing "Range and set
  predicates" roadmap entry.**  The roadmap currently lists `IS
  DISTINCT FROM` alongside `NOT IN` and `BETWEEN` as a set of missing
  predicates.  This design implements the null-safe semantics; the
  remaining items in that entry are independent.  Suggest splitting
  the roadmap entry once this lands.  Deferrable.
- **Migration window.**  This design flips the meaning of `.eq` on
  nullable column pairs.  Whether to gate the change behind a
  one-release deprecation that emits a warning when the old behavior
  would differ is open.  The check requires schema-level nullability,
  which the simplifier already knows.  Blocking implementation if the
  answer is "deprecate first."

## Out of scope

- **Full ANSI three-valued semantics for every expression form.**
  This design covers the binary comparison and boolean composition
  surface that Toasty exposes today.  Future expressions
  (`CASE WHEN`, full string predicates, aggregates with `FILTER`) get
  their three-valued semantics defined as they land.
- **Null-ordering control.**  `NULLS FIRST` / `NULLS LAST` is a
  separate roadmap item; this design does not change ordering
  behavior.
- **Migration tooling.**  No schema migration is implied.  Callers
  who relied on the previous `.eq` behavior on nullable column pairs
  migrate by changing the call site to `.sql_eq`.
- **Driver feature detection.**  Every supported backend has a native
  null-safe equality operator.  No capability flag or fallback path
  is needed in this change.
