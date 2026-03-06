use super::{Association, SourceTable, SourceTableId, TableFactor, TableRef, TableWithJoins};
use crate::{
    schema::{
        app::{ModelId, ModelRoot},
        db::TableId,
    },
    stmt::ExprArg,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Source is a model
    Model(SourceModel),

    /// Source is a database table (lowered)
    Table(SourceTable),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceModel {
    /// The source model
    pub model: ModelId,

    /// Selecting via an association
    pub via: Option<Association>,
}

impl Source {
    /// Create a source from a table with joins, providing explicit table refs
    pub fn table_with_joins(tables: Vec<TableRef>, from_item: TableWithJoins) -> Self {
        let source_table = SourceTable::new(tables, from_item);
        Self::Table(source_table)
    }

    pub fn is_model(&self) -> bool {
        matches!(self, Self::Model(_))
    }

    pub fn as_model(&self) -> Option<&SourceModel> {
        match self {
            Self::Model(source) => Some(source),
            _ => None,
        }
    }

    #[track_caller]
    pub fn as_model_unwrap(&self) -> &SourceModel {
        self.as_model()
            .expect("expected SourceModel; actual={self:#?}")
    }

    pub fn model_id(&self) -> Option<ModelId> {
        self.as_model().map(|source_model| source_model.model)
    }

    pub fn model_id_unwrap(&self) -> ModelId {
        self.as_model_unwrap().model
    }

    pub fn is_table(&self) -> bool {
        matches!(self, Self::Table(_))
    }

    pub fn table(table: impl Into<TableRef>) -> Self {
        let table_ref = table.into();
        let source_table = SourceTable::new(
            vec![table_ref],
            TableWithJoins {
                relation: TableFactor::Table(SourceTableId(0)),
                joins: vec![],
            },
        );
        Self::Table(source_table)
    }

    pub fn as_source_table(&self) -> &SourceTable {
        match self {
            Self::Table(source) => source,
            _ => todo!(),
        }
    }
}

impl From<&ModelRoot> for Source {
    fn from(value: &ModelRoot) -> Self {
        Self::from(value.id)
    }
}

impl From<ModelId> for Source {
    fn from(value: ModelId) -> Self {
        Self::Model(SourceModel {
            model: value,
            via: None,
        })
    }
}

impl From<TableId> for Source {
    fn from(value: TableId) -> Self {
        Self::table(value)
    }
}

impl From<TableRef> for Source {
    fn from(value: TableRef) -> Self {
        Self::table(value)
    }
}

impl From<ExprArg> for Source {
    fn from(value: ExprArg) -> Self {
        Source::Table(SourceTable::from(value))
    }
}
