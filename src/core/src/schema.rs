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

pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<app::Schema> {
    use anyhow::Context;
    use std::{fs, str};

    let path = path.as_ref();
    let contents = fs::read(path).with_context(|| {
        let path = path.canonicalize().unwrap_or(path.into());
        format!("Failed to read schema file from path {}", path.display())
    })?;
    let contents = str::from_utf8(&contents).unwrap();

    from_str(contents)
}

pub fn from_str(source: &str) -> Result<app::Schema> {
    let schema = crate::ast::from_str(source)?;
    let schema = app::Schema::from_ast(&schema)?;
    Ok(schema)
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
