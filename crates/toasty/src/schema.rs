// Top-level schema types for macro generation
mod field;
pub use field::{Field, FieldTy};

mod model;
pub use model::{Index, IndexField, Model, PrimaryKey};

mod relation;
pub use relation::{BelongsTo, HasMany, HasOne};

// Re-export core schema types for convenience
pub use toasty_core::schema::*;

use crate::Result;

pub fn from_macro(models: &[app::Model]) -> Result<app::Schema> {
    app::Schema::from_macro(models)
}
