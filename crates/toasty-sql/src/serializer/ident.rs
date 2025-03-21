use super::{Formatter, Params, ToSql};

pub(super) struct Ident<S>(pub(super) S);

impl<S: AsRef<str>> ToSql for Ident<S> {
    fn fmt<T: Params>(&self, f: &mut Formatter<'_, T>) {
        todo!()
    }
}
