use super::*;

pub trait IntoSelect {
    type Model: Model;

    fn into_select(self) -> Select<Self::Model>;
}

pub trait AsSelect {
    type Model: Model;

    fn as_select(&self) -> Select<Self::Model>;
}

impl<M: Model> IntoSelect for Select<M> {
    type Model = M;

    fn into_select(self) -> Select<Self::Model> {
        self
    }
}

impl<T> IntoSelect for &[T]
where
    T: AsSelect,
{
    type Model = T::Model;

    fn into_select(self) -> Select<Self::Model> {
        match self.len() {
            0 => todo!(),
            1 => self[0].as_select(),
            _ => self
                .into_iter()
                .fold(Select::unit(), |agg, stmt| agg.union(stmt.as_select())),
        }
    }
}

// TODO: make this a macro
impl<T, const N: usize> IntoSelect for &[T; N]
where
    T: AsSelect,
{
    type Model = T::Model;

    fn into_select(self) -> Select<Self::Model> {
        (&self[..]).into_select()
    }
}
