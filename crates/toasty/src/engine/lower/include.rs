//! Lowering for `Returning::Model` includes and deferred-field masking.
//!
//! `mapping::Model::default_returning` is computed at schema-build time with
//! every deferred field — top-level or nested inside an embedded type —
//! pre-masked to `Null`. Lowering starts from a clone of the default
//! expression and splices loaded forms in for fields named by `.include()`
//! paths or, for an `INSERT … RETURNING`, for every deferred field.
//!
//! The recursion is mapping-driven: each `mapping::Field` variant decides how
//! to descend into its corresponding expression. Driving off the mapping
//! tree (rather than the expression's shape) is what lets us reach a
//! deferred sub-field of an embed struct nested inside an enum variant —
//! the masked `Null` lives inside a `Match` expression, not a `Record`.
//!
//! Include entries arrive as [`stmt::Include`] values. The first thing we do
//! is flatten any `PathRoot::Variant` chain
//! into a plain projection of the form `[…parent_steps, variant_idx,
//! …local_var_field_steps]`. This LOCAL form mirrors the IR's `Match` arm
//! record (where each arm is `[disc, field_0, field_1, …]`), so descent
//! through enum variants is just two index steps: one selecting the arm by
//! variant index, the next addressing a local variant field.
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
//! Relations only live on top-level models.

use toasty_core::{
    schema::{app, mapping},
    stmt,
};

use crate::engine::lower::LowerStatement;
use crate::schema::lazy_slot;

struct FlatInclude {
    projection: stmt::Projection,
    query: Option<stmt::Query>,
}

#[derive(Default)]
struct IncludeQuery {
    filter: Option<stmt::Expr>,
    order_by: Option<stmt::OrderBy>,
}

/// The include entries that target a single field, partitioned by whether
/// they name the field itself or a sub-path within it.
///
/// Either kind activates the field. Sub-paths only matter when the field
/// is an embed — they drive the recursion into nested fields.
struct FieldIncludes {
    /// At least one include path equals `[i]` — the field is named directly.
    include_self: bool,
    /// Merged query modifiers for includes ending at this field.
    top_query: IncludeQuery,
    /// Tails of every `[i, …]` include path, with the leading index stripped.
    sub_paths: Vec<FlatInclude>,
}

impl LowerStatement<'_, '_> {
    /// Top-level entry from `visit_returning_mut` for a `Returning::Model`.
    /// Flattens each include to its projection (folding any
    /// `PathRoot::Variant` chain into discriminant-index steps), then runs
    /// the recursion against the model's fields.
    pub(super) fn process_top_level_includes(
        &mut self,
        returning: &mut stmt::Expr,
        includes: &[stmt::Include],
        is_insert: bool,
    ) {
        let stmt::Expr::Record(record) = returning else {
            return;
        };
        let flat: Vec<FlatInclude> = includes.iter().map(flatten_include).collect();
        let app_fields = &self.model_unwrap().fields;
        let mapping_fields = &self.mapping_unwrap().fields;
        self.process_fields(record, app_fields, mapping_fields, &flat, is_insert);
    }

    /// Process the fields of a struct-shaped record (top-level model or
    /// embedded struct). For each field, partition matching include paths via
    /// [`FieldIncludes`] and dispatch:
    ///
    /// - Relation → splice a subquery via [`build_include_subquery`].
    /// - Anything else → [`process_field`].
    fn process_fields(
        &mut self,
        returning: &mut stmt::ExprRecord,
        app_fields: &[app::Field],
        mapping_fields: &[mapping::Field],
        includes: &[FlatInclude],
        is_insert: bool,
    ) {
        for (i, (field, mapping)) in app_fields.iter().zip(mapping_fields).enumerate() {
            let field_includes = partition_includes(includes, i);

            if field.ty.is_relation() {
                if field_includes.self_included() {
                    self.build_include_subquery(
                        returning,
                        i,
                        &field_includes.sub_paths,
                        field_includes.top_query,
                    );
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
            *returning = lazy_slot::loaded_expr(loaded_form(field, mapping));

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
        sub_includes: &[FlatInclude],
        is_insert: bool,
    ) {
        match (self.schema().app.model(target), mapping) {
            (app::Model::EmbeddedStruct(em), mapping::Field::Struct(fs)) => {
                // A nullable struct embed (`Option<Embed>`) wraps its record in
                // a presence `Match`; descend into the `Some` arm's record so
                // its deferred sub-fields are still processed (e.g. loaded on
                // `INSERT … RETURNING`). A non-nullable embed is a bare record.
                let record = match returning {
                    stmt::Expr::Record(record) => record,
                    stmt::Expr::Match(match_expr) => {
                        let Some(stmt::Expr::Record(record)) =
                            match_expr.arms.first_mut().map(|arm| &mut arm.expr)
                        else {
                            return;
                        };
                        record
                    }
                    _ => return,
                };
                self.process_fields(
                    record,
                    em.fields.as_slice(),
                    fs.fields.as_slice(),
                    sub_includes,
                    is_insert,
                );
            }
            (app::Model::EmbeddedEnum(em), mapping::Field::Enum(fe)) => {
                self.process_enum_arms(returning, em, fe, sub_includes, is_insert);
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
        sub_includes: &[FlatInclude],
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

            // Tails of every include that targets THIS arm — i.e., paths whose
            // leading step equals `variant_idx`. The leading step is stripped.
            let arm_sub_includes: Vec<FlatInclude> = sub_includes
                .iter()
                .filter_map(|fi| {
                    let (first, rest) = fi.projection.as_slice().split_first()?;
                    (*first == variant_idx).then(|| FlatInclude {
                        projection: stmt::Projection::from(rest),
                        query: fi.query.clone(),
                    })
                })
                .collect();

            for (j, (var_field, var_mapping)) in variant_fields
                .iter()
                .zip(&variant_mapping.fields)
                .enumerate()
            {
                let field_includes = partition_includes(&arm_sub_includes, j);
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
        nested: &[FlatInclude],
        top_query: IncludeQuery,
    ) {
        let value = self.build_relation_subquery_inner(field_index, nested, top_query);
        returning[field_index] = if self.model_unwrap().fields[field_index].deferred {
            lazy_slot::loaded_expr(value)
        } else {
            value
        };
    }

    /// Build a subquery that loads the related model(s) for a
    /// `BelongsTo`/`HasOne`/`HasMany` field, run the canonical lowering
    /// pipeline on it, and return it stitched onto the parent statement as an
    /// `Expr::Arg`. This is the `.select(rel_field)` entry point;
    /// `.include(...)` goes through [`build_relation_subquery_inner`] so it
    /// can pass its nested includes and filter down.
    pub(super) fn build_relation_subquery(&mut self, field_index: usize) -> stmt::Expr {
        self.build_relation_subquery_inner(field_index, &[], IncludeQuery::default())
    }

    fn build_relation_subquery_inner(
        &mut self,
        field_index: usize,
        nested: &[FlatInclude],
        top_query: IncludeQuery,
    ) -> stmt::Expr {
        let field = &self.model_unwrap().fields[field_index];

        // A multi-step (`via`) relation reaches its target through a path of
        // existing relations. Build the child query as a single JOIN through
        // the via chain so the engine can issue one query (per include) and
        // group the children with the parent in `NestedMerge`. This relies on
        // the database executing the join, so it is SQL-only — a key-value
        // backend would need a cascade of per-step queries instead.
        let via = match &field.ty {
            app::FieldTy::Via(via) => Some(via),
            _ => None,
        };
        if let Some(via) = via {
            if !self.capability().sql {
                todo!(
                    "`.include()` / `.select()` of a multi-step `via` relation is only \
                     supported on SQL backends; query the relation directly instead"
                );
            }
            // `via` lowering does not thread per-relation filters through its
            // JOIN chain yet; reject rather than silently drop them.
            if top_query.filter.is_some()
                || top_query.order_by.is_some()
                || nested.iter().any(|fi| query_has_modifiers(&fi.query))
            {
                todo!(
                    "include query modifiers on a multi-step `via` relation are not yet supported"
                );
            }
            let nested_projections: Vec<stmt::Projection> =
                nested.iter().map(|fi| fi.projection.clone()).collect();
            return self.build_via_include_subquery(field_index, via, &nested_projections);
        }

        let (mut stmt, target_model_id) = match &field.ty {
            app::FieldTy::Has(rel) => {
                let mut query = stmt::Query::new_select(
                    rel.target,
                    stmt::Expr::eq(
                        stmt::Expr::ref_parent_model(),
                        stmt::Expr::ref_self_field(rel.pair_id),
                    ),
                );
                if rel.is_one() {
                    // To handle single relations, we need a new query modifier that
                    // returns a single record and not a list. This matters for the
                    // type system.
                    query.single = true;
                }
                (query, rel.target)
            }
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
                        target_pk_fields.push(stmt::Expr::ref_self_field(fk_field.target));
                    }

                    source_fk = stmt::Expr::record_from_vec(source_fk_fields);
                    target_pk = stmt::Expr::record_from_vec(target_pk_fields);
                }

                let mut query =
                    stmt::Query::new_select(rel.target, stmt::Expr::eq(source_fk, target_pk));
                query.single = true;
                (query, rel.target)
            }
            _ => unreachable!("build_include_subquery called on non-relation field"),
        };

        // AND the user-supplied filter (if any) onto the join predicate; the
        // pipeline below lowers it like any other filter on the target.
        if let Some(filter) = top_query.filter {
            stmt.add_filter(filter);
        }
        stmt.order_by = top_query.order_by;

        // Attach each non-empty remainder as a nested include on the
        // subquery, carrying any deeper-level filter forward. Empty remainders
        // (from a bare `.include(posts())`) need no nested include — the
        // subquery itself satisfies them. The lowering pipeline will
        // recursively group and process the nested includes when it encounters
        // `Returning::Model` on this subquery.
        for fi in nested {
            if !fi.projection.is_empty() {
                stmt.include(stmt::Include {
                    path: stmt::Path {
                        root: stmt::PathRoot::Model(target_model_id),
                        projection: fi.projection.clone(),
                    },
                    query: fi.query.clone(),
                });
            }
        }

        // Run the canonical pipeline (pre-lower simplify, lowering walk,
        // post-lower simplify) on the synthesized subquery, stitching it onto
        // the parent as an `Expr::Arg`.
        self.lower_sub_stmt(stmt::Statement::Query(stmt))
    }
}

impl FieldIncludes {
    /// True when at least one include path activates this field.
    fn self_included(&self) -> bool {
        self.include_self || !self.sub_paths.is_empty()
    }
}

/// Partitions includes for a field and merges modifiers on the field itself.
fn partition_includes(includes: &[FlatInclude], i: usize) -> FieldIncludes {
    let mut include_self = false;
    let mut unfiltered_self = false;
    let mut top_filter: Option<stmt::Expr> = None;
    let mut top_order_by = None;
    let mut sub_paths = Vec::new();
    for fi in includes {
        if let Some((first, rest)) = fi.projection.as_slice().split_first()
            && *first == i
        {
            if rest.is_empty() {
                include_self = true;
                top_order_by = fi.query.as_ref().and_then(|query| query.order_by.clone());
                match query_filter_expr(&fi.query) {
                    Some(f) if !unfiltered_self => {
                        top_filter = Some(match top_filter.take() {
                            Some(prev) => stmt::Expr::or(prev, f),
                            None => f,
                        });
                    }
                    Some(_) => {}
                    None => {
                        unfiltered_self = true;
                        top_filter = None;
                    }
                }
            } else {
                sub_paths.push(FlatInclude {
                    projection: stmt::Projection::from(rest),
                    query: fi.query.clone(),
                });
            }
        }
    }
    FieldIncludes {
        include_self,
        top_query: IncludeQuery {
            filter: top_filter,
            order_by: top_order_by,
        },
        sub_paths,
    }
}

fn flatten_include(include: &stmt::Include) -> FlatInclude {
    FlatInclude {
        projection: flatten_path(&include.path),
        query: include.query.clone(),
    }
}

fn query_filter_expr(query: &Option<stmt::Query>) -> Option<stmt::Expr> {
    match &query.as_ref()?.body {
        stmt::ExprSet::Select(select) => select.filter.expr.clone(),
        _ => None,
    }
}

fn query_has_modifiers(query: &Option<stmt::Query>) -> bool {
    query.as_ref().is_some_and(|query| {
        query_filter_expr(&Some(query.clone())).is_some() || query.order_by.is_some()
    })
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
