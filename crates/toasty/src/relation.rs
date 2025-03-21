mod belongs_to;
pub use belongs_to::BelongsTo;

mod has_many;
pub use has_many::HasMany;

mod has_one;
pub use has_one::HasOne;

pub mod option;

use super::Model;

use toasty_core::schema::app::FieldId;

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
    /// The target model
    type Model: Model;

    type Query;

    /// HasMany relation type
    type Many;

    type ManyField;

    type One;

    type OneField;

    /// Option fields
    type OptionOne;

    fn field_name_to_id(name: &str) -> FieldId;

    fn nullable() -> bool {
        false
    }
}
