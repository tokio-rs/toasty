use super::*;

#[derive(Debug, Clone)]
pub enum Source {
    /// Source is a model
    Model(SourceModel),

    /// Source is a database table (lowered)
    Table(Vec<TableWithJoins>),
}

#[derive(Debug, Clone)]
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
        matches!(self, Self::Model(_))
    }

    #[track_caller]
    pub fn as_model(&self) -> &SourceModel {
        match self {
            Self::Model(source) => source,
            Self::Table(_) => todo!(),
        }
    }

    pub fn as_model_id(&self) -> ModelId {
        self.as_model().model
    }

    pub fn is_table(&self) -> bool {
        matches!(self, Self::Table(_))
    }

    pub fn table(table: impl Into<TableRef>) -> Self {
        Self::Table(vec![TableWithJoins {
            table: table.into(),
            joins: vec![],
        }])
    }

    pub fn as_table_with_joins(&self) -> &[TableWithJoins] {
        match self {
            Self::Table(source) => source,
            _ => todo!(),
        }
    }
}

impl From<&Model> for Source {
    fn from(value: &Model) -> Self {
        Self::from(value.id)
    }
}

impl From<ModelId> for Source {
    fn from(value: ModelId) -> Self {
        Self::Model(SourceModel {
            model: value,
            via: None,
            include: vec![],
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

impl From<TableWithJoins> for Source {
    fn from(value: TableWithJoins) -> Self {
        Self::Table(vec![value])
    }
}
