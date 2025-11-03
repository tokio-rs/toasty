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
    UpdateAssignments {
        assignments: &'a stmt::Assignments,
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

            let Ok(row) = returning.as_expr_unwrap().eval(input) else {
                return;
            };

            constantized.push(row);
        }

        *returning = stmt::Returning::Value(stmt::Value::List(constantized));
    }

    pub(super) fn constantize_update_returning(
        &self,
        returning: &mut stmt::Returning,
        assignments: &stmt::Assignments,
    ) {
        let input = ConstantizeReturning {
            cx: self.expr_cx,
            source: ConstantizeSource::UpdateAssignments { assignments },
        };

        let Ok(row) = returning.as_expr_unwrap().eval(input) else {
            return;
        };

        *returning = stmt::Returning::Expr(row.into());
    }
}

impl stmt::Input for ConstantizeReturning<'_> {
    fn resolve_ref(
        &mut self,
        expr_reference: &stmt::ExprReference,
        projection: &stmt::Projection,
    ) -> Option<stmt::Expr> {
        assert_eq!(0, expr_reference.nesting(), "TODO");

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
            ConstantizeSource::UpdateAssignments { assignments } => {
                if let Some(assignment) = assignments.get(&needle.id.index) {
                    assert!(assignment.op.is_set(), "TODO");
                    assert!(assignment.expr.is_const(), "TODO");

                    Some(assignment.expr.clone())
                } else {
                    None
                }
            }
        }
    }
}
