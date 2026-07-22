//! Procedural macros for the Toasty ORM.
//!
//! This crate provides `#[derive(Model)]`, `#[derive(Embed)]`, and related
//! attribute macros that generate query builders, schema registration, and
//! database mapping code.

#![warn(missing_docs)]

extern crate proc_macro;

mod create;
mod model;
mod query;
mod update;

use proc_macro::TokenStream;

/// Derive macro that turns a struct into a Toasty model backed by a database
/// table.
///
/// For a tutorial-style introduction, see the [Toasty guide].
///
#[doc = include_str!(concat!(env!("OUT_DIR"), "/guide_link.md"))]
///
/// # Overview
///
/// Applying `#[derive(Model)]` to a named struct generates:
///
/// - A [`Model`] trait implementation, including the associated `Query`,
///   `Create`, and `Update` builder types.
/// - A [`Load`] implementation for deserializing rows from the database.
/// - The [`Model`] trait's schema-registration methods (`id`, `schema`,
///   `register`) used to register the model at runtime.
/// - Static query and mutation methods such as `all()`, `filter(expr)`,
///   `filter_by_<field>()`, `get_by_<key>()`, and `upsert_by_<field>()`.
/// - Instance methods `update()` and `delete()`.
/// - A `Fields` struct returned by `<Model>::fields()` for building typed
///   filter expressions.
///
/// The struct must have named fields and no generic parameters.
///
/// [`Model`]: toasty::schema::Model
/// [`Load`]: toasty::schema::Load
///
/// # Struct-level attributes
///
/// ## `#[key(...)]` — primary key
///
/// Defines the primary key at the struct level. Mutually exclusive with
/// field-level `#[key]`.
///
/// Toasty generates an `upsert_by_*` method that takes every primary-key field.
///
/// **Simple form** — every listed field becomes a partition key:
///
/// ```
/// # use toasty::Model;
/// #[derive(Model)]
/// #[key(name)]
/// struct Widget {
///     name: String,
///     value: i64,
/// }
/// ```
///
/// **Composite key with partition/local scoping:**
///
/// ```
/// # use toasty::Model;
/// #[derive(Model)]
/// #[key(partition = user_id, local = id)]
/// struct Todo {
///     #[auto]
///     id: uuid::Uuid,
///     user_id: String,
///     title: String,
/// }
/// ```
///
/// The `partition` fields determine data distribution (relevant for
/// DynamoDB); `local` fields scope within a partition. For SQL databases
/// both behave as a regular composite primary key.
///
/// Multiple `partition` and `local` fields are allowed using bracket syntax:
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// #[key(partition = [tenant, org], local = [id])]
/// # struct Example { tenant: String, org: String, id: String }
/// ```
///
/// When using named `partition`/`local` syntax, at least one of each is
/// required. You cannot mix the simple and named forms.
///
/// ## `#[table = "name"]` — custom table name
///
/// Overrides the default table name. Without this attribute the table name
/// is the pluralized, snake_case form of the struct name (e.g. `User` →
/// `users`).
///
/// ```
/// # use toasty::Model;
/// #[derive(Model)]
/// #[table = "legacy_users"]
/// struct User {
///     #[key]
///     #[auto]
///     id: i64,
///     name: String,
/// }
/// ```
///
/// # Field-level attributes
///
/// ## `#[key]` — mark a field as a primary key column
///
/// Marks one or more fields as the primary key. When used on multiple
/// fields each becomes a partition key column (equivalent to listing them
/// in `#[key(...)]` at the struct level).
///
/// Toasty generates an `upsert_by_*` method that takes every primary-key field.
///
/// Cannot be combined with a struct-level `#[key(...)]` attribute.
///
/// ```
/// # use toasty::Model;
/// #[derive(Model)]
/// struct User {
///     #[key]
///     #[auto]
///     id: i64,
///     name: String,
/// }
/// ```
///
/// ## `#[auto]` — automatic value generation
///
/// Tells Toasty to generate this field's value automatically. The strategy
/// depends on the field type and optional arguments:
///
/// | Syntax | Behavior |
/// |--------|----------|
/// | `#[auto]` on `uuid::Uuid` | UUID v7 (timestamp-sortable) |
/// | `#[auto(uuid(v4))]` | UUID v4 (random) |
/// | `#[auto(uuid(v7))]` | UUID v7 (explicit) |
/// | `#[auto]` on integer types (`i8`–`i64`, `u8`–`u64`) | Auto-increment |
/// | `#[auto(increment)]` | Auto-increment (explicit) |
/// | `#[auto]` on a field named `created_at` | Expands to `#[default(jiff::Timestamp::now())]` |
/// | `#[auto]` on a field named `updated_at` | Expands to `#[update(jiff::Timestamp::now())]` |
///
/// The `created_at`/`updated_at` expansion requires the `jiff` feature and
/// a field type compatible with `jiff::Timestamp`.
///
/// Cannot be combined with `#[default]` or `#[update]` on the same field.
///
/// ## `#[default(expr)]` — default value on create
///
/// Sets a default value that is used when the field is not explicitly
/// provided during creation or on an upsert's create branch. The expression is
/// any valid Rust expression.
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[default(0)]
/// view_count: i64,
///
/// #[default("draft".to_string())]
/// status: String,
/// # }
/// ```
///
/// The default can be overridden by calling the corresponding setter on the
/// create builder.
///
/// Cannot be combined with `#[auto]` on the same field. Can be combined
/// with `#[update]` (the default applies on create; the update expression
/// applies on subsequent updates).
///
/// ## `#[update(expr)]` — value applied on create and update
///
/// Sets a value that Toasty applies every time a record is created or updated,
/// including both branches of an upsert, unless the field is explicitly set on
/// the builder.
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[update(jiff::Timestamp::now())]
/// updated_at: jiff::Timestamp,
/// # }
/// ```
///
/// Cannot be combined with `#[auto]` on the same field.
///
/// ## `#[index]` — add a database index
///
/// Creates a non-unique index on the field. Toasty generates a
/// `filter_by_<field>` method for indexed fields.
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[index]
/// email: String,
/// # }
/// ```
///
/// ## `#[unique]` — add a unique constraint
///
/// Creates a unique index on the field. Like `#[index]`, this generates
/// `filter_by_<field>`. It also generates `upsert_by_<field>`, which creates a
/// record or updates the record selected by this constraint. The database
/// enforces uniqueness.
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[unique]
/// email: String,
/// # }
/// ```
///
/// ## `#[column(...)]` — customize the database column
///
/// Overrides the column name and/or type for a field.
///
/// **Custom name:**
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[column("user_email")]
/// email: String,
/// # }
/// ```
///
/// **Custom type:**
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[column(type = varchar(255))]
/// email: String,
/// # }
/// ```
///
/// **Both:**
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[column("user_email", type = varchar(255))]
/// email: String,
/// # }
/// ```
///
/// ### Supported column types
///
/// | Syntax | Description |
/// |--------|-------------|
/// | `boolean` | Boolean |
/// | `i8`, `i16`, `i32`, `i64` | Signed integer (1/2/4/8 bytes) |
/// | `int(N)` | Signed integer with N-byte width |
/// | `u8`, `u16`, `u32`, `u64` | Unsigned integer (1/2/4/8 bytes) |
/// | `uint(N)` | Unsigned integer with N-byte width |
/// | `text` | Unbounded text |
/// | `varchar(N)` | Text with max length N |
/// | `numeric` | Arbitrary-precision numeric |
/// | `numeric(P, S)` | Numeric with precision P and scale S |
/// | `binary(N)` | Fixed-size binary with N bytes |
/// | `blob` | Variable-length binary |
/// | `timestamp(P)` | Timestamp with P fractional-second digits |
/// | `date` | Date without time |
/// | `time(P)` | Time with P fractional-second digits |
/// | `datetime(P)` | Date and time with P fractional-second digits |
/// | `"custom"` | Arbitrary type string passed through to the driver |
///
/// Cannot be used on relation fields.
///
/// ## JSON-encoded fields via [`Json<T>`](toasty::stmt::Json)
///
/// Wrap a serde-typed value in [`toasty::Json<T>`](toasty::stmt::Json) to
/// serialize it as JSON in the database. Every JSON field must select its
/// database column type with `#[column(type = ...)]`. Use `text` for
/// text-backed JSON. JSON fields require the `serde` feature and
/// `T: serde::Serialize + serde::Deserialize`.
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[column(type = text)]
/// tags: toasty::Json<Vec<String>>,
/// # }
/// ```
///
/// For nullable JSON columns, wrap `Json<T>` in `Option` — `None` maps to
/// SQL `NULL`:
///
/// ```
/// # use toasty::Model;
/// # use std::collections::HashMap;
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[column(type = text)]
/// metadata: Option<toasty::Json<HashMap<String, String>>>,
/// # }
/// ```
///
/// To instead store `None` as the JSON literal `"null"` (no SQL `NULL`),
/// wrap the other way: `Json<Option<T>>`.
///
/// # Relation attributes
///
/// Relation fields can be lazy or eager. Wrap the relation value in
/// `toasty::Deferred<_>` for lazy loading; ordinary queries leave the field
/// unloaded until the generated relation accessor or `.include(...)` loads it.
/// Use the relation value directly for eager loading; every query that returns
/// the model loads the relation as if the query included that field.
///
/// | Attribute | Lazy field type | Eager field type |
/// |-----------|-----------------|------------------|
/// | `#[belongs_to]` | `toasty::Deferred<T>` or `toasty::Deferred<Option<T>>` | `T` or `Option<T>` |
/// | `#[has_many]` | `toasty::Deferred<Vec<T>>` | `Vec<T>` |
/// | `#[has_one]` | `toasty::Deferred<T>` or `toasty::Deferred<Option<T>>` | `T` or `Option<T>` |
///
/// Toasty rejects schemas with eager-load cycles. If two relation paths point
/// back to each other, wrap at least one field in `toasty::Deferred<_>`.
///
/// ## `#[belongs_to(...)]` — foreign-key reference
///
/// Declares a many-to-one (or one-to-one) association through a foreign
/// key stored on this model.
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// # }
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     user_id: i64,
/// #[belongs_to(key = user_id, references = id)]
/// user: toasty::Deferred<User>,
/// # }
/// ```
///
/// To load the relation with every `Example` query, omit `Deferred`:
///
/// ```ignore
/// #[belongs_to(key = user_id, references = id)]
/// user: User,
/// ```
///
/// | Parameter | Meaning |
/// |-----------|---------|
/// | `key = <field>` | Local field holding the foreign key value |
/// | `references = <field>` | Field on the target model being referenced |
///
/// For composite foreign keys, pass arrays to `key` and `references`:
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # #[key(id, tenant_id)]
/// # struct Org {
/// #     id: i64,
/// #     tenant_id: i64,
/// # }
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     org_id: i64,
/// #     tenant_id: i64,
/// #[belongs_to(key = [org_id, tenant_id], references = [id, tenant_id])]
/// org: toasty::Deferred<Org>,
/// # }
/// ```
///
/// The number of fields in `key` must equal the number of fields in
/// `references`.
///
/// Wrap the target type in `Option` for an optional (nullable) foreign key:
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// # }
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[index]
/// manager_id: Option<i64>,
///
/// #[belongs_to(key = manager_id, references = id)]
/// manager: toasty::Deferred<Option<User>>,
/// # }
/// ```
///
/// ## `#[has_many]` — one-to-many association
///
/// Declares a collection of related models. The target model must have a
/// `#[belongs_to]` field pointing back to this model.
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Post {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[index]
/// #     example_id: i64,
/// #     #[belongs_to(key = example_id, references = id)]
/// #     example: toasty::Deferred<Example>,
/// # }
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[has_many]
/// posts: toasty::Deferred<Vec<Post>>,
/// # }
/// ```
///
/// To load the collection with every `Example` query, use `Vec<Post>`:
///
/// ```ignore
/// #[has_many]
/// posts: Vec<Post>,
/// ```
///
/// Toasty generates an accessor method (e.g. `.posts()`) and an insert
/// helper (e.g. `.insert_post()`), where the insert helper name is the
/// auto-singularized field name.
///
/// ### `pair` — disambiguate self-referential or multiple relations
///
/// When the target model has more than one `#[belongs_to]` pointing to
/// the same model (or points to itself), use `pair` to specify which
/// `belongs_to` field this `has_many` corresponds to:
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Person {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[index]
/// #     parent_id: Option<i64>,
/// #     #[belongs_to(key = parent_id, references = id)]
/// #     parent: toasty::Deferred<Option<Self>>,
/// #[has_many(pair = parent)]
/// children: toasty::Deferred<Vec<Person>>,
/// # }
/// ```
///
/// ### `via` — multi-step relations
///
/// Instead of pairing with a `belongs_to`, a `has_many` can reach its target
/// through a path of existing relations with `via`. The path is a dotted
/// chain of relation fields, read left to right starting from this model. A
/// `via` relation owns no foreign key — it is derived from the relations it
/// traverses — so it takes no `pair`:
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Comment {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[index]
/// #     user_id: i64,
/// #     #[belongs_to(key = user_id, references = id)]
/// #     user: toasty::Deferred<User>,
/// #     #[index]
/// #     article_id: i64,
/// #     #[belongs_to(key = article_id, references = id)]
/// #     article: toasty::Deferred<Article>,
/// # }
/// # #[derive(Model)]
/// # struct Article {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[has_many]
/// #     comments: toasty::Deferred<Vec<Comment>>,
/// # }
/// # #[derive(Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[has_many]
/// #     comments: toasty::Deferred<Vec<Comment>>,
/// // User → comments → article
/// #[has_many(via = comments.article)]
/// commented_articles: toasty::Deferred<Vec<Article>>,
/// # }
/// ```
///
/// The target type is `Article` because the path `comments.article` ends
/// there. A `via` relation is read-only and yields distinct targets — a target
/// reached through several intermediates appears once. Query, filter, and order
/// it like any other relation. Preloading it with `.include()` or projecting it
/// with `.select()` is supported on SQL backends; both are not yet available on
/// DynamoDB.
///
/// #### Many-to-many through a join model
///
/// Model a many-to-many relationship with a join model that belongs to both
/// endpoints. Each endpoint has a direct `has_many` relation to the join model
/// and a derived `has_many(via = ...)` relation to the opposite endpoint:
///
/// ```
/// # use toasty::Model;
/// #[derive(Debug, toasty::Model)]
/// struct User {
///     #[key]
///     #[auto]
///     id: i64,
///
///     #[has_many]
///     memberships: toasty::Deferred<Vec<Membership>>,
///
///     #[has_many(via = memberships.group)]
///     groups: toasty::Deferred<Vec<Group>>,
/// }
///
/// #[derive(Debug, toasty::Model)]
/// struct Group {
///     #[key]
///     #[auto]
///     id: i64,
///
///     #[has_many]
///     memberships: toasty::Deferred<Vec<Membership>>,
///
///     #[has_many(via = memberships.user)]
///     users: toasty::Deferred<Vec<User>>,
/// }
///
/// #[derive(Debug, toasty::Model)]
/// #[key(user_id, group_id)]
/// struct Membership {
///     #[index]
///     user_id: i64,
///
///     #[belongs_to(key = user_id, references = id)]
///     user: toasty::Deferred<User>,
///
///     #[index]
///     group_id: i64,
///
///     #[belongs_to(key = group_id, references = id)]
///     group: toasty::Deferred<Group>,
///
///     role: String,
/// }
/// ```
///
/// The composite key prevents duplicate user-group links. Fields such as
/// `role` belong on the join model because they describe one connection. The
/// derived `groups` and `users` relations return distinct endpoints and are
/// read-only; create, update, or delete `Membership` records to change links.
/// Call `.any()` on a derived field to filter by the opposite endpoint, or on
/// `memberships` to filter by join-model fields. Traversing, filtering,
/// preloading, or projecting the derived `via` fields requires a SQL backend.
///
/// ## `#[has_one]` — one-to-one association
///
/// Declares a single related model. The target model must have a
/// `#[belongs_to]` field pointing back to this model.
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Profile {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[index]
/// #     example_id: i64,
/// #     #[belongs_to(key = example_id, references = id)]
/// #     example: toasty::Deferred<Example>,
/// # }
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[has_one]
/// profile: toasty::Deferred<Profile>,
/// # }
/// ```
///
/// To load the relation with every `Example` query, omit `Deferred`:
///
/// ```ignore
/// #[has_one]
/// profile: Profile,
/// ```
///
/// Wrap in `Option` for an optional association:
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Profile {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[index]
/// #     example_id: i64,
/// #     #[belongs_to(key = example_id, references = id)]
/// #     example: toasty::Deferred<Example>,
/// # }
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[has_one]
/// profile: toasty::Deferred<Option<Profile>>,
/// # }
/// ```
///
/// The eager optional form is `Option<Profile>`.
///
/// ### `via` — multi-step relations
///
/// Like `#[has_many]`, a `#[has_one]` can reach its target through a path of
/// existing relations with `via` (see the `#[has_many]` `via` section above for
/// the full rules). Declare it when the path is expected to reach at most one
/// target:
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Subscription {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[unique]
/// #     account_id: Option<i64>,
/// #     #[belongs_to(key = account_id, references = id)]
/// #     account: toasty::Deferred<Option<Account>>,
/// # }
/// # #[derive(Model)]
/// # struct Account {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[unique]
/// #     user_id: Option<i64>,
/// #     #[belongs_to(key = user_id, references = id)]
/// #     user: toasty::Deferred<Option<User>>,
/// #     #[has_one]
/// #     subscription: toasty::Deferred<Option<Subscription>>,
/// # }
/// # #[derive(Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[has_one]
/// #     account: toasty::Deferred<Option<Account>>,
/// // User → account → subscription
/// #[has_one(via = account.subscription)]
/// subscription: toasty::Deferred<Option<Subscription>>,
/// # }
/// ```
///
/// # Constraints
///
/// - The struct must have named fields (tuple structs are not supported).
/// - Generic parameters are not supported.
/// - Every root model must have a primary key, defined either by a
///   struct-level `#[key(...)]` or by one or more field-level `#[key]`
///   attributes, but not both.
/// - `#[auto]` cannot be combined with `#[default]` or `#[update]` on the
///   same field.
/// - `#[column]`, `#[default]`, and `#[update]` cannot be used on relation
///   fields (`BelongsTo`, `HasMany`, `HasOne`).
/// - A field can have at most one relation attribute.
/// - Eager relation fields cannot form a cycle. Use `toasty::Deferred<_>` on at
///   least one edge of a bidirectional relation.
/// - `Self` can be used as a type in relation fields for self-referential
///   models.
///
/// # Full example
///
/// ```
/// #[derive(Debug, toasty::Model)]
/// struct User {
///     #[key]
///     #[auto]
///     id: i64,
///
///     #[unique]
///     email: String,
///
///     name: String,
///
///     #[default(jiff::Timestamp::now())]
///     created_at: jiff::Timestamp,
///
///     #[update(jiff::Timestamp::now())]
///     updated_at: jiff::Timestamp,
///
///     #[has_many]
///     posts: toasty::Deferred<Vec<Post>>,
/// }
///
/// #[derive(Debug, toasty::Model)]
/// struct Post {
///     #[key]
///     #[auto]
///     id: i64,
///
///     title: String,
///
///     #[column(type = text)]
///     tags: toasty::Json<Vec<String>>,
///
///     #[index]
///     user_id: i64,
///
///     #[belongs_to(key = user_id, references = id)]
///     user: toasty::Deferred<User>,
/// }
/// ```
#[proc_macro_derive(
    Model,
    attributes(
        key, auto, default, update, column, index, unique, table, has_many, has_one, belongs_to,
        version, shared, document
    )
)]
pub fn derive_model(input: TokenStream) -> TokenStream {
    match model::generate_model(input.into()) {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Derive macro that turns a struct or enum into an embedded type stored
/// inline in a parent model's table.
///
/// Embedded types do not have their own tables or primary keys. Their
/// fields are flattened into the parent model's columns. Use `Embed` for
/// value objects (addresses, coordinates, metadata) and enums
/// (status codes, contact info variants).
///
/// # Structs
///
/// An embedded struct's fields become columns in the parent table, prefixed
/// with the field name. For example, an `address: Address` field with
/// `street` and `city` produces columns `address_street` and
/// `address_city`.
///
/// ```
/// #[derive(toasty::Embed)]
/// struct Address {
///     street: String,
///     city: String,
/// }
///
/// #[derive(toasty::Model)]
/// struct User {
///     #[key]
///     #[auto]
///     id: i64,
///     name: String,
///     address: Address,
/// }
/// ```
///
/// Applying `#[derive(Embed)]` to a struct generates:
///
/// - An [`Embed`] trait implementation (`id` and `schema` methods).
/// - A `Fields` struct returned by `<Type>::fields()` for building
///   filter expressions on individual fields.
/// - An `Update` struct used by the parent model's update builder for
///   partial field updates.
///
/// ## Nesting
///
/// Embedded structs can contain other embedded types. Columns are
/// flattened with chained prefixes:
///
/// ```
/// #[derive(toasty::Embed)]
/// struct Location {
///     lat: i64,
///     lon: i64,
/// }
///
/// #[derive(toasty::Embed)]
/// struct Address {
///     street: String,
///     city: Location,
/// }
/// ```
///
/// When `Address` is embedded as `address` in a parent model, this
/// produces columns `address_street`, `address_city_lat`, and
/// `address_city_lon`.
///
/// # Enums
///
/// An embedded enum stores a discriminant value identifying the active
/// variant. By default, Toasty derives a string label for each variant by
/// converting its Rust name to `snake_case`. Use
/// `#[column(rename_all = "...")]` on the enum to select another naming
/// convention, or `#[column(variant = "...")]` on a variant to set one label.
///
/// **Unit-only enum:**
///
/// ```
/// #[derive(toasty::Embed)]
/// enum Status {
///     Pending,
///     InProgress,
///     Archived,
/// }
/// ```
///
/// A unit-only enum occupies a single column in the parent table. The
/// example stores the labels `pending`, `in_progress`, and `archived`.
///
/// **Data-carrying enum:**
///
/// ```
/// #[derive(toasty::Embed)]
/// enum ContactInfo {
///     Email { address: String },
///     Phone { number: String },
/// }
/// ```
///
/// A data-carrying enum stores the discriminant column plus one nullable
/// column per variant field. For example, a `contact: ContactInfo` field
/// produces columns `contact` (discriminant), `contact_address`, and
/// `contact_number`. Only the columns belonging to the active variant
/// contain values; the rest are `NULL`.
///
/// **Mixed enum** (unit and data variants together):
///
/// ```
/// #[derive(toasty::Embed)]
/// enum Status {
///     Pending,
///     Failed { reason: String },
///     Done,
/// }
/// ```
///
/// Applying `#[derive(Embed)]` to an enum generates:
///
/// - An [`Embed`] trait implementation (`id` and `schema` methods).
/// - A `Fields` struct with `is_<variant>()` methods and comparison
///   methods (`eq`, `ne`, `in_list`).
/// - For data-carrying variants, per-variant handle types with a
///   `matches(closure)` method for pattern matching and field access.
///
/// # Newtype `Auto` proxying
///
/// A tuple-newtype embedded struct (one unnamed field) automatically
/// implements `Auto` whenever its inner type does — no annotation
/// required. Toasty emits a `NewtypeOf` marker carrying the inner type
/// and a blanket `Auto` impl resolves through it:
///
/// ```
/// #[derive(toasty::Embed)]
/// struct UserId(uuid::Uuid);
///
/// #[derive(toasty::Model)]
/// struct User {
///     #[key]
///     #[auto]
///     id: UserId,
///     name: String,
/// }
/// ```
///
/// Newtypes wrapping non-`Auto` types stay non-`Auto`; nesting works
/// transparently (`Outer(Inner(u64))` proxies through both layers).
///
/// # Attributes
///
/// ## `#[column(...)]` — customize the database column
///
/// **On struct fields**, overrides the column name and/or type:
///
/// ```
/// #[derive(toasty::Embed)]
/// struct Address {
///     #[column("addr_street")]
///     street: String,
///
///     #[column(type = varchar(255))]
///     city: String,
/// }
/// ```
///
/// See [`Model`][`derive@Model`] for the full list of supported column
/// types.
///
/// **Changing stored enum discriminants.** On an enum,
/// `#[column(rename_all = "...")]` changes how Toasty derives string labels
/// for variants without an explicit label:
///
/// ```
/// #[derive(toasty::Embed)]
/// #[column(rename_all = "SCREAMING_SNAKE_CASE")]
/// enum PartyKind {
///     Customer,
///     PreferredSupplier,
/// }
/// ```
///
/// This example uses the labels `CUSTOMER` and `PREFERRED_SUPPLIER`. Without
/// `rename_all`, Toasty uses `snake_case`.
///
/// The supported rules and their result for `PreferredSupplier` are:
///
/// | Rule | Label |
/// | --- | --- |
/// | `lowercase` | `preferredsupplier` |
/// | `UPPERCASE` | `PREFERREDSUPPLIER` |
/// | `PascalCase` | `PreferredSupplier` |
/// | `camelCase` | `preferredSupplier` |
/// | `snake_case` | `preferred_supplier` |
/// | `SCREAMING_SNAKE_CASE` | `PREFERRED_SUPPLIER` |
/// | `kebab-case` | `preferred-supplier` |
/// | `SCREAMING-KEBAB-CASE` | `PREFERRED-SUPPLIER` |
///
/// Use `#[column(variant = "...")]` to set individual labels:
///
/// ```
/// #[derive(toasty::Embed)]
/// enum PartyKind {
///     #[column(variant = "customer")]
///     Customer,
///     #[column(variant = "preferred-supplier")]
///     PreferredSupplier,
/// }
/// ```
///
/// An explicit variant label takes precedence over `rename_all` when an enum
/// uses both attributes.
///
/// String-label enums use Toasty's enum storage by default. Use
/// `#[column(type = enum("type_name"))]` to set the database enum type name,
/// or `#[column(type = text)]` or `#[column(type = varchar(N))]` to use a
/// plain string column. `rename_all` changes variant labels only; it does not
/// change the enum type name.
///
/// To store integers instead, assign an integer to every variant:
///
/// ```
/// #[derive(toasty::Embed)]
/// enum Priority {
///     #[column(variant = 10)]
///     Low,
///     #[column(variant = 20)]
///     High,
/// }
/// ```
///
/// An enum cannot mix string and integer discriminants. Integer discriminants
/// use `i64` storage by default. Add an integer enum-level override such as
/// `#[column(type = u8)]` to request narrower storage. The type applies to
/// flattened discriminant columns, through transparent field wrappers, and to
/// each element of `Vec<unit-enum>`. The same attribute on a model field
/// overrides the enum default for that use; on a collection it selects the
/// element type. Every discriminant must fit the selected type. Enum embeds
/// inside `#[document]` fields are not supported. Integer-discriminant enums do
/// not support `rename_all`. All discriminant values must be unique. String
/// labels may contain at most 63 bytes.
///
/// ## `#[index]` — add a database index
///
/// Creates a non-unique index on the field's flattened column.
///
/// ```
/// #[derive(toasty::Embed)]
/// struct Contact {
///     #[index]
///     country: String,
/// }
/// ```
///
/// ## `#[unique]` — add a unique constraint
///
/// Creates a unique index on the field's flattened column. The database
/// enforces uniqueness.
///
/// ```
/// #[derive(toasty::Embed)]
/// struct Contact {
///     #[unique]
///     email: String,
/// }
/// ```
///
/// ## `#[shared(ident)]` — share a column across enum variants
///
/// Declares a shared logical field on the enum. Variant fields declaring
/// the same identifier are backed by a single nullable column instead of
/// one column per variant. The identifier — not the Rust field names,
/// which may differ per variant — names the field: the column name derives
/// from it (`{enum_field}_{ident}`), and enum-level `#[index]` /
/// `#[unique]` attributes reference it.
///
/// ```
/// #[derive(toasty::Embed)]
/// enum Creature {
///     #[column(variant = 1)]
///     Human {
///         #[shared(name)]
///         full_name: String,
///         profession: String,
///     },
///     #[column(variant = 2)]
///     Animal {
///         #[shared(name)]
///         nickname: String,
///         species: String,
///     },
/// }
/// // Columns: creature, creature_name (shared), creature_profession,
/// // creature_species
/// ```
///
/// Fields sharing an identifier must have the same type. To rename the
/// shared column, add `#[column("...")]` to any one member of the group
/// (if several declare it, they must agree):
///
/// ```
/// # #[derive(toasty::Embed)]
/// # enum Example {
/// # #[column(variant = 1)]
/// # V {
/// #[shared(name)]
/// #[column("legacy_name")]
/// name: String,
/// # },
/// # }
/// ```
///
/// ## Enum-level `#[index(...)]` / `#[unique(...)]`
///
/// On the enum itself, `#[index(...)]` and `#[unique(...)]` create an
/// index over variant-field columns. Each reference is a shared field
/// identifier or a `variant::field` path naming a variant field that owns
/// its column; the two forms compose into composite indices.
///
/// ```
/// #[derive(toasty::Embed)]
/// #[unique(name)]
/// #[index(name, human::profession)]
/// enum Creature {
///     #[column(variant = 1)]
///     Human {
///         #[shared(name)]
///         name: String,
///         profession: String,
///     },
///     #[column(variant = 2)]
///     Animal {
///         #[shared(name)]
///         name: String,
///     },
/// }
/// ```
///
/// An index on a shared column covers rows of **every** variant: with
/// `#[unique(name)]` above, a `Human` named "Bob" and an `Animal` named
/// "Bob" conflict. Rows of variants that do not declare the shared field
/// store `NULL` and never conflict. For this reason, field-level
/// `#[index]` / `#[unique]` on a `#[shared]` field is a compile error
/// pointing at the enum-level form.
///
/// # Using embedded types in a model
///
/// Reference an embedded type as a field on a [`Model`][`derive@Model`]
/// struct. The parent model's create and update builders gain a setter for
/// the embedded field. Partial updates of individual sub-fields use
/// `stmt::patch`:
///
/// ```no_run
/// # #[derive(toasty::Embed)]
/// # struct Address { street: String, city: String }
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// #     address: Address,
/// # }
/// # async fn example(mut db: toasty::Db, mut user: User) -> toasty::Result<()> {
/// use toasty::stmt;
///
/// // Full replacement
/// user.update()
///     .address(Address { street: "456 Oak Ave".into(), city: "Seattle".into() })
///     .exec(&mut db).await?;
///
/// // Partial update — updates city, leaves street unchanged
/// user.update()
///     .address(stmt::patch(Address::fields().city(), "Portland"))
///     .exec(&mut db).await?;
/// # Ok(())
/// # }
/// ```
///
/// Embedded struct fields are queryable through the parent model's
/// `fields()` accessor:
///
/// ```no_run
/// # #[derive(toasty::Embed)]
/// # struct Address { street: String, city: String }
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// #     address: Address,
/// # }
/// # async fn example(mut db: toasty::Db) -> toasty::Result<()> {
/// let users = User::filter(User::fields().address().city().eq("Seattle"))
///     .exec(&mut db).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Constraints
///
/// - Embedded structs must have named fields (tuple structs are not
///   supported).
/// - Generic parameters are not supported.
/// - Enum discriminants must all be strings or all be integers. Integer
///   discriminants must be specified on every variant.
/// - `#[column(rename_all = "...")]` applies only to string labels.
/// - Enum variants may be unit variants or have named fields. Tuple
///   variants are not supported.
/// - Embedded types cannot have primary keys, relations, `#[auto]`,
///   `#[default]`, or `#[update]` attributes.
///
/// # Full example
///
/// ```no_run
/// # async fn example(mut db: toasty::Db) -> toasty::Result<()> {
/// #[derive(Debug, PartialEq, toasty::Embed)]
/// #[column(rename_all = "SCREAMING_SNAKE_CASE")]
/// enum Priority {
///     Low,
///     Normal,
///     High,
/// }
///
/// #[derive(Debug, toasty::Embed)]
/// struct Metadata {
///     version: i64,
///     status: String,
///     priority: Priority,
/// }
///
/// #[derive(Debug, toasty::Model)]
/// struct Document {
///     #[key]
///     #[auto]
///     id: i64,
///
///     title: String,
///
///     #[unique]
///     slug: String,
///
///     meta: Metadata,
/// }
///
/// // Create
/// let mut doc = Document::create()
///     .title("Design doc")
///     .slug("design-doc")
///     .meta(Metadata {
///         version: 1,
///         status: "draft".to_string(),
///         priority: Priority::Normal,
///     })
///     .exec(&mut db).await?;
///
/// // Query by embedded field
/// let drafts = Document::filter(
///     Document::fields().meta().status().eq("draft")
/// ).exec(&mut db).await?;
///
/// // Partial update
/// use toasty::stmt;
/// doc.update()
///     .meta(stmt::apply([
///         stmt::patch(Metadata::fields().version(), 2),
///         stmt::patch(Metadata::fields().status(), "published"),
///     ]))
///     .exec(&mut db).await?;
/// # Ok(())
/// # }
/// ```
///
/// [`Embed`]: toasty::Embed
#[proc_macro_derive(Embed, attributes(column, document, index, unique, shared))]
pub fn derive_embed(input: TokenStream) -> TokenStream {
    match model::generate_embed(input.into()) {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Builds a query using the Toasty query language. The macro expands into
/// the equivalent method-chain calls on the query builder API. It does
/// not execute the query — chain `.exec(&mut db).await?` on the result to run
/// it.
///
/// # Syntax
///
/// ```text
/// query!(Source [FILTER expr] [ORDER BY .field ASC|DESC] [OFFSET n] [LIMIT n])
/// ```
///
/// `Source` is a model type path (e.g., `User`). All clauses are optional and
/// can appear in any combination, but must follow the order shown above when
/// present. All keywords are case-insensitive: `FILTER`, `filter`, and `Filter`
/// all work.
///
/// # Basic queries
///
/// With no clauses, `query!` returns all records of the given model.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// #     age: i64,
/// #     active: bool,
/// # }
/// // Returns all users — expands to User::all()
/// let _ = toasty::query!(User);
/// ```
///
/// # Filter expressions
///
/// The `FILTER` clause accepts an expression built from field comparisons,
/// boolean operators, and external references.
///
/// ## Comparison operators
///
/// Dot-prefixed field paths (`.name`, `.age`) refer to fields on the source
/// model. The right-hand side is a literal or external reference.
///
/// | Operator | Expansion         |
/// |----------|-------------------|
/// | `==`     | `.eq(val)`        |
/// | `!=`     | `.ne(val)`        |
/// | `>`      | `.gt(val)`        |
/// | `>=`     | `.ge(val)`        |
/// | `<`      | `.lt(val)`        |
/// | `<=`     | `.le(val)`        |
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// #     age: i64,
/// #     active: bool,
/// # }
/// // Equality — expands to User::filter(User::fields().name().eq("Alice"))
/// let _ = toasty::query!(User FILTER .name == "Alice");
///
/// // Not equal
/// let _ = toasty::query!(User FILTER .name != "Bob");
///
/// // Greater than
/// let _ = toasty::query!(User FILTER .age > 18);
///
/// // Greater than or equal
/// let _ = toasty::query!(User FILTER .age >= 21);
///
/// // Less than
/// let _ = toasty::query!(User FILTER .age < 65);
///
/// // Less than or equal
/// let _ = toasty::query!(User FILTER .age <= 99);
/// ```
///
/// ## Boolean operators
///
/// `AND`, `OR`, and `NOT` combine filter expressions. Precedence follows
/// standard boolean logic: `NOT` binds tightest, then `AND`, then `OR`.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// #     age: i64,
/// #     active: bool,
/// # }
/// // AND — both conditions must match
/// let _ = toasty::query!(User FILTER .name == "Alice" AND .age > 18);
///
/// // OR — either condition matches
/// let _ = toasty::query!(User FILTER .name == "Alice" OR .name == "Bob");
///
/// // NOT — negates the following expression
/// let _ = toasty::query!(User FILTER NOT .active == true);
///
/// // Combining all three
/// let _ = toasty::query!(User FILTER NOT .active == true AND (.name == "Alice" OR .age >= 21));
/// ```
///
/// ## Operator precedence
///
/// Without parentheses, `NOT` binds tightest, then `AND`, then `OR`. Use
/// parentheses to override.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// #     age: i64,
/// #     active: bool,
/// # }
/// // Without parens: parsed as (.name == "A" AND .age > 0) OR .active == false
/// let _ = toasty::query!(User FILTER .name == "A" AND .age > 0 OR .active == false);
///
/// // With parens: forces OR to bind first
/// let _ = toasty::query!(User FILTER .name == "A" AND (.age > 0 OR .active == false));
/// ```
///
/// ## Boolean and integer literals
///
/// Boolean fields can be compared against `true` and `false` literals.
/// Integer literals work as expected.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// #     age: i64,
/// #     active: bool,
/// # }
/// let _ = toasty::query!(User FILTER .active == true);
/// let _ = toasty::query!(User FILTER .active == false);
/// let _ = toasty::query!(User FILTER .age == 42);
/// ```
///
/// # Referencing surrounding code
///
/// `#ident` pulls a variable from the surrounding scope. `#(expr)` embeds an
/// arbitrary Rust expression.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// #     age: i64,
/// #     active: bool,
/// # }
/// // Variable reference — expands to User::filter(User::fields().name().eq(name))
/// let name = "Carl";
/// let _ = toasty::query!(User FILTER .name == #name);
///
/// // Expression reference
/// fn min_age() -> i64 { 18 }
/// let _ = toasty::query!(User FILTER .age > #(min_age()));
/// ```
///
/// # Dot-prefixed field paths
///
/// A leading `.` starts a field path rooted at the source model's `fields()`
/// method. Chained dots navigate multi-segment paths.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// #     age: i64,
/// #     active: bool,
/// # }
/// // .name expands to User::fields().name()
/// let _ = toasty::query!(User FILTER .name == "Alice");
///
/// // Multiple fields in a single expression
/// let _ = toasty::query!(User FILTER .id == 1 AND .name == "X" AND .age > 0);
/// ```
///
/// # ORDER BY
///
/// Sort results by a field in ascending (`ASC`) or descending (`DESC`) order.
/// If no direction is specified, ascending is the default.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// #     age: i64,
/// #     active: bool,
/// # }
/// // Ascending order (explicit)
/// let _ = toasty::query!(User ORDER BY .name ASC);
///
/// // Descending order
/// let _ = toasty::query!(User ORDER BY .age DESC);
///
/// // Combined with filter
/// let _ = toasty::query!(User FILTER .active == true ORDER BY .name ASC);
/// ```
///
/// # LIMIT and OFFSET
///
/// `LIMIT` restricts the number of returned records. `OFFSET` skips a number
/// of records before returning. Both accept integer literals, `#ident`
/// variables, and `#(expr)` expressions.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// #     age: i64,
/// #     active: bool,
/// # }
/// // Return at most 10 records
/// let _ = toasty::query!(User LIMIT 10);
///
/// // Skip 20, then return 10
/// let _ = toasty::query!(User OFFSET 20 LIMIT 10);
///
/// // Variable pagination
/// let page_size = 25usize;
/// let _ = toasty::query!(User LIMIT #page_size);
///
/// // Expression pagination
/// let _ = toasty::query!(User LIMIT #(5 + 5));
/// ```
///
/// # Combining clauses
///
/// All clauses can be combined. When present, they must appear in this order:
/// `FILTER`, `ORDER BY`, `OFFSET`, `LIMIT`.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// #     age: i64,
/// #     active: bool,
/// # }
/// let _ = toasty::query!(User FILTER .active == true ORDER BY .name ASC LIMIT 10);
/// let _ = toasty::query!(User FILTER .age > 18 ORDER BY .age DESC OFFSET 0 LIMIT 50);
/// ```
///
/// # Case-insensitive keywords
///
/// All keywords — `FILTER`, `AND`, `OR`, `NOT`, `ORDER`, `BY`, `ASC`, `DESC`,
/// `OFFSET`, `LIMIT` — are matched case-insensitively. Any casing works.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// #     age: i64,
/// #     active: bool,
/// # }
/// // These are all equivalent
/// let _ = toasty::query!(User FILTER .name == "A");
/// let _ = toasty::query!(User filter .name == "A");
/// let _ = toasty::query!(User Filter .name == "A");
/// ```
///
/// # Expansion details
///
/// The macro translates each syntactic element into method-chain calls on the
/// query builder.
///
/// ## No filter
///
/// ```text
/// query!(User)          →  User::all()
/// ```
///
/// ## Filter
///
/// ```text
/// query!(User FILTER .name == "A")
///     →  User::filter(User::fields().name().eq("A"))
/// ```
///
/// ## Logical operators
///
/// ```text
/// query!(User FILTER .a == 1 AND .b == 2)
///     →  User::filter(User::fields().a().eq(1).and(User::fields().b().eq(2)))
///
/// query!(User FILTER .a == 1 OR .b == 2)
///     →  User::filter(User::fields().a().eq(1).or(User::fields().b().eq(2)))
///
/// query!(User FILTER NOT .a == 1)
///     →  User::filter((User::fields().a().eq(1)).not())
/// ```
///
/// ## ORDER BY
///
/// ```text
/// query!(User ORDER BY .name ASC)
///     →  { let mut q = User::all(); q = q.order_by(User::fields().name().asc()); q }
/// ```
///
/// ## LIMIT / OFFSET
///
/// ```text
/// query!(User LIMIT 10)
///     →  { let mut q = User::all(); q = q.limit(10); q }
///
/// query!(User OFFSET 5 LIMIT 10)
///     →  { let mut q = User::all(); q = q.limit(10); q = q.offset(5); q }
/// ```
///
/// Note: in the expansion, `limit` is called before `offset` because the
/// API requires it.
///
/// ## External references
///
/// ```text
/// let x = "Carl";
/// query!(User FILTER .name == #x)
///     →  User::filter(User::fields().name().eq(x))
///
/// query!(User FILTER .age > #(compute()))
///     →  User::filter(User::fields().age().gt(compute()))
/// ```
///
/// # Errors
///
/// The macro produces compile-time errors for:
///
/// - **Missing model path**: the first token must be a valid type path.
/// - **Unknown fields**: dot-prefixed paths that don't match a field on the
///   model produce a type error from the generated `fields()` method.
/// - **Type mismatches**: comparing a field to a value of the wrong type
///   produces a standard Rust type error (e.g., `.age == "not a number"`).
/// - **Unexpected tokens**: tokens after the last recognized clause cause
///   `"unexpected tokens after query"`.
/// - **Invalid clause order**: placing `FILTER` after `ORDER BY` or `LIMIT`
///   before `OFFSET` causes a parse error since the clauses are parsed in
///   fixed order.
/// - **Missing `BY` after `ORDER`**: writing `ORDER .name` instead of
///   `ORDER BY .name` produces `"expected 'BY' after 'ORDER'"`.
/// - **Invalid pagination value**: `LIMIT` and `OFFSET` require an integer
///   literal, `#variable`, or `#(expression)`.
#[proc_macro]
pub fn query(input: TokenStream) -> TokenStream {
    match query::generate(input.into()) {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Expands struct-literal syntax into create builder method chains. Returns one
/// or more create builders — call `.exec(&mut db).await?` to insert the
/// record(s).
///
/// # Syntax forms
///
/// ## Field syntax
///
/// Fields inside `{ ... }` can use either explicit or shorthand syntax:
///
/// - **Explicit:** `field: expr` — sets the field to the given expression.
/// - **Shorthand:** `field` — equivalent to `field: field`, using a variable
///   with the same name as the field.
///
/// These can be mixed freely, just like Rust struct literals:
///
/// ```ignore
/// let name = "Alice".to_string();
/// toasty::create!(User { name, email: "alice@example.com" })
/// ```
///
/// ## Single creation
///
/// ```ignore
/// toasty::create!(Type { field: value, ... })
/// ```
///
/// Expands to `Type::create().field(value)...` and returns the model's create
/// builder (e.g., `UserCreate`).
///
/// ```no_run
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// #     email: String,
/// # }
/// # async fn example(mut db: toasty::Db) -> toasty::Result<()> {
/// let user = toasty::create!(User {
///     name: "Alice",
///     email: "alice@example.com"
/// })
/// .exec(&mut db)
/// .await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Scoped creation
///
/// ```ignore
/// toasty::create!(in expr { field: value, ... })
/// ```
///
/// Expands to `expr.create().field(value)...`. Creates a record through a
/// relation accessor. The foreign key is set automatically.
///
/// ```no_run
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// #     #[has_many]
/// #     todos: toasty::Deferred<Vec<Todo>>,
/// # }
/// # #[derive(toasty::Model)]
/// # struct Todo {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     title: String,
/// #     #[index]
/// #     user_id: i64,
/// #     #[belongs_to(key = user_id, references = id)]
/// #     user: toasty::Deferred<User>,
/// # }
/// # async fn example(mut db: toasty::Db, user: User) -> toasty::Result<()> {
/// let todo = toasty::create!(in user.todos() { title: "buy milk" })
///     .exec(&mut db)
///     .await?;
///
/// // todo.user_id == user.id
/// # Ok(())
/// # }
/// ```
///
/// ## Typed batch
///
/// ```ignore
/// toasty::create!(Type::[ { fields }, { fields }, ... ])
/// ```
///
/// Expands to `toasty::batch([builder1, builder2, ...])` and returns
/// `Vec<Type>` when executed:
///
/// ```no_run
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// # }
/// # async fn example(mut db: toasty::Db) -> toasty::Result<()> {
/// let users = toasty::create!(User::[
///     { name: "Alice" },
///     { name: "Bob" },
/// ])
/// .exec(&mut db)
/// .await?;
/// // users: Vec<User>
/// # Ok(())
/// # }
/// ```
///
/// ## Tuple
///
/// ```ignore
/// toasty::create!((
///     Type1 { fields },
///     Type2 { fields },
///     ...
/// ))
/// ```
///
/// Expands to `toasty::batch((builder1, builder2, ...))` and returns a
/// tuple matching the input types:
///
/// ```no_run
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// # }
/// # #[derive(toasty::Model)]
/// # struct Post {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     title: String,
/// # }
/// # async fn example(mut db: toasty::Db) -> toasty::Result<()> {
/// let (user, post) = toasty::create!((
///     User { name: "Alice" },
///     Post { title: "Hello" },
/// ))
/// .exec(&mut db)
/// .await?;
/// // (User, Post)
/// # Ok(())
/// # }
/// ```
///
/// ## Mixed tuple
///
/// Typed batches and single creates can be mixed inside a tuple:
///
/// ```no_run
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// # }
/// # #[derive(toasty::Model)]
/// # struct Post {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     title: String,
/// # }
/// # async fn example(mut db: toasty::Db) -> toasty::Result<()> {
/// let (users, post) = toasty::create!((
///     User::[ { name: "Alice" }, { name: "Bob" } ],
///     Post { title: "Hello" },
/// ))
/// .exec(&mut db)
/// .await?;
/// // (Vec<User>, Post)
/// # Ok(())
/// # }
/// ```
///
/// # Field values
///
/// ## Expressions
///
/// Any Rust expression is valid as a field value — literals, variables, and
/// function calls all work. When a variable has the same name as the field,
/// you can use the shorthand syntax (just `name` instead of `name: name`):
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// #     email: String,
/// # }
/// let name = "Alice";
/// let _ = toasty::create!(User { name, email: format!("{}@example.com", name) });
/// ```
///
/// When the variable name differs from the field name, use the explicit
/// `field: expr` form:
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// # }
/// let user_name = "Alice";
/// let _ = toasty::create!(User { name: user_name });
/// ```
///
/// ## Nested struct (BelongsTo / HasOne)
///
/// Use `{ ... }` **without** a type prefix to create a related record inline.
/// The macro expands the nested fields into a create builder and passes it
/// to the field's setter method.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// # }
/// # #[derive(toasty::Model)]
/// # struct Todo {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     title: String,
/// #     #[index]
/// #     user_id: i64,
/// #     #[belongs_to(key = user_id, references = id)]
/// #     user: toasty::Deferred<User>,
/// # }
/// let _ = toasty::create!(Todo {
///     title: "buy milk",
///     user: { name: "Alice" }
/// });
/// // Expands to:
/// // Todo::create()
/// //     .title("buy milk")
/// //     .user(Todo::fields().user().create().name("Alice"))
/// ```
///
/// The related record is created first and the foreign key is set
/// automatically.
///
/// ## Nested list (HasMany)
///
/// Use `[{ ... }, { ... }]` to create multiple related records. The macro
/// expands each entry into a create builder and passes them as an array to
/// the plural field setter.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// #     #[has_many]
/// #     todos: toasty::Deferred<Vec<Todo>>,
/// # }
/// # #[derive(toasty::Model)]
/// # struct Todo {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     title: String,
/// #     #[index]
/// #     user_id: i64,
/// #     #[belongs_to(key = user_id, references = id)]
/// #     user: toasty::Deferred<User>,
/// # }
/// let _ = toasty::create!(User {
///     name: "Alice",
///     todos: [{ title: "first" }, { title: "second" }]
/// });
/// // Expands to:
/// // User::create()
/// //     .name("Alice")
/// //     .todos([
/// //         User::fields().todos().create().title("first"),
/// //         User::fields().todos().create().title("second"),
/// //     ])
/// ```
///
/// Items in a nested list can also be plain expressions (e.g., an existing
/// builder value).
///
/// ## Deep nesting
///
/// Nesting composes to arbitrary depth:
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// #     #[has_many]
/// #     todos: toasty::Deferred<Vec<Todo>>,
/// # }
/// # #[derive(toasty::Model)]
/// # struct Todo {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     title: String,
/// #     #[index]
/// #     user_id: i64,
/// #     #[belongs_to(key = user_id, references = id)]
/// #     user: toasty::Deferred<User>,
/// #     #[has_many]
/// #     tags: toasty::Deferred<Vec<Tag>>,
/// # }
/// # #[derive(toasty::Model)]
/// # struct Tag {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// #     #[index]
/// #     todo_id: i64,
/// #     #[belongs_to(key = todo_id, references = id)]
/// #     todo: toasty::Deferred<Todo>,
/// # }
/// let _ = toasty::create!(User {
///     name: "Alice",
///     todos: [{
///         title: "task",
///         tags: [{ name: "urgent" }, { name: "work" }]
///     }]
/// });
/// ```
///
/// This creates a `User`, then a `Todo` linked to that user, then two `Tag`
/// records linked to that todo.
///
/// # Fields that can be omitted
///
/// | Field type | Behavior when omitted |
/// |---|---|
/// | `#[auto]` | Value generated by the database or Toasty |
/// | `Option<T>` | Defaults to `None` (`NULL`) |
/// | `#[default(expr)]` | Uses the default expression |
/// | `#[update(expr)]` | Uses the expression as the initial value |
/// | `#[has_many] Deferred<Vec<T>>` or `#[has_many] Vec<T>` | No related records created |
/// | `#[has_one] Deferred<Option<T>>` or `#[has_one] Option<T>` | No related record created |
/// | `#[belongs_to] Deferred<Option<T>>` or `#[belongs_to] Option<T>` | Foreign key set to `NULL` |
///
/// Required fields (`String`, `i64`, non-optional `BelongsTo`, etc.) that are
/// missing do not cause a compile-time error. The insert fails at runtime with
/// a database constraint violation.
///
/// # Compile errors
///
/// **Type prefix on nested struct:**
///
/// ```compile_fail
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// # }
/// # #[derive(toasty::Model)]
/// # struct Todo {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[index]
/// #     user_id: i64,
/// #     #[belongs_to(key = user_id, references = id)]
/// #     user: toasty::Deferred<User>,
/// # }
/// // Error: remove the type prefix `User` — use `{ ... }` without a type name
/// toasty::create!(Todo { user: User { name: "Alice" } })
/// ```
///
/// Correct:
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// # }
/// # #[derive(toasty::Model)]
/// # struct Todo {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[index]
/// #     user_id: i64,
/// #     #[belongs_to(key = user_id, references = id)]
/// #     user: toasty::Deferred<User>,
/// # }
/// let _ = toasty::create!(Todo { user: { name: "Alice" } });
/// ```
///
/// Nested struct values infer their type from the field.
///
/// **Nested lists:**
///
/// ```compile_fail
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     field: String,
/// # }
/// // Error: nested lists are not supported in create!
/// toasty::create!(User { field: [[{ }]] })
/// ```
///
/// **Missing braces or batch bracket:**
///
/// ```compile_fail
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// # }
/// // Error: expected `{` for single creation or `::[` for batch creation after type path
/// toasty::create!(User)
/// ```
///
/// # Return type
///
/// | Form | Returns |
/// |---|---|
/// | `Type { ... }` | `TypeCreate` (single builder) |
/// | `in expr { ... }` | Builder for the relation's model |
/// | `Type::[ ... ]` | `Batch` — executes to `Vec<Type>` |
/// | `( ... )` | `Batch` — executes to tuple of results |
///
/// Single and scoped forms return a builder — call `.exec(&mut db).await?`.
/// Batch and tuple forms return a `Batch` — also call `.exec(&mut db).await?`.
#[proc_macro]
pub fn create(input: TokenStream) -> TokenStream {
    match create::generate(input.into()) {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Expands struct-literal syntax into update-builder method chains. Returns
/// the same builder `target.update()` would return — call
/// `.exec(&mut db).await?` to execute the update.
///
/// # Syntax
///
/// ```ignore
/// toasty::update!(target { field: value, ... })
/// ```
///
/// `target` is any expression that has an `.update()` method — a model
/// instance, a query builder, or a scoped relation accessor.
///
/// ```no_run
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// # }
/// # async fn example(mut db: toasty::Db, mut user: User, id: i64) -> toasty::Result<()> {
/// // Instance target
/// toasty::update!(user { name: "Alice Smith" })
///     .exec(&mut db).await?;
///
/// // Query target
/// toasty::update!(User::filter_by_id(id) { name: "Bob" })
///     .exec(&mut db).await?;
/// # Ok(())
/// # }
/// ```
///
/// Instance targets do not consume the binding — the macro expands to
/// `user.update()`, which auto-borrows `&mut user` the same way the
/// chain form does. `user` stays owned after the macro returns.
///
/// Value expressions are evaluated before the target is borrowed, so
/// they may read the target's own fields:
///
/// ```no_run
/// # #[derive(toasty::Model)]
/// # struct Todo {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     done: bool,
/// # }
/// # async fn example(mut db: toasty::Db, mut todo: Todo) -> toasty::Result<()> {
/// toasty::update!(todo { done: !todo.done }).exec(&mut db).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Field shapes
///
/// ## Explicit
///
/// `field: expr` sets the field to `expr`:
///
/// ```no_run
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// #     email: String,
/// # }
/// # async fn example(mut db: toasty::Db, mut user: User) -> toasty::Result<()> {
/// toasty::update!(user {
///     name: "Alice Smith",
///     email: "alice.smith@example.com",
/// }).exec(&mut db).await?;
/// # Ok(())
/// # }
/// ```
///
/// `expr` is any Rust expression. For collection fields, pass a
/// `toasty::stmt::*` combinator (e.g. `stmt::push("x")`,
/// `stmt::apply([...])`) for non-set semantics.
///
/// ## Shorthand
///
/// `field` alone is equivalent to `field: field`, matching Rust struct
/// literal shorthand:
///
/// ```no_run
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// # }
/// # async fn example(mut db: toasty::Db, mut user: User) -> toasty::Result<()> {
/// let name = "Alice Smith";
/// toasty::update!(user { name }).exec(&mut db).await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Method shorthand
///
/// `field.combinator(args)` is shorthand for
/// `field: toasty::stmt::combinator(args)`. Any function in `toasty::stmt`
/// works; missing functions surface as ordinary "no function" errors:
///
/// ```no_run
/// # #[derive(toasty::Model)]
/// # struct Article {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     tags: Vec<String>,
/// # }
/// # async fn example(mut db: toasty::Db, mut article: Article) -> toasty::Result<()> {
/// // tags.push("rust") expands to tags: stmt::push("rust")
/// toasty::update!(article { tags.push("rust") })
///     .exec(&mut db).await?;
/// # Ok(())
/// # }
/// ```
///
/// The shorthand is one method call deep. For chained expressions, use
/// the explicit `field: expr` form.
///
/// ## Embedded patch
///
/// `field: { sub: val, ... }` partially updates an embedded struct
/// field, leaving sub-fields not listed unchanged. Expands to
/// `stmt::apply([stmt::patch(...), ...])`:
///
/// ```no_run
/// # #[derive(toasty::Embed)]
/// # struct Metadata { version: i64, status: String }
/// # #[derive(toasty::Model)]
/// # struct Document {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     meta: Metadata,
/// # }
/// # async fn example(mut db: toasty::Db, mut doc: Document) -> toasty::Result<()> {
/// toasty::update!(doc {
///     meta: { version: 2, status: "published" },
/// }).exec(&mut db).await?;
/// # Ok(())
/// # }
/// ```
///
/// Sub-fields nest to arbitrary depth. To replace an embedded value
/// wholesale, pass the typed value directly: `meta: Metadata { ... }`.
///
/// ## Has-many insert
///
/// `field: [{ ... }, ...]` inserts new children of a has-many relation.
/// Each `{ ... }` becomes a create builder wrapped in
/// `stmt::insert(...)`; the whole list is wrapped in
/// `stmt::apply([...])`:
///
/// ```no_run
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// #     #[has_many]
/// #     todos: toasty::Deferred<Vec<Todo>>,
/// # }
/// # #[derive(toasty::Model)]
/// # struct Todo {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     title: String,
/// #     #[index]
/// #     user_id: i64,
/// #     #[belongs_to(key = user_id, references = id)]
/// #     user: toasty::Deferred<User>,
/// # }
/// # async fn example(mut db: toasty::Db, mut user: User) -> toasty::Result<()> {
/// toasty::update!(user {
///     todos: [{ title: "buy milk" }, { title: "walk dog" }],
/// }).exec(&mut db).await?;
/// # Ok(())
/// # }
/// ```
///
/// Items can also be plain expressions, mixed in with builder
/// shorthands — useful for combining inserts and removals:
///
/// ```no_run
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[has_many]
/// #     todos: toasty::Deferred<Vec<Todo>>,
/// # }
/// # #[derive(toasty::Model)]
/// # struct Todo {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     title: String,
/// #     #[index]
/// #     user_id: i64,
/// #     #[belongs_to(key = user_id, references = id)]
/// #     user: toasty::Deferred<User>,
/// # }
/// # async fn example(mut db: toasty::Db, mut user: User, old: Todo) -> toasty::Result<()> {
/// toasty::update!(user {
///     todos: [{ title: "new" }, toasty::stmt::remove(&old)],
/// }).exec(&mut db).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Field validation
///
/// The macro emits a method call per named field on the update builder.
/// A field name the model does not expose for update fails with the
/// compiler's standard "no method named …" error at the macro call
/// site.
#[proc_macro]
pub fn update(input: TokenStream) -> TokenStream {
    match update::generate(input.into()) {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
