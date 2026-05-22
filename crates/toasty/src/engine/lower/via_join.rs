//! Lowering for `.include()` / `.select()` of multi-step (`via`) relations.
//!
//! A `via` relation reaches its target through a path of existing relations.
//! [`ViaJoin`] resolves that path into a single JOIN from the target back to
//! the root so the engine can issue one query per include and group children
//! with their parent in `NestedMerge`. This relies on the database executing
//! the join, so it is SQL-only — a key-value backend would need a cascade of
//! per-step queries instead.

use toasty_core::{
    schema::{app, mapping},
    stmt,
};

use crate::engine::lower::LowerStatement;

impl LowerStatement<'_, '_> {
    /// Build the include subquery for a multi-step (`via`) relation.
    ///
    /// The child query is rooted at the via target and joins back to the root
    /// through every intermediate model (see [`ViaJoin`]), projecting the
    /// linking foreign-key column so `NestedMerge` can group children by
    /// parent. It is emitted in fully-lowered form, so the standard lowering
    /// walk only has to rewrite the cross-statement parent-key reference into
    /// an `Arg::Ref`. Each child row is `[link_key, target_record]`; the
    /// trailing projection drops the link key so the parent sees only the
    /// targets (a `Map` over the list for `has_many`, a direct project for a
    /// single `has_one`).
    pub(super) fn build_via_include_subquery(
        &mut self,
        field_index: usize,
        via: &app::Via,
        nested: &[stmt::Projection],
    ) -> stmt::Expr {
        if !nested.is_empty() {
            todo!("nested `.include()` through a multi-step `via` relation");
        }

        let schema = self.schema();
        let model = self.model_unwrap();
        let single = !model.fields[field_index].ty.is_has_many();
        let nullable = model.fields[field_index].nullable();
        let join = ViaJoin::resolve(schema, model.id, via);

        // WHERE: the linking column (on the root-adjacent model) equals the
        // parent's referenced key. Use the field's model-level expression
        // (column ref + any storage→model cast) so it lines up type-wise with
        // the parent's lowered key both here and in the `NestedMerge` group
        // key.
        let (link_field, parent_key_field) = join.link();
        let link_col = model_level_column_expr(schema, link_field, join.slot(1));
        let filter = stmt::Expr::eq(link_col.clone(), stmt::Expr::ref_field(1, parent_key_field));

        // RETURNING `[link_col, target_record]`. The link column lands in
        // `load_data_select_items` so the qualification resolves to a
        // `SortLookup`; `target_record` is the schema's `default_returning`,
        // whose column refs already point at table slot 0 (the target).
        let target_record = schema.mapping_for(join.target()).default_returning.clone();
        let returning = stmt::Expr::record_from_vec(vec![link_col, target_record]);

        // `DISTINCT` collapses duplicate targets produced when the path fans
        // out (e.g. two comments on the same article) — matching a direct via
        // query's distinct-target semantics.
        let mut select = stmt::Select::new(join.build_source(schema), filter);
        select.returning = stmt::Returning::Project(returning);
        select.distinct = true;

        let mut query = stmt::Query::builder(select).build();
        query.single = single;

        // The query is already lowered, so this is mostly a no-op beyond
        // rewriting the parent-key `Reference::Field { nesting: 1 }` into an
        // `Arg::Ref`.
        let sub_expr = self.lower_sub_stmt(stmt::Statement::Query(query));

        // Drop the link key from each `[link_key, target_record]` row; the
        // parent wants only the target.
        if !single {
            // A `has_many` via yields a list, so map over it (`arg(0)` is the
            // item) and project the target out of each row.
            return stmt::Expr::map(sub_expr, stmt::Expr::project(stmt::Expr::arg(0), [1usize]));
        }

        // A single (`has_one`) via yields one `[link_key, target_record]`
        // record; project the target out. A nullable single relation, though,
        // produces `Null` when the `INNER JOIN` matched nothing, and projecting
        // into `Null` would panic — so strip the link key only on the non-null
        // branch.
        if nullable {
            super::map_nullable_single(sub_expr, stmt::Expr::project(stmt::Expr::arg(0), [1usize]))
        } else {
            stmt::Expr::project(sub_expr, [1usize])
        }
    }
}

/// A multi-step (`via`) relation resolved into a JOIN from the target back to
/// the root.
///
/// `models` is the path `[root, …intermediates, target]`; `edges[i]` is the
/// foreign key joining `models[i]` and `models[i + 1]`.
///
/// The child query lays its tables out target-first so the target's pre-built
/// `default_returning` (whose column refs point at slot 0) is reused verbatim:
///
/// ```text
///   slot 0       FROM   target
///   slot 1..     JOIN   intermediates, target-adjacent first
/// ```
///
/// The root is not a table — it is the parent query, reached through the WHERE
/// filter on `edges[0]`.
struct ViaJoin {
    models: Vec<app::ModelId>,
    edges: Vec<Edge>,
}

impl ViaJoin {
    fn resolve(schema: &toasty_core::Schema, root: app::ModelId, via: &app::Via) -> ViaJoin {
        let steps = flatten_via_steps(schema, root, via.path.projection.as_slice());
        assert!(
            !steps.is_empty(),
            "via path must have at least one step (validated at schema build time)"
        );

        let mut models = Vec::with_capacity(steps.len() + 1);
        let mut edges = Vec::with_capacity(steps.len());
        models.push(root);
        for &field_id in &steps {
            debug_assert_eq!(field_id.model, *models.last().unwrap());
            models.push(schema.app.field(field_id).relation_target_id().unwrap());
            edges.push(Edge::resolve(schema, field_id));
        }

        ViaJoin { models, edges }
    }

    /// The via target — the model whose rows the include loads.
    fn target(&self) -> app::ModelId {
        *self.models.last().unwrap()
    }

    /// Table slot for the model at chain position `pos` (`1..=edges.len()`).
    /// Tables are target-first, so the target (highest position) is slot 0 and
    /// the root-adjacent model (position 1) is the last slot.
    fn slot(&self, pos: usize) -> usize {
        self.models.len() - 1 - pos
    }

    /// The `(link, parent_key)` field pair: `link` lives on the root-adjacent
    /// model and matches `parent_key` on the root in the WHERE filter.
    fn link(&self) -> (app::FieldId, app::FieldId) {
        let root_edge = &self.edges[0];
        (root_edge.target_side, root_edge.root_side)
    }

    /// The `FROM target JOIN …intermediates` source.
    fn build_source(&self, schema: &toasty_core::Schema) -> stmt::Source {
        let mut tables = Vec::with_capacity(self.edges.len());
        let mut joins = Vec::with_capacity(self.edges.len().saturating_sub(1));

        tables.push(stmt::TableRef::Table(schema.table_id_for(self.target())));

        // Walk intermediates target-adjacent first so table slots increase
        // toward the root. `edges[pos]` joins this intermediate (`models[pos]`)
        // to its already-placed neighbour (`models[pos + 1]`).
        for pos in (1..self.edges.len()).rev() {
            tables.push(stmt::TableRef::Table(schema.table_id_for(self.models[pos])));

            let edge = &self.edges[pos];
            joins.push(stmt::Join {
                table: stmt::SourceTableId(self.slot(pos)),
                constraint: stmt::JoinOp::Inner(stmt::Expr::eq(
                    raw_column(schema, self.slot(pos), edge.root_side),
                    raw_column(schema, self.slot(pos + 1), edge.target_side),
                )),
            });
        }

        stmt::Source::Table(stmt::SourceTable {
            tables,
            from: vec![stmt::TableWithJoins {
                relation: stmt::TableFactor::Table(stmt::SourceTableId(0)),
                joins,
            }],
        })
    }
}

/// A foreign-key edge between two adjacent models on a via path, with the FK
/// field on each side resolved regardless of which side declared the relation.
struct Edge {
    /// FK field on the model nearer the root.
    root_side: app::FieldId,
    /// FK field on the model nearer the target.
    target_side: app::FieldId,
}

impl Edge {
    fn resolve(schema: &toasty_core::Schema, field_id: app::FieldId) -> Edge {
        let field = schema.app.field(field_id);

        // A has-relation is reached through its paired `BelongsTo`, which lives
        // on the target-side model and owns the FK; a `BelongsTo` step is the
        // edge itself, on the root-side model.
        let (belongs_to, owner_is_target_side) = match &field.ty {
            app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                let pair = field
                    .pair()
                    .expect("via paths are unfolded into direct steps before edge resolution");
                (schema.app.field(pair).ty.as_belongs_to_unwrap(), true)
            }
            app::FieldTy::BelongsTo(belongs_to) => (belongs_to, false),
            _ => unreachable!("via step is not a relation field"),
        };

        // The FK source lives on the BT owner, the target on the model it
        // references; map those onto root/target sides via the owner's side.
        let [fk] = &belongs_to.foreign_key.fields[..] else {
            todo!("composite foreign keys in via include path");
        };
        if owner_is_target_side {
            Edge {
                root_side: fk.target,
                target_side: fk.source,
            }
        } else {
            Edge {
                root_side: fk.source,
                target_side: fk.target,
            }
        }
    }
}

/// Walk a via path, inlining any `via` field's own resolved path so the
/// result is a flat sequence of direct relation `FieldId`s.
fn flatten_via_steps(
    schema: &toasty_core::Schema,
    source_model_id: app::ModelId,
    initial_steps: &[usize],
) -> Vec<app::FieldId> {
    let mut result = Vec::with_capacity(initial_steps.len());
    let mut current_model = source_model_id;
    let mut queue: Vec<usize> = initial_steps.to_vec();
    queue.reverse(); // pop from the back

    while let Some(idx) = queue.pop() {
        let field = &schema.app.model(current_model).as_root_unwrap().fields[idx];
        let field_id = app::FieldId {
            model: current_model,
            index: idx,
        };

        // If this step itself names a `via` relation, splice the nested
        // path in place of it and continue (handles via-of-via naturally).
        let nested_via = match &field.ty {
            app::FieldTy::HasMany(rel) => rel.kind.via(),
            app::FieldTy::HasOne(rel) => rel.kind.via(),
            _ => None,
        };
        if let Some(via) = nested_via {
            for step in via.path.projection.as_slice().iter().rev() {
                queue.push(*step);
            }
            continue;
        }

        current_model = field
            .relation_target_id()
            .expect("via path step is a relation");
        result.push(field_id);
    }

    result
}

/// The single-column mapping for a foreign-key field. Via include paths only
/// resolve FK source/target fields, which the schema guarantees are primitive.
fn fk_primitive(schema: &toasty_core::Schema, field_id: app::FieldId) -> &mapping::FieldPrimitive {
    schema.mapping_for(field_id.model).fields[field_id.index]
        .as_primitive()
        .expect("FK field maps to a single column")
}

/// A raw (storage-level) column reference at table `slot` for a FK field.
/// Used in JOIN constraints, which compare stored values directly and so need
/// no storage→model cast.
fn raw_column(schema: &toasty_core::Schema, slot: usize, field_id: app::FieldId) -> stmt::Expr {
    stmt::Expr::column(stmt::ExprReference::column(
        slot,
        fk_primitive(schema, field_id).column.index,
    ))
}

/// The model-level expression for a FK field — its column reference wrapped in
/// the storage→model cast when the storage type differs (e.g. `Uuid` stored as
/// `Bytes`) — re-pointed at table `slot`.
///
/// `FieldPrimitive::column_expr` is the schema's pre-built table→model
/// expression at slot 0; we rewrite each column ref's slot.
fn model_level_column_expr(
    schema: &toasty_core::Schema,
    field_id: app::FieldId,
    slot: usize,
) -> stmt::Expr {
    let mut expr = fk_primitive(schema, field_id).column_expr.clone();

    stmt::visit_mut::for_each_expr_mut(&mut expr, |e| {
        if let stmt::Expr::Reference(stmt::ExprReference::Column(col)) = e {
            col.table = slot;
        }
    });

    expr
}
