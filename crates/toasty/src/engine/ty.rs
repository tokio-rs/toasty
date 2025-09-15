use toasty_core::{
    schema::{app::FieldId, Schema},
    stmt,
};

pub(crate) trait Resolve {
    fn resolve_column(&self, stmt: &stmt::ExprColumn) -> &stmt::Type;

    fn resolve_field(&self, field_id: FieldId) -> &stmt::Type;
}

struct NoopResolve;

impl Resolve for Schema {
    fn resolve_column(&self, stmt: &stmt::ExprColumn) -> &stmt::Type {
        &self
            .db
            .column(stmt.try_to_column_id().expect("not referencing column"))
            .ty
    }

    fn resolve_field(&self, field_id: FieldId) -> &stmt::Type {
        self.app.field(field_id).expr_ty()
    }
}

impl Resolve for NoopResolve {
    fn resolve_column(&self, _stmt: &stmt::ExprColumn) -> &stmt::Type {
        panic!("expression should not reference columns")
    }

    fn resolve_field(&self, _field_id: FieldId) -> &stmt::Type {
        panic!("expression should not reference fields")
    }
}

pub(crate) fn infer_eval_expr_ty(expr: &stmt::Expr, args: &[stmt::Type]) -> stmt::Type {
    infer_expr_ty(expr, args, &NoopResolve)
}

/// Infer the type of an expression
pub(crate) fn infer_expr_ty(
    expr: &stmt::Expr,
    args: &[stmt::Type],
    resolve: &impl Resolve,
) -> stmt::Type {
    use std::mem;
    use stmt::Expr::*;

    match expr {
        Arg(e) => args[e.position].clone(),
        And(_) => stmt::Type::Bool,
        BinaryOp(_) => stmt::Type::Bool,
        Cast(e) => e.ty.clone(),
        Column(e) => resolve.resolve_column(e).clone(),
        Reference(stmt::ExprReference::Field {
            model,
            index,
            nesting: _,
        }) => {
            let field_id = FieldId {
                model: *model,
                index: *index,
            };
            resolve.resolve_field(field_id).clone()
        }
        IsNull(_) => stmt::Type::Bool,
        Map(e) => {
            let base = infer_expr_ty(&e.base, args, resolve);
            let ty = infer_expr_ty(&e.map, &[base], resolve);
            stmt::Type::list(ty)
        }
        Or(_) => stmt::Type::Bool,
        Project(e) => {
            let mut base = infer_expr_ty(&e.base, args, resolve);

            for step in e.projection.iter() {
                base = match &mut base {
                    stmt::Type::Record(fields) => {
                        mem::replace(&mut fields[*step], stmt::Type::Null)
                    }
                    expr => todo!("expr={expr:#?}"),
                }
            }

            base
        }
        Record(e) => stmt::Type::Record(
            e.fields
                .iter()
                .map(|field| infer_expr_ty(field, args, resolve))
                .collect(),
        ),
        Value(value) => infer_value_ty(value),
        // -- hax
        DecodeEnum(_, ty, _) => ty.clone(),
        _ => todo!("{expr:#?}"),
    }
}

/// Infer the type of a value
pub(crate) fn infer_value_ty(value: &stmt::Value) -> stmt::Type {
    use stmt::Value::*;

    match value {
        Bool(_) => stmt::Type::Bool,
        I64(_) => stmt::Type::I64,
        Id(v) => stmt::Type::Id(v.model_id()),
        SparseRecord(v) => stmt::Type::SparseRecord(v.fields.clone()),
        Null => stmt::Type::Null,
        Record(v) => stmt::Type::Record(v.fields.iter().map(infer_value_ty).collect()),
        String(_) => stmt::Type::String,
        _ => todo!("{value:#?}"),
    }
}
