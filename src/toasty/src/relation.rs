mod belongs_to;
pub use belongs_to::BelongsTo;

mod has_many;
pub use has_many::HasMany;
use toasty_core::schema::app::{FieldId, ModelId};

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

    /// Fields type
    type Fields;

    /// Query type
    type Query;

    /// HasMany relation type
    type Many;

    type ManyField;

    type One;

    type OneField;

    /// Option fields
    type OptionOne;

    fn field_name_to_id(name: &str) -> FieldId;
}
