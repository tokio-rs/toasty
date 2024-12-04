use super::*;

use stmt::ValueRecord;

#[derive(Clone, Debug)]
pub struct ExprRecord {
    pub fields: Vec<Expr>,
}

impl Expr {
    pub fn record_from_vec(fields: Vec<Expr>) -> Expr {
        ExprRecord { fields }.into()
    }
}

impl ExprRecord {
    pub(crate) fn from_stmt(stmt: stmt::ExprRecord, convert: &mut impl Convert) -> ExprRecord {
        ExprRecord {
            fields: stmt
                .fields
                .into_iter()
                .map(|stmt| Expr::from_stmt_by_ref(stmt, convert))
                .collect(),
        }
    }

    pub(crate) fn eval_ref(&self, input: &mut impl Input) -> crate::Result<ValueRecord> {
        let mut applied = vec![];

        for expr in &self.fields {
            applied.push(expr.eval_ref(input)?);
        }

        Ok(ValueRecord::from_vec(applied))
    }
}

impl From<ExprRecord> for Expr {
    fn from(value: ExprRecord) -> Self {
        Expr::Record(value)
    }
}
