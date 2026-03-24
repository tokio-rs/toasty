use super::{Create, Load, Model};

use toasty_core::schema::app::FieldId;

pub trait Relation: Load<Output = Self> + Create<Self::Model> {
    /// The target model
    type Model: Model;

    /// The target expression (e.g. `Option<Model>`)
    type Expr;

    type Query;

    /// HasMany relation type
    type Many;

    type ManyField<Origin>;

    type One;

    type OneField<Origin>;

    /// Option fields
    type OptionOne;

    fn field_name_to_id(name: &str) -> FieldId;

    fn nullable() -> bool {
        false
    }
}
