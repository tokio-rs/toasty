mod field;
pub use field::Field;

mod model;
pub use model::Model;

use super::*;

use indexmap::IndexMap;

/// Maps an app-level schema to a database-level schema
#[derive(Debug, Clone)]
pub struct Mapping {
    /// How to map each model to a table
    pub models: IndexMap<ModelId, Model>,
}

impl Mapping {
    pub fn model(&self, id: impl Into<ModelId>) -> &Model {
        self.models.get(&id.into()).expect("invalid model ID")
    }

    pub fn model_mut(&mut self, id: impl Into<ModelId>) -> &mut Model {
        self.models.get_mut(&id.into()).expect("invalid model ID")
    }
}
