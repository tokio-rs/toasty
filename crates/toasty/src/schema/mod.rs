pub mod model;
pub use model::{Auto, Field, Model};

pub mod relation;
pub use relation::{BelongsTo, HasMany, HasOne, Relation};

use crate::Result;

pub use toasty_core::schema::{app, db, mapping};

pub fn from_macro(models: &[app::Model]) -> Result<app::Schema> {
    app::Schema::from_macro(models)
}
