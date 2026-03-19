mod auto;
pub use auto::Auto;

mod belongs_to;
pub use belongs_to::BelongsTo;

mod field;
pub use field::Field;

#[cfg(feature = "jiff")]
mod field_jiff;

mod has_many;
pub use has_many::HasMany;

mod has_one;
pub use has_one::HasOne;

mod model;
pub use model::Model;

pub mod option;

mod relation;
pub use relation::Relation;

use crate::Result;

pub use toasty_core::schema::{app, db, mapping};

pub fn from_macro(models: &[app::Model]) -> Result<app::Schema> {
    app::Schema::from_macro(models)
}
