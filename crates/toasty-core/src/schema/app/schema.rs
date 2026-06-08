use super::{EnumVariant, Field, FieldId, FieldPrimitive, FieldTy, Model, ModelId, VariantId};

use crate::{Result, stmt};
use indexmap::IndexMap;
use std::collections::HashSet;

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

    /// The document field layout of the embedded struct `id`, resolved on
    /// demand: each field's name and `stmt::Type`, in declaration order.
    ///
    /// This is the on-demand replacement for the former `TypeDocument` — the
    /// embedded model is the single source of truth for a document column's
    /// shape. A field typed `Type::Model(nested)` (or `List(Model(nested))`)
    /// signals a nested document the caller recurses into.
    ///
    /// Panics if `id` is not an embedded struct, or a field is unnamed; both
    /// are rejected at schema build, so neither occurs at runtime.
    pub fn document_fields(&self, id: ModelId) -> impl Iterator<Item = (&str, &stmt::Type)> {
        let Model::EmbeddedStruct(embedded) = self.model(id) else {
            panic!("document type {id:?} is not an embedded struct");
        };
        embedded.fields.iter().map(|field| {
            let name = field
                .name
                .app
                .as_deref()
                .expect("document field must have an app name");
            (name, field.expr_ty())
        })
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
                // A `#[document]` embed stores as one column whose sub-fields
                // live in the document type rather than as `app::Field`s. The
                // remaining steps index into the document; validate them and
                // resolve to the document field itself (the leaf has no
                // `app::Field`). The path was already type-checked by the
                // generated accessors.
                FieldTy::Primitive(FieldPrimitive {
                    ty: stmt::Type::Document(doc),
                    ..
                }) => {
                    return resolve_document_steps(doc, *step, &mut steps)
                        .then_some(Resolved::Field(current_field));
                }
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

        // Find all BelongsTo relations that reference the model
        let belongs_to: Vec<_> = target
            .as_root_unwrap()
            .fields
            .iter()
            .filter(|field| match &field.ty {
                FieldTy::BelongsTo(rel) => rel.target == src,
                _ => false,
            })
            .collect();

        match &belongs_to[..] {
            [field] => Ok(Some(field.id)),
            [] => Ok(None),
            _ => Err(crate::Error::invalid_schema(format!(
                "model `{}` has more than one `BelongsTo` relation targeting `{}`; \
                 disambiguate by adding `pair = <field>` on the paired `has_many`/`has_one` \
                 field",
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
            _ => Err(crate::Error::invalid_schema(format!(
                "field `{}::{}` specifies `pair = {}`, but `{}::{}` is not a `BelongsTo` \
                 targeting `{}`",
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

/// Validates that a projection descends a `#[document]` type's fields: `first`
/// indexes into `doc`, then each remaining step descends into the nested
/// document at that field (if any). Returns `false` if a step is out of range
/// or descends past a scalar leaf.
fn resolve_document_steps(
    doc: &stmt::TypeDocument,
    first: usize,
    rest: &mut std::slice::Iter<'_, usize>,
) -> bool {
    let Some(field) = doc.fields.get(first) else {
        return false;
    };

    match &field.ty {
        stmt::Type::Document(nested) => match rest.next() {
            Some(&next) => resolve_document_steps(nested, next, rest),
            // Path stops at a sub-document (the whole sub-embed) — valid.
            None => true,
        },
        // A scalar leaf: the path must end here.
        _ => rest.next().is_none(),
    }
}
