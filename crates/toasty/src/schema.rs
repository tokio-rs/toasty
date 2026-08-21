mod auto;
pub use auto::Auto;

mod deferred;
pub use deferred::Deferred;

mod embed;
pub use embed::Embed;

mod field;
pub use field::{Document, Field, Scalar};

#[cfg(feature = "jiff")]
mod jiff;

#[cfg(feature = "net")]
mod net;

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
pub use via::{ViaMany, ViaManyField, ViaPath, ViaTarget};

pub use toasty_core::schema::{app, app::ModelSet, db, diff, mapping};
