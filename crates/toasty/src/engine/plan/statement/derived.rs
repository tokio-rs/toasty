use super::*;

impl PlanStatement<'_, '_> {
    /// Read a derived query's output, then filter and project those rows.
    /// In particular, its LIMIT/OFFSET has already taken effect.
    pub(super) fn plan_derived_source(
        &mut self,
        mut stmt: stmt::Statement,
        source: hir::StmtId,
    ) -> Result<mir::NodeId> {
        let query = stmt.as_query_mut().ok_or_else(|| {
            toasty_core::Error::unsupported_feature("NoSQL derived sources require a SELECT")
        })?;
        let select = query.body.as_select_unwrap();
        if query.limit.is_some()
            || query.order_by.is_some()
            || query.with.is_some()
            || !query.locks.is_empty()
            || select.distinct
            || !self.load_data.inputs.is_empty()
        {
            return Err(toasty_core::Error::unsupported_feature(
                "NoSQL derived queries support filtering and projection without external arguments",
            ));
        }

        let mut filter = query.body.as_select_mut_unwrap().filter.expr.take();
        let input = self.planner.hir[source]
            .output
            .get()
            .expect("derived input planned");
        if matches!(&self.planner.mir[input].op, mir::Operation::Const(c) if c.value == stmt::Value::List(vec![]))
        {
            let ty = self
                .load_data
                .select_items
                .infer_record_list_ty(&self.planner.engine.expr_cx_for(&stmt));
            return Ok(self.insert_mir_with_deps(mir::Const {
                value: stmt::Value::List(vec![]),
                ty,
            }));
        }
        if let Some(filter) = &mut filter {
            Self::rewrite_derived_columns(filter);
        }
        let ty = self.planner.mir[input].ty().clone();
        let input = self.apply_post_filter(input, filter, ty);
        let row_ty = self.planner.mir[input].ty().as_list_unwrap().clone();

        let mut projection = stmt::Expr::record(
            self.load_data
                .select_items
                .iter()
                .map(|item| item.to_expr()),
        );
        Self::rewrite_derived_columns(&mut projection);
        let projection = eval::Func::from_stmt(projection, vec![row_ty]);
        let node = mir::Eval::map_over(&self.planner.mir, input, IndexSet::new(), projection);
        Ok(self.insert_mir_with_deps(node))
    }

    fn rewrite_derived_columns(expr: &mut stmt::Expr) {
        visit_mut::walk_expr_scoped_mut(expr, 0, |expr, depth| {
            if depth == 0
                && let stmt::Expr::Reference(stmt::ExprReference::Column(column)) = expr
            {
                assert_eq!(column.nesting, 0);
                assert_eq!(column.table, 0);
                *expr = stmt::Expr::arg_project(0, [column.column]);
            }
            true
        });
    }
}
