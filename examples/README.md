# Toasty Examples

Each example is a self-contained program that demonstrates a set of Toasty
features in a realistic scenario. Start with `quickstart-blog`, then read
whichever example covers what you need. The [user guide](../docs/guide) links to
these examples for runnable versions of what it describes.

## Contents

| Example | What it covers |
| --- | --- |
| [quickstart-blog](quickstart-blog) | Defining models and keys, and the create/read/update/delete loop over a `has_many`/`belongs_to` relationship. Start here. |
| [forum-relationships](forum-relationships) | `has_one`, traversing a relationship in both directions, preloading with `.include()` to avoid N+1 queries, multi-step `via` relations, and association filters. |
| [product-search](product-search) | Reading data: filter expressions, sorting, `limit`/`offset`, cursor pagination, column projection, and a composite index. |
| [cms-article-fields](cms-article-fields) | Field options: create and update defaults, auto timestamps, `Json<T>`, a queryable `Vec<scalar>`, deferred columns, and custom table/column names. |
| [crm-embedded](crm-embedded) | Embedded value types: flattened structs, tagged-union enums, newtype keys, and partial embed updates with `stmt::patch`. |
| [store-operations](store-operations) | Writes: interactive transactions, savepoints, batch inserts, query-based update and delete, and raw SQL. |
| [service-ops](service-ops) | Project layout: shared models in a library, an application binary, and a migration CLI built from `toasty-cli`. |

## Running an example

Every example runs against an in-memory SQLite database by default, so no setup
is required:

```sh
cargo run -p example-quickstart-blog
```

Each program prints what it does as it runs.

### Running against PostgreSQL or MySQL

The examples read the connection URL from `TOASTY_CONNECTION_URL` and fall back
to `sqlite::memory:`. To run one against another SQL backend, set the URL and
enable the matching driver feature (`sqlite` is the default; `postgresql` and
`mysql` are also available):

```sh
TOASTY_CONNECTION_URL=postgresql://user:pass@localhost/mydb \
  cargo run -p example-product-search --features postgresql
```

### service-ops

`service-ops` has two binaries. `cargo run -p example-service-ops` runs the
`server` binary (the default). The migration CLI is a separate binary; run it
from the example's directory so it finds `Toasty.toml`:

```sh
cd examples/service-ops

# Generate a migration after editing the models in src/lib.rs:
cargo run --bin migrate -- migration generate --name <name>

# Apply pending migrations to a persistent database:
TOASTY_CONNECTION_URL=sqlite:./service.db cargo run --bin migrate -- migration apply
```

Each example's `src/main.rs` (or `src/lib.rs`) opens with a comment describing
what it teaches.
