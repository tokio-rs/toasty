//! Application-level schema

mod arg;
pub use arg::Arg;

mod auto;
pub use auto::Auto;

mod context;
use context::Context;

mod field;
pub use field::{Field, FieldId, FieldPrimitive, FieldTy};

mod model;
pub use model::{Model, ModelId, ModelIndex, ModelIndexField, ModelIndexId};

mod query;
pub use query::{Query, QueryId};

mod relation;
pub use relation::{BelongsTo, HasMany, HasOne};

mod schema;
pub use schema::Schema;

mod scope;
pub use scope::ScopedQuery;

use super::{
    db::{IndexOp, IndexScope},
    Lowering, Name,
};
use crate::{ast, stmt};

// TODO: remove
use crate::schema::{lowering::IndexLowering, ColumnId, IndexId, TableId};
