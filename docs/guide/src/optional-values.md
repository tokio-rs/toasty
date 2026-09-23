# Optional Values

Use `Option<T>` for an application value that may be absent. `None` is a value:
it compares equal to `None` and differs from every `Some(...)`. Comparisons
and membership return ordinary booleans, including when an operand is absent.

## Comparing and filtering

An optional field accepts either `Option<T>` or a value of `T`. Passing a value
of `T` means `Some(value)`:

```rust
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     id: uuid::Uuid,
#     nickname: Option<String>,
# }
let named = User::fields().nickname().eq("hello");
let other = User::fields().nickname().ne("hello");
let missing = User::fields().nickname().eq(None::<String>);
let present = User::fields().nickname().is_some();
```

`named` matches `Some("hello")`. `other` matches every other value, including
`None`. `missing` is equivalent to `.is_none()`. Negating equality produces
the same result as inequality.

| Field value | `.eq("hello")` | `.ne("hello")` | `.eq(None::<String>)` |
|---|---|---|---|
| `None` | `false` | `true` | `true` |
| `Some("hello")` | `true` | `false` | `false` |
| `Some("world")` | `false` | `true` | `false` |

List membership uses the same equality:

```rust
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     id: uuid::Uuid,
#     nickname: Option<String>,
# }
let choices = [None, Some("hello".to_owned())];
let selected = User::fields().nickname().in_list(choices);
let excluded = selected.clone().not();
```

`selected` matches absent nicknames and `"hello"`. An empty list matches
nothing. `in_query` compares against the values returned by another query,
including its absent values. A query returning `T` can supply candidates for
an `Option<T>` subject.

Optional relation handles also provide `.is_none()` and `.is_some()`.
Relations compare models by identity; an absent relation never matches a
present model.

## Selecting optional fields

Field presence and row presence are separate. Selecting an optional field
and calling `.first()` returns `Option<Option<T>>`:

- `None`: no row matched.
- `Some(None)`: a row matched and its field is absent.
- `Some(Some(value))`: a row matched and its field is present.

Selecting a comparison returns `bool`, even when the compared field is
optional. Optional string predicates such as `.starts_with()` and `.like()`
return `false` for `None`; their negations return `true`. For present strings,
`LIKE` and `ILIKE` retain the backend behavior described in
[Filtering with Expressions](./filtering-with-expressions.md).

## Ordering and writes

Ordered comparisons place `None` before `Some(...)`. Sorting places absent
values first for `.asc()` and last for `.desc()`. Pagination uses the same
presence ordering.

Omitting a field from an update leaves it unchanged. Assigning `None` clears
an optional field. On insert, omitting a field allows its default to apply;
explicit `None` supplies an absent value.

Stored nested options such as `Option<Option<String>>`, including aliases and
transparent pointer wrappers, are rejected because the storage mapping cannot
preserve every presence state. Nested options in query results remain valid.

PostgreSQL preserves floating-point NaN; comparisons follow Rust, where
`Some(NaN)` is unequal to itself. SQLite, MySQL, MariaDB, and DynamoDB reject
NaN storage. Toasty does not convert `Some(NaN)` to `None`.

## Database values

Toasty maps application options to the target database's storage representation.
Database nullability is separate from application optionality: an embedded
value can require several columns, including a column that records presence.
Raw SQL and the low-level statement API retain database operators and null
behavior.

Use `Expr::some()` to lift an expression into an application option. Custom AST
adapters can use `cast_unchecked()` to retag a representation they already
know is compatible; retagging does not construct `Some`.
