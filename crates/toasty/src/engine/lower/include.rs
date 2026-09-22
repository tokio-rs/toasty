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
//!     └─► process_fields ──┬──► relation field: build_relation_subquery_inner
//!         ▲                └──► non-relation: process_field
//!         │                          ├── deferred: splice + process_embed
//!         │                          └── eager embed: process_embed
//!         │                                    ├── struct → process_fields
//!         └──────────────────────────────────  └── enum   → process_enum_arms
//! ```

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
    /// At least one include path names this field or one of its descendants.
    included: bool,
    /// Merged query modifiers for includes ending at this field.
    top_query: IncludeQuery,
    /// Tails of every `[i, …]` include path, with the leading index stripped.
    sub_paths: Vec<FlatInclude>,
}

impl LowerStatement<'_, '_> {
    /// Load relations in a query projection.
    pub(super) fn process_projected_returning(&mut self, value: &mut stmt::Expr) {
        if let Some((field, mapping, path)) =
            projected_field(self.schema(), self.model_unwrap().id, value)
        {
            match &field.ty {
                app::FieldTy::Embedded(embedded) => {
                    use stmt::VisitMut;
                    self.visit_expr_mut(value);
                    super::Simplify::with_context(self.expr_cx, self.capability())
                        .visit_expr_mut(value);
                    self.process_embed(value, embedded.target, mapping, &[], &path);
                }
                _ if field.ty.is_relation()
                    && matches!(
                        value,
                        stmt::Expr::Reference(stmt::ExprReference::Field { nesting: 0, .. })
                    ) =>
                {
                    *value = self.build_relation_subquery(field.id.index);
                }
                _ => {}
            }
            // A scalar projection must not load relations in its base embed.
            return;
        }
        if let stmt::Expr::Record(record) = value {
            for field in &mut record.fields {
                self.process_projected_returning(field);
            }
        }
    }

    /// Load complete returned embeds and refresh relations whose keys changed.
    pub(super) fn process_update_embedded_relations(&mut self, returning: &mut stmt::Returning) {
        let stmt::Returning::Project(value) = returning else {
            return;
        };
        let Some(model) = self.model() else { return };
        let mapping = self.mapping_unwrap();
        self.lower_returning().process_sparse_embeds(
            value,
            &model.fields,
            &mapping.fields,
            &stmt::Path::model(model.id),
        );
    }

    fn process_sparse_embeds(
        &mut self,
        value: &mut stmt::Expr,
        fields: &[app::Field],
        mappings: &[mapping::Field],
        host: &stmt::Path,
    ) {
        let stmt::Expr::Cast(cast) = value else {
            return;
        };
        let stmt::Type::SparseRecord(indices) = &mut cast.ty else {
            return;
        };
        let stmt::Expr::Record(record) = &mut *cast.expr else {
            return;
        };
        if !host.projection.is_empty() {
            for (index, field) in fields.iter().enumerate() {
                let app::FieldTy::BelongsTo(relation) = &field.ty else {
                    continue;
                };
                if field.deferred
                    || !relation
                        .foreign_key
                        .fields
                        .iter()
                        .any(|fk| indices.contains(fk.source.index))
                {
                    continue;
                }
                let load =
                    self.build_relation_subquery_inner(field, host, &[], IncludeQuery::default());
                let position = indices.iter().take_while(|i| *i < index).count();
                if indices.contains(index) {
                    record.fields[position] = load;
                } else {
                    indices.insert(index);
                    record.fields.insert(position, load);
                }
            }
        }
        for (index, value) in indices.iter().zip(&mut record.fields) {
            let app::FieldTy::Embedded(embedded) = &fields[index].ty else {
                continue;
            };
            let path = field_path(host, index);
            if matches!(value, stmt::Expr::Cast(cast) if matches!(cast.ty, stmt::Type::SparseRecord(_)))
            {
                let app::Model::EmbeddedStruct(model) = self.schema().app.model(embedded.target)
                else {
                    unreachable!("only structs support partial updates")
                };
                self.process_sparse_embeds(
                    value,
                    &model.fields,
                    &mappings[index].as_struct().unwrap().fields,
                    &path,
                );
            } else {
                use stmt::VisitMut;
                self.visit_expr_mut(value);
                self.process_embed(value, embedded.target, &mappings[index], &[], &path);
            }
        }
    }

    /// Process a model's returning record, including per-row insert results.
    /// Flattens each include to its projection (folding any
    /// `PathRoot::Variant` chain into discriminant-index steps), then runs
    /// the recursion against the model's fields.
    pub(super) fn process_top_level_includes(
        &mut self,
        record: &mut stmt::ExprRecord,
        includes: &[stmt::Include],
    ) {
        let flat: Vec<FlatInclude> = includes.iter().map(flatten_include).collect();
        let app_fields = &self.model_unwrap().fields;
        let mapping_fields = &self.mapping_unwrap().fields;
        self.process_fields(
            &mut record.fields,
            app_fields,
            mapping_fields,
            &flat,
            &stmt::Path::model(self.model_unwrap().id),
        );
    }

    /// Process the fields of a struct-shaped record (top-level model or
    /// embedded struct). For each field, partition matching include paths via
    /// [`FieldIncludes`] and dispatch:
    ///
    /// - Relation → splice a subquery via [`Self::build_relation_subquery_inner`].
    /// - Anything else → [`process_field`].
    fn process_fields(
        &mut self,
        returning: &mut [stmt::Expr],
        app_fields: &[app::Field],
        mapping_fields: &[mapping::Field],
        includes: &[FlatInclude],
        host: &stmt::Path,
    ) {
        for (i, (field, mapping)) in app_fields.iter().zip(mapping_fields).enumerate() {
            let field_includes = partition_includes(includes, i);

            if field.ty.is_relation() {
                // Relation loads need a specific insert row to read its keys.
                // We add them later, when processing each row.
                if self.cx.is_insert_without_row() {
                    continue;
                }

                // Insert planning already filled has-one and has-many results.
                if self.cx.is_insert_with_row() && !field.ty.is_belongs_to() {
                    continue;
                }

                if field_includes.included || !field.deferred {
                    let value = self.build_relation_subquery_inner(
                        field,
                        host,
                        &field_includes.sub_paths,
                        field_includes.top_query,
                    );
                    returning[i] = if field.deferred {
                        lazy_slot::loaded_expr(value)
                    } else {
                        value
                    };
                }
                continue;
            }

            self.process_field(
                &mut returning[i],
                field,
                mapping,
                &field_includes,
                &field_path(host, i),
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
        path: &stmt::Path,
    ) {
        if field.deferred {
            if !self.cx.is_insert() && !matches.included {
                return;
            }
            if !self.cx.is_insert_with_row() {
                *returning = lazy_slot::loaded_expr(loaded_form(field, mapping));
            }
        }

        if let app::FieldTy::Embedded(embedded) = &field.ty {
            let returning = if field.deferred {
                let stmt::Expr::Record(outer) = returning else {
                    unreachable!("just-wrapped record");
                };
                &mut outer[0]
            } else {
                returning
            };
            self.process_embed(
                returning,
                embedded.target,
                mapping,
                &matches.sub_paths,
                path,
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
        path: &stmt::Path,
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
                    &mut record.fields,
                    em.fields.as_slice(),
                    fs.fields.as_slice(),
                    sub_includes,
                    path,
                );
            }
            (app::Model::EmbeddedEnum(em), mapping::Field::Enum(fe)) => {
                self.process_enum_arms(returning, em, fe, sub_includes, path);
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
    /// field index per variant field. Insert contexts also activate deferred
    /// non-relation fields for `INSERT … RETURNING`.
    fn process_enum_arms(
        &mut self,
        returning: &mut stmt::Expr,
        app_enum: &app::EmbeddedEnum,
        mapping: &mapping::FieldEnum,
        sub_includes: &[FlatInclude],
        path: &stmt::Path,
    ) {
        let stmt::Expr::Match(match_expr) = returning else {
            return;
        };

        for (variant_idx, arm) in match_expr.arms.iter_mut().enumerate() {
            let variant_fields = app_enum.variant_fields(variant_idx);
            if variant_fields.is_empty() {
                continue;
            }
            let stmt::Expr::Record(arm_record) = &mut arm.expr else {
                continue;
            };
            let variant_mapping = &mapping.variants[variant_idx];
            let host = stmt::Path::from_variant(
                path.clone(),
                app::VariantId {
                    model: app_enum.id,
                    index: variant_idx,
                },
            );

            let matches = partition_includes(sub_includes, variant_idx);

            self.process_fields(
                &mut arm_record.fields[1..],
                variant_fields,
                &variant_mapping.fields,
                &matches.sub_paths,
                &host,
            );
        }
    }

    /// Build a subquery that loads the related model(s) for a
    /// `BelongsTo`/`HasOne`/`HasMany` field, run the canonical lowering
    /// pipeline on it, and return it stitched onto the parent statement as an
    /// `Expr::Arg`. This is the `.select(rel_field)` entry point;
    /// `.include(...)` goes through [`build_relation_subquery_inner`] so it
    /// can pass its nested includes and filter down.
    pub(super) fn build_relation_subquery(&mut self, field_index: usize) -> stmt::Expr {
        self.build_relation_subquery_inner(
            &self.model_unwrap().fields[field_index],
            &stmt::Path::model(self.model_unwrap().id),
            &[],
            IncludeQuery::default(),
        )
    }

    fn build_relation_subquery_inner(
        &mut self,
        field: &app::Field,
        host: &stmt::Path,
        nested: &[FlatInclude],
        top_query: IncludeQuery,
    ) -> stmt::Expr {
        let field_index = field.id.index;

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
            if !self.capability().sql() {
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
                let source_fk = super::scalar_or_record(
                    rel.foreign_key
                        .fields
                        .iter()
                        .map(|fk| self.relation_source_field(host, fk.source)),
                );
                let target_pk =
                    super::key_field_refs(0, rel.foreign_key.fields.iter().map(|fk| fk.target));

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
        let mut statement = stmt::Statement::Query(stmt);

        self.state
            .engine
            .normalize_stmt(&mut statement)
            .expect("valid include subquery");

        let load = self.lower_sub_stmt(statement);

        if self.cx.is_insert_with_row() && field.ty.is_belongs_to() {
            self.order_relation_load_after_enclosing_inserts(&load);
            Self::single_relation_from_load(load)
        } else {
            load
        }
    }

    fn relation_source_field(&self, host: &stmt::Path, source_field: app::FieldId) -> stmt::Expr {
        let local = match &host.root {
            stmt::PathRoot::Variant { variant_id, .. } if host.projection.is_empty() => {
                let app::Model::EmbeddedEnum(em) = self.schema().app.model(variant_id.model) else {
                    unreachable!()
                };
                em.variant_fields(variant_id.index)
                    .iter()
                    .position(|field| field.id == source_field)
                    .unwrap()
            }
            _ => source_field.index,
        };
        let mut expr = field_path(host, local).into_stmt();
        stmt::visit_mut::for_each_expr_mut(&mut expr, |expr| {
            if let stmt::Expr::Reference(
                stmt::ExprReference::Field { nesting, .. } | stmt::ExprReference::Model { nesting },
            ) = expr
            {
                *nesting = 1;
            }
        });
        expr
    }
}

fn field_path(host: &stmt::Path, index: usize) -> stmt::Path {
    let mut path = host.clone();
    path.projection.push(index);
    path
}

fn projected_field<'a>(
    schema: &'a toasty_core::Schema,
    model: app::ModelId,
    expr: &stmt::Expr,
) -> Option<(&'a app::Field, &'a mapping::Field, stmt::Path)> {
    match expr {
        stmt::Expr::Reference(stmt::ExprReference::Field { nesting: 0, index }) => Some((
            &schema.app.model(model).fields()[*index],
            &schema.mapping_for(model).fields[*index],
            stmt::Path::field(model, *index),
        )),
        stmt::Expr::Project(project) => {
            let (field, mapping, path) = if let stmt::Expr::Variant(variant) = &*project.base {
                let (_, mapping, path) = projected_field(schema, model, &variant.base)?;
                let fields = schema
                    .app
                    .model(variant.variant.model)
                    .as_embedded_enum_unwrap()
                    .variant_fields(variant.variant.index);
                let mapping::Field::Enum(mapping) = mapping else {
                    return None;
                };
                let (&index, rest) = project.projection.as_slice().split_first()?;
                let mut path = stmt::Path::from_variant(path, variant.variant);
                path.projection.push(index);
                let field = &fields[index];
                let mapping = &mapping.variants[variant.variant.index].fields[index];
                return projected_subfield(schema, field, mapping, path, rest);
            } else {
                projected_field(schema, model, &project.base)?
            };
            projected_subfield(schema, field, mapping, path, project.projection.as_slice())
        }
        _ => None,
    }
}

fn projected_subfield<'a>(
    schema: &'a toasty_core::Schema,
    mut field: &'a app::Field,
    mut mapping: &'a mapping::Field,
    mut path: stmt::Path,
    steps: &[usize],
) -> Option<(&'a app::Field, &'a mapping::Field, stmt::Path)> {
    for index in steps {
        let app::FieldTy::Embedded(embedded) = &field.ty else {
            return None;
        };
        let mapping::Field::Struct(mapped) = mapping else {
            return None;
        };
        field = &schema.app.model(embedded.target).fields()[*index];
        mapping = &mapped.fields[*index];
        path.projection.push(*index);
    }
    Some((field, mapping, path))
}

/// Partitions includes for a field and merges modifiers on the field itself.
fn partition_includes(includes: &[FlatInclude], i: usize) -> FieldIncludes {
    let mut included = false;
    let mut unfiltered_self = false;
    let mut top_filter: Option<stmt::Expr> = None;
    let mut top_order_by = None;
    let mut sub_paths = Vec::new();
    for fi in includes {
        if let Some((first, rest)) = fi.projection.as_slice().split_first()
            && *first == i
        {
            included = true;
            if rest.is_empty() {
                top_order_by = fi.query.as_ref().and_then(|query| query.order_by.clone());
                match query_filter_expr(&fi.query).cloned() {
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
        included,
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

fn query_filter_expr(query: &Option<stmt::Query>) -> Option<&stmt::Expr> {
    match &query.as_ref()?.body {
        stmt::ExprSet::Select(select) => select.filter.expr.as_ref(),
        _ => None,
    }
}

fn query_has_modifiers(query: &Option<stmt::Query>) -> bool {
    query_filter_expr(query).is_some()
        || query.as_ref().is_some_and(|query| query.order_by.is_some())
}

/// Flatten an include [`stmt::Path`] into a single projection, folding any
/// `PathRoot::Variant` chain into a discriminant-index step.
///
/// The result uses LOCAL field indices for variant fields (matching the IR's
/// `Match` arm record convention and the local convention also used by
/// `Schema::resolve`). Include lowering walks the IR shape,
/// not the schema, so LOCAL is what `process_enum_arms` needs.
fn flatten_path(path: &stmt::Path) -> stmt::Projection {
    let mut acc = if let stmt::PathRoot::Variant { parent, variant_id } = &path.root {
        let mut acc = flatten_path(parent);
        acc.push(variant_id.index);
        acc
    } else {
        stmt::Projection::identity()
    };
    for step in path.projection.as_slice() {
        acc.push(*step);
    }
    acc
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
