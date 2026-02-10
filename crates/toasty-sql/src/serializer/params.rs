use crate::serializer::ExprContext;

use super::{Flavor, Formatter, ToSql};

use toasty_core::stmt;

pub trait Params {
    fn push(&mut self, param: &stmt::Value, type_hint: Option<&stmt::Type>) -> Placeholder;
}

pub struct Placeholder(pub usize);

#[derive(Debug, Clone)]
pub struct TypedValue {
    pub value: stmt::Value,
    pub type_hint: Option<stmt::Type>,
}

impl TypedValue {
    /// Infers the type of this value, using the type hint if available
    pub fn infer_ty(&self) -> stmt::Type {
        self.type_hint
            .clone()
            .unwrap_or_else(|| self.value.infer_ty())
    }
}

impl Params for Vec<stmt::Value> {
    fn push(&mut self, value: &stmt::Value, _type_hint: Option<&stmt::Type>) -> Placeholder {
        self.push(value.clone());
        Placeholder(self.len())
    }
}

impl Params for Vec<TypedValue> {
    fn push(&mut self, value: &stmt::Value, type_hint: Option<&stmt::Type>) -> Placeholder {
        self.push(TypedValue {
            value: value.clone(),
            type_hint: type_hint.cloned(),
        });
        Placeholder(self.len())
    }
}

impl ToSql for Placeholder {
    fn to_sql<P: Params>(self, _cx: &ExprContext<'_>, f: &mut Formatter<'_, P>) {
        use std::fmt::Write;

        match f.serializer.flavor {
            Flavor::Mysql => write!(&mut f.dst, "?").unwrap(),
            Flavor::Postgresql => write!(&mut f.dst, "${}", self.0).unwrap(),
            Flavor::Sqlite => write!(&mut f.dst, "?{}", self.0).unwrap(),
        }
    }
}
