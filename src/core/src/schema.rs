mod arg;
pub use arg::Arg;

mod auto;
pub use auto::Auto;

mod builder;
pub(crate) use builder::Builder;

mod column;
pub use column::{Column, ColumnId};

mod context;
pub(crate) use context::Context;

mod field;
pub use field::{Field, FieldId, FieldPrimitive, FieldTy};

mod index;
pub use index::{Index, IndexColumn, IndexId, IndexOp, IndexScope};

mod model;
pub use model::{Model, ModelId, ModelIndex, ModelIndexField, ModelIndexId};

mod name;
pub use name::Name;

mod query;
pub use query::{Query, QueryId};

mod relation;
pub use relation::{BelongsTo, HasMany, HasOne};

mod scope;
pub use scope::ScopedQuery;

mod table;
pub use table::{Table, TableId, TablePrimaryKey};

mod verify;

use crate::*;

#[derive(Debug, Default)]
pub struct Schema {
    pub models: Vec<Model>,
    pub tables: Vec<Table>,
    pub queries: Vec<Query>,
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
        self.models.get(id.into().0).expect("invalid model ID")
    }

    pub fn table(&self, id: impl Into<TableId>) -> &Table {
        self.tables.get(id.into().0).expect("invalid table ID")
    }

    /// Get a field by ID
    pub fn field(&self, id: FieldId) -> &Field {
        self.model(id.model)
            .fields
            .get(id.index)
            .expect("invalid field ID")
    }

    pub fn column(&self, id: impl Into<ColumnId>) -> &Column {
        let id = id.into();
        self.table(id.table)
            .columns
            .get(id.index)
            .expect("invalid column ID")
    }

    pub fn index(&self, id: IndexId) -> &Index {
        self.table(id.table)
            .indices
            .get(id.index)
            .expect("invalid index ID")
    }

    pub fn query(&self, id: impl Into<QueryId>) -> &Query {
        let id = id.into();
        &self.queries[id.0]
    }

    pub(crate) fn from_ast(ast: &ast::Schema) -> Result<Schema> {
        schema::Builder::new().from_ast(ast)
    }
}
