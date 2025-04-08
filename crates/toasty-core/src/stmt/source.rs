use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Source is a model
    Model(SourceModel),

    /// Source is a database table (lowered)
    Table(Vec<TableWithJoins>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceModel {
    /// The source model
    pub model: ModelId,

    /// Selecting via an association
    pub via: Option<Association>,

    /// Associations to include
    pub include: Vec<Path>,
}

impl Source {
    pub fn is_model(&self) -> bool {
        matches!(self, Source::Model(_))
    }

    #[track_caller]
    pub fn as_model(&self) -> &SourceModel {
        match self {
            Source::Model(source) => source,
            Source::Table(_) => todo!(),
        }
    }

    pub fn as_model_id(&self) -> ModelId {
        self.as_model().model
    }

    pub fn is_table(&self) -> bool {
        matches!(self, Source::Table(_))
    }

    pub fn table(table: impl Into<TableRef>) -> Source {
        Source::Table(vec![TableWithJoins {
            table: table.into(),
            joins: vec![],
        }])
    }

    pub fn as_table_with_joins(&self) -> &[TableWithJoins] {
        match self {
            Source::Table(source) => source,
            _ => todo!(),
        }
    }
}

impl From<&Model> for Source {
    fn from(value: &Model) -> Self {
        Source::from(value.id)
    }
}

impl From<ModelId> for Source {
    fn from(value: ModelId) -> Self {
        Source::Model(SourceModel {
            model: value,
            via: None,
            include: vec![],
        })
    }
}

impl From<TableId> for Source {
    fn from(value: TableId) -> Self {
        Source::table(value)
    }
}

impl From<TableRef> for Source {
    fn from(value: TableRef) -> Self {
        Source::table(value)
    }
}

impl From<TableWithJoins> for Source {
    fn from(value: TableWithJoins) -> Self {
        Source::Table(vec![value])
    }
}
