# Read-only Markdown driver

## Summary

Toasty gains a read-only Markdown driver that presents a directory of Markdown
files as typed models. The driver loads the complete directory into an immutable
in-memory snapshot when `Db` is built, then executes Toasty's normal reads,
filters, relations, ordering, and pagination against that snapshot. The
filesystem mapping stays in `toasty-driver-markdown`; row storage and read-query
execution live in a reusable `toasty-driver-memory` crate so another file-backed
driver, and an eventual mutable in-memory driver, can use the same evaluator.
A new `Capability::data_mutations` flag lets the engine reject creates, updates,
and deletes before it lowers or dispatches them.

## Motivation

Documentation sites, blogs, and small content repositories commonly keep data
as Markdown with YAML front matter. The files are easy to review and edit, but
applications still need typed access, filtering, relations, and pagination.
Loading the same content into a separate database adds a synchronization step
and makes the database, rather than the repository, the source of truth.

The data set is already finite and local. Loading it once makes useful queries
possible without introducing a SQL runtime or translating Toasty's statement
tree to another query language. It also gives every query in one `Db` instance
a stable view of the content.

This work exposes a second need. Toasty's expression evaluator can already
evaluate many scalar expressions, and the non-SQL planner already produces
primary-key lookups, index queries, and scans. The missing piece is a reusable
row store and executor around those facilities. Keeping that piece independent
of Markdown avoids building a one-off query engine and provides the read half
of a future in-memory Toasty driver.

Finally, read-only behavior must be an engine contract. Rejecting writes only
when they reach a driver is too late for batches, nested statements, and plans
that perform reads before writes. The engine needs to know that the entire data
source is immutable before it starts planning or executing a statement.

## User-facing API

### Opening a Markdown directory

Create a `Markdown` driver with the content root and pass it to `Db::builder()`:

```rust
use toasty::Db;
use toasty_driver_markdown::Markdown;

let driver = Markdown::new("content");
let mut db = Db::builder()
    .models(toasty::models!(Post, Author))
    .build(driver)
    .await?;
```

`build()` reads and validates the content before it returns. A successful build
means subsequent reads do not touch the filesystem.

### Directory and file conventions

By default, each immediate child directory of the root represents a database
table, and each `.md` file directly inside that directory represents one row:

```text
content/
  posts/
    hello-world.md
    release-notes.md
  authors/
    carl.md
```

Directory names match Toasty's database table names. YAML front-matter keys
match database column names. The Markdown following the front matter maps to a
string column named `body` when that column exists.

For example, `content/posts/hello-world.md` can back this model:

```rust
#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    slug: String,
    title: String,
    published: bool,
    tags: Vec<String>,
    body: String,
}
```

with this file:

```markdown
---
title: Hello, world
published: true
tags:
  - introduction
  - rust
---
# Hello, world

This is the first post.
```

The `slug` is absent from the front matter, so the default single-string-key
convention derives it from the file stem: `hello-world`. If a table does not
have exactly one string primary-key column, every key column must appear in the
front matter or have an explicit configured source.

The driver treats front matter as data and the body as an uninterpreted UTF-8
string. It does not render Markdown or extract headings and links.

### Overriding the conventions

Use `Markdown::builder()` when the repository's layout or vocabulary does not
match the defaults. Configuration is per database table and uses database
names, because mapping files to columns is a driver responsibility:

```rust
use toasty_driver_markdown::{Markdown, Table};

let driver = Markdown::builder("content")
    .table(
        "posts",
        Table::new("articles")
            .column("date", "published_at")
            .body_column("markdown")
            .key_from_stem("slug"),
    )
    .strict(true)
    .build();
```

This maps the `posts` table to `content/articles`, maps the `date` front-matter
key to the `published_at` column, maps the Markdown body to `markdown`, and uses
the file stem for `slug`.

The table configuration supports these overrides in the first release:

- directory name;
- front-matter key to column renames;
- the body column, including disabling body mapping;
- a file-stem or relative-path source for one string column; and
- recursive discovery, disabled by default.

With recursive discovery enabled, files below the configured directory are
included. A table must use front-matter keys or a relative-path key source when
two files can have the same stem. Relative paths use `/` separators on every
platform and omit the `.md` suffix.

Unknown directories and unknown front-matter keys are ignored by default so an
existing content repository can carry metadata unrelated to Toasty. Strict mode
turns both into build errors and also rejects a non-empty body with no body
column. An explicitly configured directory that does not exist is always an
error. An unconfigured table with no matching directory is an empty table.

### Querying content

Markdown models use the same read API as other Toasty models:

```rust
let posts = Post::filter(
    Post::fields()
        .published()
        .eq(true)
        .and(Post::fields().title().ilike("%rust%")),
)
.order_by(Post::fields().title().asc())
.limit(20)
.exec(&mut db)
.await?;
```

Primary-key lookups, equality and ordered comparisons, `IN`, `BETWEEN`, null
checks, boolean composition, `starts_with`, `like`, `ilike`, collection
predicates, document-path predicates, ordering, limits, offsets, and cursor
pagination are supported. Relation filters and `include` work when the files
contain the foreign-key columns described by the models; Toasty's engine keeps
responsibility for decomposing and merging those reads.

The Markdown backend defines its own string-matching behavior. `starts_with`
and `like` are case-sensitive. In `like` and `ilike`, `%` matches zero or more
Unicode scalar values, `_` matches one, and the optional escape character has
the same meaning as Toasty's existing escaped LIKE API. `ilike` compares
literals using locale-independent Unicode case folding. It does not apply
Unicode normalization or a language-specific collation.

### Read-only errors

Generated mutation methods remain available because models are independent of
the driver selected at runtime. Executing one against Markdown returns
`Error::unsupported_feature`:

```rust
let error = Post::create()
    .slug("new-post")
    .title("New post")
    .published(false)
    .tags(Vec::new())
    .body("Draft")
    .exec(&mut db)
    .await
    .unwrap_err();

assert!(error.is_unsupported_feature());
```

`db.capability().data_mutations` lets infrastructure inspect the same contract
before offering an editing workflow. It is informational for application code;
the engine check remains authoritative.

## Behavior

### Building the snapshot

`Db::builder().build(driver)` first compiles the Toasty schema, then gives that
schema to the driver for initialization. The Markdown driver walks the root in
sorted path order, reads every selected file, decodes its front matter and
body, and converts each field to the database column type. It then builds one
immutable in-memory snapshot containing all rows and declared indices. The
snapshot is shared by every pooled connection through `Arc`; reads need no
locks.

The body is loaded along with the front matter. Eagerly loading only metadata
would let a later body read observe a different filesystem version and would
make a query's result depend on when a field was projected. The snapshot uses
memory proportional to decoded rows, bodies, and indices.

Filesystem changes are not visible after `build()` returns. There is no watcher
or automatic reload. Drop the `Db` and build another one to observe a new
version. A cursor belongs to the snapshot and query shape that produced it; a
cursor with the wrong table or ordering shape returns an invalid-cursor error.

### Decoding rows

A file may start with YAML front matter delimited by `---` lines. Without a
front-matter block, its attribute map is empty and the entire file is the body.
The delimiters and their structural newline are not included in `body`; all
remaining UTF-8 text is preserved. An opening delimiter without a closing
delimiter is a build error rather than body text.

YAML values are converted according to the compiled database schema. Booleans,
integers, strings, sequences, mappings, UUIDs, decimals, and temporal values
must have a representation accepted by the target column type. The driver does
not stringify incompatible YAML values. A missing optional column becomes
`Value::Null`. A missing required column is a build error unless its value comes
from the file path. `#[auto]` does not generate values in a read-only source.

If the configured body column is present in front matter as well as in the
file body, initialization reports an ambiguous-column error. An empty body is
still a present empty string. Every decoded row is validated for width and
type, and duplicate primary or unique keys fail the build. Errors identify the
file, front-matter key or body mapping, expected Toasty type, and offending
value without printing unrelated file contents.

A unique-key value containing null is not entered in the uniqueness map, so
multiple rows may omit the same optional unique field. Primary-key columns
cannot be null.

The loader does not follow symbolic links. This avoids cycles and prevents a
content root from implicitly reading files outside itself. Only lowercase
`.md` files match the default convention.

### Executing reads

The engine continues to simplify, lower, and plan statements. It emits the
same non-SQL operations used by other key-value drivers:

- `GetByKey` for exact primary-key reads;
- `QueryPk` and `FindPkByIndex` when a declared index satisfies the query; and
- `Scan` as the fallback for all other reads.

The reusable in-memory executor applies a read in this semantic order: choose
candidate rows, evaluate the remaining predicate, order the matches, apply a
cursor or offset and limit, then project the requested columns. Index selection
is an optimization and cannot change the result.

Declared primary and secondary indices are built eagerly. A query without a
usable index scans the in-memory table. The first release does not add a query
optimizer or collect cardinality statistics; the existing Toasty planner
chooses the access operation.

Queries without `order_by` retain Toasty's unspecified-order contract. The
implementation iterates the primary-key index for repeatability, but users must
not rely on that order. Explicit ordering is stable. The executor appends the
primary key as an internal tie-breaker, places null before non-null when sorting
descending and after non-null when sorting ascending, and compares other values
according to their Toasty type. Cursor values contain the explicit ordering
values and primary key, which prevents duplicates or omissions while traversing
the immutable snapshot.

Cursor pagination without an explicit ordering uses primary-key order as its
backend cursor order. Forward and backward pagination are both supported.

Predicate evaluation is two-valued. Missing optional fields are null;
`is_none()` and `is_some()` test them directly. Equality uses Toasty value
equality, so null equals null and differs from a non-null value. Ordered,
string, and collection predicates involving null do not match. `AND`, `OR`,
and `NOT` then operate on the resulting booleans. These are the native semantics
of the in-memory backend, not SQL three-valued logic.

### Relations, batching, and transactions

Relations are not materialized inside Markdown rows. Front matter stores the
same foreign-key values another driver would store in columns. The Toasty
engine performs relation filters, batched association loads, and nested merges
using ordinary reads from the snapshot.

All connections share the same immutable snapshot, so a multi-read plan is
consistent without locking. Transaction lifecycle operations succeed as
no-ops. This lets existing read-only batches and relation-loading plans use the
normal execution path. It does not make mutation statements acceptable.

### Rejecting mutations

When `Capability::data_mutations` is false, statement verification rejects
every `Insert`, `Update`, and `Delete`. The visitor checks nested statements,
CTEs, and batches as well as the top-level statement. Verification runs before
lowering, planning, connection acquisition, or execution, so a mixed read/write
batch performs no reads before it fails.

The in-memory executor also rejects mutation `Operation` variants. That is a
defensive check for callers that invoke `Connection::exec` directly; normal
Toasty queries fail at the earlier engine boundary.

Raw SQL is unsupported because the Markdown driver reports `sql: false`.
`push_schema`, migrations, and `reset_db` return unsupported-feature errors;
`data_mutations` describes row mutations and does not claim that a driver can
rewrite its external schema or source files.

## Edge cases

- An empty root, a missing unconfigured table directory, or a table directory
  with no `.md` files produces an empty table.
- A configured directory must remain inside the content root after path
  normalization. Absolute paths and `..` escapes are rejected.
- File stems and relative-path keys must be valid UTF-8. Two paths that decode
  to the same typed key are duplicates even if their spellings differ.
- Front-matter key renames must be one-to-one. Two keys cannot populate one
  column, and one key cannot populate two columns.
- Composite keys are supported when every component is supplied explicitly.
  File-stem derivation applies to at most one string column.
- Optional front matter may use explicit YAML `null`; required columns reject
  it. An absent optional body column and a present empty body remain distinct
  only when the model type can represent absence.
- LIKE matching operates on Unicode scalar values rather than bytes or user-
  perceived grapheme clusters. Canonically equivalent but differently encoded
  strings compare as different strings.
- Offset and cursor bounds are applied after filtering and ordering. A page is
  therefore filled up to its requested size when enough matching rows remain.
- Concurrent queries can run on separate connections without coordination.
  Snapshot construction finishes before any connection becomes available.
- A malformed file fails the whole build. The driver never publishes a
  partially loaded snapshot.

## Driver integration

### Data-mutation capability

`Capability` gains one field:

```rust
pub struct Capability {
    pub data_mutations: bool,
    // existing fields
}
```

All existing in-tree drivers set it to `true`. The Markdown driver and the
initial read-only memory driver set it to `false`. The query verifier uses the
flag as described above. This capability is independent of `SchemaMutations`,
which describes how a backend changes column definitions, and independent of a
server-side read-only transaction error.

Out-of-tree drivers must add the field to their capability constant. A driver
that already supports Toasty's insert, update, and delete operations should set
it to `true`; a driver that cannot persist any of them should set it to `false`.

### Schema-aware driver initialization

Drivers currently receive the compiled schema only after a connection is asked
to execute an operation. Eager, typed loading needs it before the pool accepts
queries. `Driver` therefore gains an asynchronous hook with a no-op default:

```rust
async fn initialize(&mut self, schema: &Arc<Schema>) -> Result<()> {
    Ok(())
}
```

`Db::builder().build()` calls `initialize` after capability validation and
schema construction, but before constructing the pool. `Connect` delegates the
hook to its selected driver. Existing drivers and out-of-tree drivers require
no implementation change. The Markdown implementation parses files and builds
its shared snapshot here, so filesystem and decoding errors surface from
`build()`.

### Reusable in-memory crate

The reusable crate is `toasty-driver-memory`. It has no Markdown or filesystem
dependencies. It owns:

- database rows in database-column order using Toasty's existing `Value` and
  `ValueRecord` types;
- immutable snapshot construction and schema validation;
- primary, unique, and secondary index construction;
- execution of `GetByKey`, `QueryPk`, `FindPkByIndex`, and `Scan`;
- row projection, filtering, ordering, limit/offset handling, and cursor
  encoding; and
- a read-only `Driver` wrapper for callers that want to supply rows directly.

Its central reusable boundary is a read executor over a store interface. The
crate ships an immutable `Snapshot` implementation, while the executor depends
only on access to table rows and declared indices. A future mutable memory store
can implement the same read interface and reuse the entire query path while
adding insert, update, delete, index maintenance, and transaction behavior.
Mutation support does not belong in the initial interface merely to anticipate
that future work.

`toasty-driver-markdown` owns only source-specific behavior:

- path discovery and containment checks;
- YAML front-matter and UTF-8 body parsing;
- convention and configuration resolution;
- conversion from source values into typed database rows; and
- file-oriented diagnostics.

After initialization, the Markdown connection delegates every supported read
operation to the memory executor. Similar drivers for JSON, static assets, or
configuration repositories can build the same snapshot without copying query
execution code.

Expression evaluation remains in `toasty-core::stmt`, because the Toasty engine
already uses it for client-side filters, guards, and nested merges. The work
completes its predicate coverage for the expressions advertised by the memory
backend: `BETWEEN`, `starts_with`, `LIKE` and `ILIKE` with escapes, length,
collection membership and set predicates, and lowered document extraction.
The memory crate supplies a typed row as evaluator input; it does not implement
a second statement AST or planner. Unsupported expression shapes return
`Error::unsupported_feature` during verification rather than reaching a
`todo!` or panic in evaluation.

The memory crate has its own operation-level conformance suite. The Markdown
driver runs the read-only portion of Toasty's driver integration suite plus
loader tests for conventions, overrides, type errors, duplicate keys, snapshot
isolation, and path handling. A future mutable memory driver adds the mutation
portion without duplicating read tests.

### Scan ordering contract

The existing `scan_supports_sort` capability says a driver can order a scan,
but `Operation::Scan` does not carry the ordering expressions. The operation
gains an optional field:

```rust
pub struct Scan {
    pub table: TableId,
    pub columns: Vec<usize>,
    pub filter: Option<stmt::Expr>,
    pub order_by: Option<stmt::OrderBy>,
    pub limit: Option<Pagination>,
}
```

The planner preserves the lowered `order_by` when it chooses a scan. A driver
with `scan_supports_sort: true` must apply filtering, ordering, and pagination
in that order. A driver with the flag set to `false` continues to receive an
early unsupported-feature error for an ordered scan and never receives a scan
with `order_by: Some(_)`. DynamoDB remains in that group. SQL drivers do not
receive `Operation::Scan`. When an index can satisfy a filter but not an
arbitrary requested order, the planner may choose the sortable scan instead;
it must not choose an index access path that changes or rejects an otherwise
supported ordering.

The Markdown and read-only memory capabilities report `sql: false`,
`data_mutations: false`, `scan: true`, and `scan_supports_sort: true`. They also
advertise native support for the scalar, string, document, and collection
predicates implemented by the shared evaluator. Here, “native” means the
backend implements the operator with its documented semantics; it does not
mean the operator must come from an external database process. The
`native_ilike` documentation and verifier error are generalized accordingly;
PostgreSQL remains the only SQL backend with native `ILIKE`, while the memory
backend supplies its own documented case-insensitive matcher.

## Open questions

There are no blocking open questions. Exact Rust type names may be adjusted
during implementation, but the mapping defaults, snapshot lifecycle,
read-only contract, query semantics, and crate boundary are part of this
design.

## Out of scope

- **Writing Markdown files.** There is no create, update, delete, rename, or
  formatting-preservation behavior until a concrete mutation use case exists.
- **A mutable in-memory driver.** The shared crate is structured to support it,
  but mutation semantics, index maintenance, and transactions need their own
  design.
- **File watching and reload.** A `Db` is one immutable snapshot; rebuilding it
  is the only refresh mechanism.
- **Aggregates and grouping.** `count()`, `GROUP BY`, and other aggregates keep
  their current non-SQL unsupported-feature errors in the first release.
- **Raw SQL.** The driver evaluates Toasty operations, not a SQL language.
- **Markdown interpretation.** Rendering, full-text search, headings, links,
  and syntax extensions are application concerns.
- **Other metadata formats and extensions.** The first release accepts YAML
  front matter in lowercase `.md` files. TOML, JSON, and `.markdown` discovery
  can be added without changing the snapshot or evaluator contracts.
- **Out-of-core execution.** The design targets content sets that fit in
  memory. Streaming and spill-to-disk execution require a different lifecycle.
- **Schema management.** Toasty validates models against files but does not
  create directories, rewrite front matter, or apply migrations to content.
