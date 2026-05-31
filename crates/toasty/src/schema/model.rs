use super::Load;
use crate::stmt::{Expr, IntoExpr, IntoInsert, List, Path};

use toasty_core::schema::app::{self, FieldId, ModelId, ModelSet};

/// Trait for root models that map to database tables and can be queried.
///
/// Root models have primary keys, can be queried independently, and support
/// full CRUD operations. They carry queryability and deserialization
/// capabilities along with the relation-target metadata
/// that the [`RelationManyField`](super::RelationManyField) and
/// [`RelationOneField`](super::RelationOneField) traits project through when
/// describing a field that references this model.
pub trait Model: Load<Output = Self> + Sized {
    /// Query builder for this model, parameterized by what it executes to.
    ///
    /// The single generic `T` selects the result shape:
    ///
    /// - `Query<List<Self>>` — list query, executes to `Vec<Self>`.
    /// - `Query<Self>` — single-row query, executes to `Self`, erroring if no
    ///   row matches. Returned by non-nullable relation accessors.
    /// - `Query<Option<Self>>` — optional single-row query, executes to
    ///   `Option<Self>`. Returned by nullable relation accessors.
    type Query<T>;

    /// Create builder type for this model
    type Create: Default + IntoInsert<Model = Self> + IntoExpr<Self>;

    /// Update builder type for this model
    type Update<'a>;

    /// Update by query builder type for this model
    type UpdateQuery;

    /// A typed path from `Origin` into this model.
    type Path<Origin>;

    /// The model's primary key type.
    ///
    /// For a single-column key, this is the column's Rust type (e.g.
    /// `Uuid`, `i64`). For a composite key, this is a tuple of the
    /// column types in declaration order.
    ///
    /// Generic code can bind on this to write functions that accept any
    /// model identified by a particular PK type — for example, a request
    /// extractor for any `M: Model<PrimaryKey = Uuid>`.
    type PrimaryKey;

    /// The field accessor type used when this model appears as the "many" side
    /// of a has-many relation, parameterized by the origin model.
    type ManyField<Origin>;

    /// The field accessor type used when this model appears as the "one" side
    /// of a has-one relation, parameterized by the origin model.
    type OneField<Origin>;

    /// Unique identifier for this model within the schema.
    ///
    /// Identifiers are *not* unique across schemas.
    fn id() -> ModelId;

    /// Returns the schema definition for this model.
    fn schema() -> app::Model;

    /// Register this model and all models reachable through its fields into
    /// the given [`ModelSet`].
    ///
    /// If this model is already present in the set (checked via
    /// [`ModelSet::contains`]), the method returns immediately. Otherwise it
    /// inserts the model and recursively registers any models referenced by
    /// embedded or relation fields. This is the entry point used by
    /// [`models!`](crate::models) to register root models; embedded types are
    /// discovered transitively through their containing fields.
    fn register(model_set: &mut ModelSet);

    /// Construct a model path from a [`Path`] targeting this model.
    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin>;

    /// Construct a path rooted at this model.
    fn new_root_path() -> Self::Path<Self> {
        Self::new_path(Self::path_root())
    }

    /// An identity [`Path`] rooted at this model.
    ///
    /// This is how `Path` recovers a model's [`ModelId`] without a dedicated
    /// registration trait: the model supplies its own id via [`id`](Self::id).
    fn path_root() -> Path<Self, Self> {
        Path::from_model_id(Self::id())
    }

    /// A [`Path`] from this model to the field at `index`.
    fn path_field<U>(index: usize) -> Path<Self, U> {
        Path::field_at(Self::id(), index)
    }

    /// An identity [`Path`] rooted at a list of this model, used as the root of
    /// has-many relation scopes.
    fn path_model_list() -> Path<List<Self>, List<Self>> {
        Path::from_model_id(Self::id())
    }

    /// Return a fresh, default-initialized create builder.
    fn new_create() -> Self::Create {
        Self::Create::default()
    }

    /// Construct a [`ManyField`](Self::ManyField) from a path targeting a list
    /// of this model.
    fn new_many_field<Origin>(path: Path<Origin, List<Self>>) -> Self::ManyField<Origin>;

    /// Map a field name string to its [`FieldId`].
    ///
    /// Panics if `name` does not match any field on the model.
    fn field_name_to_id(name: &str) -> FieldId;

    /// Build a query that filters this model by its primary key.
    ///
    /// `id` takes any expression that evaluates to the model's
    /// [`PrimaryKey`](Self::PrimaryKey) type — bare PK values via their
    /// `IntoExpr` impl, subqueries that return a PK, or any other
    /// [`Expr`] of the correct type.
    fn find_by_primary_key(id: Expr<Self::PrimaryKey>) -> Self::Query<List<Self>>;

    /// Wrap a raw statement-level [`Query`](crate::stmt::Query) in this model's
    /// query builder, preserving the result shape `T`. This is the single
    /// constructor generic code uses to build any query shape; callers narrow
    /// at the statement level (`.one()`, `.first()`) before wrapping.
    fn wrap_query<T>(stmt: crate::stmt::Query<T>) -> Self::Query<T>;

    /// Narrow a list query to a single-row query (errors at exec time if no
    /// row matches). Used by the codegen for non-nullable relation accessors.
    fn query_one(query: Self::Query<List<Self>>) -> Self::Query<Self>;

    /// Narrow a list query to an optional single-row query. Used by the
    /// codegen for nullable relation accessors.
    fn query_first(query: Self::Query<List<Self>>) -> Self::Query<Option<Self>>;
}
