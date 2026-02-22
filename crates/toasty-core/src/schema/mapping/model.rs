use super::Field;
use crate::{
    schema::{
        app::ModelId,
        db::{ColumnId, TableId},
    },
    stmt,
};

/// Defines the bidirectional mapping between a single model and its backing
/// table.
///
/// This struct contains the expression templates used during lowering to
/// translate between model-level field references and table-level column
/// references. The mapping supports scenarios where field names differ from
/// column names, where type conversions are required (e.g., `Id<T>` to
/// `String`), and where multiple models share a single table.
#[derive(Debug, Clone)]
pub struct Model {
    /// The model this mapping applies to.
    pub id: ModelId,

    /// The database table that stores this model's data.
    pub table: TableId,

    /// Ordered list of columns that comprise this model's storage
    /// representation.
    ///
    /// The order corresponds to the `model_to_table` expression record: the
    /// i-th expression in `model_to_table` produces the value for the i-th
    /// column here.
    pub columns: Vec<ColumnId>,

    /// Per-field mappings.
    ///
    /// Indexed by field index within the model. Primitive fields and embedded
    /// fields have their respective mappings, while relation fields use
    /// `Field::Relation` since they don't map directly to columns.
    pub fields: Vec<Field>,

    /// Expression template for converting model field values to table column
    /// values.
    ///
    /// Used during `INSERT` and `UPDATE` lowering. Each expression in the
    /// record references model fields (via `Expr::Reference`) and produces a
    /// column value. May include type casts (e.g., `Id<T>` to `String`) or
    /// concatenations for discriminated storage formats.
    pub model_to_table: stmt::ExprRecord,

    /// Expression template for converting the model's primary key to table
    /// columns.
    ///
    /// A specialized subset of `model_to_table` containing only the expressions
    /// needed to produce the table's primary key columns from the model's key
    /// fields.
    pub model_pk_to_table: stmt::Expr,

    /// Expression template for converting table column values to model field
    /// values.
    ///
    /// Used during `SELECT` lowering to construct the `RETURNING` clause. Each
    /// expression references table columns (via `Expr::Reference`) and produces
    /// a model field value. Relation fields are initialized to `Null` and
    /// replaced with subqueries when `include()` is used.
    pub table_to_model: TableToModel,
}

/// Expression template for converting table rows into model records.
///
/// Contains one expression per model field. Each expression references table
/// columns and produces the corresponding model field value. During lowering,
/// these expressions construct `SELECT` clauses that return model-shaped data.
#[derive(Debug, Default, Clone)]
pub struct TableToModel {
    /// One expression per model field, indexed by field position.
    expr: stmt::ExprRecord,
}

impl TableToModel {
    /// Creates a new `TableToModel` from the given expression record.
    pub fn new(expr: stmt::ExprRecord) -> TableToModel {
        TableToModel { expr }
    }

    /// Returns the complete expression record for use in a `RETURNING` clause.
    pub fn lower_returning_model(&self) -> stmt::Expr {
        self.expr.clone().into()
    }

    /// Returns the expression for a single field reference.
    ///
    /// # Arguments
    ///
    /// * `nesting` - The scope nesting level. Non-zero when the reference
    ///   appears in a subquery relative to the table source.
    /// * `index` - The field index within the model.
    pub fn lower_expr_reference(&self, nesting: usize, index: usize) -> stmt::Expr {
        let mut expr = self.expr[index].clone();
        let n = nesting;

        if n > 0 {
            stmt::visit_mut::for_each_expr_mut(&mut expr, |expr| {
                if let stmt::Expr::Reference(stmt::ExprReference::Column(expr_column)) = expr {
                    expr_column.nesting = n;
                }
            });
        }

        expr
    }
}

impl Model {
    /// Resolves a projection to the corresponding field mapping.
    ///
    /// Handles both single-step projections (primitive/embedded fields) and multi-step
    /// projections (nested embedded struct fields). Supports arbitrary nesting depth.
    ///
    /// # Examples
    ///
    /// - `[2]` → field at index 2 (primitive or embedded)
    /// - `[2, 1]` → embedded field at index 2, subfield at index 1
    /// - `[2, 1, 0]` → nested embedded field at index 2, subfield 1, sub-subfield 0
    ///
    /// # Returns
    ///
    /// Returns `Some(&Field)` if the projection is valid. The field can be:
    /// - `Field::Primitive` for partial updates to a specific primitive
    /// - `Field::Embedded` for full replacement of an embedded struct
    ///
    /// Returns `None` if the projection is invalid or points to a relation field.
    pub fn resolve_field_mapping(&self, projection: &stmt::Projection) -> Option<&Field> {
        let [first, rest @ ..] = projection.as_slice() else {
            return None;
        };

        // Get the first field from the root
        let mut current_field = self.fields.get(*first)?;

        // Walk through remaining steps
        for step in rest {
            match current_field {
                Field::Embedded(field_embedded) => {
                    // Navigate into the embedded field's subfields
                    current_field = field_embedded.fields.get(*step)?;
                }
                Field::Primitive(_) => {
                    // Cannot project through primitive fields
                    return None;
                }
                _ => {
                    // Cannot project through relation fields
                    return None;
                }
            }
        }

        Some(current_field)
    }
}
