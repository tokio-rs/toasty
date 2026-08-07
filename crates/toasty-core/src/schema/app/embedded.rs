use crate::{
    schema::{app::ModelId, db},
    stmt,
};

/// A reference to an embedded model (struct or enum) that is stored inline
/// within its parent model's table rather than in a separate table.
///
/// Embedded fields are flattened into the parent table's columns at the
/// database level, but appear as nested types at the application level.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::app::Embedded;
///
/// // Embedded is typically constructed by the schema builder.
/// let embedded: &Embedded = embedded_field;
/// let target_model = schema.model(embedded.target);
/// ```
#[derive(Debug, Clone)]
pub struct Embedded {
    /// The [`ModelId`] of the embedded model being referenced.
    pub target: ModelId,

    /// The expression type of this embedded field from the application's
    /// perspective.
    pub expr_ty: stmt::Type,

    /// Optional database type override for an embedded enum's discriminant
    /// column. Embedded structs do not accept a type override because they may
    /// map to more than one column.
    pub storage_ty: Option<db::Type>,
}
