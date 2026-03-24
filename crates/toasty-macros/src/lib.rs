extern crate proc_macro;

mod create;
mod model;
mod query;

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
/// - A [`Register`] implementation for schema registration at runtime.
/// - Static query methods such as `all()`, `filter(expr)`,
///   `filter_by_<field>()`, and `get_by_<key>()`.
/// - Instance methods `update()` and `delete()`.
/// - A `Fields` struct returned by `<Model>::fields()` for building typed
///   filter expressions.
///
/// The struct must have named fields and no generic parameters.
///
/// [`Model`]: toasty::schema::Model
/// [`Load`]: toasty::schema::Load
/// [`Register`]: toasty::schema::Register
///
/// # Struct-level attributes
///
/// ## `#[key(...)]` — primary key
///
/// Defines the primary key at the struct level. Mutually exclusive with
/// field-level `#[key]`.
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
/// Multiple `partition` and `local` entries are allowed:
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// #[key(partition = tenant, partition = org, local = id)]
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
/// provided during creation. The expression is any valid Rust expression.
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
/// Sets a value that Toasty applies every time a record is created or
/// updated, unless the field is explicitly set on the builder.
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
/// `filter_by_<field>`. The database enforces uniqueness.
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
/// ## `#[serialize(json)]` — serialize complex types as JSON
///
/// Stores the field as a JSON string in the database. Requires the `serde`
/// feature and that the field type implements `serde::Serialize` and
/// `serde::Deserialize`.
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[serialize(json)]
/// tags: Vec<String>,
/// # }
/// ```
///
/// For `Option<T>` fields, add `nullable` so that `None` maps to SQL
/// `NULL` rather than the JSON string `"null"`:
///
/// ```
/// # use toasty::Model;
/// # use std::collections::HashMap;
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[serialize(json, nullable)]
/// metadata: Option<HashMap<String, String>>,
/// # }
/// ```
///
/// Cannot be used on relation fields.
///
/// # Relation attributes
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
/// user: toasty::BelongsTo<User>,
/// # }
/// ```
///
/// | Parameter | Meaning |
/// |-----------|---------|
/// | `key = <field>` | Local field holding the foreign key value |
/// | `references = <field>` | Field on the target model being referenced |
///
/// For composite foreign keys, repeat `key`/`references` pairs:
///
/// ```
/// # use toasty::Model;
/// # #[derive(Model)]
/// # struct Org {
/// #     #[key]
/// #     id: i64,
/// #     #[key]
/// #     tenant_id: i64,
/// # }
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     org_id: i64,
/// #     tenant_id: i64,
/// #[belongs_to(key = org_id, references = id, key = tenant_id, references = tenant_id)]
/// org: toasty::BelongsTo<Org>,
/// # }
/// ```
///
/// The number of `key` entries must equal the number of `references`
/// entries.
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
/// manager: toasty::BelongsTo<Option<User>>,
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
/// #     example: toasty::BelongsTo<Example>,
/// # }
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[has_many]
/// posts: toasty::HasMany<Post>,
/// # }
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
/// #     parent: toasty::BelongsTo<Option<Self>>,
/// #[has_many(pair = parent)]
/// children: toasty::HasMany<Person>,
/// # }
/// ```
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
/// #     example: toasty::BelongsTo<Example>,
/// # }
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[has_one]
/// profile: toasty::HasOne<Profile>,
/// # }
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
/// #     example: toasty::BelongsTo<Example>,
/// # }
/// # #[derive(Model)]
/// # struct Example {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #[has_one]
/// profile: toasty::HasOne<Option<Profile>>,
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
/// - `#[column]`, `#[default]`, `#[update]`, and `#[serialize]` cannot be
///   used on relation fields (`BelongsTo`, `HasMany`, `HasOne`).
/// - A field can have at most one relation attribute.
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
///     posts: toasty::HasMany<Post>,
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
///     #[serialize(json)]
///     tags: Vec<String>,
///
///     #[index]
///     user_id: i64,
///
///     #[belongs_to(key = user_id, references = id)]
///     user: toasty::BelongsTo<User>,
/// }
/// ```
#[proc_macro_derive(
    Model,
    attributes(
        key, auto, default, update, column, index, unique, table, has_many, has_one, belongs_to,
        serialize
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
/// - An [`Embed`] trait implementation (which extends [`Register`]).
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
/// variant. Each variant must have a `#[column(variant = N)]` attribute
/// assigning a stable integer discriminant.
///
/// **Unit-only enum:**
///
/// ```
/// #[derive(toasty::Embed)]
/// enum Status {
///     #[column(variant = 1)]
///     Pending,
///     #[column(variant = 2)]
///     Active,
///     #[column(variant = 3)]
///     Archived,
/// }
/// ```
///
/// A unit-only enum occupies a single column in the parent table. The
/// column stores the discriminant as an integer.
///
/// **Data-carrying enum:**
///
/// ```
/// #[derive(toasty::Embed)]
/// enum ContactInfo {
///     #[column(variant = 1)]
///     Email { address: String },
///     #[column(variant = 2)]
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
///     #[column(variant = 1)]
///     Pending,
///     #[column(variant = 2)]
///     Failed { reason: String },
///     #[column(variant = 3)]
///     Done,
/// }
/// ```
///
/// Applying `#[derive(Embed)]` to an enum generates:
///
/// - An [`Embed`] trait implementation (which extends [`Register`]).
/// - A `Fields` struct with `is_<variant>()` methods and comparison
///   methods (`eq`, `ne`, `in_list`).
/// - For data-carrying variants, per-variant handle types with a
///   `matches(closure)` method for pattern matching and field access.
///
/// # Field-level attributes
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
/// **On enum variants**, `#[column(variant = N)]` is **required** and
/// assigns the integer discriminant stored in the database:
///
/// ```
/// # #[derive(toasty::Embed)]
/// # enum Example {
/// #[column(variant = 1)]
/// Pending,
/// # }
/// ```
///
/// Discriminant values must be unique across all variants of the enum.
/// They are stored as `i64`.
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
/// # Using embedded types in a model
///
/// Reference an embedded type as a field on a [`Model`][`derive@Model`]
/// struct. The parent model's create and update builders gain a setter for
/// the embedded field. For embedded structs, a `with_<field>` method
/// supports partial updates of individual sub-fields:
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
/// // Full replacement
/// user.update()
///     .address(Address { street: "456 Oak Ave".into(), city: "Seattle".into() })
///     .exec(&mut db).await?;
///
/// // Partial update (struct only) — updates city, leaves street unchanged
/// user.update()
///     .with_address(|a| { a.city("Portland"); })
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
/// - Every enum variant must have a `#[column(variant = N)]` attribute
///   with a unique discriminant value.
/// - Enum variants may be unit variants or have named fields. Tuple
///   variants are not supported.
/// - Embedded types cannot have primary keys, relations, `#[auto]`,
///   `#[default]`, `#[update]`, or `#[serialize]` attributes.
///
/// # Full example
///
/// ```no_run
/// # async fn example(mut db: toasty::Db) -> toasty::Result<()> {
/// #[derive(Debug, PartialEq, toasty::Embed)]
/// enum Priority {
///     #[column(variant = 1)]
///     Low,
///     #[column(variant = 2)]
///     Normal,
///     #[column(variant = 3)]
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
/// doc.update()
///     .with_meta(|m| { m.version(2).status("published"); })
///     .exec(&mut db).await?;
/// # Ok(())
/// # }
/// ```
///
/// [`Embed`]: toasty::Embed
/// [`Register`]: toasty::Register
#[proc_macro_derive(Embed, attributes(column, index, unique))]
pub fn derive_embed(input: TokenStream) -> TokenStream {
    match model::generate_embed(input.into()) {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn include_schema(_input: TokenStream) -> TokenStream {
    todo!()
}

/// Builds a query from a concise, SQL-inspired syntax. The macro expands into
/// the equivalent method-chain calls on the model's query builder API. It does
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
/// # External references
///
/// `#ident` pulls a variable from the surrounding scope. `#(expr)` embeds an
/// arbitrary Rust expression. This follows the convention used by the `quote!`
/// macro.
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
/// model's generated query builder API.
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
/// #     todos: toasty::HasMany<Todo>,
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
/// #     user: toasty::BelongsTo<User>,
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
/// function calls all work.
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
/// let _ = toasty::create!(User { name: name, email: format!("{}@example.com", name) });
/// ```
///
/// ## Nested struct (BelongsTo / HasOne)
///
/// Use `{ ... }` **without** a type prefix to create a related record inline.
/// The macro calls the `with_<field>` closure setter on the builder.
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
/// #     user: toasty::BelongsTo<User>,
/// # }
/// let _ = toasty::create!(Todo {
///     title: "buy milk",
///     user: { name: "Alice" }
/// });
/// // Expands to:
/// // Todo::create()
/// //     .title("buy milk")
/// //     .with_user(|b| { let b = b.name("Alice"); b })
/// ```
///
/// The related record is created first and the foreign key is set
/// automatically.
///
/// ## Nested list (HasMany)
///
/// Use `[{ ... }, { ... }]` to create multiple related records. The macro calls
/// `with_<field>` with a `CreateMany` builder, invoking `.with_item()` for each
/// entry.
///
/// ```
/// # #[derive(toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// #     #[has_many]
/// #     todos: toasty::HasMany<Todo>,
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
/// #     user: toasty::BelongsTo<User>,
/// # }
/// let _ = toasty::create!(User {
///     name: "Alice",
///     todos: [{ title: "first" }, { title: "second" }]
/// });
/// // Expands to:
/// // User::create()
/// //     .name("Alice")
/// //     .with_todos(|b| b
/// //         .with_item(|b| { let b = b.title("first"); b })
/// //         .with_item(|b| { let b = b.title("second"); b })
/// //     )
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
/// #     todos: toasty::HasMany<Todo>,
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
/// #     user: toasty::BelongsTo<User>,
/// #     #[has_many]
/// #     tags: toasty::HasMany<Tag>,
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
/// #     todo: toasty::BelongsTo<Todo>,
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
/// | `HasMany<T>` | No related records created |
/// | `HasOne<Option<T>>` | No related record created |
/// | `BelongsTo<Option<T>>` | Foreign key set to `NULL` |
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
/// #     user: toasty::BelongsTo<User>,
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
/// #     user: toasty::BelongsTo<User>,
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
/// | `[ ... ]` | Tuple of builders (one per item) |
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
