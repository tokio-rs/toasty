use super::*;

pub trait IntoInsert<'stmt> {
    type Model: Model;

    fn into_insert(self) -> Insert<'stmt, Self::Model>;
}
