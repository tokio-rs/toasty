use super::*;

impl<T: Relation> Relation for Option<T> {
    type Many = Option<T::Many>;
    type ManyField = T::ManyField;
    type One = Option<T::One>;
    type OneField = T::OneField;
}
