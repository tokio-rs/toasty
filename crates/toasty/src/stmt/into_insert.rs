use super::Insert;
use crate::schema::Model;

pub trait IntoInsert {
    type Model: Model;

    fn into_insert(self) -> Insert<Self::Model>;
}
