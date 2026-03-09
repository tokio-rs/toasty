# Macros, Embedded Types, Serialized Fields, and Numeric/Time Types

This guide documents the next five implemented feature areas in Toasty:

11. `toasty::create!` macro
12. Embedded structs and enums (`#[derive(toasty::Embed)]`)
13. Serialized JSON fields (`#[serialize(json)]`)
14. Jiff temporal types
15. Decimal and BigDecimal support

## 11) `toasty::create!` Macro

`toasty::create!` is a concise syntax for create builders.

Simple model create:

```rust
let user = toasty::create!(User, { name: "Carl", email: "carl@example.com" })
    .exec(&db)
    .await?;
```

Scoped create through an association:

```rust
let todo = toasty::create!(user.todos(), { title: "get something done" })
    .exec(&db)
    .await?;
```

Batch create form:

```rust
let users = toasty::create!(User, [{ name: "Alice" }, { name: "Bob" }])
    .exec(&db)
    .await?;
```

Nested association create:

```rust
let user = toasty::create!(User, {
    name: "Carl",
    todos: [{ title: "first" }, { title: "second" }]
})
.exec(&db)
.await?;
```

## 12) Embedded Structs and Enums (`#[derive(toasty::Embed)]`)

Embedded types are inlined into parent model storage instead of getting their
own top-level tables.

Embedded struct:

```rust
#[derive(Debug, toasty::Embed)]
struct Address {
    street: String,
    city: String,
}

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,
    name: String,
    address: Address,
}
```

Embedded enum (unit variants):

```rust
#[derive(Debug, PartialEq, toasty::Embed)]
enum Status {
    #[column(variant = 1)]
    Pending,
    #[column(variant = 2)]
    Active,
    #[column(variant = 3)]
    Done,
}
```

Data-carrying enum variants are also supported:

```rust
#[derive(Debug, PartialEq, toasty::Embed)]
enum ContactInfo {
    #[column(variant = 1)]
    Email { address: String },
    #[column(variant = 2)]
    Phone { number: String },
}
```

## 13) Serialized JSON Fields (`#[serialize(json)]`)

Use `#[serialize(json)]` to store rich Rust types as JSON in a single column.

```rust
#[derive(Debug, toasty::Model)]
struct Article {
    #[key]
    #[auto]
    id: u64,

    #[serialize(json)]
    tags: Vec<String>,
}
```

Custom struct support (requires serde derive on the field type):

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
struct Metadata {
    version: u32,
    labels: Vec<String>,
}

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    #[auto]
    id: u64,

    #[serialize(json)]
    meta: Metadata,
}
```

Nullability behavior:

- `#[serialize(json)]` on `Option<T>` stores JSON (for `None`, JSON `null` text).
- `#[serialize(json, nullable)]` maps `None` to SQL `NULL`.

To use serialized fields, enable Toasty's `serde` feature.

## 14) Jiff Temporal Types

Toasty supports jiff date/time types when the `jiff` feature is enabled.

```toml
[dependencies]
toasty = { version = "...", features = ["jiff"] }
jiff = "..."
```

Supported families include:

- `jiff::Timestamp`
- `jiff::Zoned`
- `jiff::civil::Date`
- `jiff::civil::Time`
- `jiff::civil::DateTime`

Example:

```rust
#[derive(Debug, toasty::Model)]
struct Event {
    #[key]
    #[auto]
    id: u64,
    created_at: jiff::Timestamp,
    happened_on: jiff::civil::Date,
}
```

For storage details and precision behavior by backend, see [jiff.md](jiff.md).

## 15) Decimal and BigDecimal Support

Toasty supports:

- `rust_decimal::Decimal` (feature: `rust_decimal`)
- `bigdecimal::BigDecimal` (feature: `bigdecimal`)

```toml
[dependencies]
toasty = { version = "...", features = ["rust_decimal", "bigdecimal"] }
rust_decimal = "..."
bigdecimal = "..."
```

Model example:

```rust
#[derive(Debug, toasty::Model)]
struct PricePoint {
    #[key]
    #[auto]
    id: u64,

    price: rust_decimal::Decimal,
    precise: bigdecimal::BigDecimal,
}
```

Column type overrides are supported when needed, for example:

```rust
#[derive(Debug, toasty::Model)]
struct LedgerEntry {
    #[key]
    #[auto]
    id: u64,

    #[column(type = numeric(38, 20))]
    amount: bigdecimal::BigDecimal,
}
```

Backend capabilities vary for fixed/arbitrary precision numeric types.

For implemented advanced patterns around embedded enum filters and embedded
field indexing behavior, see
[implemented-advanced-patterns.md](implemented-advanced-patterns.md).

For the next five feature areas, continue with
[composite-keys-migrations-and-known-gaps.md](composite-keys-migrations-and-known-gaps.md).
