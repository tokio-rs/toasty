pub mod app;

mod builder;
pub(crate) use builder::Builder;

pub mod db;

mod name;
pub use name::Name;

mod verify;

use crate::*;

use app::{Field, FieldId, Model, ModelId, Query, QueryId};
use db::{ColumnId, IndexId, Table, TableId};

use std::sync::Arc;

#[derive(Debug, Default)]
pub struct Schema {
    /// Application-level schema
    pub app: app::Schema,

    /// Database-level schema
    pub db: Arc<db::Schema>,
}

pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Schema> {
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

pub fn from_str(source: &str) -> Result<Schema> {
    let schema = crate::ast::from_str(source)?;
    let schema = Schema::from_ast(&schema)?;
    Ok(schema)
}

impl Schema {
    /// Get a model by ID
    pub fn model(&self, id: impl Into<ModelId>) -> &Model {
        self.app.models.get(id.into().0).expect("invalid model ID")
    }

    /// Get a field by ID
    pub fn field(&self, id: FieldId) -> &Field {
        self.model(id.model)
            .fields
            .get(id.index)
            .expect("invalid field ID")
    }

    pub fn query(&self, id: impl Into<QueryId>) -> &Query {
        let id = id.into();
        &self.app.queries[id.0]
    }

    pub(crate) fn from_ast(ast: &ast::Schema) -> Result<Schema> {
        schema::Builder::default().from_ast(ast)
    }
}
