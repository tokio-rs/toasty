# Batch Query Execution

## Overview

Batch queries let users send multiple independent queries to the database in a
single round-trip. The results come back as a typed tuple matching the input
queries.

```rust
let (active_users, recent_posts) = toasty::stmt::batch((
    User::find_by_active(true),
    Post::find_recent(100),
)).exec(&db).await?;

// active_users: Vec<User>
// recent_posts: Vec<Post>
```

The batch composes all queries into a single `Statement` whose returning
expression is a record of subqueries. This means batch execution flows through
the existing `exec` path — no new executor methods, no new driver operations.

This design covers SQL databases only. DynamoDB support is out of scope.

## New Trait: `IntoStatement<T>`

A single new trait bridges query builders to `Statement<T>`:

```rust
pub trait IntoStatement<T> {
    fn into_statement(self) -> Statement<T>;
}
```

Query builders implement this for their model type. For example, `UserQuery`
implements `IntoStatement<User>`:

```rust
impl IntoStatement<User> for UserQuery {
    fn into_statement(self) -> Statement<User> {
        self.stmt.into()
    }
}
```

The codegen already produces `IntoSelect` impls for query builders.
`IntoStatement` can be blanket-implemented for anything that implements
`IntoSelect`:

```rust
impl<T: IntoSelect> IntoStatement<T::Model> for T {
    fn into_statement(self) -> Statement<T::Model> {
        self.into_select().into()
    }
}
```

### Tuple implementations

Tuples of `IntoStatement` types implement `IntoStatement` by composing their
inner statements into a single select whose returning expression is a record of
subqueries:

```rust
impl<T1, T2, A, B> IntoStatement<(Vec<T1>, Vec<T2>)> for (A, B)
where
    A: IntoStatement<T1>,
    B: IntoStatement<T2>,
{
    fn into_statement(self) -> Statement<(Vec<T1>, Vec<T2>)> {
        let stmt_a = self.0.into_statement().untyped;
        let stmt_b = self.1.into_statement().untyped;

        // Build: SELECT (stmt_a), (stmt_b)
        let query = stmt::Query::values(stmt::Expr::record([
            stmt::Expr::subquery(stmt_a),
            stmt::Expr::subquery(stmt_b),
        ]));

        Statement::from_raw(query.into())
    }
}
```

The resulting statement is equivalent to `SELECT (subquery_1), (subquery_2)`.
At the Toasty AST level this is a `Query` whose returning body is a
`Record([Expr::Stmt, Expr::Stmt])`. The engine handles each subquery
independently during execution and packs the results into a single
`Value::Record`.

Tuple impls for arities 2 through 8 are generated with a macro.

## `Load` for Tuples and `Vec<T>`

To deserialize the composed result, `Load` is implemented for `Vec<T>` and
for tuples:

```rust
impl<T: Load> Load for Vec<T> {
    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            Value::List(items) => items
                .into_iter()
                .map(T::load)
                .collect(),
            _ => Err(Error::type_conversion(value, "Vec<T>")),
        }
    }
}

impl<A: Load, B: Load> Load for (A, B) {
    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            Value::Record(mut record) => Ok((
                A::load(record[0].take())?,
                B::load(record[1].take())?,
            )),
            _ => Err(Error::type_conversion(value, "(A, B)")),
        }
    }
}
```

With these impls, `Load for (Vec<User>, Vec<Post>)` works automatically:
the outer tuple impl splits the record, then each `Vec<T>` impl iterates
the list and loads individual model instances.

## User-Facing API

```rust
pub fn batch<T, Q: IntoStatement<T>>(queries: Q) -> Batch<T>
where
    T: Load,
{
    Batch {
        stmt: queries.into_statement(),
    }
}

pub struct Batch<T> {
    stmt: Statement<T>,
}

impl<T: Load> Batch<T> {
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<T> {
        use ExecutorExt;
        let stream = executor.exec(self.stmt).await?;
        let value = stream.next().await
            .ok_or_else(|| Error::record_not_found("batch returned no results"))??;
        T::load(value)
    }
}
```

`Batch::exec` calls the regular `ExecutorExt::exec` method. The composed
statement flows through the standard engine pipeline. The result is a single
value (a record of lists) that `T::load` deserializes into the typed tuple.

## Execution Flow

```
User code:
    toasty::stmt::batch((UserQuery, PostQuery)).exec(&db)

IntoStatement for (A, B):
    SELECT (SELECT ... FROM users WHERE ...), (SELECT ... FROM posts ...)

Engine pipeline (standard exec path):
    lower → plan → exec

    The engine recognizes Expr::Stmt subqueries in the returning
    expression and executes each independently.

Result:
    Value::Record([
        Value::List([user1, user2, ...]),
        Value::List([post1, post2, ...]),
    ])

Load for (Vec<User>, Vec<Post>):
    (A::load(record[0]), B::load(record[1]))
    → (Vec<User>::load(list), Vec<Post>::load(list))
    → (vec![User::load(v1), ...], vec![Post::load(v1), ...])
```

## `Statement` Changes

`Statement<M>` needs a way to construct from a raw `stmt::Statement` without
requiring `M: Model`:

```rust
impl<M> Statement<M> {
    /// Build a statement from a raw untyped statement.
    ///
    /// Used by batch composition where M may be a tuple, not a model.
    pub(crate) fn from_raw(untyped: stmt::Statement) -> Self {
        Self {
            untyped,
            _p: PhantomData,
        }
    }
}
```

The existing `Statement::from_untyped` requires `M: Model` (via `IntoSelect`).
`from_raw` has no bound on `M` and is `pub(crate)` so only internal code uses
it.

## Engine Support

The engine needs to handle a `Query` whose returning expression is a record
of `Expr::Stmt` subqueries where each subquery returns multiple rows.

The lowerer already handles `Expr::Stmt` for association preloading (`INCLUDE`),
where subqueries get added to the dependency graph and executed as part of the
plan. Batch queries follow the same pattern: each `Expr::Stmt` in the returning
record becomes an independent subquery in the plan, and the exec phase collects
results into a `Value::Record` of `Value::List`s.

If the existing lowerer does not handle bare subqueries in a returning record
(outside of an `INCLUDE` context), a small extension is needed to recognize this
pattern and plan it the same way.

## Implementation Plan

### Phase 1: `IntoStatement` trait and `Load` impls

1. Add `IntoStatement<T>` trait to `crates/toasty/src/stmt/`
2. Add blanket impl `IntoStatement<T::Model> for T: IntoSelect`
3. Add `Load for Vec<T>` and `Load for (A, B)` (and higher arities via macro)
4. Add `Statement::from_raw`
5. Export `IntoStatement` from `lib.rs` and `codegen_support`

### Phase 2: Batch API

1. Add `toasty::stmt::batch()` function and `Batch<T>` struct
2. Add tuple impls of `IntoStatement<(Vec<T1>, Vec<T2>, ...)>` (via macro)
3. Wire `Batch::exec` through the standard `ExecutorExt::exec` path

### Phase 3: Engine support

1. Verify that the lowerer handles `Expr::Stmt` subqueries in a returning
   record correctly (it may already work via the `INCLUDE` path)
2. If not, extend the lowerer to plan bare record-of-subqueries statements
3. Verify the exec phase packs subquery results into `Value::Record` of
   `Value::List`s

### Phase 4: Integration tests

1. Batch two selects on different models
2. Batch a select that returns rows with a select that returns empty
3. Batch with filters, ordering, and limits
4. Batch inside a transaction
5. Batch of a single query (degenerates to normal execution)

## Files Modified

| File | Change |
|------|--------|
| `crates/toasty/src/stmt/into_statement.rs` | New: `IntoStatement<T>` trait, blanket impl |
| `crates/toasty/src/stmt.rs` | Add `Statement::from_raw`, re-export `IntoStatement` |
| `crates/toasty/src/load.rs` | Add `Load` impls for `Vec<T>` and tuples |
| `crates/toasty/src/batch.rs` | Add `batch()`, `Batch<T>`, tuple `IntoStatement` impls |
| `crates/toasty/src/lib.rs` | Re-export `batch`, `Batch`, `IntoStatement` |
| `crates/toasty/src/engine/lower.rs` | Handle record-of-subqueries in returning (if needed) |
