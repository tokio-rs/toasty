pub mod app;

mod builder;
pub use builder::Builder;

pub mod db;

pub mod mapping;
use mapping::Mapping;

mod name;
pub use name::Name;

mod verify;

use crate::*;

use app::{Field, FieldId, Model, ModelId};
use db::{ColumnId, IndexId, Table, TableId};

use std::{any::TypeId, collections::HashMap, sync::Arc};

#[derive(Debug)]
pub struct Schema {
    /// Application-level schema
    pub app: app::Schema,

    /// Database-level schema
    pub db: Arc<db::Schema>,

    /// Maps the app-level schema to the db-level schema
    pub mapping: Mapping,

    /// Maps TypeId to ModelId for type resolution
    pub type_to_model: HashMap<TypeId, ModelId>,
}

impl Schema {
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Resolve a TypeId to a ModelId
    pub fn type_to_model_id(&self, type_id: TypeId) -> Result<ModelId> {
        self.type_to_model
            .get(&type_id)
            .copied()
            .ok_or_else(|| Error::msg("Model type not registered in schema"))
    }

    pub fn mapping_for(&self, id: impl Into<ModelId>) -> &mapping::Model {
        self.mapping.model(id)
    }

    pub fn table_for(&self, id: impl Into<ModelId>) -> &Table {
        self.db.table(self.table_id_for(id))
    }

    pub fn table_id_for(&self, id: impl Into<ModelId>) -> TableId {
        self.mapping.model(id).table
    }
}
