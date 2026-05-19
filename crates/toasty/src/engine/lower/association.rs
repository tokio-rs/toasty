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
        association: stmt::Association,
    ) -> stmt::Filter {
        assert!(
            !association.path.projection.is_empty(),
            "via path must have at least one step"
        );

        // Resolve every via in the path and unfold the chain into nested
        // single-step `Source::Model { via }` wrappers. After this the path
        // is one step and the terminal field is guaranteed not to be a via.
        let mut association = self.unfold_path(association);

        // Run the visitor's overridden `visit_stmt_query_mut` on the source
        // so any `Source::Model { via: Some(_) }` introduced by unfolding is
        // rewritten on its own merits before the outer single-step filter is
        // built. The free-function walker would skip the override on the
        // source query itself.
        self.visit_stmt_query_mut(&mut association.source);

        let Some(field) = self.schema().app.resolve_field_path(&association.path) else {
            todo!()
        };

        match &field.ty {
            app::FieldTy::BelongsTo(rel) => {
                self.rewrite_association_belongs_to_as_filter(rel, association)
            }
            // Direct has-one / has-many: filter the target by its paired
            // `BelongsTo` against the source query. Via relations were
            // already unfolded, so only direct kinds reach this arm.
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

    /// Entry point for path unfolding. Pulls the seed `source_model_id` off
    /// the association's source query and delegates to the recursive
    /// [`unfold_steps`](Self::unfold_steps) helper. Returns an association
    /// whose path is a single step that does **not** name a via relation.
    fn unfold_path(&self, association: stmt::Association) -> stmt::Association {
        let stmt::Association { source, path } = association;
        let source_model_id = source.body.as_select_unwrap().source.model_id_unwrap();
        self.unfold_steps(source, source_model_id, path.projection.as_slice())
    }

    /// Walk `steps`, splicing each via relation's resolved path inline and
    /// wrapping every intermediate step in a nested `Source::Model { via }`.
    /// Returns the outer single-step association the caller filters against.
    ///
    /// Via splicing allocates a `Vec<usize>` per via segment so the recursion
    /// can borrow it as a slice. Paths are short (typically 1-3 steps) and
    /// vias are rare, so this is cheap in practice.
    fn unfold_steps(
        &self,
        source: Box<stmt::Query>,
        source_model_id: app::ModelId,
        steps: &[usize],
    ) -> stmt::Association {
        let [first, rest @ ..] = steps else {
            unreachable!("unfold_steps called with empty steps")
        };

        let field = &self
            .schema()
            .app
            .model(source_model_id)
            .as_root_unwrap()
            .fields[*first];

        // If this step names a via relation, splice the via's resolved path
        // in place of the via field and continue. Handles via-of-via
        // naturally because the recursion re-examines the spliced steps.
        let via_path = match &field.ty {
            app::FieldTy::HasMany(app::HasMany {
                kind: app::HasKind::Via(via),
                ..
            })
            | app::FieldTy::HasOne(app::HasOne {
                kind: app::HasKind::Via(via),
                ..
            }) => Some(via.path.projection.as_slice()),
            _ => None,
        };
        if let Some(via_steps) = via_path {
            let mut spliced = Vec::with_capacity(via_steps.len() + rest.len());
            spliced.extend_from_slice(via_steps);
            spliced.extend_from_slice(rest);
            return self.unfold_steps(source, source_model_id, &spliced);
        }

        // Base case: a single direct relation step stays on the outer
        // association.
        if rest.is_empty() {
            return stmt::Association {
                source,
                path: stmt::Path::from_index(source_model_id, *first),
            };
        }

        let next_model_id = match &field.ty {
            app::FieldTy::HasMany(rel) => rel.target,
            app::FieldTy::HasOne(rel) => rel.target,
            app::FieldTy::BelongsTo(rel) => rel.target,
            other => todo!("non-relation field in via path: {other:#?}"),
        };

        let inner = stmt::Association {
            source,
            path: stmt::Path::from_index(source_model_id, *first),
        };
        let new_source = Box::new(stmt::Query::new_select(
            stmt::Source::Model(stmt::SourceModel {
                id: next_model_id,
                via: Some(inner),
            }),
            stmt::Expr::Value(stmt::Value::Bool(true)),
        ));

        self.unfold_steps(new_source, next_model_id, rest)
    }

    fn rewrite_association_belongs_to_as_filter(
        &mut self,
        rel: &app::BelongsTo,
        association: stmt::Association,
    ) -> stmt::Filter {
        // The FK lives on the source model; the target model carries the
        // referenced fields. Filter is `<fk.target...> IN (SELECT
        // <fk.source...> FROM <source>)` — a single field reference on each
        // side for single-column FKs, a record of references for composite
        // FKs (lowered to a tuple-style IN by the SQL serializer).
        let target = super::key_field_refs(0, rel.foreign_key.fields.iter().map(|fk| fk.target));
        let returning = super::key_field_refs(0, rel.foreign_key.fields.iter().map(|fk| fk.source));

        let mut source = *association.source;
        source.body.as_select_mut_unwrap().returning = stmt::Returning::Project(returning);

        stmt::Expr::in_subquery(target, source).into()
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
