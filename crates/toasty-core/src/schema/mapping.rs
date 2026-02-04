mod field;
pub use field::{Field, FieldEmbedded, FieldPrimitive};

mod model;
pub use model::{Model, TableToModel};

use super::app::ModelId;
use indexmap::IndexMap;

/// Defines the correspondence between app-level models and database-level
/// tables.
///
/// The mapping is constructed during schema building and remains immutable at
/// runtime. It provides the translation layer that enables the query engine to
/// convert model-oriented statements into table-oriented statements during the
/// lowering phase.
#[derive(Debug, Clone)]
pub struct Mapping {
    /// Per-model mappings indexed by model identifier.
    pub models: IndexMap<ModelId, Model>,
}

impl Mapping {
    /// Returns the mapping for the specified model.
    ///
    /// # Panics
    ///
    /// Panics if the model ID does not exist in the mapping.
    pub fn model(&self, id: impl Into<ModelId>) -> &Model {
        self.models.get(&id.into()).expect("invalid model ID")
    }

    /// Returns a mutable reference to the mapping for the specified model.
    ///
    /// # Panics
    ///
    /// Panics if the model ID does not exist in the mapping.
    pub fn model_mut(&mut self, id: impl Into<ModelId>) -> &mut Model {
        self.models.get_mut(&id.into()).expect("invalid model ID")
    }
}
