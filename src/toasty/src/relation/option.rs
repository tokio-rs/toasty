use super::*;

impl<T: Relation> Relation for Option<T> {
    type Query = T::Query;
    type Many = T::Many;
    type ManyField = T::ManyField;
    type One = T::OptionOne;
    type OneField = T::OneField;
    type OptionOne = T::OptionOne;
}

impl<T: Relation2> Relation2 for Option<T> {
    type Model = T::Model;
    type Query = T::Query;
    type Many = T::Many;
    type ManyField = T::ManyField;
    type One = T::OptionOne;
    type OneField = T::OneField;
    type OptionOne = T::OptionOne;

    fn field_name_to_id(name: &str) -> FieldId {
        T::field_name_to_id(name)
    }

    fn nullable() -> bool {
        true
    }
}
