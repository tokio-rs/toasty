//! Lowering for `Returning::Model` includes and deferred-field masking.
//!
//! `mapping::Model::default_returning` is computed at schema-build time with
//! every `#[deferred]` slot — top-level or nested inside an embedded type —
//! pre-masked to `Null`. Lowering starts from a clone of the default
//! expression and splices loaded forms in at slots named by `.include()`
//! paths or, for an `INSERT … RETURNING`, at every deferred slot.
//!
//! The recursion is mapping-driven: each `mapping::Field` variant decides how
//! to descend into its corresponding slot expression. Driving off the mapping
//! tree (rather than the slot expression's shape) is what lets us reach a
//! `#[deferred]` sub-field of an embed struct nested inside an enum variant —
//! those slots live inside a `Match` expression, not a `Record`.
//!
//! The flow:
//!
//! ```text
//! process_top_level_includes
//!     └─► walk_record_fields ──┬──► relation slot: build_include_subquery
//!         ▲                    └──► non-relation: process_field
//!         │                              ├── deferred: splice + descend_into_embed
//!         │                              └── eager embed: descend_into_embed
//!         │                                        ├── struct → walk_record_fields
//!         └──────────────────────────────────────  └── enum   → walk_enum_arms
//! ```
//!
//! Relations don't currently live inside embedded types, so
//! `build_include_subquery` only fires from `walk_record_fields` at the
//! top-level model. When relations-in-embeds eventually lands, the same
//! function will fire from any depth without further restructuring.

use toasty_core::{
    schema::{app, mapping},
    stmt::{self, VisitMut},
};

use crate::engine::{lower::LowerStatement, simplify::Simplify};

impl LowerStatement<'_, '_> {
    /// Top-level entry from `visit_returning_mut` for a `Returning::Model`.
    /// Reads the current scope's model and mapping, then runs the recursion.
    pub(super) fn process_top_level_includes(
        &mut self,
        returning: &mut stmt::Expr,
        include_paths: &[stmt::Projection],
        is_insert: bool,
    ) {
        let stmt::Expr::Record(record) = returning else {
            return;
        };
        let app_fields = &self.model_unwrap().fields;
        let mapping_fields = &self.mapping_unwrap().fields;
        self.walk_record_fields(record, app_fields, mapping_fields, include_paths, is_insert);
    }

    /// Walk the fields of a struct-shaped record (top-level model or embedded
    /// struct). For each field, partition matching include paths via
    /// [`FieldIncludes`] and dispatch:
    ///
    /// - Relation → splice a subquery via [`build_include_subquery`]
    ///   (relations live only at the top level today; a future "relations in
    ///   embeds" feature will fire this from any depth).
    /// - Anything else → [`process_field`].
    fn walk_record_fields(
        &mut self,
        record: &mut stmt::ExprRecord,
        app_fields: &[app::Field],
        mapping_fields: &[mapping::Field],
        include_paths: &[stmt::Projection],
        is_insert: bool,
    ) {
        for (i, (field, mapping)) in app_fields.iter().zip(mapping_fields).enumerate() {
            let field_includes = partition_paths(include_paths, i);

            if field.ty.is_relation() {
                if field_includes.self_included() {
                    self.build_include_subquery(record, i, &field_includes.sub_paths);
                }
                continue;
            }

            self.process_field(&mut record[i], field, mapping, &field_includes, is_insert);
        }
    }

    /// Process one non-relation field by `mapping::Field` kind.
    ///
    /// - **Deferred field** — when activated, wraps the slot in
    ///   `Record([loaded])`. `loaded` is the column reference for primitives
    ///   or the embed's pre-computed `default_returning` for embed targets.
    ///   Then descends into the wrapped inner so nested deferred sub-fields
    ///   apply.
    /// - **Eager embed** — descends into the slot. (Eager primitives are a
    ///   no-op; the column reference already sits in `default_returning`.)
    fn process_field(
        &mut self,
        slot: &mut stmt::Expr,
        field: &app::Field,
        mapping: &mapping::Field,
        matches: &FieldIncludes,
        is_insert: bool,
    ) {
        if field.deferred {
            if !is_insert && !matches.self_included() {
                return;
            }
            *slot = stmt::Expr::record([loaded_form(field, mapping)]);

            // Descend into the wrap so nested deferred sub-fields fire too.
            if let app::FieldTy::Embedded(embedded) = &field.ty {
                let stmt::Expr::Record(outer) = slot else {
                    unreachable!("just-wrapped slot");
                };
                self.descend_into_embed(
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
            self.descend_into_embed(
                slot,
                embedded.target,
                mapping,
                &matches.sub_paths,
                is_insert,
            );
        }
    }

    /// Descend into an embed's slot. Struct embeds expose a `Record`; enum
    /// embeds expose a `Match` (or a bare column ref for unit-only enums,
    /// which has nothing nested to splice).
    fn descend_into_embed(
        &mut self,
        slot: &mut stmt::Expr,
        target: app::ModelId,
        mapping: &mapping::Field,
        sub_paths: &[stmt::Projection],
        is_insert: bool,
    ) {
        match (self.schema().app.model(target), mapping) {
            (app::Model::EmbeddedStruct(em), mapping::Field::Struct(fs)) => {
                let stmt::Expr::Record(record) = slot else {
                    return;
                };
                self.walk_record_fields(
                    record,
                    em.fields.as_slice(),
                    fs.fields.as_slice(),
                    sub_paths,
                    is_insert,
                );
            }
            (app::Model::EmbeddedEnum(em), mapping::Field::Enum(fe)) => {
                self.walk_enum_arms(slot, em, fe, is_insert);
            }
            _ => {}
        }
    }

    /// Walk the arms of an embedded enum's `Match`, processing variant
    /// fields the same way as a struct's record fields.
    ///
    /// Each data-arm record has the discriminant at position 0 and variant
    /// fields at positions `1..`. Include paths through enum variants don't
    /// exist today (the path macro has no syntax for it), so this only
    /// services the `is_insert` short-circuit and the deferred-mask walk
    /// for sub-fields nested in struct embeds inside variants.
    fn walk_enum_arms(
        &mut self,
        slot: &mut stmt::Expr,
        app_enum: &app::EmbeddedEnum,
        mapping: &mapping::FieldEnum,
        is_insert: bool,
    ) {
        let stmt::Expr::Match(match_expr) = slot else {
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

            // No path syntax routes through enum variants today, so each
            // variant field gets an empty `FieldIncludes`. `is_insert` is the
            // only thing that activates a slot here.
            let no_match = FieldIncludes {
                include_self: false,
                sub_paths: Vec::new(),
            };
            for (j, (var_field, var_mapping)) in variant_fields
                .iter()
                .zip(&variant_mapping.fields)
                .enumerate()
            {
                self.process_field(
                    &mut arm_record[j + 1],
                    var_field,
                    var_mapping,
                    &no_match,
                    is_insert,
                );
            }
        }
    }

    /// Build the relation subquery to splice into `record[field_index]` for
    /// `.include()` of a `BelongsTo`/`HasMany`/`HasOne`. Reached from
    /// [`walk_record_fields`] for relation slots only.
    fn build_include_subquery(
        &mut self,
        record: &mut stmt::ExprRecord,
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

        // Simplify the new stmt to handle relations.
        Simplify::with_context(self.expr_cx).visit_stmt_query_mut(&mut stmt);

        let mut sub_expr = stmt::Expr::stmt(stmt);

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

        record[field_index] = sub_expr;
    }
}

/// The include paths that target a single field slot, partitioned by
/// whether they name the slot itself or a sub-path within it.
///
/// Either kind activates the slot. Sub-paths only matter when the slot is
/// an embed — they drive the recursion into nested fields.
struct FieldIncludes {
    /// At least one include path equals `[i]` — the field is named directly.
    include_self: bool,
    /// Tails of every `[i, …]` include path, with the leading index stripped.
    sub_paths: Vec<stmt::Projection>,
}

impl FieldIncludes {
    /// True when at least one include path activates this slot.
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

/// Build the loaded-form inner expression for a deferred slot.
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
