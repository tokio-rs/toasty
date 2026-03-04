---
name: write-tests
description: Always use this skill before writing any test code in the Toasty repository
---

# Writing Toasty Tests

Load this skill when writing or editing tests anywhere in this project.

## Where to Write Tests

**Prefer integration tests over lib (inline `#[cfg(test)]`) tests.** Choose the location that best matches what is being tested:

| What you're testing | Where to put it |
|---|---|
| Full-stack behavior requiring a real DB | `crates/toasty-driver-integration-suite/src/tests/` |
| Full-stack behavior with no operational DB needed | `tests/tests/` |
| Public API of `toasty-core` | `crates/toasty-core/tests/` |
| SQL generation in `toasty-sql` | `crates/toasty-sql/tests/` |
| Complex internal logic (e.g., query simplification) | Inline `#[cfg(test)]` in the relevant source file |

**Never write tests for `toasty-codegen` directly.** Test macro behavior at the user level (define a model, use the generated code) in the integration suite or `tests/tests/`.

**Never write per-driver tests.** Any test that exercises a real DB goes in `toasty-driver-integration-suite` so third-party driver authors can run it too. The suite is instantiated per driver in `tests/tests/{sqlite,mysql,postgresql,dynamodb}.rs`.

## Test Style

- **Succinct over thorough-looking**: Each test function should focus on one thing. Code inside the test body should be the core behavior under test — nothing more.
- **DRY the scaffolding**: Use or write helpers for setup that is not what the test is testing. If you find yourself copy-pasting non-essential setup across tests, extract a helper.
- **assert_struct! vs assert_eq!**: Use whichever produces fewer characters and the same coverage. See the decision rule below.

## Integration Suite Tests

Tests in `crates/toasty-driver-integration-suite/src/tests/` run against every supported driver. New files are auto-discovered at compile time via `generate_test_registry!`.

### Anatomy of a suite test

```rust
use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn my_test(t: &mut Test) -> Result<()> {
    // 1. Define model(s) inline
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,       // placeholder: rewritten to u64 and uuid::Uuid by macro
        name: String,
    }

    // 2. Setup database
    let db = t.setup_db(models!(Foo)).await;

    // 3. Exercise behavior
    let foo = Foo::create().name("hi").exec(&db).await?;

    // 4. Assert
    assert_eq!(foo.name, "hi");

    Ok(())
}
```

The `id(ID)` argument names a placeholder identifier — `ID` by convention — that the macro replaces with a concrete type everywhere it appears as a type in the function body. The function is emitted twice: once with every `ID` replaced by `u64`, once with every `ID` replaced by `uuid::Uuid`. Both variants are wrapped with `#[tokio::test]` and each gets an isolated table prefix that is cleaned up after the test runs.

This is why FK fields are also typed `ID` (e.g. `user_id: ID`): the substitution is textual across the whole function, so the FK gets the same concrete type as the primary key it references.

**Attribute forms:**
- `#[driver_test(id(ID))]` — expands to two variants (u64 and uuid::Uuid); only use when it genuinely makes sense to test both ID types
- `#[driver_test(id(ID), requires(sql))]` — ID expansion with a capability gate
- `#[driver_test(requires(native_decimal))]` — no ID expansion, just a capability gate
- `#[driver_test]` — single variant, no capability gate, use a concrete ID type in the model

**Choosing an ID type:** Only use `id(ID)` expansion when running against both u64 and uuid::Uuid adds meaningful coverage. Otherwise pick the type that fits the test:
- Use `uuid::Uuid` when the test must run on non-SQL drivers (DynamoDB does not support auto-increment)
- Use `u64` when testing auto-increment behavior specifically
- Use a string, manual integer, composite key, etc. when that is what the test is about

Always use `requires(...)` to gate tests on capabilities. Never use runtime `if !t.capability().foo { return Ok(()); }` — that is what the macro is for.

### Prelude

`crates/toasty-driver-integration-suite/src/prelude.rs` re-exports everything test files need. Start new test files with `use crate::prelude::*;`.

### Key helpers

```rust
// Build a Db::Builder registering a set of models
models!(Foo, Bar, Baz)

// Look up a TableId from the schema (for driver-op assertions)
table_id(&db, "foos")      // -> TableId
column(&db, "foos", "name")             // -> ColumnId
columns(&db, "foos", &["id", "name"])   // -> Vec<ColumnId>
```

### Test struct methods

```rust
t.setup_db(models!(Foo)).await         // create DB, push schema, return Db
t.capability()                          // &'static Capability (sql, auto_increment, …)
t.log().clear()                         // clear driver operation log
t.log().pop()       -> (Operation, Response)  // inspect what the driver received
t.log().is_empty()                      // assert no extra driver calls
```

### Inspecting driver operations

Capture and assert on the raw operation the engine sent to the driver:

```rust
t.log().clear();
foo.update().name("new").exec(&db).await?;

let (op, resp) = t.log().pop();
if t.capability().sql {
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Update(_ {
            target: UpdateTarget::Table(== foo_table_id),
            assignments: #{ 1: _ { expr: == "new", .. } },
            ..
        }),
        ..
    }));
} else {
    assert_struct!(op, Operation::UpdateByKey(_ {
        table: == foo_table_id,
        keys.len(): 1,
        assignments: #{ 1: _ { expr: == "new", .. } },
        ..
    }));
}
assert_struct!(resp, _ { rows: Rows::Count(1), .. });
assert!(t.log().is_empty());
```

## Assertion Macros

### assert_struct! rule

> Use whichever form produces **fewer characters** and the **same test coverage**.

```rust
// assert_eq! wins — single field
assert_eq!(foo.name, "hello");

// assert_struct! wins — multiple fields in one call
assert_struct!(foo, _ { name: "hello", age: 30 });
// vs. assert_eq!(foo.name, "hello"); assert_eq!(foo.age, 30);

// assert_eq! wins — you have the whole value
assert_eq!(result, Foo::default());
```

### assert_struct! quick reference

Patterns compose freely. Use `_` for wildcard struct (no import needed), `..` for partial match:

```rust
assert_struct!(val, _ { field: "text", count: > 0, flag: true, .. });
assert_struct!(val, _ { opt: Some(42), res: Ok("ok"), .. });
assert_struct!(val, _ { items: [1, 2, ..], tags: #("a", "b", ..), .. });
assert_struct!(val, _ { nested.child.x: >= 0, .. });  // dot-path shorthand
assert_struct!(val, _ { items.len(): 3, .. });         // method call
```

Operator patterns at leaves avoid importing types:
```rust
// Instead of:  field: SomeEnum::Variant(42)
// Write:        field: == expected_var
```

Full pattern grammar: `~/.cargo/registry/src/index.crates.io-*/assert-struct-*/LLM.txt`

### std-util assertion macros

```rust
assert_err!(expr)      // asserts Err, returns the error value
assert_ok!(expr)       // asserts Ok, returns the inner value
assert_none!(expr)     // asserts None
assert_unique!(slice)  // asserts all elements are distinct
```

## Model Attribute Reference

Common attributes used in inline test model definitions:

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key] #[auto]          id: ID,            // primary key, auto-generated
    #[unique]               email: String,     // generates get_by_email / filter_by_email
    #[index]                name: String,      // generates filter_by_name
    #[default(0)]           score: i64,        // default value on create
    #[has_many]             todos: toasty::HasMany<Todo>,
}

#[derive(Debug, toasty::Model)]
struct Todo {
    #[key] #[auto]  id: ID,
    #[index]        user_id: ID,
    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
}

// Composite key
#[derive(Debug, toasty::Model)]
#[key(partition = team, local = name)]
struct Player { team: String, name: String }
```

## Common Query Patterns

```rust
// Get by PK (errors if missing)
let u = User::get_by_id(&db, &id).await?;

// Filter (returns cursor)
let users = User::filter_by_name("Alice").collect::<Vec<_>>(&db).await?;

// First (returns Option)
let opt = User::filter_by_email("x@x.com").first(&db).await?;

// Delete
u.delete(&db).await?;
User::filter_by_id(id).delete(&db).await?;

// Update (instance — mutates in memory too)
u.update().name("Bob").exec(&db).await?;

// Update (query-based)
User::filter_by_id(id).update().name("Bob").exec(&db).await?;

// Create with nested association
User::create().name("Alice").todo(Todo::create().title("T1")).exec(&db).await?;
```
