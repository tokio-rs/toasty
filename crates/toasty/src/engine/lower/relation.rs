use toasty_core::{
    schema::app::{self, Field, FieldId, FieldTy},
    stmt,
};

use crate::engine::lower::LowerStatement;

#[derive(Debug)]
enum Mutation {
    /// An existing value is being associated with the relation source.
    Associate {
        // Target relation expression
        expr: stmt::Expr,

        // When true, any previous values associated with the source relation
        // need to be cleared.
        exclusive: bool,
    },

    /// Disassociate an existing relation
    Disassociate {
        /// Target relation expression
        expr: stmt::Expr,
    },

    /// Disassociate any target relations
    DisassociateAll {
        // True when disasociating before deleting a record.
        delete: bool,
    },
}

trait RelationSource: std::fmt::Debug {
    /// Return a query representing the source selection
    fn selection(&self, nesting: usize) -> stmt::Query;

    /// Update a source field expression
    fn set_source_field(&mut self, field: FieldId, expr: stmt::Expr);

    /// Update a returning field expression
    fn set_returning_field(&mut self, field: FieldId, expr: stmt::Expr);

    /// Whether the source might produce zero rows. When true, relation
    /// mutations must be wrapped in a conditional to avoid FK updates when
    /// the source filter doesn't match.
    fn needs_existence_check(&self) -> bool;
}

#[derive(Debug)]
struct InsertRelationSource<'a> {
    model: &'a app::ModelRoot,
    row: &'a mut stmt::Expr,
    /// The index in stmt::Returning that represents the row
    index: usize,
    returning: &'a mut Option<stmt::Returning>,
}

#[derive(Debug)]
struct UpdateRelationSource<'a> {
    model: &'a app::ModelRoot,
    filter: &'a stmt::Filter,
    assignments: &'a mut stmt::Assignments,
    returning: &'a mut Option<stmt::Returning>,
    returning_changed: bool,
}

impl LowerStatement<'_, '_> {
    pub(super) fn plan_stmt_delete_relations(&mut self, mut stmt: &stmt::Delete) {
        // Cascading deletes are only handled at the application level
        let Some(model) = self.expr_cx.target().as_model() else {
            return;
        };

        // Handle any cascading deletes
        for field in model.fields.iter() {
            self.plan_mut_relation_field(
                field,
                Mutation::DisassociateAll { delete: true },
                &mut stmt,
            );
        }
    }

    pub(super) fn plan_stmt_insert_relations(
        &mut self,
        row: &mut stmt::Expr,
        returning: &mut Option<stmt::Returning>,
        index: usize,
    ) {
        let model = self.expr_cx.target().as_model_unwrap();

        for (i, field) in model.fields.iter().enumerate() {
            if field.is_relation() {
                let expr = row.entry_mut(i).take();

                if expr.is_value_null() || expr.is_default() {
                    if !field.nullable && field.ty.is_has_one() {
                        panic!(
                            "Insert missing non-nullable field; model={}; name={:#?}; ty={:#?}; expr={:#?}",
                            model.name.upper_camel_case(),
                            field.name,
                            field.ty,
                            expr
                        );
                    }

                    continue;
                }

                self.plan_mut_relation_field(
                    field,
                    Mutation::Associate {
                        expr,
                        exclusive: field.ty.is_belongs_to(),
                    },
                    &mut InsertRelationSource {
                        model,
                        row,
                        returning,
                        index,
                    },
                );
            }
        }
    }

    pub(super) fn plan_stmt_update_relations(
        &mut self,
        assignments: &mut stmt::Assignments,
        filter: &stmt::Filter,
        returning: &mut Option<stmt::Returning>,
        returning_changed: bool,
    ) {
        let model = self.expr_cx.target().as_model_unwrap();

        for (i, field) in model.fields.iter().enumerate() {
            if !field.is_relation() {
                continue;
            }

            let Some(assignment) = assignments.take(&[i][..]) else {
                continue;
            };

            let mutation = match assignment {
                stmt::Assignment::Set(expr) => {
                    if expr.is_value_null() {
                        Mutation::DisassociateAll { delete: false }
                    } else {
                        Mutation::Associate {
                            expr,
                            exclusive: true,
                        }
                    }
                }
                stmt::Assignment::Insert(expr) => {
                    assert!(field.ty.is_has_many());
                    debug_assert!(!expr.is_value_null());
                    Mutation::Associate {
                        expr,
                        exclusive: false,
                    }
                }
                stmt::Assignment::Remove(expr) => {
                    assert!(field.ty.is_has_many());
                    debug_assert!(!expr.is_value_null());
                    Mutation::Disassociate { expr }
                }
                stmt::Assignment::Batch(_) => {
                    todo!("batch assignments for relations")
                }
            };

            self.plan_mut_relation_field(
                field,
                mutation,
                &mut UpdateRelationSource {
                    model,
                    filter,
                    assignments: &mut *assignments,
                    returning,
                    returning_changed,
                },
            );
        }
    }

    fn plan_mut_relation_field(
        &mut self,
        field: &app::Field,
        op: Mutation,
        source: &mut dyn RelationSource,
    ) {
        match &field.ty {
            FieldTy::HasOne(..) => {
                self.relation_step(field, |lower| lower.plan_mut_has_one(field, op, source));
            }
            FieldTy::HasMany(..) => {
                self.relation_step(field, |lower| lower.plan_mut_has_many(field, op, source));
            }
            FieldTy::BelongsTo(_) => {
                self.plan_mut_belongs_to(field, op, source);
            }
            _ => (),
        }
    }

    fn plan_mut_has_many(&mut self, field: &Field, op: Mutation, source: &mut dyn RelationSource) {
        let has_many = field.ty.as_has_many_unwrap();
        let pair = self.field(has_many.pair);

        self.plan_mut_has_n(field, pair, op, source);
    }

    fn plan_mut_has_one(&mut self, field: &Field, op: Mutation, source: &mut dyn RelationSource) {
        let has_one = field.ty.as_has_one_unwrap();
        let pair = self.field(has_one.pair);

        self.plan_mut_has_n(field, pair, op, source);
    }

    fn plan_mut_has_n(
        &mut self,
        field: &Field,
        pair: &Field,
        op: Mutation,
        source: &mut dyn RelationSource,
    ) {
        match op {
            Mutation::DisassociateAll { .. } => {
                self.plan_mut_has_n_disassociate_all(pair, source);
            }
            Mutation::Associate { expr, exclusive } => {
                let deps = self.collect_dependencies(|lower| {
                    if exclusive {
                        lower.plan_mut_has_n_disassociate_all(pair, source);
                    }
                });

                self.with_dependencies(deps, |lower| {
                    lower.plan_mut_has_n_associate_expr(field, pair, expr, source);
                });
            }
            Mutation::Disassociate { expr } => {
                debug_assert!(field.ty.is_has_many());
                self.plan_mut_has_many_disassociate_expr(field, pair, expr, source);
            }
        }
    }

    fn plan_mut_has_n_associate_expr(
        &mut self,
        field: &Field,
        pair: &Field,
        expr: stmt::Expr,
        source: &mut dyn RelationSource,
    ) {
        match expr {
            stmt::Expr::List(expr_list) => {
                for expr in expr_list.items {
                    self.plan_mut_has_n_associate_expr(field, pair, expr, source);
                }
            }
            stmt::Expr::Stmt(expr_stmt) => {
                self.plan_mut_has_n_associate_stmt(field, pair, *expr_stmt.stmt, source);
            }
            stmt::Expr::Value(stmt::Value::List(value_list)) => {
                for value in value_list {
                    self.plan_mut_has_n_associate_value(pair, value, source);
                }
            }
            stmt::Expr::Value(value) => {
                self.plan_mut_has_n_associate_value(pair, value, source);
            }
            _ => todo!("field={field:#?}, expr={expr:#?}"),
        }
    }

    fn plan_mut_has_n_associate_value(
        &mut self,
        pair: &Field,
        value: stmt::Value,
        source: &mut dyn RelationSource,
    ) {
        assert!(!value.is_list());

        let mut stmt = stmt::Query::new_select(
            pair.id.model,
            stmt::Expr::eq(stmt::Expr::ref_ancestor_model(0), value),
        )
        .update();

        stmt.assignments
            .set(pair.id, stmt::Expr::stmt(source.selection(2)));

        // Needed for the existence check. Only update *if* the relation source
        // actually exists to be updated.
        if source.needs_existence_check() {
            stmt.filter.set(stmt::Expr::exists({
                let mut query = source.selection(2);
                let stmt::ExprSet::Select(select) = &mut query.body else {
                    todo!()
                };
                select.returning = stmt::Expr::record([1]).into();
                query
            }));
        }

        self.new_dependency(stmt);
    }

    fn plan_mut_has_many_disassociate_expr(
        &mut self,
        field: &Field,
        pair: &Field,
        expr: stmt::Expr,
        source: &dyn RelationSource,
    ) {
        match expr {
            stmt::Expr::Value(value) => {
                self.plan_mut_has_many_disassociate_value(pair, value, source)
            }
            _ => todo!("field={field:#?}; expr={expr:#?}"),
        }
    }

    fn plan_mut_has_many_disassociate_value(
        &mut self,
        pair: &Field,
        value: stmt::Value,
        source: &dyn RelationSource,
    ) {
        let selection = stmt::Query::new_select(
            pair.id.model,
            stmt::Expr::eq(stmt::Expr::ref_ancestor_model(0), value),
        );

        if pair.nullable {
            let mut stmt = selection.update();

            stmt.condition = stmt::Condition::new(self.relation_pair_filter(pair.id, source));
            stmt.assignments.set(pair, stmt::Value::Null);
            self.new_dependency(stmt);
        } else {
            self.new_dependency(selection.delete());
        }
    }

    fn plan_mut_has_n_disassociate_all(&mut self, pair: &Field, source: &dyn RelationSource) {
        let query = self.relation_pair_scope(pair.id, source);

        if pair.nullable {
            let mut update = query.update();
            update.assignments.set(pair.id, stmt::Value::Null);

            self.new_dependency(update);
        } else {
            self.new_dependency(query.delete());
        }
    }

    fn plan_mut_has_n_associate_stmt(
        &mut self,
        field: &Field,
        pair: &Field,
        stmt: stmt::Statement,
        source: &mut dyn RelationSource,
    ) {
        match stmt {
            stmt::Statement::Insert(stmt) => {
                self.plan_mut_has_n_associate_insert(field, pair, stmt, source)
            }
            _ => todo!("stmt={:#?}", stmt),
        }
    }

    fn plan_mut_has_n_associate_insert(
        &mut self,
        _field: &Field,
        pair: &Field,
        mut stmt: stmt::Insert,
        source: &mut dyn RelationSource,
    ) {
        debug_assert_eq!(stmt.target.model_id_unwrap(), pair.id.model);
        debug_assert!(stmt.target.is_model());

        stmt.target = self.relation_pair_scope(pair.id, source).into();

        // has_many fields represent collections and must return lists for type
        // consistency, even when inserting a single record. The insert may arrive
        // with `single = true` (semantically inserting one record into the set),
        // but we set it to false to ensure the result is wrapped in a list.
        if _field.ty.is_has_many() {
            stmt.source.single = false;
        }

        self.state.engine.simplify_stmt(&mut stmt);
        source.set_returning_field(_field.id, stmt.into());
    }

    fn plan_mut_belongs_to(
        &mut self,
        field: &Field,
        op: Mutation,
        source: &mut dyn RelationSource,
    ) {
        // BelongsTo doesn't create child inserts, so `then` is unused
        match op {
            Mutation::Associate { expr, .. } => {
                self.plan_mut_belongs_to_associate(field, expr, source)
            }
            Mutation::DisassociateAll { delete } => {
                self.plan_mut_belongs_to_disassociate(field, delete, source);
            }
            Mutation::Disassociate { .. } => {
                todo!("is this needed?")
            }
        }
    }

    fn plan_mut_belongs_to_disassociate(
        &mut self,
        field: &Field,
        delete: bool,
        source: &mut dyn RelationSource,
    ) {
        if !field.nullable && !delete {
            todo!("invalid statement. handle this case");
        }

        let belongs_to = field.ty.as_belongs_to_unwrap();

        if let Some(pair_id) = belongs_to.pair {
            let pair = self.field(pair_id);

            if pair.ty.is_has_one() && !pair.nullable {
                self.relation_step(field, |planner| {
                    let delete = planner.relation_pair_scope(pair.id, source).delete();
                    planner.new_dependency(delete);
                });
            }
        }

        if !delete {
            for fk_field in &belongs_to.foreign_key.fields {
                source.set_source_field(fk_field.source, stmt::Expr::null());
            }
        }
    }

    fn plan_mut_belongs_to_associate(
        &mut self,
        field: &Field,
        expr: stmt::Expr,
        source: &mut dyn RelationSource,
    ) {
        let dependencies = self.collect_dependencies(|lower| {
            if let Some(pair_id) = field.pair() {
                if lower.field(pair_id).ty.is_has_one() {
                    // Disassociate an existing HasOne. This handles the case where
                    // if the HasOne association is required (i.e. *not* `Option`),
                    // then the record gets deleted.
                    lower.plan_mut_belongs_to_disassociate(field, false, source);

                    // This handles disassociating any *other* instances of the current
                    // model that already are associated with the target model being
                    // passed it. This is necessary because of the 1-1 relation
                    // mapping.
                    if field.nullable {
                        lower.relation_step(field, |planner| {
                            assert!(expr.is_value());

                            let scope = stmt::Query::new_select(
                                field.id.model,
                                stmt::Expr::eq(stmt::Expr::ref_self_field(field), expr.clone()),
                            );

                            if field.nullable {
                                let mut stmt = scope.update();
                                stmt.assignments.set(field.id, stmt::Value::Null);
                                planner.new_dependency(stmt);
                            } else {
                                todo!();
                            }
                        });
                    } else {
                        todo!()
                    }
                }
            }
        });

        self.with_dependencies(dependencies, |lower| match expr {
            expr if expr.is_value() || expr.is_expr_reference() => {
                assert!(!expr.is_value_null());

                lower.set_relation_field(field, expr, source);
            }
            stmt::Expr::Stmt(expr_stmt) => {
                lower.plan_mut_belongs_to_associate_stmt(field, *expr_stmt.stmt, source);
            }
            _ => todo!("field={field:#?}; expr={expr:#?}"),
        });
    }

    fn plan_mut_belongs_to_associate_stmt(
        &mut self,
        field: &Field,
        stmt: stmt::Statement,
        source: &mut dyn RelationSource,
    ) {
        let belongs_to = field.ty.as_belongs_to_unwrap();

        match stmt {
            stmt::Statement::Insert(mut insert) => {
                debug_assert!(insert.source.single);

                if let stmt::ExprSet::Values(values) = &insert.source.body {
                    assert_eq!(1, values.rows.len());
                }

                // Only returning that makes sense here as that is the type that
                // "belongs" in this field. We translate it to the key to set
                // the FK fields in the source model.
                assert!(matches!(
                    insert.returning,
                    Some(stmt::Returning::Model { .. })
                ));

                // Previous value of returning does nothing in this
                // context
                insert.returning = Some(
                    stmt::Expr::record(
                        belongs_to
                            .foreign_key
                            .fields
                            .iter()
                            .map(|fk_field| stmt::Expr::ref_self_field(fk_field.target)),
                    )
                    .into(),
                );

                let target_id = self.new_dependency(insert);
                let stmt_info = &self.state.hir[target_id];

                let returning = stmt_info.stmt.as_ref().unwrap().returning().expect("bug");

                let expr = match returning {
                    stmt::Returning::Value(expr) if expr.is_const() => expr.clone(),
                    _ => {
                        // Make sure the source statement returns a single record
                        debug_assert!(match &**stmt_info.stmt.as_ref().unwrap() {
                            stmt::Statement::Insert(i) => i.source.single,
                            stmt::Statement::Query(stmt) => stmt.single,
                            stmt => todo!("stmt={stmt:#?}"),
                        });
                        // The result dependency is needed to get the foreign key.
                        self.new_dependency_arg(self.scope_stmt_id(), target_id)
                    }
                };

                self.set_relation_field(field, expr, source);
            }
            stmt::Statement::Query(query) => {
                // Try to extract the FK from the select without performing the query
                let fields: Vec<_> = belongs_to
                    .foreign_key
                    .fields
                    .iter()
                    .map(|fk_field| fk_field.target)
                    .collect();

                let Some(expr) = self.extract_key_expr(&fields, &query) else {
                    todo!("belongs_to={:#?}; stmt={:#?}", belongs_to, query);
                };

                self.plan_mut_belongs_to_associate(field, expr, source);
            }
            _ => todo!("stmt={:#?}", stmt),
        }
    }

    /// Extract constant key values from a subquery filter so the subquery can
    /// be eliminated.
    ///
    /// Given a query like `SELECT User WHERE id = 123 AND name = "foo"` and
    /// key fields `[id]`, this extracts `123`. For composite keys
    /// `[id, org_id]` it extracts a record `(123, 456)`.
    ///
    /// Recursively walks `And` nodes collecting equality constraints for each
    /// key field. Does not descend into `Or` or other non-conjunctive nodes.
    fn extract_key_expr(&self, key: &[FieldId], query: &stmt::Query) -> Option<stmt::Expr> {
        let cx = self.expr_cx.scope(query);

        let stmt::ExprSet::Select(select) = &query.body else {
            return None;
        };

        let mut values: Vec<Option<stmt::Expr>> = vec![None; key.len()];
        Self::collect_key_constraints(&cx, key, select.filter.as_expr(), &mut values);

        // All key fields must have been matched
        let values: Vec<stmt::Expr> = values.into_iter().collect::<Option<Vec<_>>>()?;

        if values.len() == 1 {
            Some(values.into_iter().next().unwrap())
        } else {
            Some(stmt::Expr::record(values))
        }
    }

    /// Recursively walk a filter expression collecting equality constraints
    /// for key fields. Only descends into conjunctive (`And`) nodes.
    fn collect_key_constraints(
        cx: &stmt::ExprContext,
        key: &[FieldId],
        filter: &stmt::Expr,
        values: &mut [Option<stmt::Expr>],
    ) {
        match filter {
            stmt::Expr::And(and) => {
                for operand in &and.operands {
                    Self::collect_key_constraints(cx, key, operand, values);
                }
            }
            stmt::Expr::BinaryOp(binary) => {
                Self::try_match_key_eq(cx, key, binary, values);
            }
            // Or, Not, etc. — don't walk into these; they don't guarantee
            // a single constant value for the key field.
            _ => {}
        }
    }

    /// If `binary` is an equality involving one of the key fields, record
    /// the matched value.
    fn try_match_key_eq(
        cx: &stmt::ExprContext,
        key: &[FieldId],
        binary: &stmt::ExprBinaryOp,
        values: &mut [Option<stmt::Expr>],
    ) {
        if !binary.op.is_eq() {
            return;
        }

        for (i, key_field) in key.iter().enumerate() {
            if values[i].is_some() {
                continue;
            }
            if let Some(expr) = Self::extract_eq_value(cx, *key_field, binary) {
                values[i] = Some(expr);
                return;
            }
        }
    }

    /// Extract the value for `key_field` from a single equality expression.
    fn extract_eq_value(
        cx: &stmt::ExprContext,
        key_field: FieldId,
        binary: &stmt::ExprBinaryOp,
    ) -> Option<stmt::Expr> {
        debug_assert!(binary.op.is_eq());

        match (&*binary.lhs, &*binary.rhs) {
            // Inner field ref (nesting=0) matched against outer field ref (nesting>0)
            (
                stmt::Expr::Reference(inner @ stmt::ExprReference::Field { nesting: 0, .. }),
                stmt::Expr::Reference(outer @ stmt::ExprReference::Field { nesting, .. }),
            )
            | (
                stmt::Expr::Reference(outer @ stmt::ExprReference::Field { nesting, .. }),
                stmt::Expr::Reference(inner @ stmt::ExprReference::Field { nesting: 0, .. }),
            ) if *nesting > 0 => {
                let field_ref = cx.resolve_expr_reference(inner).as_field_unwrap();
                if key_field == field_ref.id {
                    let mut ret = *outer;
                    let stmt::ExprReference::Field { nesting, .. } = &mut ret else {
                        panic!()
                    };
                    debug_assert!(*nesting > 0);
                    *nesting -= 1;
                    Some(ret.into())
                } else {
                    None
                }
            }
            // Both nesting=0 refs — not handled
            (stmt::Expr::Reference(_), stmt::Expr::Reference(_)) => None,
            // Field ref matched against a constant value
            (stmt::Expr::Reference(expr_ref), other) | (other, stmt::Expr::Reference(expr_ref)) => {
                let field_ref = cx.resolve_expr_reference(expr_ref).as_field_unwrap();
                if key_field == field_ref.id {
                    if let stmt::Expr::Value(value) = other {
                        Some(value.clone().into())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Translate a source model scope to a target model scope for a has_one
    /// relation.
    fn relation_pair_scope(&self, pair: FieldId, source: &dyn RelationSource) -> stmt::Query {
        stmt::Query::new_select(pair.model, self.relation_pair_filter(pair, source))
    }

    fn relation_pair_filter(&self, pair: FieldId, source: &dyn RelationSource) -> stmt::Expr {
        stmt::Expr::in_subquery(stmt::Expr::ref_self_field(pair), source.selection(1))
    }

    fn relation_step(&mut self, field: &Field, f: impl FnOnce(&mut LowerStatement)) {
        if let Some(pair) = field.pair() {
            if self.state.relations.last().copied() == Some(pair) {
                return;
            }
        }

        self.state.relations.push(field.id);
        f(self);
        self.state.relations.pop();
    }

    fn set_relation_field(&self, field: &Field, expr: stmt::Expr, source: &mut dyn RelationSource) {
        let app::FieldTy::BelongsTo(belongs_to) = &field.ty else {
            todo!("field={field:#?}")
        };

        let fk_fields = &belongs_to.foreign_key.fields;

        if let Some(len) = expr.record_len() {
            assert_eq!(len, fk_fields.len(), "expr={expr:#?}");
            let fk_values = expr.into_record_items().unwrap();

            for (fk_field, fk_value) in fk_fields.iter().zip(fk_values) {
                source.set_source_field(fk_field.source, fk_value);
            }
        } else {
            match expr {
                stmt::Expr::Arg(_) => {
                    for (i, fk_field) in fk_fields.iter().enumerate() {
                        source.set_source_field(
                            fk_field.source,
                            stmt::Expr::project(expr.clone(), [i]),
                        );
                    }
                }
                stmt::Expr::Value(_) | stmt::Expr::Reference(_) => {
                    let [fk_field] = &fk_fields[..] else { todo!() };

                    source.set_source_field(fk_field.source, expr);
                }
                expr => todo!("expr={expr:#?}"),
            }
        }
    }
}

impl RelationSource for &stmt::Delete {
    fn selection(&self, nesting: usize) -> stmt::Query {
        assert_eq!(nesting, 1, "TODO");
        stmt::Delete::selection(self)
    }

    fn set_source_field(&mut self, _field: FieldId, _expr: stmt::Expr) {
        unimplemented!("delete statements do not need to update field values");
    }

    fn set_returning_field(&mut self, _field: FieldId, _expr: stmt::Expr) {
        unimplemented!("delete statements do not need to update field values");
    }

    fn needs_existence_check(&self) -> bool {
        false
    }
}

impl RelationSource for UpdateRelationSource<'_> {
    fn selection(&self, _nesting: usize) -> stmt::Query {
        // In this context, the nesting does not matter. The filter entirely
        // references the returned query.
        stmt::Query::new_select(self.model, self.filter.clone())
    }

    fn set_source_field(&mut self, field: FieldId, expr: stmt::Expr) {
        self.assignments.set(field, expr);
    }

    fn set_returning_field(&mut self, field: FieldId, expr: stmt::Expr) {
        debug_assert!(self.returning_changed, "TODO");

        let Some(stmt::Returning::Expr(stmt::Expr::Cast(expr_cast))) = self.returning else {
            todo!("UpdateRelationSource={self:#?}")
        };

        let stmt::Type::SparseRecord(path_field_set) = &mut expr_cast.ty else {
            todo!("expr={expr:#?}")
        };

        let position = path_field_set
            .iter()
            .position(|field_id| field_id == field.index)
            .unwrap();

        let stmt::Expr::Record(record) = &mut *expr_cast.expr else {
            todo!()
        };

        set_returning_slot(record, position, expr);
    }

    fn needs_existence_check(&self) -> bool {
        true
    }
}

impl RelationSource for InsertRelationSource<'_> {
    fn selection(&self, nesting: usize) -> stmt::Query {
        let mut args = vec![];

        for pk_field in self.model.primary_key_fields() {
            let entry = self.row.entry(pk_field.id.index).unwrap();

            if entry.is_value() {
                args.push(entry.to_expr());
            } else if entry.is_expr_default() {
                args.push(stmt::Expr::ref_field(nesting, pk_field.id))
            } else {
                todo!("{entry:#?}");
            }
        }

        self.model.find_by_id(&args[..])
    }

    fn set_source_field(&mut self, field: FieldId, expr: stmt::Expr) {
        assert_eq!(self.model.id, field.model);
        self.row.as_record_mut_unwrap()[field.index] = expr;
    }

    fn set_returning_field(&mut self, field: FieldId, expr: stmt::Expr) {
        let record = match self.returning {
            Some(stmt::Returning::Expr(stmt::Expr::Record(record))) => record,
            Some(stmt::Returning::Value(stmt::Expr::List(rows))) => {
                rows.items[self.index].as_record_mut_unwrap()
            }
            Some(stmt::Returning::Value(stmt::Expr::Record(record))) => record,
            _ => todo!("InsertRelationSource={self:#?}"),
        };

        set_returning_slot(record, field.index, expr);
    }

    fn needs_existence_check(&self) -> bool {
        false
    }
}

fn set_returning_slot(record: &mut stmt::ExprRecord, index: usize, expr: stmt::Expr) {
    assert!(
        record.fields[index].is_value_null(),
        "TODO: probably need to merge instead of overwrite; actual={:#?}",
        record.fields[index]
    );
    record.fields[index] = expr;
}
