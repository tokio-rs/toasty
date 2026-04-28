use crate::{
    schema::app::{Model, ModelId, Schema},
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
/// let embedded: &Embedded = field.ty.as_embedded_unwrap();
/// let target_model = embedded.target(&schema);
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Embedded {
    /// The [`ModelId`] of the embedded model being referenced.
    pub target: ModelId,

    /// The expression type of this embedded field from the application's
    /// perspective.
    pub expr_ty: stmt::Type,
}

impl Embedded {
    /// Resolves the target [`Model`] from the given schema.
    pub fn target<'a>(&self, schema: &'a Schema) -> &'a Model {
        schema.model(self.target)
    }
}
