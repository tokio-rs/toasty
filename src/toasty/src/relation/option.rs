use super::*;

impl<T: Relation> Relation for Option<T> {
    type Many = T::Many;
    type ManyField = T::ManyField;
    type One = T::OptionOne;
    type OneField = T::OneField;
    type OptionOne = T::OptionOne;
}
