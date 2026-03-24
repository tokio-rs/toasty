use super::{Association, SourceTable, SourceTableId, TableFactor, TableRef, TableWithJoins};
use crate::{
    schema::{
        app::{ModelId, ModelRoot},
        db::TableId,
    },
    stmt::ExprArg,
};

/// The data source for a [`Select`](super::Select) statement's `FROM` clause.
///
/// At the model level, the source references a model (optionally navigated via
/// an association). After lowering, the source becomes a table with joins.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::Source;
/// use toasty_core::schema::app::ModelId;
///
/// let source = Source::from(ModelId(0));
/// assert!(source.is_model());
/// assert!(!source.is_table());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Source is a model (app-level).
    Model(SourceModel),

    /// Source is a database table (lowered/DB-level).
    Table(SourceTable),
}

/// A model-level data source.
///
/// References a model by ID and optionally specifies an association traversal
/// path used to reach this model from another query.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::SourceModel;
/// use toasty_core::schema::app::ModelId;
///
/// let source = SourceModel { id: ModelId(0), via: None };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SourceModel {
    /// The model being selected.
    pub id: ModelId,

    /// If set, the model is reached via this association from another query.
    pub via: Option<Association>,
}

impl Source {
    /// Creates a table source from explicit table refs and a table-with-joins
    /// specification.
    pub fn table_with_joins(tables: Vec<TableRef>, from_item: TableWithJoins) -> Self {
        let source_table = SourceTable::new(tables, from_item);
        Self::Table(source_table)
    }

    /// Returns `true` if this is a `Model` source.
    pub fn is_model(&self) -> bool {
        matches!(self, Self::Model(_))
    }

    /// Returns a reference to the inner [`SourceModel`] if this is a `Model`
    /// variant.
    pub fn as_model(&self) -> Option<&SourceModel> {
        match self {
            Self::Model(source) => Some(source),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`SourceModel`].
    ///
    /// # Panics
    ///
    /// Panics if this is not a `Model` source.
    #[track_caller]
    pub fn as_model_unwrap(&self) -> &SourceModel {
        self.as_model()
            .expect("expected SourceModel; actual={self:#?}")
    }

    /// Returns the model ID if this is a `Model` source.
    pub fn model_id(&self) -> Option<ModelId> {
        self.as_model().map(|source_model| source_model.id)
    }

    /// Returns the model ID, panicking if this is not a `Model` source.
    pub fn model_id_unwrap(&self) -> ModelId {
        self.as_model_unwrap().id
    }

    /// Returns `true` if this is a `Table` source.
    pub fn is_table(&self) -> bool {
        matches!(self, Self::Table(_))
    }

    /// Creates a `Table` source from a single table reference with no joins.
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

    /// Returns a reference to the inner [`SourceTable`] if this is a `Table`
    /// variant.
    pub fn as_table(&self) -> Option<&SourceTable> {
        match self {
            Self::Table(source) => Some(source),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`SourceTable`].
    ///
    /// # Panics
    ///
    /// Panics if this is not a `Table` source.
    #[track_caller]
    pub fn as_table_unwrap(&self) -> &SourceTable {
        self.as_table()
            .unwrap_or_else(|| panic!("expected SourceTable; actual={self:#?}"))
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
            id: value,
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
