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
    /// `true` when the rewrite is being applied to an insert's scope query
    /// (i.e. the parent handle on a `parent.<children>().create()` call).
    /// Insert-scope filters are consumed by the scope walker — for IC
    /// HasItems we want IN-subquery shapes the walker can extract literals
    /// from. Read-path filters are consumed by the database; for IC
    /// HasItems we extract literals at lowering and emit
    /// `account = <literal> AND sk STARTS_WITH <derived-prefix>` so the
    /// child's hierarchical sort-key prefix is honoured.
    insert_scope: bool,
}

impl<'a> RewriteVia<'a> {
    pub(super) fn new(cx: ExprContext<'a>) -> Self {
        Self {
            cx,
            insert_scope: false,
        }
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
            insert_scope: self.insert_scope,
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
            // Mark the rewrite as insert-scope for the duration of this
            // call; the scope walker, not the database, consumes the
            // resulting filter.
            let prev = self.insert_scope;
            self.insert_scope = true;
            self.rewrite_via_for_query(scope);
            self.insert_scope = prev;
        }
    }

    pub(super) fn rewrite_via_for_query(&mut self, stmt: &mut stmt::Query) {
        if let stmt::ExprSet::Select(select) = &mut stmt.body
            && let stmt::Source::Model(model) = &mut select.source
            && let Some(via) = model.via.take()
        {
            // Complete a scalar-terminal via used as a query source. For
            // `#[has_many(via = todos.tags.name)]`, `user.tag_names()` selects
            // the `name` column from `Tag` (the model the chain reaches), not
            // whole `Tag` records:
            //
            //     SELECT DISTINCT tag.name      -- returning: project Tag.name
            //     FROM tag                      -- source model: Tag (the via target)
            //     WHERE <tag reachable from user>  -- the chain, unfolded below
            //
            // The navigation method can't build this: it runs in user code with
            // no linked schema, so it can't name the target model and emits a
            // placeholder source id with no projection. The resolved `Via` is
            // available here, so set the source model (the FROM table) and the
            // terminal projection (the RETURNING) before unfolding the chain
            // into the WHERE filter.
            //
            // A model-terminal via has `terminal == None` — its source already
            // selects whole target records — so this is a no-op there.
            let scalar_terminal =
                self.schema()
                    .app
                    .resolve_field_path(&via.path)
                    .and_then(|field| match &field.ty {
                        app::FieldTy::Via(v) => v.terminal.map(|terminal| (v.target, terminal)),
                        _ => None,
                    });

            if let Some((target, _)) = scalar_terminal {
                model.id = target;
            }
            if let Some((target, terminal)) = scalar_terminal {
                select.returning =
                    stmt::Returning::Project(stmt::Path::field(target, terminal).into_stmt());
            }

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
            app::FieldTy::Has(has) => stmt::Expr::in_subquery(
                stmt::Expr::ref_self_field(has.pair_id),
                *association.source,
            )
            .into(),
            // `HasItems` paired with `ItemParent` (R2.9). Lowers to a
            // partition-membership IN-subquery plus a sort-key prefix
            // filter. The partition column on the parent and child share a
            // name (R2.4), so we project the parent query on its partition
            // column and constrain the child's same-named partition column
            // to be a member of that set.
            app::FieldTy::HasItems(has_items) => {
                self.rewrite_association_has_items_as_filter(has_items, association)
            }
            _ => todo!("field={field:#?}"),
        }
    }

    /// Lower a `HasItems` association into a filter expression.
    ///
    /// The emitted shape depends on whether the rewrite is happening at
    /// insert-scope or read-scope (see [`Self::insert_scope`]).
    ///
    /// **Insert scope** (`parent.<children>().create()`): the scope walker
    /// in `insert.rs` consumes the filter to populate the new row's key
    /// columns. Both partition and sort are distributed via
    /// `IN-subquery` conjuncts so the walker can extract the parent's
    /// literals from the source filter and write them into the child row.
    /// `AutoStrategy::ItemCollectionChildSortKey` then overwrites the
    /// child's sk slot with the hierarchical encoding.
    ///
    /// ```text
    /// child.<partition> IN (SELECT parent.<partition> FROM <source>)
    ///   AND child.<sort> IN (SELECT parent.<sort> FROM <source>)
    ///   AND child.<sort> STARTS_WITH "<Child>#"   (no-op for walker)
    /// ```
    ///
    /// **Read scope** (`parent.<children>().exec()`): the database
    /// consumes the filter directly. We extract the parent's `<partition>`
    /// and `<sort>` literals from the source query's filter and emit:
    ///
    /// ```text
    /// child.<partition> = <parent_partition_literal>
    ///   AND child.<sort> STARTS_WITH <derived_child_prefix>
    /// ```
    ///
    /// where `derived_child_prefix` is computed by the same find-first-`#`
    /// swap algorithm as the insert encoding, with a trailing `#` boundary
    /// marker so e.g. `Todo#<u>#` doesn't false-match a longer prefix.
    /// This honours hierarchical encoding (R7.1) — `alice.todos()` only
    /// returns rows whose sk starts with `Todo#<alice-uuid>#`, not Bob's
    /// `Todo#<bob-uuid>#…`.
    ///
    /// If literals can't be extracted from the source filter (a more
    /// general source query), this currently panics. A silent fallback to
    /// the IN-subquery form would over-fetch siblings under different
    /// parents — better a clear failure than wrong results.
    fn rewrite_association_has_items_as_filter(
        &self,
        has_items: &app::HasItems,
        association: stmt::Association,
    ) -> stmt::Filter {
        let target_root = self.schema().app.model(has_items.target).as_root_unwrap();
        let pk = &target_root.primary_key.fields;
        assert!(
            pk.len() >= 2,
            "item-collection child must have a (partition, sort) primary key"
        );
        let child_partition = pk[0];
        let child_sort = pk[1];

        // The parent's partition + sort fields have the same names as the
        // child's (R2.4). Locate them on the source (parent) model.
        let source_model_id = association
            .source
            .body
            .as_select_unwrap()
            .source
            .model_id_unwrap();
        let parent_root = self.schema().app.model(source_model_id).as_root_unwrap();
        let partition_name = self.schema().app.field(child_partition).name.app_unwrap();
        let sort_name = self.schema().app.field(child_sort).name.app_unwrap();
        let parent_partition_field_id = parent_root
            .fields
            .iter()
            .find(|f| f.name.app.as_deref() == Some(partition_name))
            .map(|f| f.id)
            .expect("item-collection parent must declare a same-name partition field (R2.4)");
        let parent_sort_field_id = parent_root
            .fields
            .iter()
            .find(|f| f.name.app.as_deref() == Some(sort_name))
            .map(|f| f.id)
            .expect("item-collection parent must declare a same-name sort field (R2.4)");

        let child_name = target_root.name.upper_camel_case();

        if self.insert_scope {
            // Distribute parent's partition AND sort into the child row via
            // two IN-subquery conjuncts. The scope walker (`insert.rs`)
            // routes both through `apply_eq_constraint`, writing the
            // parent's literals into the child row's matching slots; the
            // child sort-key strategy then re-encodes the sk slot.
            let mut partition_source = (*association.source).clone();
            partition_source.body.as_select_mut_unwrap().returning =
                stmt::Returning::Project(stmt::Expr::ref_self_field(parent_partition_field_id));

            let mut sort_source = *association.source;
            sort_source.body.as_select_mut_unwrap().returning =
                stmt::Returning::Project(stmt::Expr::ref_self_field(parent_sort_field_id));

            // STARTS_WITH on `<Child>#` stays as a (currently no-op) marker
            // for the walker; the encoding strategy is the source of truth.
            let prefix = format!("{child_name}#");

            stmt::Expr::and_from_vec(vec![
                stmt::Expr::in_subquery(
                    stmt::Expr::ref_self_field(child_partition),
                    partition_source,
                ),
                stmt::Expr::in_subquery(stmt::Expr::ref_self_field(child_sort), sort_source),
                stmt::Expr::starts_with(
                    stmt::Expr::ref_self_field(child_sort),
                    stmt::Value::String(prefix),
                ),
            ])
            .into()
        } else {
            // Read path. Extract the parent's partition + sort key shape
            // from the source filter so we can emit a precise
            // `partition = <literal> AND sort STARTS_WITH <derived prefix>`
            // shape that honours the hierarchical sort-key encoding.
            //
            // Two parent shapes are supported:
            //
            // 1. Literal handle (`Tenant::filter_by_account_and_sk(...).users()`):
            //    source carries `account = <lit> AND sk = <lit>`.
            // 2. Cascade-synthesized source (`acme.delete()` recurses through
            //    User to Todo): source carries `account = <lit> AND sk
            //    STARTS_WITH <lit>` — the post-rewrite shape of an outer
            //    HasItems delete acting as the inner cascade's parent.
            //
            // Filtered cascades (e.g. `acme.users().filter(name = ...).delete()`)
            // are explicitly rejected: the syntactic prefix-swap below would
            // silently drop the `name = ...` constraint and over-delete.
            // See plan follow-up "Filtered cascades through IC chains".
            let select = association.source.body.as_select_unwrap();
            let filter_expr = select.filter.expr.as_ref().unwrap_or_else(|| {
                panic!(
                    "HasItems read source query has no filter; cannot extract parent literals \
                     for hierarchical prefix derivation"
                )
            });
            let parent_partition_value = super::insert::find_eq_value_for_field(
                filter_expr,
                parent_partition_field_id.index,
            )
            .unwrap_or_else(|| {
                panic!(
                    "could not extract parent partition literal from HasItems source; \
                     non-literal parent handles are not yet supported"
                )
            });

            let parent_sort_str = if let Some(stmt::Value::String(s)) =
                super::insert::find_eq_value_for_field(filter_expr, parent_sort_field_id.index)
            {
                ParentSortShape::Eq(s)
            } else if let Some(s) = super::insert::find_starts_with_prefix_for_field(
                filter_expr,
                parent_sort_field_id.index,
            ) {
                ParentSortShape::StartsWith(s)
            } else {
                panic!(
                    "could not extract parent sort literal from HasItems source; \
                     non-literal parent handles are not yet supported"
                )
            };

            assert_no_extra_conjuncts(
                filter_expr,
                parent_partition_field_id.index,
                parent_sort_field_id.index,
            );

            let derived_prefix = derive_child_prefix(&child_name, &parent_sort_str);

            stmt::Expr::and(
                stmt::Expr::eq(
                    stmt::Expr::ref_self_field(child_partition),
                    stmt::Expr::Value(parent_partition_value),
                ),
                stmt::Expr::starts_with(
                    stmt::Expr::ref_self_field(child_sort),
                    stmt::Value::String(derived_prefix),
                ),
            )
            .into()
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
        // naturally because the recursion re-examines the spliced steps. A
        // scalar-terminal via contributes only its relation chain to the
        // reachability filter — the terminal field is a projection, handled by
        // the query's returning, not the filter.
        let via_path = match &field.ty {
            app::FieldTy::Via(via) => {
                let projection = via.path.projection.as_slice();
                Some(match via.terminal {
                    Some(_) => &projection[..projection.len() - 1],
                    None => projection,
                })
            }
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
            app::FieldTy::Has(rel) => rel.target,
            app::FieldTy::Via(rel) => rel.target,
            app::FieldTy::BelongsTo(rel) => rel.target,
            app::FieldTy::HasItems(_) | app::FieldTy::ItemParent(_) => {
                unreachable!("IC relations are not via steps")
            }
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

/// Shape of the parent's sort-key constraint extracted from a HasItems
/// read-scope source filter. See [`RewriteVia::rewrite_association_has_items_as_filter`].
enum ParentSortShape {
    /// Parent is pinned to a fully qualified sk: `Tenant::filter_by_account_and_sk(...)`.
    /// Value is the full literal, e.g. `"User#alice"`.
    Eq(String),
    /// Parent is bounded by a sk prefix: cascade synthesis lands here.
    /// Value is the prefix literal, e.g. `"User#"` or `"Tenant#"`.
    StartsWith(String),
}

/// Derive the child's STARTS_WITH prefix from the parent's sk shape.
///
/// `Eq("Tenant#")`         → `"User#"`
/// `Eq("User#<u-uuid>")`   → `"Todo#<u-uuid>#"`
/// `StartsWith("User#")`   → `"Todo#"`
///
/// `StartsWith` with a non-empty parent_chain (e.g. `"User#<u-uuid>#"`) is
/// rejected: a syntactic prefix-swap would emit `"Todo#<u-uuid>##"` (double
/// `#`). Real cascades do not produce that shape today (3-tier max), so panic
/// rather than silently miscompose.
fn derive_child_prefix(child_name: &str, shape: &ParentSortShape) -> String {
    let parent_str = match shape {
        ParentSortShape::Eq(s) | ParentSortShape::StartsWith(s) => s.as_str(),
    };
    let split = parent_str
        .find('#')
        .unwrap_or_else(|| panic!("parent sk `{parent_str}` must carry a `<Prefix>#…` shape"));
    let parent_chain = &parent_str[split + 1..];
    match shape {
        ParentSortShape::Eq(_) if parent_chain.is_empty() => format!("{child_name}#"),
        ParentSortShape::Eq(_) => format!("{child_name}#{parent_chain}#"),
        ParentSortShape::StartsWith(_) if parent_chain.is_empty() => format!("{child_name}#"),
        ParentSortShape::StartsWith(_) => panic!(
            "non-empty parent_chain on a STARTS_WITH parent source is not yet supported \
             (would produce a malformed double-`#` prefix); parent shape `{parent_str}`"
        ),
    }
}

/// Reject parent source filters that carry conjuncts beyond the IC bounds
/// the rewriter understands. The accepted shape is a flat AND of:
///
///   * `account = <literal>`           (parent partition; required)
///   * `sk = <literal>` *or* `sk STARTS_WITH <literal>`   (parent sort; required)
///
/// plus optionally a duplicate `sk STARTS_WITH "<Parent>#"` discriminator that
/// the simplifier may not have collapsed (defensive — should be deduped
/// upstream by `prune_starts_with_subsumed_by_eq` /
/// `prune_starts_with_subsumed_by_starts_with`).
///
/// Anything else (e.g. `name = "Alice"`) is a filtered cascade. Panic with
/// a clear message; see the plan follow-up "Filtered cascades through IC
/// chains" for the design discussion.
fn assert_no_extra_conjuncts(
    filter_expr: &stmt::Expr,
    parent_partition_index: usize,
    parent_sort_index: usize,
) {
    let operands: &[stmt::Expr] = match filter_expr {
        stmt::Expr::And(and) => &and.operands,
        single => std::slice::from_ref(single),
    };
    for operand in operands {
        if is_recognized_ic_conjunct(operand, parent_partition_index, parent_sort_index) {
            continue;
        }
        panic!(
            "HasItems read source carries an unrecognized conjunct: {operand:#?} — \
             filtered cascades through IC chains are not yet supported. \
             See plan follow-up 'Filtered cascades through IC chains'."
        );
    }
}

fn is_recognized_ic_conjunct(
    expr: &stmt::Expr,
    parent_partition_index: usize,
    parent_sort_index: usize,
) -> bool {
    match expr {
        // `account = <literal>` or `sk = <literal>` (commuted forms allowed).
        stmt::Expr::BinaryOp(e) if e.op.is_eq() => {
            let (field_idx, value_side_ok) = match (&*e.lhs, &*e.rhs) {
                (
                    stmt::Expr::Reference(stmt::ExprReference::Field { nesting: 0, index }),
                    stmt::Expr::Value(_),
                )
                | (
                    stmt::Expr::Value(_),
                    stmt::Expr::Reference(stmt::ExprReference::Field { nesting: 0, index }),
                ) => (*index, true),
                _ => return false,
            };
            value_side_ok && (field_idx == parent_partition_index || field_idx == parent_sort_index)
        }
        // `sk STARTS_WITH <literal>` — both the rewritten sort filter and the
        // discriminator land here.
        stmt::Expr::StartsWith(sw) => matches!(
            (&*sw.expr, &*sw.prefix),
            (
                stmt::Expr::Reference(stmt::ExprReference::Field { nesting: 0, index }),
                stmt::Expr::Value(stmt::Value::String(_)),
            ) if *index == parent_sort_index
        ),
        _ => false,
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
