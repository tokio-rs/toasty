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
        // Unfold multi-step paths into a chain of nested single-step
        // associations. Each iteration peels the first step off the path and
        // wraps the source query in a `Source::Model { via }` that captures
        // just that step. After this loop, `path.len() == 1`.
        while association.path.len() > 1 {
            association = self.peel_first_step(association);
        }

        // Run the visitor's overridden `visit_stmt_query_mut` on the source
        // so a `Source::Model { via: Some(_) }` introduced by unfolding (or
        // present from the caller) is rewritten on its own merits before the
        // outer single-step filter is built. The free-function walker would
        // skip the override on the source query itself.
        self.visit_stmt_query_mut(&mut association.source);

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

    /// Peel the first step off `association.path` and absorb it into a fresh
    /// `Source::Model { via }` wrapped as the new source. The returned
    /// association has its path shortened by one step and rooted at the model
    /// reached by that step.
    fn peel_first_step(&self, mut association: stmt::Association) -> stmt::Association {
        let source_model_id = association
            .source
            .body
            .as_select_unwrap()
            .source
            .model_id_unwrap();
        let source_model = self.schema().app.model(source_model_id).as_root_unwrap();

        let steps = association.path.projection.as_slice();
        let first_step = steps[0];
        let rest: Vec<usize> = steps[1..].to_vec();

        let next_model_id = match &source_model.fields[first_step].ty {
            app::FieldTy::HasMany(rel) => rel.target,
            app::FieldTy::HasOne(rel) => rel.target,
            app::FieldTy::BelongsTo(rel) => rel.target,
            other => todo!("non-relation field in via path: {other:#?}"),
        };

        let first_step_path = stmt::Path::from_index(source_model_id, first_step);
        let inner_assoc = stmt::Association {
            source: association.source,
            path: first_step_path,
        };

        let new_source = stmt::Query::new_select(
            stmt::Source::Model(stmt::SourceModel {
                id: next_model_id,
                via: Some(inner_assoc),
            }),
            stmt::Expr::Value(stmt::Value::Bool(true)),
        );

        let mut new_path = stmt::Path::model(next_model_id);
        for step in rest {
            new_path.projection.push(step);
        }

        association.source = Box::new(new_source);
        association.path = new_path;
        association
    }

    fn rewrite_association_belongs_to_as_filter(
        &mut self,
        rel: &app::BelongsTo,
        association: stmt::Association,
    ) -> stmt::Filter {
        // The FK lives on the source model; the target model carries the
        // referenced fields. Filter is: `self.<fk.target> IN (SELECT
        // <fk.source> FROM <source>)`. Single-column FKs only for now —
        // composite keys can be added by switching to tuple-style IN.
        assert_eq!(
            rel.foreign_key.fields.len(),
            1,
            "composite foreign keys in BelongsTo via paths not yet supported"
        );
        let fk = &rel.foreign_key.fields[0];

        let mut source = *association.source;
        source.body.as_select_mut_unwrap().returning =
            stmt::Returning::Project(stmt::Expr::ref_self_field(fk.source));

        stmt::Expr::in_subquery(stmt::Expr::ref_self_field(fk.target), source).into()
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
