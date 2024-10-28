use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Source is a model
    Model(SourceModel),
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
        }
    }

    pub fn as_model_id(&self) -> ModelId {
        match self {
            Source::Model(source) => source.model,
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
