mod auto;
pub use auto::Auto;

mod belongs_to;
pub use belongs_to::BelongsTo;

mod deferred;
pub use deferred::{Defer, Deferred, build_deferred_load};

mod embed;
pub use embed::Embed;

mod field;
pub use field::Field;

#[cfg(feature = "jiff")]
mod jiff;

mod has_many;
pub use has_many::HasMany;

mod has_one;
pub use has_one::HasOne;

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
pub use relation::Relation;

mod scalar;
pub use scalar::Scalar;

mod scope;
pub use scope::Scope;

use crate::Result;

pub use toasty_core::schema::{app, app::ModelSet, db, mapping};

/// Build an [`app::Schema`] from a slice of model definitions produced by
/// `#[derive(Model)]`.
///
/// This is a thin wrapper around [`app::Schema::from_macro`] exposed for
/// use by generated code.
pub fn from_macro(models: impl IntoIterator<Item = app::Model>) -> Result<app::Schema> {
    app::Schema::from_macro(models)
}
