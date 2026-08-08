use super::Expr;
use crate::schema::{app::ModelId, db::TableId};

/// The row proposed by an upsert's create branch.
///
/// Projecting a field from this expression references the proposed value rather
/// than the value already stored in the conflicting row. SQL serializers map
/// the projected expression to the backend's proposed-row syntax, such as
/// PostgreSQL's `EXCLUDED` relation.
#[derive(Clone, Debug, PartialEq)]
pub enum ExprIncoming {
    /// The proposed application-model row before lowering.
    Model(ModelId),

    /// The proposed database-table row after lowering.
    Table(TableId),
}

impl ExprIncoming {
    /// Creates an incoming application-model row.
    pub fn model(model: ModelId) -> Self {
        Self::Model(model)
    }

    /// Creates an incoming database-table row.
    pub fn table(table: TableId) -> Self {
        Self::Table(table)
    }
}

impl From<ExprIncoming> for Expr {
    fn from(value: ExprIncoming) -> Self {
        Self::Incoming(value)
    }
}
