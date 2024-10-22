use super::*;

impl<'stmt> Planner<'stmt> {
    pub(super) fn plan_unlink(&mut self, mut stmt: stmt::Unlink<'stmt>) {
        self.simplify_stmt_unlink(&mut stmt);

        let model = self.model(stmt.field.model);
        let field = model.field(stmt.field);

        // Extract the PK info from the selection
        let filter = &stmt.source.body.as_select().filter;
        let index_plan = self.plan_index_path2(model, filter);

        let mut index_filter = index_plan.index_filter;
        let table = self.schema.table(model.lowering.table);
        let index = self.schema.index(index_plan.index.lowering.index);
        self.lower_index_filter(table, model, index_plan.index, &mut index_filter);
        let Some(key) = self.try_build_key_filter(index, &index_filter) else {
            todo!("stmt={:#?}", stmt)
        };
        let key = key.eval_const();

        match &field.ty {
            FieldTy::HasMany(has_many) => {
                let pair = self.schema.field(has_many.pair);

                if pair.nullable {
                    let mut stmt = stmt.target.update(&self.schema);

                    // This protects against races.
                    stmt.condition = Some(stmt::Expr::eq(has_many.pair, key));
                    stmt.set(has_many.pair, stmt::Value::Null);

                    self.plan_update(stmt);
                } else {
                    // TODO: include a deletion condition
                    self.plan_delete(stmt::Delete {
                        selection: stmt.target,
                    });
                }
            }
            ty => todo!("ty={:#?}", ty),
        }
    }
}
