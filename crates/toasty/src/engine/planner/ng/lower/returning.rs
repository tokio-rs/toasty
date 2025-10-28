use toasty_core::{schema::db::ColumnId, stmt};

use crate::engine::{
    eval,
    planner::ng::lower::{LowerStatement, LoweringContext},
};

struct ConstantizeReturning<'a> {
    cx: stmt::ExprContext<'a>,
    source: ConstantizeSource<'a>,
}

enum ConstantizeSource<'a> {
    InsertValues {
        values: &'a stmt::Expr,
        columns: &'a [ColumnId],
    },
}

impl LowerStatement<'_, '_> {
    pub(super) fn constantize_insert_returning(
        &self,
        returning: &mut stmt::Returning,
        source: &stmt::Query,
    ) {
        let stmt::ExprSet::Values(values) = &source.body else {
            return;
        };

        let LoweringContext::Insert(columns) = &self.cx else {
            panic!("not currently lowering an insert statement")
        };

        let mut constantized = vec![];

        for row in &values.rows {
            let input = ConstantizeReturning {
                cx: self.expr_cx,
                source: ConstantizeSource::InsertValues {
                    values: row,
                    columns,
                },
            };

            match returning.as_expr_unwrap().eval(input) {
                Ok(row) => constantized.push(row),
                Err(_) => return,
            }
        }

        *returning = stmt::Returning::Value(stmt::Value::List(constantized));
    }

    fn constantize_returning(&self, returning: &mut stmt::Expr, source: ConstantizeSource<'_>) {

        /*
        struct ConstReturning<'a> {
            cx: stmt::ExprContext<'a>,
            columns: &'a [ColumnId],
        }
        */

        /*
        impl eval::Convert for ConstReturning<'_> {
            fn convert_expr_reference(&mut self, stmt: &stmt::ExprReference) -> Option<stmt::Expr> {
                let needle = self.cx.resolve_expr_reference(stmt).expect_column();

                let index = self
                    .columns
                    .iter()
                    .position(|column| needle.id == *column)
                    .unwrap();

                Some(stmt::Expr::arg_project(0, [index]))
            }
        }

        let args = stmt::Type::Record(
            insert_table
                .columns
                .iter()
                .map(|column_id| self.schema().db.column(*column_id).ty.clone())
                .collect(),
        );

        let expr = eval::Func::try_convert_from_stmt(
            returning.clone(),
            vec![args],
            ConstReturning {
                cx: stmt::ExprContext::new_with_target(self.schema(), &*stmt),
                columns: &insert_table.columns,
            },
        )
        .unwrap();

        let mut rows = vec![];

        // TODO: OPTIMIZE!
        for row in &values.rows {
            let evaled = expr.eval([row]).unwrap();
            rows.push(evaled);
        }

        // The returning portion of the statement has been extracted as a const.
        // We do not need to receive it from the database anymore.
        stmt.returning = None;

        Some((rows, stmt::Type::list(expr.ret)))
        */
    }
}

impl stmt::Input for ConstantizeReturning<'_> {
    fn resolve_ref(
        &mut self,
        expr_reference: &stmt::ExprReference,
        projection: &stmt::Projection,
    ) -> Option<stmt::Expr> {
        let needle = self
            .cx
            .resolve_expr_reference(expr_reference)
            .expect_column();

        match self.source {
            ConstantizeSource::InsertValues { values, columns } => {
                let index = columns.iter().position(|column| needle.id == *column)?;
                match values {
                    stmt::Expr::Record(row) => Some(row[index].entry(projection).to_expr()),
                    stmt::Expr::Value(stmt::Value::Record(row)) => {
                        Some(row[index].entry(projection).to_expr())
                    }
                    _ => todo!("values={values:#?}"),
                }
            }
        }
    }
}
