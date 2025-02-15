use super::*;

#[derive(Debug)]
pub enum EntryMut<'a> {
    Expr(&'a mut Expr),
    Value(&'a mut Value),
}

impl EntryMut<'_> {
    pub fn as_expr(&self) -> &Expr {
        match self {
            EntryMut::Expr(e) => e,
            _ => todo!(),
        }
    }

    pub fn as_expr_mut(&mut self) -> &mut Expr {
        match self {
            EntryMut::Expr(e) => e,
            _ => todo!(),
        }
    }

    pub fn is_expr(&self) -> bool {
        matches!(self, EntryMut::Expr(_))
    }

    pub fn is_statement(&self) -> bool {
        matches!(self, EntryMut::Expr(e) if e.is_stmt())
    }

    pub fn is_value(&self) -> bool {
        matches!(self, EntryMut::Value(_) | EntryMut::Expr(Expr::Value(_)))
    }

    pub fn is_value_null(&self) -> bool {
        matches!(
            self,
            EntryMut::Value(Value::Null) | EntryMut::Expr(Expr::Value(Value::Null))
        )
    }

    pub fn take(&mut self) -> Expr {
        match self {
            EntryMut::Expr(expr) => expr.take(),
            EntryMut::Value(value) => value.take().into(),
        }
    }

    pub fn insert(&mut self, expr: Expr) {
        match self {
            EntryMut::Expr(e) => **e = expr,
            EntryMut::Value(e) => match expr {
                Expr::Value(value) => **e = value,
                _ => panic!("cannot store expression in value entry"),
            },
        }
    }
}

impl<'a> From<&'a mut Expr> for EntryMut<'a> {
    fn from(value: &'a mut Expr) -> Self {
        EntryMut::Expr(value)
    }
}

impl<'a> From<&'a mut Value> for EntryMut<'a> {
    fn from(value: &'a mut Value) -> Self {
        EntryMut::Value(value)
    }
}
