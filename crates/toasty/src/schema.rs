mod auto;
pub use auto::Auto;

mod deferred;
pub use deferred::Deferred;

mod embed;
pub use embed::Embed;

mod field;
pub use field::{Field, Scalar};

#[cfg(feature = "jiff")]
mod jiff;

mod has_many;

pub(crate) mod lazy_slot;

mod load;
pub use load::Load;

mod model;
pub use model::{Model, QueryMany, QueryOne, QueryOptionOne};

mod option;

mod register;
pub use register::inventory;
pub use register::{DiscoverItem, generate_unique_id};

mod num;

mod relation;
pub use relation::RelationManyField;

mod relation_one;
pub use relation_one::RelationOneField;

mod scope;
pub use scope::Scope;

mod via;
pub use via::{ManyViaElem, ViaManyField, model_via_field_ty};

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
