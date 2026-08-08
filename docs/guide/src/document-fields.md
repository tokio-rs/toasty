# `#[document]` Fields

`#[document]` stores a `#[derive(toasty::Embed)]` struct in one
structured column instead of expanding its fields into separate
columns. Toasty retains the embedded schema, so queries can address
scalar fields inside the stored object.

Use document storage when a value belongs to its parent row but its
fields still participate in filters. Use [JSON Encoding](./json-encoding.md)
for an opaque serde payload whose fields do not need generated query
paths.

## Column expansion and document storage

An embedded struct expands into one column per leaf field by default:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Embed)]
struct Address {
    city: String,
    postal_code: String,
}

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    address: Address,
}
```

This form creates columns such as `address_city` and
`address_postal_code`. Adding `#[document]` stores the same `Address`
value in one column:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Embed)]
# struct Address {
#     city: String,
#     postal_code: String,
# }
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    #[document]
    address: Address,
}
```

The Rust field type and create API are unchanged. The attribute changes
the database mapping and the way field paths lower into backend
queries.

## Storage by backend

Each built-in driver chooses a document representation:

| Driver | Stored representation |
|---|---|
| PostgreSQL | `jsonb` |
| MySQL | `JSON` |
| SQLite and Turso | JSON text |
| DynamoDB | Map (`M`) |

Applications do not add `#[column(type = ...)]` to a document field.
The driver capability selects the representation, and Toasty rejects a
driver that cannot store document fields.

## Creating and reading documents

Create a record by passing the embedded value:

```rust,ignore
let user = toasty::create!(User {
    address: Address {
        city: "Seattle".to_string(),
        postal_code: "98101".to_string(),
    },
})
.exec(&mut db)
.await?;

assert_eq!(user.address.city, "Seattle");
```

The driver encodes the object using the embed's named fields. Nested
embedded structs become nested document objects rather than additional
database columns.

## Filtering document fields

Generated field accessors keep the embedded structure:

```rust,ignore
let users = User::filter(
    User::fields().address().city().eq("Seattle"),
)
.exec(&mut db)
.await?;
```

The query engine lowers `address().city()` to the backend's document
path operation. PostgreSQL extracts a JSONB path, MySQL uses its JSON
functions, SQLite and Turso use JSON1, and DynamoDB uses a document path
expression.

Equality, ordering, and optional-field checks work on supported scalar
leaves. Comparing the entire document to an `Address` value is not yet
supported.

## Updating documents

Assigning a new embedded value replaces the complete document:

```rust,ignore
user.update()
    .address(Address {
        city: "Portland".to_string(),
        postal_code: "97205".to_string(),
    })
    .exec(&mut db)
    .await?;
```

`stmt::patch` updates individual fields of a column-expanded embed, but
it does not yet produce an in-place mutation for a `#[document]`
column. Replace the document when one of its fields changes.

## Collections of documents

A `Vec<T>` where `T` is an embedded struct stores a document array. No
attribute is required:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Embed)]
struct LineItem {
    sku: String,
    quantity: i64,
}

#[derive(Debug, toasty::Model)]
struct Order {
    #[key]
    #[auto]
    id: u64,

    items: Vec<LineItem>,
}
```

PostgreSQL, MySQL, SQLite, and Turso encode the field as an array of JSON
objects. DynamoDB stores it as a List (`L`) of Map (`M`) values.

Whole-value create, read, and replacement work for document
collections. `stmt::push` appends one embedded value. Element predicates
and the removal operations are not yet supported for document
collections.

## Supported document fields

The document root must be a named embedded struct or a collection of
named embedded structs. Document keys come from Rust field names.

Nested embedded structs, scalar lists, optional scalar fields, decimal
types, and supported `jiff` temporal types can appear inside a document.
Toasty uses canonical text encodings for temporal and decimal leaves so
filters compare the same values that reads reconstruct.

Current restrictions:

- Embedded enums cannot be encoded inside a document.
- Tuple structs are rejected because their fields have no names for
  document keys.
- Relations cannot appear inside a document.
- `jiff::Zoned` is rejected because its IANA time-zone annotation does
  not have a supported document representation.
- `Vec<u8>` is rejected because JSON has no binary scalar type.
- `#[column]` renames inside an embedded document are rejected because
  document keys use Rust field names.
- `#[index]`, `#[unique]`, and `#[column]` cannot be placed on the
  `#[document]` field.
- An optional document root is not yet supported; optional fields inside
  the document are supported.

Toasty validates these rules while building the schema, before the
driver creates a table.

## Document values at the driver boundary

The query engine represents a document as a named object whose shape
comes from the application schema. Before calling a driver, Toasty
converts the embedded record into that object representation.

Drivers encode and decode named objects without consulting the
application schema. The schema-aware conversion remains in the engine,
while the driver handles only the backend's JSON, map, or list format.

This differs from [`Json<T>`](./json-encoding.md), which crosses the
engine and driver boundary as an already serialized string. The
document representation preserves typed field paths; JSON encoding
accepts arbitrary serde types.
