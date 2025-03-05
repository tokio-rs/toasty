mod belongs_to;
pub use belongs_to::BelongsTo;

mod has_many;
pub use has_many::HasMany;
use toasty_core::schema::app::ModelId;

pub mod option;

pub trait Relation {
    /// Query type
    type Query;

    /// HasMany relation type
    type Many;

    type ManyField;

    type One;

    type OneField;

    /// Option fields
    type OptionOne;
}

pub trait Relation2 {
    const ID: ModelId;

    /// Query type
    type Query;

    /// HasMany relation type
    type Many;

    type ManyField;

    type One;

    type OneField;

    /// Option fields
    type OptionOne;
}
