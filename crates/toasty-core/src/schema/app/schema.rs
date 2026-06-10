use super::{
    AutoStrategy, EnumVariant, Field, FieldId, FieldTy, Model, ModelId, ModelRoot, VariantId,
};

use crate::{Result, stmt};
use indexmap::IndexMap;
use std::collections::HashSet;

/// Resolve the `(partition, sort)` field IDs for an item-collection root.
///
/// Roots accept two syntactic key forms:
///
///   - simple `#[key(a, b)]` — both idents land in the partition vector;
///     reinterpreted as `(partition=a, sort=b)` here.
///   - named `#[key(partition = a, local = b)]` — already split into one
///     partition field and one local field.
fn pk_partition_and_sort_fields(root: &ModelRoot) -> Result<(FieldId, FieldId)> {
    let pk_index = &root.indices[root.primary_key.index.index];
    let partition_fields_count = pk_index.partition_fields().len();
    let local_fields_count = pk_index.local_fields().len();

    if local_fields_count == 0 && partition_fields_count == 2 {
        Ok((
            pk_index.partition_fields()[0].field,
            pk_index.partition_fields()[1].field,
        ))
    } else if partition_fields_count == 1 && local_fields_count == 1 {
        Ok((
            pk_index.partition_fields()[0].field,
            pk_index.local_fields()[0].field,
        ))
    } else {
        Err(crate::Error::invalid_schema(format!(
            "root model `{}` must declare a `(partition, sort)` key — \
             either `#[key(<partition>, <sort>)]` or \
             `#[key(partition = <p>, local = <s>)]`. Found {} partition \
             field(s) and {} local field(s).",
            root.name.upper_camel_case(),
            partition_fields_count,
            local_fields_count,
        )))
    }
}

/// Render a primitive field's application-level type as a short, user-facing
/// string suitable for inclusion in schema-validation error messages.
///
/// Non-primitive fields (relations, embedded types) are not expected as item
/// collection key components, so they render as `<non-primitive>` — a marker
/// that's clearly wrong if it ever surfaces.
fn field_ty_repr(field: &Field) -> String {
    match &field.ty {
        FieldTy::Primitive(p) => match &p.ty {
            stmt::Type::Bool => "bool".to_string(),
            stmt::Type::String => "String".to_string(),
            stmt::Type::I8 => "i8".to_string(),
            stmt::Type::I16 => "i16".to_string(),
            stmt::Type::I32 => "i32".to_string(),
            stmt::Type::I64 => "i64".to_string(),
            stmt::Type::U8 => "u8".to_string(),
            stmt::Type::U16 => "u16".to_string(),
            stmt::Type::U32 => "u32".to_string(),
            stmt::Type::U64 => "u64".to_string(),
            stmt::Type::F32 => "f32".to_string(),
            stmt::Type::F64 => "f64".to_string(),
            stmt::Type::Uuid => "Uuid".to_string(),
            stmt::Type::Bytes => "Bytes".to_string(),
            other => format!("{other:?}"),
        },
        _ => "<non-primitive>".to_string(),
    }
}

fn field_ty_is_string(field: &Field) -> bool {
    field_ty_repr(field) == "String"
}

/// The result of resolving a [`stmt::Projection`] through the application
/// schema.
///
/// A projection can resolve to either a concrete [`Field`] or an
/// [`EnumVariant`] (when the projection stops at a variant discriminant
/// without descending into the variant's data fields).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::app::Resolved;
///
/// match schema.resolve(root_model, &projection) {
///     Some(Resolved::Field(f)) => println!("field: {}", f.name),
///     Some(Resolved::Variant(v)) => println!("variant: {}", v.discriminant),
///     None => println!("could not resolve"),
/// }
/// ```
#[derive(Debug)]
pub enum Resolved<'a> {
    /// The projection resolved to a concrete field.
    Field(&'a Field),
    /// The projection resolved to an enum variant (discriminant-only access).
    Variant(&'a EnumVariant),
}

/// The top-level application schema, containing all registered models.
///
/// `Schema` is the entry point for looking up models, fields, and variants by
/// their IDs, and for resolving projections through the model graph.
///
/// Schemas are typically constructed via `Schema::from_macro` (called by the
/// `#[derive(Model)]` proc macro) or built manually for testing.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::Schema;
///
/// let schema = Schema::default();
/// assert_eq!(schema.models().count(), 0);
/// ```
#[derive(Debug, Default)]
pub struct Schema {
    /// All models in the schema, keyed by [`ModelId`].
    pub models: IndexMap<ModelId, Model>,
}

#[derive(Default)]
struct Builder {
    models: IndexMap<ModelId, Model>,
}

impl Schema {
    /// Builds a `Schema` from a slice of models, linking relations and
    /// validating consistency.
    ///
    /// This is the primary constructor used by the derive macro infrastructure.
    pub fn from_macro(models: impl IntoIterator<Item = Model>) -> Result<Self> {
        Builder::from_macro(models)
    }

    /// Returns a reference to the [`Field`] identified by `id`.
    ///
    /// # Panics
    ///
    /// Panics if the model or field index is invalid.
    pub fn field(&self, id: FieldId) -> &Field {
        let fields = match self.model(id.model) {
            Model::Root(root) => &root.fields,
            Model::EmbeddedStruct(embedded) => &embedded.fields,
            Model::EmbeddedEnum(e) => &e.fields,
        };
        fields.get(id.index).expect("invalid field ID")
    }

    /// Returns a reference to the [`EnumVariant`] identified by `id`.
    ///
    /// # Panics
    ///
    /// Panics if the model is not an [`EmbeddedEnum`](super::EmbeddedEnum) or
    /// the variant index is out of bounds.
    pub fn variant(&self, id: VariantId) -> &EnumVariant {
        let Model::EmbeddedEnum(e) = self.model(id.model) else {
            panic!("VariantId references a non-enum model");
        };
        e.variants.get(id.index).expect("invalid variant index")
    }

    /// Returns an iterator over all models in the schema.
    pub fn models(&self) -> impl Iterator<Item = &Model> {
        self.models.values()
    }

    /// Try to get a model by ID, returning `None` if not found.
    pub fn get_model(&self, id: impl Into<ModelId>) -> Option<&Model> {
        self.models.get(&id.into())
    }

    /// Returns a reference to the [`Model`] identified by `id`.
    ///
    /// # Panics
    ///
    /// Panics if no model with the given ID exists in the schema.
    pub fn model(&self, id: impl Into<ModelId>) -> &Model {
        self.models.get(&id.into()).expect("invalid model ID")
    }

    /// Resolve a projection through the schema, returning either a field or
    /// an enum variant.
    ///
    /// Starting from the root model, walks through each step of the projection,
    /// resolving fields, following relations/embedded types, and recognizing
    /// enum variant discriminant access.
    ///
    /// Returns `None` if:
    /// - The projection is empty
    /// - Any step references an invalid field/variant index
    /// - A step tries to project through a primitive type
    pub fn resolve<'a>(
        &'a self,
        root: &'a Model,
        projection: &stmt::Projection,
    ) -> Option<Resolved<'a>> {
        let [first, rest @ ..] = projection.as_slice() else {
            return None;
        };

        // Get the first field from the root model
        let mut current_field = root.as_root_unwrap().fields.get(*first)?;

        // Walk through remaining steps. Uses a manual iterator because
        // embedded enums consume two steps (variant discriminant + field index).
        let mut steps = rest.iter();
        while let Some(step) = steps.next() {
            match &current_field.ty {
                FieldTy::Primitive(..) => {
                    // Cannot project through primitive fields
                    return None;
                }
                FieldTy::Embedded(embedded) => {
                    let target = self.model(embedded.target);
                    match target {
                        Model::EmbeddedStruct(s) => {
                            current_field = s.fields.get(*step)?;
                        }
                        Model::EmbeddedEnum(e) => {
                            let variant = e.variants.get(*step)?;

                            // Check if there's a field index step after the variant
                            if let Some(field_step) = steps.next() {
                                // Two steps: variant disc + field index → field
                                current_field = e.fields.get(*field_step)?;
                            } else {
                                // Single step: variant discriminant only → variant
                                return Some(Resolved::Variant(variant));
                            }
                        }
                        _ => return None,
                    }
                }
                FieldTy::BelongsTo(belongs_to) => {
                    current_field = belongs_to.target(self).as_root_unwrap().fields.get(*step)?;
                }
                FieldTy::Has(has) => {
                    current_field = has.target(self).as_root_unwrap().fields.get(*step)?;
                }
                FieldTy::HasItems(has_items) => {
                    // HasItems walks into the child (target) model the same
                    // way `Has` does at the projection layer; the distinction
                    // is in lowering (R2.9), not projection resolution.
                    current_field = has_items.target(self).as_root_unwrap().fields.get(*step)?;
                }
                FieldTy::ItemParent(item_parent) => {
                    // Item-parent navigation walks into the parent model the
                    // same way `BelongsTo` does at the projection layer; the
                    // distinction is in lowering (R2.9), which lands in
                    // B4.8/B4.9 — not in projection resolution.
                    current_field = item_parent
                        .target(self)
                        .as_root_unwrap()
                        .fields
                        .get(*step)?;
                }
                FieldTy::Via(via) => {
                    current_field = via.target(self).as_root_unwrap().fields.get(*step)?;
                }
            };
        }

        Some(Resolved::Field(current_field))
    }

    /// Resolve a projection to a field, walking through the schema.
    ///
    /// Returns `None` if the projection is empty, invalid, or resolves to an
    /// enum variant rather than a field.
    pub fn resolve_field<'a>(
        &'a self,
        root: &'a Model,
        projection: &stmt::Projection,
    ) -> Option<&'a Field> {
        match self.resolve(root, projection) {
            Some(Resolved::Field(field)) => Some(field),
            _ => None,
        }
    }

    /// Resolves a [`stmt::Path`] to a [`Field`] by extracting the root model
    /// from the path and delegating to [`resolve_field`](Schema::resolve_field).
    pub fn resolve_field_path<'a>(&'a self, path: &stmt::Path) -> Option<&'a Field> {
        let model = self.model(path.root.as_model_unwrap());
        self.resolve_field(model, &path.projection)
    }
}

impl Builder {
    pub(crate) fn from_macro(models: impl IntoIterator<Item = Model>) -> Result<Schema> {
        let mut builder = Self { ..Self::default() };

        for model in models {
            builder.models.insert(model.id(), model);
        }

        builder.process_models()?;
        builder.into_schema()
    }

    fn into_schema(self) -> Result<Schema> {
        Ok(Schema {
            models: self.models,
        })
    }

    fn process_models(&mut self) -> Result<()> {
        // All models have been discovered and initialized at some level, now do
        // the relation linking.
        self.link_relations()?;
        self.resolve_via_targets()?;
        self.verify_no_eager_load_cycles()?;
        self.validate_item_collections()?;

        // Promote `FieldTy::Has` whose resolved pair points at an
        // `ItemParent` into `FieldTy::HasItems` (R2.9). This must run
        // after `validate_item_collections` (whose inverse-Has check
        // expects parent fields to still be `Has`) and before
        // `verify_relations_are_indexed` (which walks `Has` and assumes
        // BelongsTo pairing).
        self.promote_has_items_from_item_parent_pairs();

        Ok(())
    }

    /// Resolve the `target` of every scalar-terminal `via` relation.
    ///
    /// A relation-terminal via knows its target at macro-expansion time (the
    /// field's element type). A scalar-terminal via does not — the model that
    /// owns the projected field is whatever the relation chain reaches — so the
    /// derive leaves `target` unset and it is computed here by walking the
    /// chain. Runs after [`link_relations`](Self::link_relations) so every
    /// `Has`/`BelongsTo` target is final.
    fn resolve_via_targets(&mut self) -> crate::Result<()> {
        // Collect first; the walk borrows other models immutably.
        let mut updates = Vec::new();

        for curr in 0..self.models.len() {
            if self.models[curr].is_embedded() {
                continue;
            }
            let src = self.models[curr].id();
            for index in 0..self.models[curr].as_root_unwrap().fields.len() {
                let field = &self.models[curr].as_root_unwrap().fields[index];
                let FieldTy::Via(via) = &field.ty else {
                    continue;
                };
                let Some(terminal) = via.terminal else {
                    continue;
                };

                // The relation chain is the path minus its terminal field.
                let projection = via.path.projection.as_slice();
                let relation_steps = &projection[..projection.len() - 1];
                let field_name = field.name.app_unwrap().to_string();
                let target = self.walk_via_relation_chain(src, relation_steps, &field_name)?;

                // The terminal must be a stored scalar on the reached model.
                let terminal_field = &self.models[&target].as_root_unwrap().fields[terminal];
                if !matches!(terminal_field.ty, FieldTy::Primitive(_)) {
                    return Err(crate::Error::invalid_schema(format!(
                        "the `via` terminal `{}::{}` is not a scalar field",
                        self.models[&target].name().upper_camel_case(),
                        terminal_field.name.app_unwrap(),
                    )));
                }

                updates.push((curr, index, target));
            }
        }

        for (curr, index, target) in updates {
            if let FieldTy::Via(via) = &mut self.models[curr].as_root_mut_unwrap().fields[index].ty
            {
                via.target = target;
            }
        }

        Ok(())
    }

    /// Walk a via relation chain, splicing any nested via's own chain, and
    /// return the model it reaches. Every step must be a relation.
    fn walk_via_relation_chain(
        &self,
        declaring: ModelId,
        steps: &[usize],
        field_name: &str,
    ) -> crate::Result<ModelId> {
        let mut current = declaring;
        let mut queue: Vec<usize> = steps.iter().rev().copied().collect();

        while let Some(idx) = queue.pop() {
            let field = &self.models[&current].as_root_unwrap().fields[idx];
            match &field.ty {
                FieldTy::Has(has) => current = has.target,
                FieldTy::BelongsTo(belongs_to) => current = belongs_to.target,
                // A nested via contributes its own relation chain (its terminal,
                // if scalar, is not part of the path through it).
                FieldTy::Via(inner) => {
                    let inner_projection = inner.path.projection.as_slice();
                    let inner_steps = match inner.terminal {
                        Some(_) => &inner_projection[..inner_projection.len() - 1],
                        None => inner_projection,
                    };
                    for step in inner_steps.iter().rev() {
                        queue.push(*step);
                    }
                }
                _ => {
                    return Err(crate::Error::invalid_schema(format!(
                        "the `via` path for `{}::{}` traverses `{}`, which is not a relation",
                        self.models[&declaring].name().upper_camel_case(),
                        field_name,
                        field.name.app_unwrap(),
                    )));
                }
            }
        }

        Ok(current)
    }

    /// Replace every `FieldTy::Has` whose resolved `pair_id` points at a
    /// `FieldTy::ItemParent` with `FieldTy::HasItems` carrying the same
    /// `target`/`expr_ty`/`cardinality`/`pair_id`. The macro emits `Has`
    /// for every `#[has_many]`/`#[has_one]` because at expansion time it
    /// can't see the target's `ItemParent` declaration; we promote here,
    /// after `link_relations` resolves the pair, so downstream verifiers
    /// and lowering see the final shape.
    fn promote_has_items_from_item_parent_pairs(&mut self) {
        // Collect promotions in a pass over the resolved schema, then
        // apply them in a second pass to avoid borrowing `self.models`
        // mutably during the lookup.
        let mut promotions: Vec<(ModelId, usize)> = Vec::new();
        for (model_id, model) in &self.models {
            let Some(root) = model.as_root() else {
                continue;
            };
            for (idx, field) in root.fields.iter().enumerate() {
                let FieldTy::Has(has) = &field.ty else {
                    continue;
                };
                if has.pair_id.is_placeholder() {
                    continue;
                }
                let pair_model = match self.models.get(&has.pair_id.model) {
                    Some(m) => m,
                    None => continue,
                };
                let pair_field = match pair_model.as_root() {
                    Some(r) => &r.fields[has.pair_id.index],
                    None => continue,
                };
                if matches!(pair_field.ty, FieldTy::ItemParent(_)) {
                    promotions.push((*model_id, idx));
                }
            }
        }

        for (model_id, idx) in promotions {
            let root = self
                .models
                .get_mut(&model_id)
                .expect("promotion target is registered")
                .as_root_mut_unwrap();
            // Re-wrap the same data as `HasItems`. The first pass already
            // confirmed this slot holds a `Has`, so clone its data out and
            // overwrite the variant — no placeholder swap is needed.
            let FieldTy::Has(has) = &root.fields[idx].ty else {
                unreachable!("promotion targeted a non-Has field");
            };
            let promoted = FieldTy::HasItems(super::HasItems {
                target: has.target,
                expr_ty: has.expr_ty.clone(),
                cardinality: has.cardinality.clone(),
                pair_id: has.pair_id,
            });
            root.fields[idx].ty = promoted;
        }
    }

    /// Validate item-collection key inheritance at schema-build time.
    ///
    /// For each child (a model with `parent.is_some()`):
    ///
    /// 1. Walk the `parent` chain to locate the root, with cycle detection.
    /// 2. Confirm the root has exactly one partition-key and one local
    ///    (sort-key) field — item-collection roots use a single
    ///    `#[key(partition, sort)]` pair.
    /// 3. Confirm the root's sort field is `String` (R7.1).
    /// 4. For every model in the chain except the root itself, confirm it has
    ///    fields whose names AND types match the root's partition + sort
    ///    fields.
    /// 5. Require every model in the chain to tag its sort field `#[auto]`
    ///    (R2.6) and promote `AutoStrategy::String` →
    ///    `AutoStrategy::ItemCollectionRootSortKey` (for the chain root) or
    ///    `AutoStrategy::ItemCollectionChildSortKey` (for descendants) (R7.5)
    ///    so the row's sort column carries the appropriate hierarchical
    ///    encoding.
    ///
    /// Cross-model invariants like (4) cannot be checked at macro-expansion
    /// time — each `#[derive(Model)]` invocation sees only its own struct.
    fn validate_item_collections(&mut self) -> Result<()> {
        // Triples of (member_model_id, sort_field_index, is_root) collected
        // during the immutable validation pass; applied as a `String` ->
        // `ItemCollectionRoot/ChildSortKey` promotion in a follow-up mutable
        // pass so we don't borrow `self.models` mutably during the chain walk.
        let mut promotions: Vec<(ModelId, usize, bool)> = Vec::new();

        let model_ids: Vec<ModelId> = self.models.keys().copied().collect();
        for id in model_ids {
            let child_root = match self.models.get(&id) {
                Some(Model::Root(r)) => r,
                _ => continue,
            };

            // Skip non-children (models that aren't part of any item collection).
            if child_root.parent.is_none() {
                continue;
            }

            let starting_name = child_root.name.upper_camel_case();

            // Walk the parent chain to find the root, with cycle detection.
            let mut visited: HashSet<ModelId> = HashSet::new();
            visited.insert(id);
            let mut cursor = id;
            loop {
                let m = self.models.get(&cursor).ok_or_else(|| {
                    crate::Error::invalid_schema(format!(
                        "item-collection chain starting at `{}` references an unregistered \
                         parent model; ensure all ancestors appear in `Db::builder().models(...)`",
                        starting_name,
                    ))
                })?;
                let r = m.as_root_unwrap();
                match r.parent {
                    None => break,
                    Some(parent_id) => {
                        if !visited.insert(parent_id) {
                            return Err(crate::Error::invalid_schema(format!(
                                "item-collection cycle detected starting at `{}`",
                                starting_name,
                            )));
                        }
                        cursor = parent_id;
                    }
                }
            }
            let root = self
                .models
                .get(&cursor)
                .expect("walked to a registered model")
                .as_root_unwrap();

            // Read the root's primary key. Item-collection roots accept two
            // syntactic forms:
            //
            //   - simple `#[key(a, b)]` — every bare ident lands in
            //     `partition`; here we reinterpret the two-arg case as
            //     `(partition=a, sort=b)`.
            //   - named `#[key(partition = a, local = b)]` — already split
            //     into one partition field and one local field.
            //
            // Both forms produce the same downstream PK shape; the user picks
            // whichever they prefer per model.
            let (partition_field_id, sort_field_id) = pk_partition_and_sort_fields(root)?;

            let partition_root = &root.fields[partition_field_id.index];
            let sort_root = &root.fields[sort_field_id.index];

            // R7.1: the sort field on the root must be `String`.
            if !field_ty_is_string(sort_root) {
                return Err(crate::Error::invalid_schema(format!(
                    "sort field `{}` must be `String`; found `{}`",
                    sort_root.name.app_unwrap(),
                    field_ty_repr(sort_root),
                )));
            }

            let partition_name = partition_root.name.app_unwrap().to_string();
            let partition_repr = field_ty_repr(partition_root);
            let sort_name = sort_root.name.app_unwrap().to_string();
            let sort_repr = field_ty_repr(sort_root);
            let root_name = root.name.upper_camel_case();

            // For every model in the chain except the root itself, confirm it
            // declares fields whose names AND types match the root's
            // partition + sort fields.
            for child_id in &visited {
                if *child_id == cursor {
                    continue;
                }
                let child = self
                    .models
                    .get(child_id)
                    .expect("visited model is registered")
                    .as_root_unwrap();

                for (role, expected_name, expected_repr) in [
                    (
                        "partition",
                        partition_name.as_str(),
                        partition_repr.as_str(),
                    ),
                    ("sort", sort_name.as_str(), sort_repr.as_str()),
                ] {
                    let actual = child
                        .fields
                        .iter()
                        .find(|f| f.name.app.as_deref() == Some(expected_name));
                    let actual = actual.ok_or_else(|| {
                        crate::Error::invalid_schema(format!(
                            "expected field `{}: {}` matching root `{}`'s {} key, found none on `{}`",
                            expected_name,
                            expected_repr,
                            root_name,
                            role,
                            child.name.upper_camel_case(),
                        ))
                    })?;
                    if field_ty_repr(actual) != expected_repr {
                        return Err(crate::Error::invalid_schema(format!(
                            "field `{}` on `{}` has type `{}`; root `{}` declares `{}`",
                            expected_name,
                            child.name.upper_camel_case(),
                            field_ty_repr(actual),
                            root_name,
                            expected_repr,
                        )));
                    }
                }
            }

            // R2.6 + R7.5: every model in the chain must tag its sort field
            // `#[auto]`; promote `AutoStrategy::String` ->
            // `AutoStrategy::ItemCollectionRootSortKey` (for the chain root)
            // or `AutoStrategy::ItemCollectionChildSortKey` (for descendants).
            // Resolve the sort field index for each member by name, then
            // validate the existing `#[auto]` strategy. Apply the promotion
            // in a second pass below.
            for member_id in &visited {
                let member = self
                    .models
                    .get(member_id)
                    .expect("visited model is registered")
                    .as_root_unwrap();
                let member_sort_idx = member
                    .fields
                    .iter()
                    .position(|f| f.name.app.as_deref() == Some(sort_name.as_str()))
                    .expect("sort field name verified above");
                let member_sort = &member.fields[member_sort_idx];
                let is_root = *member_id == cursor;
                match &member_sort.auto {
                    None => {
                        return Err(crate::Error::invalid_schema(format!(
                            "sort field `{}` on item-collection model `{}` must be tagged `#[auto]`",
                            member_sort.name.app_unwrap(),
                            member.name.upper_camel_case(),
                        )));
                    }
                    Some(AutoStrategy::String) => {
                        promotions.push((*member_id, member_sort_idx, is_root));
                    }
                    Some(other) => {
                        return Err(crate::Error::invalid_schema(format!(
                            "sort field `{}` on `{}` has incompatible auto strategy `{:?}`; \
                             item-collection sort fields must be `String` (which implies `AutoStrategy::String`)",
                            member_sort.name.app_unwrap(),
                            member.name.upper_camel_case(),
                            other,
                        )));
                    }
                }
            }
        }

        // Apply collected promotions: bare `String` -> `ItemCollectionRoot/
        // ChildSortKey` per the chain position recorded above.
        for (model_id, field_idx, is_root) in promotions {
            let root = self
                .models
                .get_mut(&model_id)
                .expect("promotion target is registered")
                .as_root_mut_unwrap();
            let strategy = if is_root {
                AutoStrategy::ItemCollectionRootSortKey
            } else {
                AutoStrategy::ItemCollectionChildSortKey
            };
            root.fields[field_idx].auto = Some(strategy);
        }

        // R1.5: every `#[item_parent]` field on a child requires a matching
        // inverse `#[has_many]` / `#[has_one]` on the parent that targets
        // the child. Item-collection membership is symmetric — proc macros
        // only see one struct at a time, so the parent's `#[derive(Model)]`
        // can't synthesise the inverse; the user declares it and schema
        // build verifies the round-trip.
        //
        // Validation runs **before** `promote_has_items_from_item_parent_pairs`,
        // so the parent's inverse field is still `FieldTy::Has` (the macro
        // emits `Has` for every `#[has_many]`/`#[has_one]`).
        self.validate_item_parent_inverse_has()?;

        Ok(())
    }

    /// Verify that every model with a `FieldTy::ItemParent { target }` has
    /// at least one `FieldTy::Has` field on `target` whose own `target`
    /// resolves to the child's `ModelId`. Item-collection parents must
    /// expose their members.
    fn validate_item_parent_inverse_has(&self) -> Result<()> {
        for (child_id, child_model) in &self.models {
            let Some(child_root) = child_model.as_root() else {
                continue;
            };
            for field in &child_root.fields {
                let FieldTy::ItemParent(item_parent) = &field.ty else {
                    continue;
                };
                let parent_id = item_parent.target;
                let parent_root = match self.models.get(&parent_id) {
                    Some(m) => m.as_root().expect("ItemParent target must be a root model"),
                    None => continue,
                };

                let inverse_exists = parent_root.fields.iter().any(|pf| match &pf.ty {
                    FieldTy::Has(has) => has.target == *child_id,
                    _ => false,
                });

                if !inverse_exists {
                    return Err(crate::Error::invalid_schema(format!(
                        "model `{parent}` is the target of `#[item_parent]` on `{child}` but \
                         declares no `#[has_many]` or `#[has_one]` field of type \
                         `Deferred<Vec<{child}>>` (or `Deferred<{child}>`). Item-collection \
                         parents must expose their members.",
                        parent = parent_root.name.upper_camel_case(),
                        child = child_root.name.upper_camel_case(),
                    )));
                }
            }
        }
        Ok(())
    }

    fn verify_no_eager_load_cycles(&self) -> crate::Result<()> {
        let mut visited = HashSet::new();
        let mut model_stack = Vec::new();
        let mut field_stack = Vec::new();

        for model in self.models.values() {
            if model.is_embedded() {
                continue;
            }
            self.visit_eager_load_graph(
                model.id(),
                &mut visited,
                &mut model_stack,
                &mut field_stack,
            )?;
        }

        Ok(())
    }

    fn visit_eager_load_graph(
        &self,
        model_id: ModelId,
        visited: &mut HashSet<ModelId>,
        model_stack: &mut Vec<ModelId>,
        field_stack: &mut Vec<FieldId>,
    ) -> crate::Result<()> {
        if model_stack.contains(&model_id) {
            return Ok(());
        }

        if !visited.insert(model_id) {
            return Ok(());
        }

        model_stack.push(model_id);

        let model = self.models[&model_id].as_root_unwrap();
        for field in &model.fields {
            let Some(target) = eager_relation_target(field) else {
                continue;
            };

            if let Some(pos) = model_stack.iter().position(|id| *id == target) {
                let mut cycle = field_stack[pos..].to_vec();
                cycle.push(field.id);
                return Err(crate::Error::invalid_schema(format!(
                    "eager relation cycle detected: {}",
                    self.format_eager_load_cycle(&cycle, target)
                )));
            }

            field_stack.push(field.id);
            self.visit_eager_load_graph(target, visited, model_stack, field_stack)?;
            field_stack.pop();
        }

        model_stack.pop();
        Ok(())
    }

    fn format_eager_load_cycle(&self, fields: &[FieldId], target: ModelId) -> String {
        let mut parts = Vec::new();
        for field_id in fields {
            let model = &self.models[&field_id.model];
            let field = &model.as_root_unwrap().fields[field_id.index];
            parts.push(format!(
                "{}::{}",
                model.name().upper_camel_case(),
                field.name.app_unwrap()
            ));
        }
        parts.push(self.models[&target].name().upper_camel_case());
        parts.join(" -> ")
    }

    /// Go through all relations and link them to their pairs
    fn link_relations(&mut self) -> crate::Result<()> {
        // Because arbitrary models will be mutated throughout the linking
        // process, models cannot be iterated as that would hold a reference to
        // `self`. Instead, we use index based iteration.

        // First, link all has-many relations. Has-manys are linked first because
        // linking them may result in converting has-one relations to BelongTo.
        // We need this conversion to happen before any of the other processing.
        for curr in 0..self.models.len() {
            if self.models[curr].is_embedded() {
                continue;
            }
            for index in 0..self.models[curr].as_root_unwrap().fields.len() {
                let model = &self.models[curr];
                let src = model.id();
                let field = &model.as_root_unwrap().fields[index];

                if let FieldTy::Has(has) = &field.ty
                    && has.is_many()
                {
                    let target = has.target;
                    let field_name = field.name.app_unwrap().to_string();
                    let pair = if has.pair_id.is_placeholder() {
                        self.find_has_many_pair(src, target, &field_name)?
                    } else {
                        self.validate_pair(src, target, &field_name, has.pair_id)?;
                        has.pair_id
                    };
                    self.models[curr].as_root_mut_unwrap().fields[index]
                        .ty
                        .as_has_mut_unwrap()
                        .pair_id = pair;
                }
            }
        }

        // Link has-one relations and compute BelongsTo foreign keys
        for curr in 0..self.models.len() {
            if self.models[curr].is_embedded() {
                continue;
            }
            for index in 0..self.models[curr].as_root_unwrap().fields.len() {
                let model = &self.models[curr];
                let src = model.id();
                let field = &model.as_root_unwrap().fields[index];

                match &field.ty {
                    FieldTy::Has(has) if has.is_one() => {
                        let target = has.target;
                        let field_name = field.name.app_unwrap().to_string();
                        let pair = if has.pair_id.is_placeholder() {
                            match self.find_belongs_to_pair(src, target, &field_name)? {
                                Some(pair) => pair,
                                None => {
                                    return Err(crate::Error::invalid_schema(format!(
                                        "field `{}::{}` has no matching `BelongsTo` relation on the target model",
                                        self.models[curr].name().upper_camel_case(),
                                        field_name,
                                    )));
                                }
                            }
                        } else {
                            self.validate_pair(src, target, &field_name, has.pair_id)?;
                            has.pair_id
                        };

                        self.models[curr].as_root_mut_unwrap().fields[index]
                            .ty
                            .as_has_mut_unwrap()
                            .pair_id = pair;
                    }
                    FieldTy::BelongsTo(belongs_to) => {
                        assert!(!belongs_to.foreign_key.is_placeholder());
                        continue;
                    }
                    _ => {}
                }
            }
        }

        // Finally, link BelongsTo relations with their pairs
        for curr in 0..self.models.len() {
            if self.models[curr].is_embedded() {
                continue;
            }
            for index in 0..self.models[curr].as_root_unwrap().fields.len() {
                let model = &self.models[curr];
                let field_id = model.as_root_unwrap().fields[index].id;

                let pair = match &self.models[curr].as_root_unwrap().fields[index].ty {
                    FieldTy::BelongsTo(belongs_to) => {
                        let mut pair = None;
                        let target = match self.models.get_index_of(&belongs_to.target) {
                            Some(target) => target,
                            None => {
                                let model = &self.models[curr];
                                return Err(crate::Error::invalid_schema(format!(
                                    "field `{}::{}` references a model that was not registered \
                                     with the schema; did you forget to register it with `Db::builder()`?",
                                    model.name().upper_camel_case(),
                                    model.as_root_unwrap().fields[index].name(),
                                )));
                            }
                        };

                        for target_index in 0..self.models[target].as_root_unwrap().fields.len() {
                            pair = match &self.models[target].as_root_unwrap().fields[target_index]
                                .ty
                            {
                                FieldTy::Has(has) if has.pair_id == field_id => {
                                    assert!(pair.is_none());
                                    Some(
                                        self.models[target].as_root_unwrap().fields[target_index]
                                            .id,
                                    )
                                }
                                _ => continue,
                            }
                        }

                        if pair.is_none() {
                            continue;
                        }

                        pair
                    }
                    _ => continue,
                };

                self.models[curr].as_root_mut_unwrap().fields[index]
                    .ty
                    .as_belongs_to_mut_unwrap()
                    .pair = pair;
            }
        }

        Ok(())
    }

    /// Locate the inverse field on `target` that pairs with a `Has` on `src`.
    ///
    /// A `Has`/`HasOne` on the parent pairs with either:
    ///   - a `BelongsTo` on the target (classic FK relationship), or
    ///   - an `ItemParent` on the target (item-collection child whose
    ///     primary key already encodes the parent — symmetric IC, R2.9).
    ///
    /// Both candidate kinds satisfy the same role here: a single field on
    /// `target` that names `src` as its parent. The schema linker promotes
    /// the parent's `Has` to `HasItems` after this resolution if the
    /// matched candidate turned out to be `ItemParent` (Step 4 of B4.9).
    fn find_belongs_to_pair(
        &self,
        src: ModelId,
        target: ModelId,
        field_name: &str,
    ) -> crate::Result<Option<FieldId>> {
        let src_model = &self.models[&src];

        let target = match self.models.get(&target) {
            Some(target) => target,
            None => {
                return Err(crate::Error::invalid_schema(format!(
                    "field `{}::{}` references a model that was not registered with the schema; \
                     did you forget to register it with `Db::builder()`?",
                    src_model.name().upper_camel_case(),
                    field_name,
                )));
            }
        };

        // Find all candidate inverse relations that reference the model:
        // either a `BelongsTo` (classic FK pairing) or an `ItemParent`
        // (item-collection symmetric-key pairing, R2.9).
        let candidates: Vec<_> = target
            .as_root_unwrap()
            .fields
            .iter()
            .filter(|field| match &field.ty {
                FieldTy::BelongsTo(rel) => rel.target == src,
                FieldTy::ItemParent(rel) => rel.target == src,
                _ => false,
            })
            .collect();

        match &candidates[..] {
            [field] => Ok(Some(field.id)),
            [] => Ok(None),
            _ => Err(crate::Error::invalid_schema(format!(
                "model `{}` has more than one `BelongsTo` or `ItemParent` relation \
                 targeting `{}`; disambiguate by adding `pair = <field>` on the paired \
                 `has_many`/`has_one` field",
                target.name().upper_camel_case(),
                src_model.name().upper_camel_case(),
            ))),
        }
    }

    fn find_has_many_pair(
        &mut self,
        src: ModelId,
        target: ModelId,
        field_name: &str,
    ) -> crate::Result<FieldId> {
        if let Some(field_id) = self.find_belongs_to_pair(src, target, field_name)? {
            return Ok(field_id);
        }

        Err(crate::Error::invalid_schema(format!(
            "field `{}::{}` has no matching `BelongsTo` relation on the target model",
            self.models[&src].name().upper_camel_case(),
            field_name,
        )))
    }

    /// Verify that `pair` — resolved from `#[has_many(pair = <field>)]` or
    /// `#[has_one(pair = <field>)]` via `field_name_to_id` on the target —
    /// names a `BelongsTo` field on `target` that points back at `src`.
    fn validate_pair(
        &self,
        src: ModelId,
        target: ModelId,
        field_name: &str,
        pair: FieldId,
    ) -> crate::Result<()> {
        let src_model = &self.models[&src];

        let target_model = match self.models.get(&target) {
            Some(target) => target,
            None => {
                return Err(crate::Error::invalid_schema(format!(
                    "field `{}::{}` references a model that was not registered with the schema; \
                     did you forget to register it with `Db::builder()`?",
                    src_model.name().upper_camel_case(),
                    field_name,
                )));
            }
        };

        if pair.model != target {
            return Err(crate::Error::invalid_schema(format!(
                "field `{}::{}` specifies a `pair` on a model other than its target `{}`",
                src_model.name().upper_camel_case(),
                field_name,
                target_model.name().upper_camel_case(),
            )));
        }

        let paired = &target_model.as_root_unwrap().fields[pair.index];
        match &paired.ty {
            FieldTy::BelongsTo(rel) if rel.target == src => Ok(()),
            FieldTy::ItemParent(rel) if rel.target == src => Ok(()),
            _ => Err(crate::Error::invalid_schema(format!(
                "field `{}::{}` specifies `pair = {}`, but `{}::{}` is not a `BelongsTo` \
                 or `ItemParent` targeting `{}`",
                src_model.name().upper_camel_case(),
                field_name,
                paired.name.app_unwrap(),
                target_model.name().upper_camel_case(),
                paired.name.app_unwrap(),
                src_model.name().upper_camel_case(),
            ))),
        }
    }
}

fn eager_relation_target(field: &Field) -> Option<ModelId> {
    if field.deferred {
        return None;
    }

    field.relation_target_id()
}
