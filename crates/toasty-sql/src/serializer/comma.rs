use super::{Formatter, Params, ToSql};

pub(super) struct Comma<'a, S>(pub(super) &'a [S]);

impl<S> ToSql for Comma<'_, S> {
    fn fmt<T: Params>(&self, f: &mut Formatter<'_, T>) {
        todo!()
    }
}