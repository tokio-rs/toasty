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

    /// Associations to include
    pub include: Vec<Path>,
}

impl Source {
    pub fn as_model(&self) -> &SourceModel {
        match self {
            Source::Model(source) => source,
            Source::Table(_) => todo!(),
        }
    }

    pub fn as_model_id(&self) -> ModelId {
        self.as_model().model
    }

    pub fn table(table: impl Into<TableId>) -> Source {
        Source::Table(vec![TableWithJoins {
            table: table.into(),
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
            include: vec![],
        })
    }
}
