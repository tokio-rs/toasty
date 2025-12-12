use indexmap::{IndexMap, IndexSet};
use toasty_core::{schema::db::ColumnId, stmt};

use crate::engine::lower::{LowerStatement, LoweringContext};

#[derive(Debug)]
struct ConstantizeReturning<'a> {
    cx: stmt::ExprContext<'a>,
    source: ConstantizeSource<'a>,
}

#[derive(Debug)]
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
        use indexmap::map::Entry;

        let stmt::ExprSet::Values(values) = &source.body else {
            return;
        };

        assert!(!values.is_empty(), "TODO: handle this case");

        let LoweringContext::Insert(columns) = &self.cx else {
            panic!("not currently lowering an insert statement")
        };

        let stmt::Returning::Expr(project) = returning else {
            return;
        };

        // First, go through the returning expression and extract the list of
        // columns that is used in the returning clause.
        //
        // TODO: this information probably could be stored on the stmt_info
        // level. I think it is used in the HIR -> MIR conversion.
        let mut columns_are_stable_and_equal = IndexMap::new();
        let mut all_const = true;

        stmt::visit::for_each_expr(project, |expr| {
            if let stmt::Expr::Reference(stmt::ExprReference::Column(expr_column)) = expr {
                assert!(expr_column.nesting == 0, "TODO");

                // If there already is an entry for this column, then there is no more work to do.
                let e = match columns_are_stable_and_equal.entry(*expr_column) {
                    Entry::Occupied(_) => return,
                    Entry::Vacant(e) => e,
                };

                // Find the index in the row
                let index = columns
                    .iter()
                    .position(|column| column.index == expr_column.column)
                    .unwrap();

                let first = &values.rows[0].as_record_unwrap().fields[index];
                all_const &= first.is_const();

                let mut all_stable_and_equal = first.is_stable();

                for row in &values.rows[1..] {
                    let field = &row.as_record_unwrap().fields[index];

                    if all_stable_and_equal {
                        all_stable_and_equal &= first == field;
                    }

                    if all_const {
                        all_const &= field.is_const();
                    }

                    if !all_stable_and_equal && !all_const {
                        break;
                    }
                }

                e.insert(if all_stable_and_equal {
                    Some(first)
                } else {
                    None
                });
            }
        });

        if all_const {
            // Now, for each column, iterate the rows to see if either a) the column is specified as a constant for each row and b) if the value is equivalent for each row.

            let mut constantized = vec![];

            for row in &values.rows {
                let input = ConstantizeReturning {
                    cx: self.expr_cx,
                    source: ConstantizeSource::InsertValues {
                        values: row,
                        columns,
                    },
                };

                let Ok(row) = project.eval(input) else {
                    return;
                };

                constantized.push(row);
            }

            *returning = stmt::Returning::Value(if source.single {
                constantized
                    .into_iter()
                    .next()
                    .unwrap_or(stmt::Value::Null)
                    .into()
            } else {
                stmt::Value::List(constantized).into()
            });
        } else {
            stmt::visit_mut::for_each_expr_mut(project, |expr| {
                if let stmt::Expr::Reference(stmt::ExprReference::Column(expr_column)) = expr {
                    if let Some(new_expr) = columns_are_stable_and_equal[&*expr_column] {
                        *expr = new_expr.clone();
                    }
                }
            });
        }
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
        debug_assert_eq!(0, expr_reference.as_expr_column_unwrap().nesting, "TODO");

        let needle = self
            .cx
            .resolve_expr_reference(expr_reference)
            .expect_column();

        match self.source {
            ConstantizeSource::InsertValues { values, columns } => {
                let index = columns.iter().position(|column| needle.id == *column)?;
                match values {
                    stmt::Expr::Record(row) => {
                        Some(row[index].entry(projection).unwrap().to_expr())
                    }
                    stmt::Expr::Value(stmt::Value::Record(row)) => {
                        Some(row[index].entry(projection).to_expr())
                    }
                    _ => todo!("values={values:#?}"),
                }
            }
            ConstantizeSource::UpdateAssignments { assignments } => {
                if let Some(assignment) = assignments.get(&needle.id.index) {
                    assert!(assignment.op.is_set(), "TODO");
                    assert!(
                        assignment.expr.is_const(),
                        "TODO; assignment={assignment:#?}"
                    );

                    Some(assignment.expr.clone())
                } else {
                    None
                }
            }
        }
    }
}
