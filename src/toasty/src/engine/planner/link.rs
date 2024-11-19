use super::*;

impl<'stmt> Planner<'stmt> {
    pub(super) fn plan_link(&mut self, mut stmt: stmt::Link<'stmt>) {
        self.simplify_stmt_link(&mut stmt);

        // TODO: this should be heavily optimized to avoid multiple queries if
        // possible...

        // TODO: implement this for SQL
        self.plan_link_kv(stmt);
    }

    fn plan_link_kv(&mut self, stmt: stmt::Link<'stmt>) {
        // TODO: this should be heavily optimized to avoid multiple queries if
        // possible...

        let model = self.model(stmt.field.model);
        let field = model.field(stmt.field);

        // TODO: for now, this is required. The belongs_to FK must be the target
        // model's PK.
        if let Some(has_many) = field.ty.as_has_many() {
            let belongs_to = has_many.pair(self.schema);

            assert_eq!(
                belongs_to.foreign_key.fields.len(),
                model.primary_key_fields().len()
            );

            for (fk, pk) in belongs_to
                .foreign_key
                .fields
                .iter()
                .zip(model.primary_key_fields())
            {
                assert_eq!(fk.target, pk.id);
            }
        } else {
            todo!();
        }

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

        let mut stmt = stmt.target.update(self.schema);

        match &field.ty {
            FieldTy::HasMany(has_many) => {
                let key = key.eval_const();
                stmt.assignments.set(has_many.pair, key);
            }
            ty => todo!("ty={:#?}", ty),
        }

        // Translate the unlink to an update
        self.plan_update(stmt);
    }
}
