use indexmap::IndexMap;
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
    /// Attempts to evaluate an INSERT statement's RETURNING clause at compile
    /// time.
    ///
    /// This optimization transforms runtime RETURNING expressions into
    /// compile-time constant values when possible. This is especially important
    /// for databases like MySQL that don't support RETURNING clauses - by
    /// constantizing the return values, we can avoid working around the lack o
    /// database support.
    ///
    /// # What this does
    ///
    /// Converts `RETURNING` from an expression to be evaluated at runtime into
    /// a constant value that's known at planning time.
    ///
    /// **Example:**
    /// ```sql
    /// INSERT INTO users (id, name) VALUES ('123', 'Alice'), ('456', 'Bob')
    /// RETURNING id, name;
    /// ```
    ///
    /// Can be constantized to:
    /// ```text
    /// stmt::Returning::Value(vec![
    ///     Record { id: '123', name: 'Alice' },
    ///     Record { id: '456', name: 'Bob' },
    /// ])
    /// ```
    ///
    /// # How it works
    ///
    /// The algorithm has two main paths:
    ///
    /// ## Path 1: Full Constantization (all columns are constant)
    ///
    /// When ALL columns in the RETURNING clause can be evaluated to constants:
    ///
    /// 1. **Analyze** each column referenced in RETURNING to see if its values are:
    ///    - Constant across all rows (e.g., literal values like `'Alice'`, `123`,
    ///      `uuid()`)
    ///    - Stable and equal (same expression across all rows, e.g., all rows use
    ///      `DEFAULT`)
    ///
    /// 2. **Evaluate** the RETURNING expression for each row using the constant values:
    ///    - For each INSERT row, evaluate the RETURNING projection
    ///    - This produces a `stmt::Value` for each row
    ///
    /// 3. **Replace** `stmt::Returning::Expr(projection)` with
    ///    `stmt::Returning::Value(values)`
    ///    - Single-row inserts return a single value
    ///    - Multi-row inserts return a list of values
    ///
    /// ## Path 2: Partial Constantization (some columns are stable)
    ///
    /// When SOME columns are stable/equal across all rows but not all are constant:
    ///
    /// 1. **Identify** which column references have identical expressions across
    ///    all rows
    /// 2. **Replace** those column references in the RETURNING expression with
    ///    the actual expression
    /// 3. Leave other columns as-is (will be evaluated at runtime)
    ///
    /// This reduces the work needed at runtime without fully eliminating it.
    pub(super) fn constantize_insert_returning(
        &self,
        returning: &mut stmt::Returning,
        source: &stmt::Query,
    ) {
        match returning {
            stmt::Returning::Expr(project) => {
                if let Some(xformed_returning) =
                    self.constantize_insert_returning_projection(project, source)
                {
                    *returning = xformed_returning;
                }
            }
            stmt::Returning::Value(expr) => self.constantize_insert_returning_expr(expr, source),
            _ => {}
        }
    }

    fn constantize_insert_returning_projection(
        &self,
        project: &mut stmt::Expr,
        source: &stmt::Query,
    ) -> Option<stmt::Returning> {
        use indexmap::map::Entry;

        // Only handle INSERT with VALUES (not INSERT from SELECT, etc.)
        let stmt::ExprSet::Values(values) = &source.body else {
            return None;
        };

        assert!(!values.is_empty(), "TODO: handle this case");

        let LoweringContext::Insert(columns) = &self.cx else {
            panic!("not currently lowering an insert statement")
        };

        // ==== Phase 1: Analyze which columns can be constantized ====
        //
        // For each column referenced in the RETURNING clause, determine if:
        // - Its value is constant across all rows (can be evaluated now)
        // - Its expression is stable and identical across all rows (can be simplified)
        //
        // We track this in `columns_are_stable_and_equal`:
        // - Some(expr) = all rows have the same stable expression for this column
        // - None = column values vary across rows

        let mut columns_are_stable_and_equal = IndexMap::new();
        let mut all_const = true; // Track if ALL referenced columns are constant

        stmt::visit::for_each_expr(project, |expr| {
            if let stmt::Expr::Reference(stmt::ExprReference::Column(expr_column)) = expr {
                assert!(expr_column.nesting == 0, "TODO");

                // Skip if we've already analyzed this column
                let e = match columns_are_stable_and_equal.entry(*expr_column) {
                    Entry::Occupied(_) => return,
                    Entry::Vacant(e) => e,
                };

                // Find which field in the row corresponds to this column reference
                let index = columns
                    .iter()
                    .position(|column| column.index == expr_column.column)
                    .unwrap();

                // Check the first row to see if this field is constant
                let first = &values.rows[0].as_record_unwrap().fields[index];
                all_const &= first.is_const();

                // Check if this field has the same expression across all rows
                let mut all_stable_and_equal = first.is_stable();

                for row in &values.rows[1..] {
                    let field = &row.as_record_unwrap().fields[index];

                    // Check if this row's field equals the first row's field
                    if all_stable_and_equal {
                        all_stable_and_equal &= first == field;
                    }

                    // Check if this row's field is constant
                    if all_const {
                        all_const &= field.is_const();
                    }

                    // Early exit if both checks have failed
                    if !all_stable_and_equal && !all_const {
                        break;
                    }
                }

                // Store the result: Some(expr) if stable and equal, None otherwise
                e.insert(if all_stable_and_equal {
                    Some(first)
                } else {
                    None
                });
            }
        });

        // ==== Phase 2: Apply constantization based on analysis ====

        if all_const {
            // **Full constantization path**
            // All columns are constant - we can evaluate the RETURNING clause now
            // and replace it with a Value variant.

            let mut constantized = vec![];

            // Evaluate the RETURNING expression for each row
            for row in &values.rows {
                let input = ConstantizeReturning {
                    cx: self.expr_cx,
                    source: ConstantizeSource::InsertValues {
                        values: row,
                        columns,
                    },
                };

                // Try to evaluate the projection expression with this row's values
                let Ok(row) = project.eval(input) else {
                    // If evaluation fails, give up on constantization
                    return None;
                };

                constantized.push(row);
            }

            // Replace the expression-based RETURNING with a constant value
            Some(stmt::Returning::Value(if source.single {
                // Single row insert: return just the one value
                constantized
                    .into_iter()
                    .next()
                    .unwrap_or(stmt::Value::Null)
                    .into()
            } else {
                // Multi-row insert: return a list of values
                stmt::Value::List(constantized).into()
            }))
        } else {
            // **Partial constantization path**
            // Some columns are stable but not all are constant.
            // Replace stable column references with their actual expressions.

            stmt::visit_mut::for_each_expr_mut(project, |expr| {
                if let stmt::Expr::Reference(stmt::ExprReference::Column(expr_column)) = expr {
                    // If this column has a stable expression across all rows, inline it
                    if let Some(new_expr) = columns_are_stable_and_equal[&*expr_column] {
                        *expr = new_expr.clone();
                    }
                }
            });

            None
        }
    }

    fn constantize_insert_returning_expr(&self, expr: &mut stmt::Expr, source: &stmt::Query) {
        // Only handle INSERT with VALUES (not INSERT from SELECT, etc.)
        let stmt::ExprSet::Values(values) = &source.body else {
            return;
        };

        assert!(!values.is_empty(), "TODO: handle this case");

        let LoweringContext::Insert(columns) = &self.cx else {
            panic!("not currently lowering an insert statement")
        };

        #[derive(Debug)]
        struct Input<'a>(&'a [ColumnId], &'a stmt::Values);

        impl stmt::Input for Input<'_> {
            fn resolve_ref(
                &mut self,
                expr_reference: &stmt::ExprReference,
                projection: &stmt::Projection,
            ) -> Option<stmt::Expr> {
                let stmt::ExprReference::Column(expr_column) = expr_reference else {
                    return None;
                };

                let [row] = projection.as_slice() else {
                    return None;
                };

                // Find which field in the row corresponds to this column reference
                let index = self
                    .0
                    .iter()
                    .position(|column| column.index == expr_column.column)
                    .unwrap();

                let field = &self.1.rows[*row].as_record_unwrap()[index];

                if field.is_eval() {
                    Some(field.clone())
                } else {
                    None
                }
            }
        }

        expr.substitute(Input(columns, values));
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

        let stmt::Returning::Expr(project) = returning else {
            todo!("returning={returning:#?}")
        };

        project.substitute(input);

        if let Ok(row) = project.eval_const() {
            *returning = stmt::Returning::Expr(row.into());
        }
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
