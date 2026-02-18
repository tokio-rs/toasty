pub mod app;

mod builder;
pub use builder::Builder;

pub mod db;

pub mod mapping;
use mapping::Mapping;

mod name;
pub use name::Name;

mod verify;

use crate::Result;
use app::ModelId;
use db::{Table, TableId};
use std::sync::Arc;

#[derive(Debug)]
pub struct Schema {
    /// Application-level schema
    pub app: app::Schema,

    /// Database-level schema
    pub db: Arc<db::Schema>,

    /// Maps the app-level schema to the db-level schema
    pub mapping: Mapping,
}

impl Schema {
    pub fn builder() -> Builder {
        Builder::default()
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
