use super::Formatter;

macro_rules! fmt {
    ($f:expr, $( $fragments:expr )*) => {{
        $(
            $fragments.to_sql($f);
        )*
    }};
}

pub(super) trait ToSql {
    fn to_sql(self, f: &mut Formatter<'_>);
}

impl ToSql for &str {
    fn to_sql(self, f: &mut Formatter<'_>) {
        f.dst.push_str(self);
    }
}

impl ToSql for String {
    fn to_sql(self, f: &mut Formatter<'_>) {
        f.dst.push_str(&self);
    }
}

impl<T: ToSql> ToSql for Option<T> {
    fn to_sql(self, f: &mut Formatter<'_>) {
        if let Some(inner) = self {
            inner.to_sql(f);
        }
    }
}

impl<T> ToSql for &Option<T>
where
    for<'a> &'a T: ToSql,
{
    fn to_sql(self, f: &mut Formatter<'_>) {
        if let Some(inner) = self {
            inner.to_sql(f);
        }
    }
}

impl<T1, T2> ToSql for (T1, T2)
where
    T1: ToSql,
    T2: ToSql,
{
    fn to_sql(self, f: &mut Formatter<'_>) {
        fmt!(f, self.0 self.1);
    }
}

impl<T1, T2, T3> ToSql for (T1, T2, T3)
where
    T1: ToSql,
    T2: ToSql,
    T3: ToSql,
{
    fn to_sql(self, f: &mut Formatter<'_>) {
        fmt!(f, self.0 self.1 self.2);
    }
}

macro_rules! fmt_numeric {
    ( $( $ty:ident ),* ) => {
        $(
            impl ToSql for $ty {
                fn to_sql(self, f: &mut Formatter<'_>) {
                    use std::fmt::Write;
                    write!(f.dst, "{self}").unwrap();
                }
            }

            impl ToSql for &$ty {
                fn to_sql(self, f: &mut Formatter<'_>) {
                    use std::fmt::Write;
                    write!(f.dst, "{self}").unwrap();
                }
            }
        )*
    };
}

fmt_numeric!(u8, u16, u32, u64, usize);
