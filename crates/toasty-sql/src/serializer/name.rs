use super::{Params, ToSql};

use crate::stmt;

impl ToSql for stmt::Name {
    fn fmt<T: Params>(&self, f: &mut super::Formatter<'_, T>) {
        todo!()
    }
}
