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

pub struct TableToModel<T>(pub T);

impl<'stmt> Input<'stmt> for TableToModel<&ExprRecord<'stmt>> {
    fn resolve_column(&mut self, expr_column: &ExprColumn) -> Option<Expr<'stmt>> {
        todo!("column = {:#?}; self={:#?}", expr_column, self.0);
    }
}

// TODO: a bit of a hack
impl<'stmt> Input<'stmt> for TableToModel<(&Model, &[ColumnId])> {
    fn resolve_column(&mut self, expr_column: &ExprColumn) -> Option<Expr<'stmt>> {
        let (index, _) = self
            .0
             .1
            .iter()
            .enumerate()
            .find(|(_, column_id)| **column_id == expr_column.column)
            .unwrap();

        Some(stmt::Expr::project(stmt::Expr::arg(0), [index]))
    }
}

pub struct ModelToTable<T>(pub T);

impl<'stmt> Input<'stmt> for ModelToTable<&ExprRecord<'stmt>> {
    fn resolve_field(&mut self, expr_field: &ExprField) -> Option<Expr<'stmt>> {
        Some(self.0[expr_field.field.index].clone())
    }
}

pub struct Args<T>(pub T);

impl<'stmt> Input<'stmt> for Args<&[Value<'stmt>]> {
    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Option<Expr<'stmt>> {
        todo!("arg = {:#?}; self={:#?}", expr_arg, self.0);
    }
}

/*
pub struct Args<T>(T);

impl<'stmt> Input<'stmt> for &Value<'stmt> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Expr<'stmt> {
        let mut ret = &**self;

        for step in projection {
            ret = match ret {
                Value::Record(record) => &record[step.into_usize()],
                _ => todo!(),
            };
        }

        Expr::Value(ret.clone())
    }

    fn resolve_arg(&mut self, _expr_arg: &ExprArg) -> Expr<'stmt> {
        panic!("no argument source provided")
    }
}

impl<'stmt> Input<'stmt> for &ExprRecord<'stmt> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Expr<'stmt> {
        resolve::resolve(&**self, projection).unwrap().clone()
    }

    fn resolve_arg(&mut self, _expr_arg: &ExprArg) -> Expr<'stmt> {
        panic!("no argument source provided")
    }
}

impl<'stmt> Input<'stmt> for &[Expr<'stmt>] {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Expr<'stmt> {
        resolve::resolve(&**self, projection).unwrap().clone()
    }

    fn resolve_arg(&mut self, _expr_arg: &ExprArg) -> Expr<'stmt> {
        panic!("no argument source provided")
    }
}

pub fn args<T>(input: T) -> Args<T> {
    Args(input)
}

impl<'stmt> Input<'stmt> for Args<&[Value<'stmt>]> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Expr<'stmt> {
        /*
        Expr::Project(ExprProject {
            base: ProjectBase::ExprSelf,
            projection: projection.clone(),
        })
        */
        todo!()
    }

    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Expr<'stmt> {
        Expr::Value(self.0[expr_arg.position].clone())
    }
}

impl<'stmt> Input<'stmt> for Args<&[Expr<'stmt>]> {
    fn resolve_self_projection(&mut self, projection: &Projection) -> Expr<'stmt> {
        /*
        Expr::Project(ExprProject {
            base: ProjectBase::ExprSelf,
            projection: projection.clone(),
        })
        */
        todo!()
    }

    fn resolve_arg(&mut self, expr_arg: &ExprArg) -> Expr<'stmt> {
        self.0[expr_arg.position].clone()
    }
}
    */
