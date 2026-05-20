//! Lowering for `Returning::Model` includes and deferred-field masking.
//!
//! `mapping::Model::default_returning` is computed at schema-build time with
//! every `#[deferred]` field — top-level or nested inside an embedded type —
//! pre-masked to `Null`. Lowering starts from a clone of the default
//! expression and splices loaded forms in for fields named by `.include()`
//! paths or, for an `INSERT … RETURNING`, for every deferred field.
//!
//! The recursion is mapping-driven: each `mapping::Field` variant decides how
//! to descend into its corresponding expression. Driving off the mapping
//! tree (rather than the expression's shape) is what lets us reach a
//! `#[deferred]` sub-field of an embed struct nested inside an enum variant —
//! the masked `Null` lives inside a `Match` expression, not a `Record`.
//!
//! Include paths arrive as [`stmt::Path`] values. The first thing we do is
//! flatten any `PathRoot::Variant` chain into a plain projection of the form
//! `[…parent_steps, variant_idx, …local_var_field_steps]`. This LOCAL form
//! mirrors the IR's `Match` arm record (where each arm is `[disc, field_0,
//! field_1, …]`), so descent through enum variants is just two index steps:
//! one selecting the arm by variant index, the next addressing a local
//! variant field.
//!
//! The flow:
//!
//! ```text
//! process_top_level_includes
//!     └─► process_fields ──┬──► relation field: build_include_subquery
//!         ▲                └──► non-relation: process_field
//!         │                          ├── deferred: splice + process_embed
//!         │                          └── eager embed: process_embed
//!         │                                    ├── struct → process_fields
//!         └──────────────────────────────────  └── enum   → process_enum_arms
//! ```
//!
//! Relations don't currently live inside embedded types, so
//! `build_include_subquery` only fires from `process_fields` at the
//! top-level model. When relations-in-embeds eventually lands, the same
//! function will fire from any depth without further restructuring.

use toasty_core::{
    schema::{app, mapping},
    stmt,
};

use crate::engine::lower::LowerStatement;

/// The include paths that target a single field, partitioned by whether
/// they name the field itself or a sub-path within it.
///
/// Either kind activates the field. Sub-paths only matter when the field
/// is an embed — they drive the recursion into nested fields.
struct FieldIncludes {
    /// At least one include path equals `[i]` — the field is named directly.
    include_self: bool,
    /// Tails of every `[i, …]` include path, with the leading index stripped.
    sub_paths: Vec<stmt::Projection>,
}

impl LowerStatement<'_, '_> {
    /// Top-level entry from `visit_returning_mut` for a `Returning::Model`.
    /// Flattens each include `Path` to its projection (folding any
    /// `PathRoot::Variant` chain into discriminant-index steps), then runs
    /// the recursion against the model's fields.
    pub(super) fn process_top_level_includes(
        &mut self,
        returning: &mut stmt::Expr,
        include_paths: &[stmt::Path],
        is_insert: bool,
    ) {
        let stmt::Expr::Record(record) = returning else {
            return;
        };
        let projections: Vec<stmt::Projection> = include_paths.iter().map(flatten_path).collect();
        let app_fields = &self.model_unwrap().fields;
        let mapping_fields = &self.mapping_unwrap().fields;
        self.process_fields(record, app_fields, mapping_fields, &projections, is_insert);
    }

    /// Process the fields of a struct-shaped record (top-level model or
    /// embedded struct). For each field, partition matching include paths via
    /// [`FieldIncludes`] and dispatch:
    ///
    /// - Relation → splice a subquery via [`build_include_subquery`]
    ///   (relations live only at the top level today; a future "relations in
    ///   embeds" feature will fire this from any depth).
    /// - Anything else → [`process_field`].
    fn process_fields(
        &mut self,
        returning: &mut stmt::ExprRecord,
        app_fields: &[app::Field],
        mapping_fields: &[mapping::Field],
        include_paths: &[stmt::Projection],
        is_insert: bool,
    ) {
        for (i, (field, mapping)) in app_fields.iter().zip(mapping_fields).enumerate() {
            let field_includes = partition_paths(include_paths, i);

            if field.ty.is_relation() {
                if field_includes.self_included() {
                    self.build_include_subquery(returning, i, &field_includes.sub_paths);
                }
                continue;
            }

            self.process_field(
                &mut returning[i],
                field,
                mapping,
                &field_includes,
                is_insert,
            );
        }
    }

    /// Process one non-relation field by `mapping::Field` kind.
    ///
    /// - **Deferred field** — when activated, replaces the masked `Null`
    ///   with `Record([loaded])`. `loaded` is the column reference for
    ///   primitives or the embed's pre-computed `default_returning` for
    ///   embed targets. For an embed, `default_returning` has its own
    ///   deferred sub-fields pre-masked, so recurse to splice loaded forms
    ///   in for those too.
    /// - **Eager embed** — recurses into the embed's expression. (Eager
    ///   primitives are a no-op; the column reference already sits in
    ///   `default_returning`.)
    fn process_field(
        &mut self,
        returning: &mut stmt::Expr,
        field: &app::Field,
        mapping: &mapping::Field,
        matches: &FieldIncludes,
        is_insert: bool,
    ) {
        if field.deferred {
            if !is_insert && !matches.self_included() {
                return;
            }
            *returning = stmt::Expr::record([loaded_form(field, mapping)]);

            // For an embed, the loaded form is the embed's `default_returning`, which
            // has its own deferred sub-fields pre-masked. Recurse so those get loaded
            // forms spliced in too.
            if let app::FieldTy::Embedded(embedded) = &field.ty {
                let stmt::Expr::Record(outer) = returning else {
                    unreachable!("just-wrapped record");
                };
                self.process_embed(
                    &mut outer[0],
                    embedded.target,
                    mapping,
                    &matches.sub_paths,
                    is_insert,
                );
            }
            return;
        }

        if let app::FieldTy::Embedded(embedded) = &field.ty
            && (is_insert || !matches.sub_paths.is_empty())
        {
            self.process_embed(
                returning,
                embedded.target,
                mapping,
                &matches.sub_paths,
                is_insert,
            );
        }
    }

    /// Process an embed's expression. Struct embeds expose a `Record`; enum
    /// embeds expose a `Match` (or a bare column ref for unit-only enums,
    /// which has nothing nested to splice).
    fn process_embed(
        &mut self,
        returning: &mut stmt::Expr,
        target: app::ModelId,
        mapping: &mapping::Field,
        sub_paths: &[stmt::Projection],
        is_insert: bool,
    ) {
        match (self.schema().app.model(target), mapping) {
            (app::Model::EmbeddedStruct(em), mapping::Field::Struct(fs)) => {
                let stmt::Expr::Record(record) = returning else {
                    return;
                };
                self.process_fields(
                    record,
                    em.fields.as_slice(),
                    fs.fields.as_slice(),
                    sub_paths,
                    is_insert,
                );
            }
            (app::Model::EmbeddedEnum(em), mapping::Field::Enum(fe)) => {
                self.process_enum_arms(returning, em, fe, sub_paths, is_insert);
            }
            _ => {}
        }
    }

    /// Process the arms of an embedded enum's `Match`, handling variant
    /// fields the same way as a struct's record fields.
    ///
    /// Each data-arm record has the discriminant at position 0 and variant
    /// fields at positions `1..`. `sub_paths` are the path remainders that
    /// have already had the parent field index stripped off — within them,
    /// the leading step is a variant index and the next is a local variant
    /// field index. We partition by variant index per arm and then by local
    /// field index per variant field. `is_insert` independently activates
    /// every field for `INSERT … RETURNING`.
    fn process_enum_arms(
        &mut self,
        returning: &mut stmt::Expr,
        app_enum: &app::EmbeddedEnum,
        mapping: &mapping::FieldEnum,
        sub_paths: &[stmt::Projection],
        is_insert: bool,
    ) {
        let stmt::Expr::Match(match_expr) = returning else {
            return;
        };

        for (variant_idx, arm) in match_expr.arms.iter_mut().enumerate() {
            let variant_fields: Vec<&app::Field> = app_enum.variant_fields(variant_idx).collect();
            if variant_fields.is_empty() {
                continue;
            }
            let stmt::Expr::Record(arm_record) = &mut arm.expr else {
                continue;
            };
            let variant_mapping = &mapping.variants[variant_idx];

            // Tails of every path that targets THIS arm — i.e., paths whose
            // leading step equals `variant_idx`. The leading step is stripped.
            let arm_sub_paths: Vec<stmt::Projection> = sub_paths
                .iter()
                .filter_map(|p| {
                    let (first, rest) = p.as_slice().split_first()?;
                    (*first == variant_idx).then(|| stmt::Projection::from(rest))
                })
                .collect();

            for (j, (var_field, var_mapping)) in variant_fields
                .iter()
                .zip(&variant_mapping.fields)
                .enumerate()
            {
                let field_includes = partition_paths(&arm_sub_paths, j);
                self.process_field(
                    &mut arm_record[j + 1],
                    var_field,
                    var_mapping,
                    &field_includes,
                    is_insert,
                );
            }
        }
    }

    /// Build the relation subquery to splice into `returning[field_index]`
    /// for `.include()` of a `BelongsTo`/`HasMany`/`HasOne`. Reached from
    /// [`process_fields`] for relation fields only.
    fn build_include_subquery(
        &mut self,
        returning: &mut stmt::ExprRecord,
        field_index: usize,
        nested: &[stmt::Projection],
    ) {
        returning[field_index] = self.build_relation_subquery(field_index, nested);
    }

    /// Build a subquery that loads the related model(s) for a
    /// `BelongsTo`/`HasOne`/`HasMany` field, run the canonical lowering
    /// pipeline on it, and return it stitched onto the parent statement
    /// as an `Expr::Arg`.  Used both by `.include(...)` (which splices
    /// the result into a record slot) and by `.select(rel_field)` (which
    /// uses the result as the entire projection expression).
    pub(super) fn build_relation_subquery(
        &mut self,
        field_index: usize,
        nested: &[stmt::Projection],
    ) -> stmt::Expr {
        let field = &self.model_unwrap().fields[field_index];

        // A multi-step (`via`) relation reaches its target through a path of
        // existing relations. Build the child query as a single JOIN through
        // the via chain so the engine can issue one query (per include) and
        // group the children with the parent in `NestedMerge`. This relies on
        // the database executing the join, so it is SQL-only — a key-value
        // backend would need a cascade of per-step queries instead.
        let via = match &field.ty {
            app::FieldTy::HasMany(rel) => rel.kind.via(),
            app::FieldTy::HasOne(rel) => rel.kind.via(),
            _ => None,
        };
        if let Some(via) = via {
            if !self.capability().sql {
                todo!(
                    "`.include()` / `.select()` of a multi-step `via` relation is only \
                     supported on SQL backends; query the relation directly instead"
                );
            }
            return self.build_via_include_subquery(field_index, via, nested);
        }

        let (mut stmt, target_model_id) = match &field.ty {
            app::FieldTy::HasMany(rel) => (
                stmt::Query::new_select(
                    rel.target,
                    stmt::Expr::eq(
                        stmt::Expr::ref_parent_model(),
                        stmt::Expr::ref_self_field(direct_pair(&rel.kind)),
                    ),
                ),
                rel.target,
            ),
            // To handle single relations, we need a new query modifier that
            // returns a single record and not a list. This matters for the
            // type system.
            app::FieldTy::BelongsTo(rel) => {
                let source_fk;
                let target_pk;

                if let [fk_field] = &rel.foreign_key.fields[..] {
                    source_fk = stmt::Expr::ref_parent_field(fk_field.source);
                    target_pk = stmt::Expr::ref_self_field(fk_field.target);
                } else {
                    let mut source_fk_fields = vec![];
                    let mut target_pk_fields = vec![];

                    for fk_field in &rel.foreign_key.fields {
                        source_fk_fields.push(stmt::Expr::ref_parent_field(fk_field.source));
                        target_pk_fields.push(stmt::Expr::ref_parent_field(fk_field.source));
                    }

                    source_fk = stmt::Expr::record_from_vec(source_fk_fields);
                    target_pk = stmt::Expr::record_from_vec(target_pk_fields);
                }

                let mut query =
                    stmt::Query::new_select(rel.target, stmt::Expr::eq(source_fk, target_pk));
                query.single = true;
                (query, rel.target)
            }
            app::FieldTy::HasOne(rel) => {
                let mut query = stmt::Query::new_select(
                    rel.target,
                    stmt::Expr::eq(
                        stmt::Expr::ref_parent_model(),
                        stmt::Expr::ref_self_field(direct_pair(&rel.kind)),
                    ),
                );
                query.single = true;
                (query, rel.target)
            }
            _ => unreachable!("build_include_subquery called on non-relation field"),
        };

        // Attach each non-empty remainder as a nested include on the
        // subquery. Empty remainders (from a bare `.include(posts())`) need
        // no nested include — the subquery itself satisfies them. The
        // lowering pipeline will recursively group and process the nested
        // includes when it encounters `Returning::Model` on this subquery.
        for rest in nested {
            if !rest.is_empty() {
                stmt.include(stmt::Path {
                    root: stmt::PathRoot::Model(target_model_id),
                    projection: rest.clone(),
                });
            }
        }

        // Run the canonical pipeline (pre-lower simplify, lowering walk,
        // post-lower simplify) on the synthesized subquery, stitching it onto
        // the parent as an `Expr::Arg`.
        let mut sub_expr = self.lower_sub_stmt(stmt::Statement::Query(stmt));

        // For nullable single relations (HasOne<Option<T>>, BelongsTo<Option<T>>),
        // wrap the sub-expression with a Let + Match to encode the result
        // using variant-encoded values that distinguish loaded-None from
        // unloaded.
        //
        //   Let {
        //     binding: Stmt(query),
        //     body: Match {
        //       subject: Arg(0),
        //       arms: [Null → I64(0)],
        //       else_: Arg(0)
        //     }
        //   }
        if field.nullable() && !field.ty.is_has_many() {
            sub_expr = stmt::Expr::Let(stmt::ExprLet {
                bindings: vec![sub_expr],
                body: Box::new(stmt::Expr::match_expr(
                    stmt::Expr::arg(0),
                    vec![stmt::MatchArm {
                        pattern: stmt::Value::Null,
                        expr: stmt::Expr::from(0i64),
                    }],
                    // Non-null: pass through as-is (raw model record)
                    stmt::Expr::arg(0),
                )),
            });
        }

        sub_expr
    }

    /// Build the include subquery for a multi-step (`via`) relation.
    ///
    /// The synthesized child query is rooted at the via target and walks
    /// back to the root through `INNER JOIN`s of every intermediate model,
    /// projecting the linking foreign-key column (so `NestedMerge` can group
    /// children by parent) alongside the target row. The query is emitted in
    /// fully-lowered form (`Source::Table` + `Joins`) so the standard
    /// lowering walk is a no-op for it; only the cross-statement parent-PK
    /// reference is rewritten to an `Arg::Ref` on the way through
    /// `lower_sub_stmt`.
    ///
    /// To hide the linking column from the parent's view, the returned
    /// `Expr::Arg` is wrapped in a `Map` that projects field `1`
    /// (`target_record`) of each child row record `[link_key, target_record]`.
    fn build_via_include_subquery(
        &mut self,
        field_index: usize,
        via: &app::Via,
        nested: &[stmt::Projection],
    ) -> stmt::Expr {
        if !nested.is_empty() {
            todo!("nested `.include()` through a multi-step `via` relation");
        }

        let parent_model = self.model_unwrap();
        let parent_model_id = parent_model.id;
        let field = &parent_model.fields[field_index];
        let is_has_many = matches!(field.ty, app::FieldTy::HasMany(_));

        let schema = self.schema();

        // Unfold the via path into a flat list of direct relation steps,
        // expanding any nested vias.
        let steps = flatten_via_steps(schema, parent_model_id, via.path.projection.as_slice());
        let n = steps.len();
        assert!(
            n >= 1,
            "via path must have at least one step (validated at schema build time)"
        );

        // The chain of models the path traverses: `chain[0]` is the model
        // declaring the via, `chain[n]` the target.
        let mut chain = Vec::with_capacity(n + 1);
        chain.push(parent_model_id);
        for &field_id in &steps {
            debug_assert_eq!(field_id.model, *chain.last().unwrap());
            chain.push(schema.app.field(field_id).relation_target_id().unwrap());
        }
        let target_model_id = chain[n];

        // The BelongsTo edge linking each pair of adjacent models, plus which
        // side owns it (and thus the FK column).
        let edges: Vec<Edge> = steps
            .iter()
            .map(|&field_id| Edge::resolve(schema, field_id))
            .collect();

        for edge in &edges {
            assert_eq!(
                1,
                edge.fk.fields.len(),
                "TODO: composite foreign keys in via include path"
            );
        }

        // Build the lowered `Source::Table`. `tables[0]` is the target;
        // `tables[k]` (k in 1..n) is `chain[n - k]` — the intermediate
        // models, ordered from target-adjacent back to root-adjacent.
        let table_idx_for_chain = |chain_idx: usize| -> usize { n - chain_idx };

        let mut tables: Vec<stmt::TableRef> = Vec::with_capacity(n);
        let mut joins: Vec<stmt::Join> = Vec::with_capacity(n.saturating_sub(1));

        tables.push(stmt::TableRef::Table(schema.table_id_for(target_model_id)));

        // Add an INNER JOIN for each intermediate. Edge `i` connects
        // `chain[i]` and `chain[i+1]`; the FK source lives on the BT
        // owner's model, FK target on the BT target's model.
        for k in 1..n {
            let chain_idx = n - k;
            tables.push(stmt::TableRef::Table(schema.table_id_for(chain[chain_idx])));

            let edge = &edges[chain_idx];
            let fk = &edge.fk.fields[0];
            let fk_source_chain_idx = if edge.bt_lives_on_target_side {
                chain_idx + 1
            } else {
                chain_idx
            };
            let fk_target_chain_idx = if edge.bt_lives_on_target_side {
                chain_idx
            } else {
                chain_idx + 1
            };

            let fk_source_table = table_idx_for_chain(fk_source_chain_idx);
            let fk_target_table = table_idx_for_chain(fk_target_chain_idx);

            let fk_source_col = column_index_for_field(schema, fk.source);
            let fk_target_col = column_index_for_field(schema, fk.target);

            joins.push(stmt::Join {
                table: stmt::SourceTableId(k),
                constraint: stmt::JoinOp::Inner(stmt::Expr::eq(
                    stmt::Expr::column(stmt::ExprReference::column(fk_source_table, fk_source_col)),
                    stmt::Expr::column(stmt::ExprReference::column(fk_target_table, fk_target_col)),
                )),
            });
        }

        let source = stmt::Source::Table(stmt::SourceTable {
            tables,
            from: vec![stmt::TableWithJoins {
                relation: stmt::TableFactor::Table(stmt::SourceTableId(0)),
                joins,
            }],
        });

        // Build the linking column (on `chain[1]`) and the matching parent-PK
        // reference, then the WHERE filter `link == parent_pk`.
        let root_edge = &edges[0];
        let root_fk = &root_edge.fk.fields[0];
        let (link_field_id, parent_pk_field_id) = if root_edge.bt_lives_on_target_side {
            // Has-relation: pair BT on chain[1]; FK source on chain[1].
            (root_fk.source, root_fk.target)
        } else {
            // BelongsTo from root: BT on chain[0]; FK target on chain[1].
            (root_fk.target, root_fk.source)
        };

        // Sanity-check: parent PK must be a single column for now (the
        // composite case is guarded by the composite-FK assertion above).
        assert_eq!(
            parent_pk_field_id.model, parent_model_id,
            "via root step does not reference the source model's PK"
        );

        // Use the field's model-level expression (its column ref wrapped in
        // the storage→model cast where the types differ) rather than a raw
        // column ref, so it lines up type-wise with the parent's lowered PK
        // value both in the WHERE comparison and in the `NestedMerge` group
        // key. Re-point the column at the linking intermediate's table.
        let link_table_idx = table_idx_for_chain(1);
        let link_col_expr = model_level_column_expr(schema, link_field_id, link_table_idx);

        let parent_pk_ref = stmt::Expr::ref_field(1, parent_pk_field_id);
        let filter = stmt::Expr::eq(link_col_expr.clone(), parent_pk_ref);

        // The returning is `Record([link_col, target_record])`. The link
        // column lands in `load_data_select_items` so the `NestedMerge`
        // qualification can resolve to a `SortLookup` on it. The target
        // record is the schema's pre-computed `default_returning`, which
        // references columns at `table = 0` — matching our table layout.
        let target_record = schema
            .mapping_for(target_model_id)
            .default_returning
            .clone();
        let returning_record = stmt::Expr::record_from_vec(vec![link_col_expr, target_record]);

        // `DISTINCT` collapses duplicate target rows produced when the via
        // path fans out (e.g. a user with two comments on the same article
        // reaches that article twice) — matching the distinct-target
        // semantics of a direct via query.
        let mut select = stmt::Select::new(source, filter);
        select.returning = stmt::Returning::Project(returning_record);
        select.distinct = true;

        let mut query = stmt::Query::builder(select).build();
        if !is_has_many {
            query.single = true;
        }

        // Run the synthesized child through the canonical sub-statement
        // pipeline, registering it as an `Arg::Sub`. The query is already in
        // lowered form, so the pre-lower rewrites and the lowering walk are
        // largely no-ops; the one thing the walk does is rewrite the
        // cross-statement parent-PK `Reference::Field { nesting: 1 }` into an
        // `Arg::Ref`.
        let sub_expr = self.lower_sub_stmt(stmt::Statement::Query(query));

        // Each child row is `Record([link_key, target_record])`, but the
        // parent only wants the target — wrap the list in a `Map` that
        // projects field `1` out of each item. In a `Map` body, `arg(0)` is
        // the whole item; field access is an explicit projection.
        stmt::Expr::map(sub_expr, stmt::Expr::project(stmt::Expr::arg(0), [1usize]))
    }
}

/// One step in a via path, resolved to its edge `BelongsTo`.
#[derive(Debug)]
struct Edge {
    /// The foreign key of the edge BT.
    fk: app::ForeignKey,
    /// `true` when the BT lives on the "later" model in the chain
    /// (`HasMany` / `HasOne` step — the pair BT lives on the relation's
    /// target). `false` when the BT lives on the "earlier" model
    /// (a `BelongsTo` step is itself the edge).
    bt_lives_on_target_side: bool,
}

impl Edge {
    fn resolve(schema: &toasty_core::Schema, field_id: app::FieldId) -> Edge {
        let field = schema.app.field(field_id);
        match &field.ty {
            // A has-relation is reached through its paired `BelongsTo`, which
            // lives on the target model and owns the FK.
            app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                let pair_id = field
                    .pair()
                    .expect("via paths are unfolded into direct steps before edge resolution");
                Edge {
                    fk: schema
                        .app
                        .field(pair_id)
                        .ty
                        .as_belongs_to_unwrap()
                        .foreign_key
                        .clone(),
                    bt_lives_on_target_side: true,
                }
            }
            app::FieldTy::BelongsTo(bt) => Edge {
                fk: bt.foreign_key.clone(),
                bt_lives_on_target_side: false,
            },
            _ => unreachable!("via step is not a relation field"),
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

/// Resolve a primitive `FieldId` to its single backing column's index in
/// that model's table. Panics for non-primitive (multi-column) fields —
/// the via include path only ever resolves FK source/target fields, which
/// are guaranteed primitive by the schema linker.
fn column_index_for_field(schema: &toasty_core::Schema, field_id: app::FieldId) -> usize {
    let mapping = schema.mapping_for(field_id.model);
    mapping.fields[field_id.index]
        .as_primitive()
        .expect("FK field is primitive")
        .column
        .index
}

/// Build the model-level expression for a primitive field — its column
/// reference wrapped in the storage→model cast when the storage type
/// differs (e.g. `Uuid` stored as `Bytes`) — re-pointed at `table_idx`
/// within a multi-table `Source::Table`.
///
/// `FieldPrimitive::column_expr` is the schema's pre-built table→model
/// expression, which references `table = 0`; we rewrite each column ref's
/// table index to the supplied join slot.
fn model_level_column_expr(
    schema: &toasty_core::Schema,
    field_id: app::FieldId,
    table_idx: usize,
) -> stmt::Expr {
    let mapping = schema.mapping_for(field_id.model);
    let mut expr = mapping.fields[field_id.index]
        .as_primitive()
        .expect("FK field is primitive")
        .column_expr
        .clone();

    stmt::visit_mut::for_each_expr_mut(&mut expr, |e| {
        if let stmt::Expr::Reference(stmt::ExprReference::Column(col)) = e {
            col.table = table_idx;
        }
    });

    expr
}

impl FieldIncludes {
    /// True when at least one include path activates this field.
    fn self_included(&self) -> bool {
        self.include_self || !self.sub_paths.is_empty()
    }
}

/// The paired `BelongsTo` field of a direct has-relation. `.include()` of a
/// `via` relation is rejected earlier in `build_relation_subquery`, so any
/// relation reaching the direct-relation path has a pair.
fn direct_pair(kind: &app::HasKind) -> app::FieldId {
    kind.pair_id()
        .expect("`via` relation reached the direct-relation include path")
}

/// Find the include paths that target field index `i` and split them by
/// whether they name the field itself or a sub-path within it.
fn partition_paths(paths: &[stmt::Projection], i: usize) -> FieldIncludes {
    let mut include_self = false;
    let mut sub_paths = Vec::new();
    for path in paths {
        if let Some((first, rest)) = path.as_slice().split_first()
            && *first == i
        {
            if rest.is_empty() {
                include_self = true;
            } else {
                sub_paths.push(stmt::Projection::from(rest));
            }
        }
    }
    FieldIncludes {
        include_self,
        sub_paths,
    }
}

/// Flatten an include [`stmt::Path`] into a single projection, folding any
/// `PathRoot::Variant` chain into a discriminant-index step.
///
/// The result uses LOCAL field indices for variant fields (matching the IR's
/// `Match` arm record convention), not the GLOBAL `EmbeddedEnum::fields`
/// indices used by `Schema::resolve`. Include lowering walks the IR shape,
/// not the schema, so LOCAL is what `process_enum_arms` needs.
fn flatten_path(path: &stmt::Path) -> stmt::Projection {
    let mut acc = stmt::Projection::identity();
    push_root_steps(&path.root, &mut acc);
    for step in path.projection.as_slice() {
        acc.push(*step);
    }
    acc
}

fn push_root_steps(root: &stmt::PathRoot, acc: &mut stmt::Projection) {
    if let stmt::PathRoot::Variant { parent, variant_id } = root {
        push_root_steps(&parent.root, acc);
        for step in parent.projection.as_slice() {
            acc.push(*step);
        }
        acc.push(variant_id.index);
    }
}

/// Build the loaded-form inner expression for a deferred field.
///
/// - Primitive — the cached column reference (with any storage-type cast).
/// - Embed (struct or enum) — the embed's pre-computed `default_returning`.
fn loaded_form(field: &app::Field, mapping: &mapping::Field) -> stmt::Expr {
    match (&field.ty, mapping) {
        (app::FieldTy::Primitive(_), mapping::Field::Primitive(p)) => p.column_expr.clone(),
        (app::FieldTy::Embedded(_), mapping::Field::Struct(s)) => s.default_returning.clone(),
        (app::FieldTy::Embedded(_), mapping::Field::Enum(e)) => e.default_returning.clone(),
        _ => unreachable!("deferred field has unexpected mapping shape"),
    }
}
