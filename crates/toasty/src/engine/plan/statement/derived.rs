use super::*;

impl PlanStatement<'_, '_> {
    /// Read a derived query's output, then filter and project those rows.
    /// In particular, its LIMIT/OFFSET has already taken effect.
    pub(super) fn plan_derived_source(
        &mut self,
        stmt: stmt::Statement,
        source: hir::StmtId,
    ) -> Result<mir::NodeId> {
        let query = stmt.as_query().ok_or_else(|| {
            toasty_core::Error::unsupported_feature("NoSQL derived sources require a SELECT")
        })?;
        let select = query.body.as_select_unwrap();
        if query.limit.is_some()
            || query.order_by.is_some()
            || query.with.is_some()
            || !query.locks.is_empty()
            || select.distinct
        {
            return Err(toasty_core::Error::unsupported_feature(
                "NoSQL derived queries support filtering and projection",
            ));
        }

        let input = self.planner.hir[source]
            .output
            .get()
            .expect("derived input planned");
        let input = self.filter_derived_rows(input, &select.filter);
        let row_ty = self.planner.mir[input].ty().as_list_unwrap().clone();

        let mut projection = stmt::Expr::record(
            self.load_data
                .select_items
                .iter()
                .map(|item| item.to_expr()),
        );
        Self::rewrite_derived_columns(&mut projection, &row_ty);
        let projection = eval::Func::from_stmt(projection, vec![row_ty]);
        let node = mir::Eval::map_over(&self.planner.mir, input, IndexSet::new(), projection);
        Ok(self.insert_mir_with_deps(node))
    }

    fn filter_derived_rows(&mut self, input: mir::NodeId, filter: &stmt::Filter) -> mir::NodeId {
        let Some(mut predicate) = filter.expr.clone() else {
            return input;
        };
        let input_ty = self.planner.mir[input].ty().clone();
        let row_ty = input_ty.as_list_unwrap().clone();

        // Filter args have already been rewritten to load_data positions.
        // Reserve arg(0) for the current input row before rewriting columns.
        visit_mut::walk_expr_scoped_mut(&mut predicate, 0, |expr, depth| {
            if let stmt::Expr::Arg(arg) = expr
                && arg.nesting == depth
            {
                arg.position += 1;
            }
            true
        });
        Self::rewrite_derived_columns(&mut predicate, &row_ty);

        let mut arg_tys = vec![row_ty];
        arg_tys.extend(
            self.load_data
                .inputs
                .iter()
                .map(|id| self.planner.mir[*id].ty().clone()),
        );
        let predicate = eval::Func::from_stmt(predicate, arg_tys);
        self.insert_mir_with_deps(mir::Filter {
            input,
            args: self.load_data.inputs.clone(),
            predicate,
            ty: input_ty,
        })
    }

    fn rewrite_derived_columns(expr: &mut stmt::Expr, row_ty: &stmt::Type) {
        visit_mut::walk_expr_scoped_mut(expr, 0, |expr, depth| {
            if depth == 0
                && let stmt::Expr::Reference(stmt::ExprReference::Column(column)) = expr
            {
                assert_eq!(column.nesting, 0);
                assert_eq!(column.table, 0);
                *expr = if row_ty.is_record() {
                    stmt::Expr::arg_project(0, [column.column])
                } else {
                    assert_eq!(column.column, 0);
                    stmt::Expr::arg(0)
                };
            }
            true
        });
    }
}
