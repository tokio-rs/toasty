use super::*;

impl Planner<'_> {
    /// Infer the type of an expression
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

    /// Infer the type of a value
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

    /// The return type of a model record. This is a record type with the fields
    /// used to instantiate models.
    pub(crate) fn model_record_ty(&self, model: &Model) -> stmt::Type {
        stmt::Type::Record(
            model
                .fields
                .iter()
                .map(|field| field.expr_ty().clone())
                .collect(),
        )
    }

    pub(crate) fn index_key_ty(&self, index: &Index) -> stmt::Type {
        match &index.columns[..] {
            [id] => self.schema.column(id).ty.clone(),
            ids => stmt::Type::Record(
                ids.iter()
                    .map(|id| self.schema.column(id).ty.clone())
                    .collect(),
            ),
        }
    }
}
