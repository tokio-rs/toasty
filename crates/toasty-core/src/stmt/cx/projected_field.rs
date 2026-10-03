use super::ExprContext;
use crate::{
    schema::{app, mapping},
    stmt::{Expr, ExprReference, Path},
};

/// A field resolved from an expression, including its storage mapping and scope.
#[derive(Debug)]
pub struct ProjectedField<'a> {
    /// The application field at the end of the projection.
    pub field: &'a app::Field,

    /// The field's mapping within the referenced root model.
    pub mapping: &'a mapping::Field,

    /// The schema path from the referenced root model to the field.
    ///
    /// This path carries no query scope. Converting it to an expression with
    /// [`Path::into_stmt`] produces a current-scope reference; callers must
    /// preserve [`nesting`](Self::nesting) when reconstructing an outer reference.
    pub path: Path,

    /// How many query scopes outward the root reference points.
    ///
    /// Zero references the current scope, one references its parent, and so on.
    pub nesting: usize,
}

impl<'a> ExprContext<'a> {
    /// Resolves a field reference or embedded-field projection in its query scope.
    ///
    /// Walks embedded structs and explicit enum variant selections, preserving
    /// the root reference's nesting. Relation fields can be returned, but the
    /// walk does not continue through their target models.
    ///
    /// Returns `None` for expressions that do not resolve to a field, invalid
    /// field or variant projections, and references to non-model scopes.
    ///
    /// # Panics
    ///
    /// Panics if a field reference's nesting exceeds the available parent scopes.
    pub fn resolve_projected_field(&self, expr: &Expr) -> Option<ProjectedField<'a>> {
        match expr {
            Expr::Reference(ExprReference::Field { nesting, index }) => {
                let model = self.target_at(*nesting).as_model()?;
                Some(ProjectedField {
                    field: model.fields.get(*index)?,
                    mapping: self.schema.mapping_for(model.id).fields.get(*index)?,
                    path: Path::field(model.id, *index),
                    nesting: *nesting,
                })
            }
            Expr::Project(project) => {
                let projected = if let Expr::Variant(variant) = &*project.base {
                    let projected = self.resolve_projected_field(&variant.base)?;
                    let app::FieldTy::Embedded(embedded) = &projected.field.ty else {
                        return None;
                    };
                    if embedded.target != variant.variant.model {
                        return None;
                    }
                    let mapping::Field::Enum(mapping) = projected.mapping else {
                        return None;
                    };
                    let mapping = mapping.variants.get(variant.variant.index)?;
                    let model = self
                        .schema
                        .app
                        .model(embedded.target)
                        .as_embedded_enum_unwrap();
                    let fields = model.variant_fields(variant.variant.index);
                    let (&index, rest) = project.projection.as_slice().split_first()?;
                    let mut path = Path::from_variant(projected.path, variant.variant);
                    path.projection.push(index);

                    return self.resolve_projected_subfield(
                        ProjectedField {
                            field: fields.get(index)?,
                            mapping: mapping.fields.get(index)?,
                            path,
                            nesting: projected.nesting,
                        },
                        rest,
                    );
                } else {
                    self.resolve_projected_field(&project.base)?
                };
                self.resolve_projected_subfield(projected, project.projection.as_slice())
            }
            _ => None,
        }
    }

    fn resolve_projected_subfield(
        &self,
        mut projected: ProjectedField<'a>,
        steps: &[usize],
    ) -> Option<ProjectedField<'a>> {
        for index in steps {
            let app::FieldTy::Embedded(embedded) = &projected.field.ty else {
                return None;
            };
            let mapping::Field::Struct(mapped) = projected.mapping else {
                return None;
            };
            projected.field = self
                .schema
                .app
                .model(embedded.target)
                .fields()
                .get(*index)?;
            projected.mapping = mapped.fields.get(*index)?;
            projected.path.projection.push(*index);
        }
        Some(projected)
    }
}
