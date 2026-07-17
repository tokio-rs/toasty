# Native JSON Values

## Summary

Toasty adds `JsonValue`, a field type for storing arbitrary
`serde_json::Value` data in a database-native document column. Use it for
values whose fields are not known when the model is defined, such as HTTP
request and response bodies. `JsonValue` supports whole-value create, load,
and update operations without adding dynamic path queries.

## Motivation

`Json<T>` stores a serde value through Toasty's string field type. This works
for opaque application data, but it does not request a native document column
from the driver. `#[document]` uses a native document column, but requires a
`#[derive(Embed)]` struct whose fields are known at compile time.

Applications that record third-party payloads often know neither the keys nor
the nesting in advance. They need to preserve JSON objects, arrays, scalars,
and null values without defining an embed type for each payload format.

## User-facing API

Enable Toasty's `serde` feature, then use `toasty::JsonValue` as a model or
embedded-struct field:

```rust
#[derive(toasty::Model)]
struct Exchange {
    #[key]
    #[auto]
    id: uuid::Uuid,

    request_body: toasty::JsonValue,
    response_body: Option<toasty::JsonValue>,
}
```

`JsonValue` wraps `serde_json::Value`. Constructors and setters accept either
the wrapper or a bare `serde_json::Value`:

```rust
let request = serde_json::json!({
    "action": "create",
    "arguments": { "name": "Alice" }
});

let exchange = toasty::create!(Exchange {
    request_body: request,
})
.exec(&mut db)
.await?;

assert_eq!(exchange.request_body["action"], "create");
```

An embed can contain a dynamic JSON field. When the embed is column-expanded,
the JSON field gets its own document column:

```rust
#[derive(toasty::Embed)]
struct Profile {
    name: String,
    extra: toasty::JsonValue,
}
```

When the embed itself uses `#[document]`, the JSON value remains nested JSON
instead of becoming an escaped JSON string:

```rust
#[derive(toasty::Model)]
struct User {
    #[key]
    id: u64,

    #[document]
    profile: Profile,
}
```

For the value `json!({"role": "admin"})`, the stored document contains:

```json
{
  "name": "Alice",
  "extra": { "role": "admin" }
}
```

Use `Json<T>` when the database should treat a serde-encoded value as an
opaque string. Use `JsonValue` when the database should store the value as a
native JSON document.

## Behavior

`JsonValue` preserves the JSON data model: objects, arrays, strings, numbers,
booleans, and JSON null round-trip through every supported driver. Create and
update operations replace the complete JSON value. Loading returns the
corresponding `serde_json::Value` through the wrapper.

`JsonValue(serde_json::Value::Null)` stores JSON null. For an
`Option<JsonValue>` field, `None` stores database null and `Some(JsonValue(
serde_json::Value::Null))` stores JSON null. Drivers must preserve this
distinction.

Malformed JSON returned by a database produces a Toasty error. JSON numbers
that cannot be represented by `serde_json::Number` also produce an error
instead of truncating or wrapping.

`JsonValue` supports whole-value equality only if Toasty later defines
portable equality semantics. The initial API does not generate filter methods,
foreign-key support, collection operators, or update helpers for JSON paths.

## Edge cases

JSON object key order is not part of the contract. PostgreSQL `jsonb` and
DynamoDB maps may reorder keys. Applications that need byte-for-byte request
replay must store the original body separately as `String` or `Vec<u8>`.

`serde_json::Value` rejects non-finite floating-point values, so NaN and
infinity cannot reach `JsonValue`. Large integer handling follows
`serde_json::Number`.

The empty object, empty array, and JSON null are values, not database null.
`Option<JsonValue>` is the only nullable form.

Schema migrations treat `JsonValue` as one document column. Changing keys or
nested values does not produce a schema migration.

## Driver integration

Drivers store `JsonValue` in the same native document representation used for
`#[document]` fields:

| Driver | Storage |
|---|---|
| PostgreSQL | `jsonb` |
| MySQL | `JSON` |
| SQLite and Turso | JSON text |
| DynamoDB | Native map, list, scalar, or null attribute |

Drivers decode the complete value without consulting an application schema.
SQL drivers receive a document-typed bind value. DynamoDB converts recursively
between JSON values and `AttributeValue` variants.

The existing document-storage capability gates `JsonValue`; no new operation
variant is required. Out-of-tree drivers that already support document columns
must accept dynamic JSON values. Drivers without document support reject the
schema during database setup.

## Alternatives considered

Changing `Json<T>` to use native document storage would alter existing schemas
and migrations. `Json<T>` remains an opaque serde-to-string wrapper.

Implementing `Field` directly for `serde_json::Value` would make a third-party
type part of Toasty's model API and leave no wrapper for future behavior or
metadata. `JsonValue` makes the storage contract explicit and provides room
for later path APIs.

Using `#[document]` on `serde_json::Value` would conflate typed embeds with
dynamic documents. `#[document]` continues to describe the storage of a known
embed type.

Storing request bodies only as `Vec<u8>` preserves exact bytes but prevents
drivers from using native JSON validation and storage. Applications may keep
both forms when they need structured storage and exact replay.

## Open questions

There are no blocking questions for whole-value storage. The syntax and
semantics of dynamic JSON paths are deferred.

## Out of scope

- Dynamic path filters and projections require a separate typed conversion API.
- Partial JSON updates require backend-specific path mutation support.
- JSON indexes require explicit backend capabilities and index configuration.
- Exact source-text preservation belongs to `String` or `Vec<u8>` fields.
- Automatic migration from `Json<serde_json::Value>` to `JsonValue` is
  database-specific.
