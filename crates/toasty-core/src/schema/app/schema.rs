use super::{EnumVariant, Field, FieldId, FieldTy, Model, ModelId, VariantId};

use crate::{Result, stmt};
use indexmap::IndexMap;

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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
                FieldTy::HasMany(has_many) => {
                    current_field = has_many.target(self).as_root_unwrap().fields.get(*step)?;
                }
                FieldTy::HasOne(has_one) => {
                    current_field = has_one.target(self).as_root_unwrap().fields.get(*step)?;
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

        Ok(())
    }

    /// Go through all relations and link them to their pairs
    fn link_relations(&mut self) -> crate::Result<()> {
        // Because arbitrary models will be mutated throughout the linking
        // process, models cannot be iterated as that would hold a reference to
        // `self`. Instead, we use index based iteration.

        // First, link all HasMany relations. HasManys are linked first because
        // linking them may result in converting HasOne relations to BelongTo.
        // We need this conversion to happen before any of the other processing.
        for curr in 0..self.models.len() {
            if self.models[curr].is_embedded() {
                continue;
            }
            for index in 0..self.models[curr].as_root_unwrap().fields.len() {
                let model = &self.models[curr];
                let src = model.id();
                let field = &model.as_root_unwrap().fields[index];

                if let FieldTy::HasMany(has_many) = &field.ty {
                    let target = has_many.target;
                    let field_name = field.name.app_unwrap().to_string();
                    let pair = if has_many.pair.is_placeholder() {
                        self.find_has_many_pair(src, target, &field_name)?
                    } else {
                        self.validate_pair(src, target, &field_name, has_many.pair)?;
                        has_many.pair
                    };
                    self.models[curr].as_root_mut_unwrap().fields[index]
                        .ty
                        .as_has_many_mut_unwrap()
                        .pair = pair;
                }
            }
        }

        // Link HasOne relations and compute BelongsTo foreign keys
        for curr in 0..self.models.len() {
            if self.models[curr].is_embedded() {
                continue;
            }
            for index in 0..self.models[curr].as_root_unwrap().fields.len() {
                let model = &self.models[curr];
                let src = model.id();
                let field = &model.as_root_unwrap().fields[index];

                match &field.ty {
                    FieldTy::HasOne(has_one) => {
                        let target = has_one.target;
                        let field_name = field.name.app_unwrap().to_string();
                        let pair = if has_one.pair.is_placeholder() {
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
                            self.validate_pair(src, target, &field_name, has_one.pair)?;
                            has_one.pair
                        };

                        self.models[curr].as_root_mut_unwrap().fields[index]
                            .ty
                            .as_has_one_mut_unwrap()
                            .pair = pair;
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
                                FieldTy::HasMany(has_many) if has_many.pair == field_id => {
                                    assert!(pair.is_none());
                                    Some(
                                        self.models[target].as_root_unwrap().fields[target_index]
                                            .id,
                                    )
                                }
                                FieldTy::HasOne(has_one) if has_one.pair == field_id => {
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
