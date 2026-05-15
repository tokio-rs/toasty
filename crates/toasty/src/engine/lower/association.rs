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

        // The association path is always a single step. Multi-step (`via`)
        // relations are still one step here — the step names the `via`
        // relation field itself, which `expand_via` then unfolds into a chain
        // of single-step associations.
        assert!(association.path.len() == 1, "TODO");

        let Some(field) = self.schema().app.resolve_field_path(&association.path) else {
            todo!()
        };

        match &field.ty {
            // A multi-step (`via`) relation: unfold the path into a chain of
            // single-step associations and rewrite that instead.
            app::FieldTy::HasMany(app::HasMany {
                kind: app::HasKind::Via(via),
                ..
            })
            | app::FieldTy::HasOne(app::HasOne {
                kind: app::HasKind::Via(via),
                ..
            }) => {
                let via_path = via.path().clone();
                let expanded = self.expand_via(association, &via_path);
                self.rewrite_association_as_filter(expanded)
            }
            app::FieldTy::BelongsTo(rel) => {
                self.rewrite_association_belongs_to_as_filter(rel, association)
            }
            // Direct has-one / has-many: filter the target by its paired
            // `BelongsTo` against the source query.
            app::FieldTy::HasOne(app::HasOne {
                kind: app::HasKind::Direct(pair),
                ..
            })
            | app::FieldTy::HasMany(app::HasMany {
                kind: app::HasKind::Direct(pair),
                ..
            }) => stmt::Expr::in_subquery(stmt::Expr::ref_self_field(*pair), *association.source)
                .into(),
            _ => todo!("field={field:#?}"),
        }
    }

    /// Expand a multi-step (`via`) relation's association into a chain of
    /// nested single-step associations.
    ///
    /// `via_path` is the relation's resolved field path, rooted at the model
    /// `association.source` selects. Every step but the last is folded into a
    /// nested `Source::Model { via }` query; the returned association pairs
    /// that nested source with the path's final step. The surrounding walk
    /// then rewrites each nested `via` in turn.
    fn expand_via(
        &self,
        association: stmt::Association,
        via_path: &stmt::Path,
    ) -> stmt::Association {
        let mut model = via_path.root.as_model_unwrap();
        let steps = via_path.projection.as_slice();
        assert!(!steps.is_empty(), "via path must have at least one step");

        let mut source = *association.source;

        for &field_index in &steps[..steps.len() - 1] {
            let field = &self.schema().app.model(model).as_root_unwrap().fields[field_index];
            let next = relation_target(&field.ty);

            let assoc = stmt::Association {
                source: Box::new(source),
                path: stmt::Path::field(model, field_index),
            };
            source = stmt::Query::builder(stmt::SourceModel {
                id: next,
                via: Some(assoc),
            })
            .build();
            model = next;
        }

        let last = *steps.last().unwrap();
        stmt::Association {
            source: Box::new(source),
            path: stmt::Path::field(model, last),
        }
    }

    fn rewrite_association_belongs_to_as_filter(
        &mut self,
        rel: &app::BelongsTo,
        association: stmt::Association,
    ) -> stmt::Filter {
        // The surrounding statement selects `rel.target`. `association.source`
        // selects the model that holds this `BelongsTo`. Keep the target rows
        // that some source row's foreign key points at:
        //
        //   <target>.<fk.target> IN (SELECT <source>.<fk.source> FROM <source>)
        let [fk] = &rel.foreign_key.fields[..] else {
            todo!("composite foreign keys in `via` paths");
        };

        let mut source = *association.source;
        source.body.as_select_mut_unwrap().returning =
            stmt::Returning::Project(stmt::Expr::ref_self_field(fk.source));

        stmt::Expr::in_subquery(stmt::Expr::ref_self_field(fk.target), source).into()
    }
}

/// The target model of a relation field. Panics if the field is not a
/// relation — a `via` path resolves only through relations.
fn relation_target(ty: &app::FieldTy) -> app::ModelId {
    match ty {
        app::FieldTy::BelongsTo(rel) => rel.target,
        app::FieldTy::HasMany(rel) => rel.target,
        app::FieldTy::HasOne(rel) => rel.target,
        _ => panic!("via path step is not a relation: {ty:#?}"),
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
