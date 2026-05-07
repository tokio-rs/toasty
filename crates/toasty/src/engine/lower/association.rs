use toasty_core::{
    schema::app,
    stmt::{self, ExprContext, IntoExprTarget, VisitMut},
};

/// Pre-lowering pass that rewrites `Source::Model { via: Some(_) }` into
/// an explicit WHERE filter on the surrounding statement.
///
/// `via` associations are an app-level construct. They appear when a
/// query is built from a relation traversal, e.g.
/// `user.todos().delete(...)`, and the lowering walk converts
/// `Source::Model` into `Source::Table` once the association has been
/// rewritten as a filter. This pass runs before the lowering walk so
/// that the walk only sees the rewritten form.
pub(super) struct RewriteVia<'a> {
    cx: ExprContext<'a>,
}

impl<'a> RewriteVia<'a> {
    pub(super) fn new(cx: ExprContext<'a>) -> Self {
        Self { cx }
    }

    /// Walk a statement and apply the via-association rewrite to every
    /// Delete, Insert, and Query node it contains.
    pub(super) fn rewrite(&mut self, stmt: &mut stmt::Statement) {
        self.visit_mut(stmt);
    }

    fn schema(&self) -> &'a toasty_core::Schema {
        self.cx.schema()
    }

    fn scope<'scope>(&'scope self, target: impl IntoExprTarget<'scope>) -> RewriteVia<'scope> {
        RewriteVia {
            cx: self.cx.scope(target),
        }
    }

    pub(super) fn rewrite_via_for_delete(&mut self, stmt: &mut stmt::Delete) {
        if let stmt::Source::Model(model) = &mut stmt.from
            && let Some(via) = model.via.take()
        {
            // Create a new scope to indicate we are operating in the
            // context of stmt.from
            let mut s = self.scope(&stmt.from);

            let filter = s.rewrite_association_as_filter(via);
            stmt.filter = stmt::Filter::and(stmt.filter.take(), filter);
        }
    }

    pub(super) fn rewrite_via_for_insert(&mut self, stmt: &mut stmt::Insert) {
        if let stmt::InsertTarget::Scope(scope) = &mut stmt.target {
            self.rewrite_via_for_query(scope);
        }
    }

    pub(super) fn rewrite_via_for_query(&mut self, stmt: &mut stmt::Query) {
        if let stmt::ExprSet::Select(select) = &mut stmt.body
            && let stmt::Source::Model(model) = &mut select.source
            && let Some(via) = model.via.take()
        {
            // Create a new scope to indicate we are operating in the
            // context of stmt.target
            let mut s = self.scope(&select.source);

            let filter = s.rewrite_association_as_filter(via);
            select.filter = stmt::Filter::and(select.filter.take(), filter);
        }
    }

    pub(super) fn rewrite_association_as_filter(
        &mut self,
        mut association: stmt::Association,
    ) -> stmt::Filter {
        // First, recurse into the association source so any nested via
        // associations are rewritten before the outer filter is built.
        stmt::visit_mut::visit_stmt_query_mut(self, &mut association.source);

        // For now, we only support paths with a single step
        assert!(association.path.len() == 1, "TODO");

        let Some(field) = self.schema().app.resolve_field_path(&association.path) else {
            todo!()
        };

        match &field.ty {
            app::FieldTy::BelongsTo(rel) => {
                self.rewrite_association_belongs_to_as_filter(rel, association)
            }
            app::FieldTy::HasOne(rel) => {
                stmt::Expr::in_subquery(stmt::Expr::ref_self_field(rel.pair), *association.source)
                    .into()
            }
            app::FieldTy::HasMany(rel) => {
                stmt::Expr::in_subquery(stmt::Expr::ref_self_field(rel.pair), *association.source)
                    .into()
            }
            _ => todo!("field={field:#?}"),
        }
    }

    fn rewrite_association_belongs_to_as_filter(
        &mut self,
        rel: &app::BelongsTo,
        association: stmt::Association,
    ) -> stmt::Filter {
        todo!("rel={rel:#?}, association={association:#?}");
    }
}

impl VisitMut for RewriteVia<'_> {
    fn visit_stmt_delete_mut(&mut self, i: &mut stmt::Delete) {
        self.rewrite_via_for_delete(i);
        stmt::visit_mut::visit_stmt_delete_mut(self, i);
    }

    fn visit_stmt_insert_mut(&mut self, i: &mut stmt::Insert) {
        self.rewrite_via_for_insert(i);
        stmt::visit_mut::visit_stmt_insert_mut(self, i);
    }

    fn visit_stmt_query_mut(&mut self, i: &mut stmt::Query) {
        self.rewrite_via_for_query(i);
        stmt::visit_mut::visit_stmt_query_mut(self, i);
    }
}
