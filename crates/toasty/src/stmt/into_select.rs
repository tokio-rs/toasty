use super::*;

pub trait IntoSelect {
    type Model: Model;

    fn into_select(self) -> Select<Self::Model>;
}

impl<M: Model> IntoSelect for Select<M> {
    type Model = M;

    fn into_select(self) -> Select<Self::Model> {
        self
    }
}
