//! Application-level schema

mod arg;
pub use arg::Arg;

mod auto;
pub use auto::{AutoStrategy, UuidVersion};

mod constraint;
pub use constraint::{Constraint, ConstraintLength};

mod field;
pub use field::{Field, FieldId, FieldName, FieldPrimitive, FieldTy};

mod fk;
pub use fk::{ForeignKey, ForeignKeyField};

mod index;
pub use index::{Index, IndexField, IndexId};

mod model;
pub use model::{Model, ModelId};

mod pk;
pub use pk::PrimaryKey;

mod relation;
pub use relation::{BelongsTo, HasMany, HasOne};

mod schema;
pub use schema::Schema;

use super::Name;
