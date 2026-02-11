use super::{Field, FieldId, FieldTy, Model, ModelId};

use crate::{stmt, Result};
use indexmap::IndexMap;

#[derive(Debug, Default)]
pub struct Schema {
    pub models: IndexMap<ModelId, Model>,
}

#[derive(Default)]
struct Builder {
    models: IndexMap<ModelId, Model>,
}

impl Schema {
    pub fn from_macro(models: &[Model]) -> Result<Self> {
        Builder::from_macro(models)
    }

    /// Get a field by ID
    pub fn field(&self, id: FieldId) -> &Field {
        self.model(id.model)
            .fields
            .get(id.index)
            .expect("invalid field ID")
    }

    pub fn models(&self) -> impl Iterator<Item = &Model> {
        self.models.values()
    }

    /// Get a model by ID
    pub fn model(&self, id: impl Into<ModelId>) -> &Model {
        self.models.get(&id.into()).expect("invalid model ID")
    }

    /// Resolve a projection to a field, walking through the schema
    ///
    /// Starting from the root model, walks through each step of the projection,
    /// resolving fields and following relations/embedded types as needed.
    ///
    /// Returns None if:
    /// - The projection is empty
    /// - Any step references an invalid field index
    /// - A step tries to project through a primitive type
    pub fn resolve_field<'a>(
        &'a self,
        root: &'a Model,
        projection: &stmt::Projection,
    ) -> Option<&'a Field> {
        let [first, rest @ ..] = projection.as_slice() else {
            return None;
        };

        // Get the first field from the root model
        let mut current_field = root.fields.get(*first)?;

        // Walk through remaining steps
        for step in rest {
            let target_model = match &current_field.ty {
                FieldTy::Primitive(..) => {
                    // Cannot project through primitive fields
                    return None;
                }
                FieldTy::Embedded(embedded) => {
                    // For embedded fields, resolve to the embedded struct's model
                    self.model(embedded.target)
                }
                FieldTy::BelongsTo(belongs_to) => belongs_to.target(self),
                FieldTy::HasMany(has_many) => has_many.target(self),
                FieldTy::HasOne(has_one) => has_one.target(self),
            };

            current_field = target_model.fields.get(*step)?;
        }

        Some(current_field)
    }
}

impl Builder {
    pub(crate) fn from_macro(models: &[Model]) -> Result<Schema> {
        let mut builder = Self { ..Self::default() };

        for model in models {
            builder.models.insert(model.id, model.clone());
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
            for index in 0..self.models[curr].fields.len() {
                let model = &self.models[curr];
                let src = model.id;
                let field = &model.fields[index];

                if let FieldTy::HasMany(has_many) = &field.ty {
                    let pair = self.find_has_many_pair(src, has_many.target);
                    self.models[curr].fields[index]
                        .ty
                        .expect_has_many_mut()
                        .pair = pair;
                }
            }
        }

        // Link HasOne relations and compute BelongsTo foreign keys
        for curr in 0..self.models.len() {
            for index in 0..self.models[curr].fields.len() {
                let model = &self.models[curr];
                let src = model.id;
                let field = &model.fields[index];

                match &field.ty {
                    FieldTy::HasOne(has_one) => {
                        let pair = match self.find_belongs_to_pair(src, has_one.target) {
                            Some(pair) => pair,
                            None => {
                                let model = &self.models[curr];
                                panic!(
                                    "no relation pair for {}::{}",
                                    model.name.upper_camel_case(),
                                    model.fields[index].name.app_name
                                );
                            }
                        };

                        self.models[curr].fields[index].ty.expect_has_one_mut().pair = pair;
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
            for index in 0..self.models[curr].fields.len() {
                let model = &self.models[curr];
                let field_id = model.fields[index].id;

                let pair = match &self.models[curr].fields[index].ty {
                    FieldTy::BelongsTo(belongs_to) => {
                        let mut pair = None;
                        let target = self.models.get_index_of(&belongs_to.target).unwrap();

                        for target_index in 0..self.models[target].fields.len() {
                            pair = match &self.models[target].fields[target_index].ty {
                                FieldTy::HasMany(has_many) if has_many.pair == field_id => {
                                    assert!(pair.is_none());
                                    Some(self.models[target].fields[target_index].id)
                                }
                                FieldTy::HasOne(has_one) if has_one.pair == field_id => {
                                    assert!(pair.is_none());
                                    Some(self.models[target].fields[target_index].id)
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

                self.models[curr].fields[index]
                    .ty
                    .expect_belongs_to_mut()
                    .pair = pair;
            }
        }

        Ok(())
    }

    fn find_belongs_to_pair(&self, src: ModelId, target: ModelId) -> Option<FieldId> {
        let target = match self.models.get(&target) {
            Some(target) => target,
            None => todo!("lol no"),
        };

        // Find all BelongsTo relations that reference the model
        let belongs_to: Vec<_> = target
            .fields
            .iter()
            .filter(|field| match &field.ty {
                FieldTy::BelongsTo(rel) => rel.target == src,
                _ => false,
            })
            .collect();

        match &belongs_to[..] {
            [field] => Some(field.id),
            [] => None,
            _ => todo!("more than one belongs_to"),
        }
    }

    fn find_has_many_pair(&mut self, src: ModelId, target: ModelId) -> FieldId {
        if let Some(field_id) = self.find_belongs_to_pair(src, target) {
            return field_id;
        }

        todo!(
            "missing relation attribute; source={:#?}; target={:#?}",
            src,
            self.models.get(&target)
        );
    }
}
