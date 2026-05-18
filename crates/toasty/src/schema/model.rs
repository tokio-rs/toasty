use super::create_meta::CreateMeta;
use super::{Load, Register};
use crate::stmt::{Expr, IntoExpr, IntoInsert, Path};

/// Trait for root models that map to database tables and can be queried.
///
/// Root models have primary keys, can be queried independently, and support
/// full CRUD operations. They extend `Register` with queryability and
/// deserialization capabilities.
pub trait Model: Register + Load<Output = Self> + Sized {
    /// Query builder type for this model
    type Query;

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

    /// Metadata about the model's fields for compile-time validation of
    /// `create!` invocations.
    const CREATE_META: CreateMeta;

    /// Construct a model path from a [`Path`] targeting this model.
    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin>;

    /// Build a query that filters this model by its primary key.
    ///
    /// `id` takes any expression that evaluates to the model's
    /// [`PrimaryKey`](Self::PrimaryKey) type — bare PK values via their
    /// `IntoExpr` impl, subqueries that return a PK, or any other
    /// [`Expr`] of the correct type.
    fn find_by_primary_key(id: Expr<Self::PrimaryKey>) -> Self::Query;

    /// Construct a path rooted at this model.
    fn new_root_path() -> Self::Path<Self> {
        Self::new_path(Path::root())
    }

    /// Return a fresh, default-initialized create builder.
    fn new_create() -> Self::Create {
        Self::Create::default()
    }
}
