use super::{Flavor, Formatter, Params, ToSql};

pub(super) struct Ident<S>(pub(super) S);

impl<S: AsRef<str>> ToSql for Ident<S> {
    fn to_sql<T: Params>(self, f: &mut Formatter<'_, T>) {
        match f.serializer.flavor {
            Flavor::Mysql => {
                f.dst.push('`');
                f.dst.push_str(self.0.as_ref());
                f.dst.push('`');
            }
            _ => {
                f.dst.push('"');
                f.dst.push_str(self.0.as_ref());
                f.dst.push('"');
            }
        }
    }
}
