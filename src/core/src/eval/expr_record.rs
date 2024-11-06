use super::*;

#[derive(Clone, Debug)]
pub struct ExprRecord<'stmt> {
    pub fields: Vec<Expr<'stmt>>,
}

impl<'stmt> Expr<'stmt> {
    pub fn record_from_vec(fields: Vec<Expr<'stmt>>) -> Expr<'stmt> {
        ExprRecord { fields }.into()
    }
}

impl<'stmt> ExprRecord<'stmt> {
    pub(crate) fn from_stmt(stmt: stmt::ExprRecord<'stmt>) -> ExprRecord<'stmt> {
        ExprRecord {
            fields: stmt.fields.into_iter().map(Expr::from_stmt).collect(),
        }
    }

    pub(crate) fn eval_ref(&self, input: &mut impl Input<'stmt>) -> crate::Result<Record<'stmt>> {
        let mut applied = vec![];

        for expr in &self.fields {
            applied.push(expr.eval_ref(input)?);
        }

        Ok(Record::from_vec(applied))
    }
}

impl<'stmt> From<ExprRecord<'stmt>> for Expr<'stmt> {
    fn from(value: ExprRecord<'stmt>) -> Self {
        Expr::Record(value)
    }
}
