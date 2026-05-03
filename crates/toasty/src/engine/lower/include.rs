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
        let field = &self.model_unwrap().fields[field_index];

        let (mut stmt, target_model_id) = match &field.ty {
            app::FieldTy::HasMany(rel) => (
                stmt::Query::new_select(
                    rel.target,
                    stmt::Expr::eq(
                        stmt::Expr::ref_parent_model(),
                        stmt::Expr::ref_self_field(rel.pair),
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
                        stmt::Expr::ref_self_field(rel.pair),
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

        returning[field_index] = sub_expr;
    }
}

impl FieldIncludes {
    /// True when at least one include path activates this field.
    fn self_included(&self) -> bool {
        self.include_self || !self.sub_paths.is_empty()
    }
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
