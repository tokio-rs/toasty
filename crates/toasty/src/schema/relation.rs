use super::{Load, Model};
use crate::stmt::{IntoExpr, IntoInsert, List, Path, Query};

use toasty_core::schema::app::FieldId;

/// Describes how a model participates in associations.
///
/// This trait is implemented by `#[derive(Model)]` and provides the associated
/// types that the generated `HasMany`, `HasOne`, and `BelongsTo` wrappers use
/// to construct query builders, create builders, and field accessors for
/// the relation target.
///
/// Users do not implement this trait manually.
pub trait Relation: Load<Output = Self> {
    /// The target model
    type Model: Model;

    /// The target expression (e.g. `Option<Model>`)
    type Expr;

    /// The query builder type for querying this relation's target.
    type Query;

    /// Create builder type for this relation's target model
    type Create: Default + IntoInsert<Model = Self::Model> + IntoExpr<Self::Model>;

    /// HasMany relation type
    type Many;

    /// The field accessor type used when this model appears as the "many" side
    /// of a has-many relation, parameterized by the origin model.
    type ManyField<Origin>;

    /// The has-one relation wrapper type for this model.
    type One;

    /// The field accessor type used when this model appears as the "one" side
    /// of a has-one relation, parameterized by the origin model.
    type OneField<Origin>;

    /// The optional has-one relation wrapper type. Used when the foreign key
    /// is nullable, making the association optional.
    type OptionOne;

    /// The element type produced when narrowing a `Query<List<Model>>` to a
    /// single-result query. For a required relation this is `Model`; for an
    /// optional relation (`Option<Model>`) this is `Option<Model>`.
    type NarrowQuery;

    /// Narrow a list query into a single-result query.
    ///
    /// Required relations narrow via `.one()`, optional relations via
    /// `.first()`.
    fn narrow_query(query: Query<List<Self::Model>>) -> Query<Self::NarrowQuery>;

    /// Return a fresh, default-initialized create builder.
    fn new_create() -> Self::Create {
        Self::Create::default()
    }

    /// Construct a `ManyField` from a path targeting a list of the model.
    fn new_many_field<Origin>(path: Path<Origin, List<Self::Model>>) -> Self::ManyField<Origin>;

    /// Map a field name string to its [`FieldId`].
    ///
    /// Panics if `name` does not match any field on the model.
    fn field_name_to_id(name: &str) -> FieldId;

    /// Returns `true` if this relation target is nullable (i.e., wrapped in
    /// `Option`). The default is `false`.
    fn nullable() -> bool {
        false
    }
}
