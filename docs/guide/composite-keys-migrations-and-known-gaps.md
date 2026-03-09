# Composite Keys, Migrations, and Known Gaps

This guide documents the next five feature areas in Toasty:

16. Composite-key tested workflows (implemented paths)
17. Migration CLI (`generate`, `apply`, `snapshot`, `drop`, `reset`)
18. `Db::reset_db()` runtime reset path
19. Full composite-key parity (partial)
20. Multi-column convenience ordering via `.then_by()` (partial)

## 16) Composite-Key Tested Workflows

Toasty supports two composite-key modeling styles that are exercised by
integration tests.

Field-level composite key:

```rust
#[derive(Debug, toasty::Model)]
struct Foo {
    #[key]
    one: String,
    #[key]
    two: String,
}
```

Model-level partition/local key (useful for DynamoDB-style access patterns):

```rust
#[derive(Debug, toasty::Model)]
#[key(partition = user_id, local = id)]
struct Todo {
    #[auto]
    id: uuid::Uuid,
    user_id: String,
    title: String,
}
```

Composite generated methods include tuple-style batch lookup:

```rust
let foos: Vec<_> = Foo::filter_by_one_and_two_batch([
    (&"foo-1".to_string(), &"bar-1".to_string()),
    (&"foo-2".to_string(), &"bar-2".to_string()),
])
.collect(&db)
.await?;
```

Partition-only query/update/delete paths are also tested for partition/local
models.

## 17) Migration CLI

Toasty provides migration tooling via `toasty-cli` (typically embedded in your
application CLI).

Available migration subcommands:

- `migration generate` (optional `--name` / `-n`)
- `migration apply`
- `migration snapshot`
- `migration drop` (supports `--latest` or `--name`)
- `migration reset` (supports `--skip-migrations`)

Example command sequence (from your CLI binary):

```bash
my-app-cli migration generate --name add_status_to_todos
my-app-cli migration apply
my-app-cli migration snapshot
```

Drop/reset examples:

```bash
my-app-cli migration drop --latest
my-app-cli migration reset --skip-migrations
```

## 18) `Db::reset_db()` Runtime Reset Path

`Db::reset_db()` drops and recreates an empty database state without applying
migrations.

```rust
db.reset_db().await?;
```

Typical usage:

- Integration tests.
- Local development reset flows.
- CLI reset command internals (often followed by migration apply).

If you reset during application code paths, you usually need to re-apply schema
or migrations before normal queries.

## 19) Full Composite-Key Parity Is Partial

Composite keys work in multiple tested paths, but full parity is not complete.

Known examples of remaining gaps:

- Simplification paths with explicit single-field assumptions in subquery
  lifting.
- Remaining `todo!()` / `panic!()` composite branches in engine and some
  DynamoDB edge operations.
- Stubbed integration test cases for some relation+composite combinations.

Practical guidance:

- Prefer currently tested patterns (composite lookup/batch lookup,
  partition/local query/update/delete).
- Validate relationship-heavy composite flows per target backend before relying
  on them in production.
- Track progress in [roadmap/composite-keys.md](../roadmap/composite-keys.md).

## 20) `.then_by()` Is Not Implemented Yet

Convenience chaining with `.then_by()` is still pending.

Current workaround: build a multi-column `OrderBy` explicitly.

```rust
use toasty::stmt::OrderBy;

let order = OrderBy::from([
    Post::fields().status().asc(),
    Post::fields().created_at().desc(),
]);

let page = Post::all()
    .order_by(order)
    .paginate(10)
    .collect(&db)
    .await?;
```

This workaround is the recommended path until `.then_by()` lands.

For the next five gap areas, continue with
[gaps-query-macros-and-many-to-many.md](gaps-query-macros-and-many-to-many.md).
