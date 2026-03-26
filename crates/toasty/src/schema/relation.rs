use super::{Load, Model};
use crate::stmt::{IntoExpr, IntoInsert};

use toasty_core::schema::app::FieldId;

pub trait Relation: Load<Output = Self> {
    /// The target model
    type Model: Model;

    /// The target expression (e.g. `Option<Model>`)
    type Expr;

    type Query;

    /// Create builder type for this relation's target model
    type Create: Default + IntoInsert<Model = Self::Model> + IntoExpr<Self::Model>;

    /// HasMany relation type
    type Many;

    type ManyField<Origin>;

    type One;

    type OneField<Origin>;

    /// Option fields
    type OptionOne;

    /// Return a fresh, default-initialized create builder.
    fn new_create() -> Self::Create {
        Self::Create::default()
    }

    fn field_name_to_id(name: &str) -> FieldId;

    fn nullable() -> bool {
        false
    }
}
