use super::Field;
use crate::{
    schema::{
        app::ModelId,
        db::{ColumnId, TableId},
    },
    stmt,
};

#[derive(Debug, Clone)]
pub struct Model {
    /// Model identiier
    pub id: ModelId,

    /// Table that the model maps to
    pub table: TableId,

    /// Table columns used to represent the model.
    pub columns: Vec<ColumnId>,

    /// Primitive fields map to column fields
    pub fields: Vec<Option<Field>>,

    /// How to map a model expression to a table expression
    pub model_to_table: stmt::ExprRecord,

    /// How to map the model's primary key to the table's primary key
    pub model_pk_to_table: stmt::Expr,

    /// How to map a table record to a model record
    pub table_to_model: stmt::ExprRecord,

    /// The cached structural record type for this model at the query engine level.
    ///
    /// This represents how the model appears after type erasure - primitive fields
    /// use their actual types (Id, String, etc.) while association fields use semantic
    /// types (Model(ModelId), List(Model(ModelId))) to prevent infinite recursion in
    /// cyclic relationships.
    ///
    /// ## Type Erasure Strategy
    ///
    /// Since model associations can be cyclic (User ↔ Post), we cannot always expand
    /// associations to their full record structure or we'd get infinite recursion:
    /// ```text
    /// User → Record([Id, String, List(Post)])
    /// Post → Record([Id, String, User])  
    /// User → Record([Id, String, List(Record([Id, String, User]))]) → ∞
    /// ```
    ///
    /// Instead, `record_ty` uses semantic placeholders:
    /// - `Model(ModelId)` for belongs_to/has_one associations  
    /// - `List(Model(ModelId))` for has_many associations
    ///
    /// ## Query-Time Expansion
    ///
    /// When a query actually includes associations (`.include(User::posts)`), the
    /// planner creates an expanded type by replacing semantic types with the target
    /// model's actual record structure - but only for the included associations,
    /// avoiding infinite expansion.
    ///
    /// Built during schema construction from `field.expr_ty()` values.
    pub record_ty: stmt::Type,
}
