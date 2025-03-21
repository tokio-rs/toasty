use super::{Formatter, Params};

macro_rules! fmt {
    ($f:expr, $( $fragments:expr )*) => {{
        $(
            $fragments.to_sql($f);
        )*
    }};
}

pub(super) trait ToSql {
    fn to_sql<T: Params>(self, f: &mut Formatter<'_, T>);
}

impl ToSql for &str {
    fn to_sql<T: Params>(self, f: &mut Formatter<'_, T>) {
        f.dst.push_str(self);
    }
}
