use super::*;

pub trait IntoSelect<'stmt> {
    type Model: Model;

    fn into_select(self) -> Select<'stmt, Self::Model>;
}

pub trait AsSelect {
    type Model: Model;

    fn as_select(&self) -> Select<'_, Self::Model>;
}

impl<'stmt, M: Model> IntoSelect<'stmt> for Select<'stmt, M> {
    type Model = M;

    fn into_select(self) -> Select<'stmt, Self::Model> {
        self
    }
}

impl<'a, T> IntoSelect<'a> for &'a [T]
where
    T: AsSelect,
{
    type Model = T::Model;

    fn into_select(self) -> Select<'a, Self::Model> {
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
impl<'a, T> IntoSelect<'a> for &'a [T; 3]
where
    T: AsSelect,
{
    type Model = T::Model;

    fn into_select(self) -> Select<'a, Self::Model> {
        (&self[..]).into_select()
    }
}
