# Item Collections

## Summary

An `#[item_collection]` model attribute declares that a model participates in a
single DynamoDB table shared with a related model. Toasty synthesizes a composite
sort key from the ancestry chain, collapses `.include()` and chain-navigation
calls into single DynamoDB requests, and rejects item-collection schemas at
connection time on SQL drivers. Item-collection is a DynamoDB-only storage
directive; the Rust API is otherwise unchanged.

## Motivation

DynamoDB single-table design co-locates hierarchical data under one partition
key to get cheap subtree reads and fewer round trips on hierarchy loads.
Toasty users with a `Tenant → User → Todo → Subtask` shape today have two poor
options: one table per model with an access pattern that pays N queries for
hierarchy loads, or hand-rolled sort keys and discriminators that skip
Toasty's relation API entirely.

The single-table idiom requires coordination across macros, schema, engine, and
driver: composite sort-key synthesis on write, prefix-based filter translation
on read, per-row model dispatch when a query returns multiple row types. The
plumbing does not exist today. This design adds it.

Affected:

- **Toasty users** writing DynamoDB-backed applications with nested relations.
- **DynamoDB driver maintainers** (implementation target).
- **SQL driver maintainers** (add one rejection path at schema setup; no other
  changes).

## User-facing API

### Declaring an item collection

Mark every model that participates in the shared table with
`#[item_collection]`. The root uses the bare form; children name their direct
parent.

```rust
#[derive(Debug, toasty::Model)]
#[item_collection]
#[key(partition = id, local = sk)]
struct Tenant {
    #[auto]
    id: Uuid,

    name: String,

    #[version]
    version: u64,

    #[has_many]
    users: HasMany<User>,
}

#[derive(Debug, toasty::Model)]
#[item_collection(Tenant)]
#[key(partition = tenant_id, local = sk)]
struct User {
    #[auto]
    id: Uuid,

    tenant_id: Id<Tenant>,

    name: String,

    #[version]
    version: u64,

    #[belongs_to]
    tenant: BelongsTo<Tenant>,

    #[has_many]
    todos: HasMany<Todo>,
}

#[derive(Debug, toasty::Model)]
#[item_collection(User)]
#[key(partition = tenant_id, local = sk)]
struct Todo {
    #[auto]
    id: Uuid,

    tenant_id: Id<Tenant>,

    title: String,

    #[version]
    version: u64,

    #[belongs_to]
    user: BelongsTo<User>,
}
```

Three things to notice:

- **Every child declares the root's id as its partition key field**
  (`tenant_id: Id<Tenant>`). All rows in an item collection share the same
  partition key. Intermediate ancestor ids — a Subtask's `user_id` or
  `todo_id` — are *not* declared. Ancestry lives inside the sort key, which
  Toasty manages; models never expose or construct it.

- **`Id<T>` is a bare `Uuid`.** Same meaning as on standalone models. There is
  no opaque compound identifier type for item-collection members.

- **`#[belongs_to]` arguments are optional.** For item-collection children the
  macro uses the ancestry chain from `#[item_collection(Parent)]` to derive
  `key` and `references` from the partition-key field and the parent's primary
  key. Bare `#[belongs_to]` is the idiomatic form. Explicit
  `#[belongs_to(key = …, references = …)]` is accepted when it matches the
  implicit derivation and rejected at compile time when it does not.

### Creating rows

Roots are created the usual way:

```rust
let tenant = Tenant::create().name("acme").exec(&mut db).await?;
```

Children **must** be created through a parent handle. The unscoped form
(`User::create()`) is not generated for item-collection children:

```rust
let user = tenant.users().create().name("alice").exec(&mut db).await?;

let todo = user.todos().create().title("buy milk").exec(&mut db).await?;
```

Toasty reads the parent's sort key, appends a new segment for the child, and
writes the resulting value. The user never types or sees the composite sort key.

Attempting `User::create()` or `Todo::create()` is a compile error. The error
points at the scoped form (`tenant.users().create()`).

### Reading rows

Navigate from the root through the relation chain. The planner collapses
all-equality chains that terminate in a single-row operation into one DynamoDB
`GetItem`:

```rust
// One DynamoDB GetItem.
let todo = tenant.users().filter_by_id(user_id).todos().filter_by_id(todo_id).get(&mut db).await?;
```

Top-level `Todo::get_by_*` is not generated for item-collection children. The
only way to fetch a child is through its parent handle.

`HasMany` queries load the immediate tier:

```rust
// Returns user's Todos only; the engine issues begins_with(sk, user.sk + "#")
// and discards any descendant rows returned by the prefix match.
let todos = user.todos().exec(&mut db).await?;
```

### Loading a hierarchy with `.include()`

`.include()` chains across an item collection collapse into **one** DynamoDB
`Query`. Every row the query returns is dispatched to its model's mapping based
on the sort-key prefix; only the models named by `.include()` are populated.

```rust
// One DynamoDB Query.
let tenants = Tenant::filter_by_id(tenant_id)
    .include(Tenant::fields().users())
    .include(Tenant::fields().users().todos())
    .exec(&mut db)
    .await?;
```

The returned `Tenant` has `users` populated; each `User` has `todos` populated.
Any model that is not named by `.include()` stays unloaded.

A query with a non-key filter on any included relation falls back to per-
relation queries; the collapsed path requires every predicate to reduce to a
sort-key key condition.

### Secondary indexes

Declare GSIs on item-collection models with the standard `#[index]` attribute.
No item-collection-specific syntax:

```rust
#[derive(Debug, toasty::Model)]
#[item_collection(User)]
#[key(partition = tenant_id, local = sk)]
struct Todo {
    #[auto]
    id: Uuid,

    tenant_id: Id<Tenant>,

    #[index]
    priority: Priority,

    #[belongs_to]
    user: BelongsTo<User>,
}
```

Toasty renames columns on item-collection models with the declaring model's
snake-case prefix, so the GSI above is keyed on `todo__priority`, not
`priority`. The prefix is what makes the index sparse: rows for `User` or any
other model in the shared table have no `todo__priority` attribute and
therefore do not appear in the GSI. Queries against `Todo`'s GSI return only
`Todo` rows by construction.

### Delete

A model with a `#[has_many]` attribute causes Toasty to cascade deletes from the model to its children.
With an RDBMS, this cascade is wrapped in a transaction and is atomic. DynamoDB transactions are limited to 100 items,
so Toasty cannot guarantee atomic cascade deletes on DynamoDB.

When using `#[item_collection]`, deletes do not cascade to children.

### Rename resistance with `sort_prefix`

The default sort-key prefix for a model is its type name. Renaming a model
would otherwise require rewriting every stored row. Override the prefix to keep
written data compatible:

```rust
// Even after the Rust type is renamed from `User` to `Member`, stored rows
// keep their `User#…` sort-key prefix and continue to round-trip.
#[item_collection(Tenant, sort_prefix = "User")]
struct Member { /* … */ }
```

Two models in the same collection resolving to the same prefix (default or
overridden) is a schema-build error.

### `#[version]`

Optimistic concurrency via `#[version] version: u64` works the same as on
standalone models. DynamoDB's conditional-write path enforces the check; item
collections do not change the semantics.

## Behavior

### Happy path

- Inserts populate the sort key from the parent context; one `PutItem` call per
  row.
- Chain-navigated single-row reads compile to one `GetItem`.
- `.include()` chains that stay within the collection compile to one `Query`;
  each row is hydrated into the relation field that matches its sort-key shape.
- `HasMany` queries filter results to the immediate tier; deeper rows returned
  by the underlying prefix query are discarded client-side.

### Errors

**Compile-time** (from the macro):

- `User::create()` on an item-collection child fails with "item-collection
  child; use `tenant.users().create()`".
- `User::get_by_id(…)` on an item-collection child fails with "item-collection
  child; reach via parent handle".
- `#[belongs_to(key = …, references = …)]` on an item-collection child whose
  explicit arguments do not match the implicit derivation fails with a message
  naming both the expected and provided fields. Explicit arguments that match
  the implicit derivation are accepted.
- `#[belongs_to] parent: BelongsTo<Wrong>` where `Wrong` is not the declared
  direct parent fails with a message naming the expected type.

**Schema-build time** (on first connection):

- Duplicate sort prefixes across models in one collection.
- Cycle in the `#[item_collection]` ancestry chain.
- A child's partition-key field type does not match the root's id type.
- Two models produce colliding column names in the shared table.

**Driver-time** (on first connection):

- Any SQL driver rejects a schema containing any item-collection model at
  `push_schema` with an `unsupported_feature` error naming the offending model.
- The DynamoDB driver rejects any model with a composite primary key declared
  via multiple field-level `#[key]` attributes. Composite primary keys on DDB
  require struct-level `#[key(partition = …, local = …)]` so the
  partition-versus-range assignment is explicit. SQL drivers accept either
  declaration form without distinction (the two forms produce equivalent SQL
  schemas).

**Query-time**:

- Malformed sort-key key conditions (a range predicate without an equality on
  the preceding component) are rejected with a clear message before the call
  reaches DynamoDB.

### Defaults

- Sort-key prefix defaults to the model type name in upper-camel-case.
- `HasMany` on an item-collection relation returns immediate-tier rows only.
- `.include()` on an item-collection relation collapses into one `Query` when
  eligible; falls back to per-relation queries otherwise.

### Interactions with other features

- **`#[version]`**: unchanged. DynamoDB's conditional-write path enforces the
  version check.
- **`#[auto]`, `#[default]`, `#[update]`**: unchanged on user-authored fields.
- **`#[embed]` structs and enums**: work on item-collection models. Columns are
  prefixed with the model name the same way primitive fields are, so
  `todo__priority_color` rather than `priority_color`.
- **GSIs (`#[index]`)**: work as on standalone models. Sparsity falls out of
  per-model column prefixing — a GSI keyed on a `Todo`-specific attribute only
  indexes `Todo` rows.
- **Non-item-collection models**: coexist in the same schema and are unaffected
  by this feature.

## Edge cases

### Deep-hierarchy over-fetch

A `HasMany` load at the top of a deep tree over-fetches. `user.todos()` on a
three-level `User → Todo → Subtask` hierarchy issues
`begins_with(sk, user.sk + "#")`, which DynamoDB expands to every Todo *and*
every Subtask under those Todos. The engine discards the Subtask rows before
returning, but the RCUs are paid.

No `BETWEEN` range over the sort key selects only the immediate tier. In
sort-key order, each row is followed by its own descendants before the next
sibling:

```
User#u1#Todo#t1
User#u1#Todo#t1#Subtask#s1
User#u1#Todo#t1#Subtask#s2
User#u1#Todo#t2
User#u1#Todo#t2#Subtask#s3
```

Any range that includes the three `Todo#…` rows also includes the `Subtask#…`
rows between them. The engine issues the prefix query and discards the
descendant rows client-side.

Users with deep trees who need the full subtree should prefer one `.include()`
call that names every level — the collapsed path issues one query regardless of
depth. Users who need only the immediate tier on a deep hierarchy will pay the
over-fetch cost; a GSI-backed opt-out is a future optimization.

### Non-UUID ids

The sort-key synthesis and dispatch machinery work with any id type. The
pagination and range-predicate paths have been designed for UUID ids; other id
types may produce different over-fetch or lexicographic-ordering profiles. The
feature is not restricted to UUIDs, but UUID is the documented happy path for
v1.

### Parent sort key always loaded

The engine needs each parent's sort-key value to synthesize child sort keys
during scoped creation. Item-collection models always include the sort key in
the returning of any load that produces an instance — there is no partial-load
mode that omits it. This is invisible to users but relevant for driver authors
implementing or interacting with the returning contract.

## Storage layout

Every row in an item-collection table shares the root's id as its partition
key. The sort key encodes the ancestry chain from root to self, as segments
joined by `#`. Two tenants with some users and todos produce a table like
this:

| Partition key (`id`) | Sort key (`__sk`) | Other attributes |
|---|---|---|
| `t1` | `Tenant#t1` | `tenant__name = "acme"`, `tenant__version = 1` |
| `t1` | `Tenant#t1#User#u1` | `user__name = "alice"`, `user__tenant_id = t1`, `user__version = 1` |
| `t1` | `Tenant#t1#User#u1#Todo#td1` | `todo__title = "buy milk"`, `todo__tenant_id = t1`, `todo__version = 1` |
| `t1` | `Tenant#t1#User#u1#Todo#td2` | `todo__title = "walk dog"`, `todo__tenant_id = t1`, `todo__version = 1` |
| `t1` | `Tenant#t1#User#u2` | `user__name = "bob"`, `user__tenant_id = t1`, `user__version = 1` |
| `t1` | `Tenant#t1#User#u2#Todo#td3` | `todo__title = "write docs"`, `todo__tenant_id = t1`, `todo__version = 1` |
| `t2` | `Tenant#t2` | `tenant__name = "beta"`, `tenant__version = 1` |
| `t2` | `Tenant#t2#User#u3` | `user__name = "carol"`, `user__tenant_id = t2`, `user__version = 1` |

A few properties of this layout:

- Non-key attributes are prefixed with the declaring model's snake-case name
  (`tenant__name`, `user__name`, `todo__title`). A User row has no
  `todo__title` attribute; a Todo row has no `user__name` attribute.
- There is no discriminator column. A row's model is identified by the
  leading segments of its sort key.
- Every row carries its own `#[version]` value. Optimistic concurrency is
  per-row, as with standalone models.

### How user-facing operations translate

Every user-facing API example from earlier lands on one of these DynamoDB
call shapes. The engine manages partition keys, sort-key synthesis, and
key-condition construction; the user never builds any of these by hand.

| User code | DynamoDB call |
|---|---|
| `tenant.users().exec(&db)` | `Query(pk = t1, begins_with(sk, "Tenant#t1#User#"))` + engine tier-split |
| `user.todos().exec(&db)` | `Query(pk = t1, begins_with(sk, "Tenant#t1#User#u1#Todo#"))` + engine tier-split |
| `tenant.users().filter_by_id(u1).todos().filter_by_id(td1).get(&db)` | `GetItem(pk = t1, sk = "Tenant#t1#User#u1#Todo#td1")` |
| `Tenant::filter_by_id(t1).include(users()).include(users().todos())` | `Query(pk = t1, begins_with(sk, "Tenant#t1#"))` + engine per-row dispatch |
| `tenant.users().create().name("alice")…exec(&db)` | `PutItem(pk = t1, sk = "Tenant#t1#User#<new_id>", ...)` |

Tier-split means the engine filters returned rows by sort-key segment count
so a `HasMany` call returns only the immediate tier. See [Edge cases](#edge-cases)
for the over-fetch trade-off this introduces on deep hierarchies.

## Driver integration

### DynamoDB

Implementation target. The DDB driver:

- Serializes the engine's structured sort-key value as a single `AttributeValue::S`
  attribute named `__sk`, formatted `Prefix#id#Prefix#id…` with no trailing `#`
  separator.
- Translates the engine's sort-key-prefix filter predicate into a DynamoDB
  `KeyConditionExpression` of the form `begins_with(__sk, :prefix)`.
- Handles the full DynamoDB key-condition grammar: a left-anchored sequence of
  equality predicates on sort-key components, optionally followed by one
  trailing range predicate (`>`, `<`, `>=`, `<=`, `BETWEEN`, `begins_with`) on
  the next component. Malformed shapes produce a query-planning error before
  the call is issued.

The DynamoDB table declared for an item-collection group uses exactly two key
attributes: the partition key (named by the root's `#[key(partition = …)]`)
and `__sk` (type `S`). Any additional attributes on the table correspond to
GSI key schemas. Non-key columns never appear in `AttributeDefinition`.

### Other drivers

SQL drivers (SQLite, PostgreSQL, MySQL) do not support item collections. Each
one rejects item-collection schemas at `push_schema`:

```rust
if schema.contains_item_collections() {
    return Err(Error::unsupported_feature(format!(
        "item collections are only supported on DynamoDB; model `{}` declares #[item_collection]",
        offending_model.name(),
    )));
}
```

A helper on `Schema` (`contains_item_collections()`) returns true iff any
registered model has `ModelRoot::item_collection.is_some()`.

New variants in `stmt::Value` (`SortKey`) and `stmt::Expr` (sort-key-prefix
predicate) are added to `toasty-core`. Every driver — in-tree or out-of-tree —
must add match arms for the new variants to satisfy exhaustive matching.
Drivers that do not intend to support item collections can write the arms as
`unreachable!()`; the rejection pattern above ensures those branches never
execute at runtime. Driver authors implement nothing beyond the rejection and
the exhaustive match arms.

### Capability additions

None. Item-collection support is indicated by a driver's willingness to handle
the new `stmt::Value::SortKey` and prefix predicate; the DynamoDB driver is
the only current driver that does. The rejection path substitutes for a
capability flag.

## Alternatives considered

**Inferred item-collection membership from `pk: Id<Root>` plus relation chain.**
No explicit `#[item_collection(Parent)]` on children; the macro infers
membership from the partition-key field type and the relation graph. Rejected.
The implicit form creates a chicken-and-egg loop with the implicit
`#[belongs_to]` form this design also introduces (the `BelongsTo` inference
reads the `#[item_collection(Parent)]` annotation). Explicit annotation also
makes ancestry visible at each model's declaration site, which matters for
code review and schema comprehension.

**Literal `pk` / `sk` field names with opaque compound `Id<T>`.** The earlier
2026-04-24 plan used `pk: Id<Self>; sk: Id<Self>` on every model and
represented `Id<T>` for item-collection members as a distinct wrapper
backed by `String`. Rejected. Changes what `Id<T>` means per-model, which
propagates through application code that is otherwise unaware of storage
layout. The domain-named-field approach keeps `Id<T>` a single type with
uniform semantics.

**Retain the proof-of-concept's `__model` discriminator column.** The POC
carried an engine-level `__model: String` column whose value was rewritten by
the driver into the leading segment of `__sk`. Every code path that touched
`__model` required a special case: insert skipped it, read discarded it, the
key-expression builder decomposed it into a prefix predicate, the filter
injector emitted a column equality later rewritten by the driver, and the
nested-merge planner tripped on it because the column was in the filter but
not the selection. Rejected. Removing the column eliminates five special cases
simultaneously. The sort-key's leading segments alone identify a row's model
type.


## Open questions

**Breaking change for DDB users with composite-key schemas — blocks
acceptance.** Multi-field primary keys currently accept two field-level
`#[key]` attributes; the macro assigns both fields `IndexScope::Partition`
and the DynamoDB driver papers over the mismatch with a positional fallback.
On DDB, partition-versus-range is a physical storage decision — the first
field becomes the hash key, the second the range key — and the field-level
shorthand does not carry that decision explicitly. This design removes the
fallback. DDB-targeted composite-key models must use struct-level
`#[key(partition = …, local = …)]`; the DynamoDB driver rejects the
field-level form at `push_schema`. SQL drivers are unaffected: their
composite-key representation is a tuple with no partition-versus-range
distinction, and both declaration forms produce equivalent schemas. Any
DDB user on the existing multi-field `#[key]` syntax will need to migrate
when they upgrade. Toasty is pre-1.0, so the breakage is on the table.

**Over-fetch**
Using `begins_with` on the sort key prefix will match all children under the current entity.
This can result in fetching items that are then thrown away. If the child id had a known length (UUIDs for instance), 
then `between` could be used to limit the query to the current item. Another possibility is to automatically 
load all children when a parent is loaded via `include`. This behavior would diverges from how `include` works 
in a non `item_collection` context though. 

**Cascade atomicity sibling design — not blocking.** How to model DynamoDB's transaction limit with `HasMany` cascading deletes?
The simplest is to disable cascading deletes for DynamoDB. This is a difference with how RDBMSes work, but 
a RDBMS has more powerful transaction capabilities than DynamoDB. 

## Out of scope

- **Item collections on SQL drivers.** SQL has joins and transactions; the
  single-table idiom does not benefit SQL users, and offering it would require
  parallel implementation of every piece of this design for no payoff.
- **Heterogeneous result APIs** — returning `Vec<Either<A, B>>` or
  `Vec<Box<dyn Entity>>` from one query. Not a Toasty idiom. The `.include()`
  collapse covers the use case with strongly-typed nodes.
- **Top-level getters on item-collection children.** Chain navigation with
  planner collapse covers the one-row case. A parallel `get_by_*` surface for
  item-collection children would be a second way to do it.
- **Skip-level `BelongsTo`** — `Subtask::tenant()` without going through
  `Subtask::todo().user().tenant()`. Chain navigation is the one form.
- **Migration tooling.** Converting a standalone DynamoDB schema to an
  item-collection schema requires rewriting every sort-key value. Users do
  that migration manually; Toasty provides no helpers.
- **User-visible composite sort-key accessor.** The sort key is engine-owned.
  Exposing it for logging or debugging is a future consideration; not needed
  for correctness in v1.
- **Multi-root collections in one table** — one DynamoDB table carrying two
  independent item-collection trees. Every model traces to exactly one root.
- **Cross-model GSI sharing** — one GSI indexing an attribute present on two
  models in the same collection. Each `#[index]` belongs to one model.
- **Non-UUID id performance characterization.** UUIDs are the documented happy
  path for v1. Other id types work but their cost profile is not part of this
  design.
