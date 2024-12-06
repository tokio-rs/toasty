use super::*;

impl Planner<'_> {
    pub(crate) fn infer_expr_ty(&self, expr: &stmt::Expr) -> stmt::Type {
        use stmt::Expr::*;

        match expr {
            And(_) => stmt::Type::Bool,
            BinaryOp(_) => stmt::Type::Bool,
            Cast(e) => e.ty.clone(),
            Column(e) => self.schema.column(e.column).ty.clone(),
            Field(e) => self.schema.field(e.field).expr_ty().clone(),
            Value(value) => self.infer_value_ty(value),
            Or(_) => stmt::Type::Bool,
            Record(e) => stmt::Type::Record(
                e.fields
                    .iter()
                    .map(|field| self.infer_expr_ty(field))
                    .collect(),
            ),
            DecodeEnum(_, ty, _) => ty.clone(),
            _ => todo!("{expr:#?}"),
        }
    }

    pub(crate) fn infer_value_ty(&self, value: &stmt::Value) -> stmt::Type {
        use stmt::Value::*;

        match value {
            Bool(_) => stmt::Type::Bool,
            I64(_) => stmt::Type::I64,
            Id(v) => stmt::Type::Id(v.model_id()),
            SparseRecord(v) => stmt::Type::SparseRecord(v.fields.clone()),
            Null => stmt::Type::Null,
            Record(v) => stmt::Type::Record(
                v.fields
                    .iter()
                    .map(|field| self.infer_value_ty(field))
                    .collect(),
            ),
            String(_) => stmt::Type::String,
            _ => todo!("{value:#?}"),
        }
    }
}
