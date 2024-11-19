use super::*;

pub trait Input {
    fn resolve_field(&mut self, expr_field: &ExprField) -> Option<Expr> {
        None
    }

    fn resolve_column(&mut self, expr_column: &ExprColumn) -> Option<Expr> {
        None
    }

    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Option<Expr> {
        None
    }
}

impl Input for &Model {
    fn resolve_column(&mut self, expr_column: &ExprColumn) -> Option<Expr> {
        let (index, _) = self
            .lowering
            .columns
            .iter()
            .enumerate()
            .find(|(_, column_id)| **column_id == expr_column.column)
            .unwrap();

        Some(stmt::Expr::project(stmt::Expr::arg(0), [index]))
    }

    fn resolve_field(&mut self, expr_field: &ExprField) -> Option<Expr> {
        todo!()
    }
}

pub struct TableToModel<T>(pub T);

pub struct ModelToTable<T>(pub T);

impl Input for ModelToTable<&ExprRecord> {
    fn resolve_field(&mut self, expr_field: &ExprField) -> Option<Expr> {
        Some(self.0[expr_field.field.index].clone())
    }
}

impl Input for ModelToTable<(FieldId, &Expr)> {
    fn resolve_field(&mut self, expr_field: &ExprField) -> Option<Expr> {
        assert_eq!(self.0 .0, expr_field.field);
        Some(self.0 .1.clone())
    }
}

impl Input for ModelToTable<&Model> {
    fn resolve_field(&mut self, expr_field: &ExprField) -> Option<Expr> {
        assert!(
            !self.0.lowering.table_to_model[expr_field.field.index].is_null(),
            "field={expr_field:#?}; lowering={:#?}; ty={:#?}",
            self.0.lowering.table_to_model,
            self.0.fields[expr_field.field.index].ty,
        );
        Some(self.0.lowering.table_to_model[expr_field.field.index].clone())
    }
}

pub struct Args<T>(pub T);

impl Input for Args<&[Value]> {
    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Option<Expr> {
        Some(self.0[expr_arg.position].clone().into())
    }
}
