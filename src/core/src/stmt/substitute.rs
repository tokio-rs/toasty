use super::*;

pub trait Input<'stmt> {
    fn resolve_field(&mut self, expr_field: &ExprField) -> Option<Expr<'stmt>> {
        None
    }

    fn resolve_column(&mut self, expr_column: &ExprColumn) -> Option<Expr<'stmt>> {
        None
    }

    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Option<Expr<'stmt>> {
        None
    }
}

impl<'stmt> Input<'stmt> for &Model {
    fn resolve_column(&mut self, expr_column: &ExprColumn) -> Option<Expr<'stmt>> {
        let (index, _) = self
            .lowering
            .columns
            .iter()
            .enumerate()
            .find(|(_, column_id)| **column_id == expr_column.column)
            .unwrap();

        Some(stmt::Expr::project(stmt::Expr::arg(0), [index]))
    }

    fn resolve_field(&mut self, expr_field: &ExprField) -> Option<Expr<'stmt>> {
        todo!()
    }
}

pub struct TableToModel<T>(pub T);

pub struct ModelToTable<T>(pub T);

impl<'stmt> Input<'stmt> for ModelToTable<&ExprRecord<'stmt>> {
    fn resolve_field(&mut self, expr_field: &ExprField) -> Option<Expr<'stmt>> {
        Some(self.0[expr_field.field.index].clone())
    }
}

impl<'stmt> Input<'stmt> for ModelToTable<(FieldId, &Expr<'stmt>)> {
    fn resolve_field(&mut self, expr_field: &ExprField) -> Option<Expr<'stmt>> {
        assert_eq!(self.0 .0, expr_field.field);
        Some(self.0 .1.clone())
    }
}

impl<'stmt> Input<'stmt> for ModelToTable<&Model> {
    fn resolve_field(&mut self, expr_field: &ExprField) -> Option<Expr<'stmt>> {
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

impl<'stmt> Input<'stmt> for Args<&[Value<'stmt>]> {
    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Option<Expr<'stmt>> {
        Some(self.0[expr_arg.position].clone().into())
    }
}
