use super::{Formatter, Params};

macro_rules! fmt {
    ($f:expr, $( $fragments:expr )*) => {{
        use $crate::serializer::ToSql;
        $(
            $fragments.fmt($f);
        )*
    }};
}

pub(super) trait ToSql {
    fn fmt<T: Params>(&self, f: &mut Formatter<'_, T>);
}

impl ToSql for &str {
    fn fmt<T: Params>(&self, f: &mut Formatter<'_, T>) {
        f.dst.push_str(self);
    }
}
