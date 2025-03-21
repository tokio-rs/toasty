use super::{Formatter, ToSql};

use toasty_core::stmt;

pub trait Params {
    fn push(&mut self, param: &stmt::Value) -> Placeholder;
}

pub struct Placeholder(pub usize);

impl Params for Vec<stmt::Value> {
    fn push(&mut self, value: &stmt::Value) -> Placeholder {
        self.push(value.clone());
        Placeholder(self.len())
    }
}

impl ToSql for Placeholder {
    fn to_sql<T: Params>(self, f: &mut Formatter<'_, T>) {
        match f.serializer.flavor {
            _ => todo!(),
        }
    }
}
