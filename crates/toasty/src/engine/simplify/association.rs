use super::*;

impl Simplify<'_> {
    pub(super) fn simplify_via_association_for_delete(&mut self, stmt: &mut stmt::Delete) {
        if let stmt::Source::Model(model) = &mut stmt.from {
            if let Some(via) = model.via.take() {
                let filter = self.rewrite_association_as_filter(via);
                stmt.filter = stmt::Expr::and(stmt.filter.take(), filter);
            }
        }
    }

    pub(super) fn simplify_via_association_for_insert(&mut self, stmt: &mut stmt::Insert) {
        if let stmt::InsertTarget::Scope(scope) = &mut stmt.target {
            self.simplify_via_association_for_query(scope);
        }
    }

    pub(super) fn simplify_via_association_for_query(&mut self, stmt: &mut stmt::Query) {
        if let stmt::ExprSet::Select(select) = &mut stmt.body {
            if let stmt::Source::Model(model) = &mut select.source {
                if let Some(via) = model.via.take() {
                    let filter = self.rewrite_association_as_filter(via);
                    select.filter = stmt::Expr::and(select.filter.take(), filter);
                }
            }
        }
    }

    fn rewrite_association_as_filter(&mut self, mut association: stmt::Association) -> stmt::Expr {
        // First, we want to simplify the association source.
        stmt::visit_mut::visit_stmt_query_mut(self, &mut association.source);

        // For now, we only support paths with a single step
        assert!(association.path.len() == 1, "TODO");

        let field = association.path.resolve_field(&self.schema.app);

        match &field.ty {
            app::FieldTy::BelongsTo(rel) => {
                self.rewrite_association_belongs_to_as_filter(rel, association)
            }
            app::FieldTy::HasOne(rel) => stmt::Expr::in_subquery(rel.pair, *association.source),
            app::FieldTy::HasMany(rel) => stmt::Expr::in_subquery(rel.pair, *association.source),
            _ => todo!("field={field:#?}"),
        }
    }

    fn rewrite_association_belongs_to_as_filter(
        &mut self,
        rel: &app::BelongsTo,
        association: stmt::Association,
    ) -> stmt::Expr {
        /*
        let operands = rel.foreign_key.fields.iter().map(|fk_field| {
            stmt::Expr::eq(todo!())
        });
        */

        todo!("rel={rel:#?}, association={association:#?}");
    }
}
