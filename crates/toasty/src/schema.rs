mod auto;
pub use auto::Auto;

mod belongs_to;

/// Compile-time metadata and validation for `create!` macro field checking.
pub mod create_meta;
pub use create_meta::{CreateField, CreateMeta, ValidateCreate};

mod deferred;
pub use deferred::Deferred;

mod embed;
pub use embed::Embed;

mod field;
pub use field::{Field, Scalar};

#[cfg(feature = "jiff")]
mod jiff;

mod has_many;

mod has_one;

pub(crate) mod lazy_slot;

mod load;
pub use load::Load;

mod model;
pub use model::Model;

mod option;

mod register;
pub use register::inventory;
pub use register::{DiscoverItem, Register, generate_unique_id};

mod num;

mod relation;
pub use relation::{BelongsToField, HasManyField, HasOneField, Relation};

mod scope;
pub use scope::Scope;

use crate::Result;

pub use toasty_core::schema::{app, app::ModelSet, db, diff, mapping};

/// Build an [`app::Schema`] from a slice of model definitions produced by
/// `#[derive(Model)]`.
///
/// This is a thin wrapper around [`app::Schema::from_macro`] exposed for
/// use by generated code.
pub fn from_macro(models: impl IntoIterator<Item = app::Model>) -> Result<app::Schema> {
    app::Schema::from_macro(models)
}
