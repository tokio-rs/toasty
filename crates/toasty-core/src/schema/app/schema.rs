use super::*;

use crate::Result;
use std::any::TypeId;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Schema {
    pub models: Vec<Model>,
    /// Mapping from TypeId to ModelId for runtime resolution
    pub type_to_model: HashMap<TypeId, ModelId>,
}

#[derive(Default)]
struct Builder {
    models: Vec<Model>,
    type_to_model: HashMap<TypeId, ModelId>,
}

impl Schema {
    pub fn from_macro(models: &[Model]) -> Result<Self> {
        Self::from_macro_with_mapping(models, HashMap::new())
    }

    pub fn from_macro_with_mapping(
        models: &[Model],
        type_to_model: HashMap<TypeId, ModelId>,
    ) -> Result<Self> {
        Builder::from_macro_with_mapping(models, type_to_model)
    }

    /// Get a field by ID
    pub fn field(&self, id: FieldId) -> &Field {
        self.model(id.model)
            .fields
            .get(id.index)
            .expect("invalid field ID")
    }

    /// Get a field by ExprReference (if it's a field reference)
    pub fn field_from_expr(&self, expr_ref: &crate::stmt::ExprReference) -> Option<&Field> {
        expr_ref.as_field_id().map(|field_id| self.field(field_id))
    }

    pub fn models(&self) -> impl Iterator<Item = &Model> {
        self.models.iter()
    }

    /// Get a model by ID
    pub fn model(&self, id: impl Into<ModelId>) -> &Model {
        let model_id = id.into();
        &self.models[model_id.0]
    }

    /// Resolve a TypeId to a ModelId
    pub fn type_to_model_id(&self, type_id: TypeId) -> Result<ModelId> {
        self.type_to_model.get(&type_id).copied().ok_or_else(|| {
            crate::Error::msg(format!(
                "TypeId {:?} not found in schema - model may not be registered",
                type_id
            ))
        })
    }
}

impl Builder {
    pub(crate) fn from_macro(models: &[Model]) -> Result<Schema> {
        Self::from_macro_with_mapping(models, HashMap::new())
    }

    pub(crate) fn from_macro_with_mapping(
        models: &[Model],
        type_to_model: HashMap<TypeId, ModelId>,
    ) -> Result<Schema> {
        let mut builder = Self {
            models: Vec::with_capacity(models.len()),
            type_to_model,
        };

        // Create a Vec with the correct capacity
        // Sort models by their ModelId to ensure correct order
        let mut sorted_models: Vec<_> = models.iter().collect();
        sorted_models.sort_by_key(|model| model.id.0);

        // Verify sequential ModelIds and insert in order
        for (expected_index, model) in sorted_models.iter().enumerate() {
            assert_eq!(
                model.id.0, expected_index,
                "ModelIds must be sequential starting from 0. Expected {} but found {}.",
                expected_index, model.id.0
            );
            builder.models.push((*model).clone());
        }

        builder.process_models()?;
        builder.into_schema()
    }

    fn into_schema(self) -> Result<Schema> {
        Ok(Schema {
            models: self.models,
            type_to_model: self.type_to_model,
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
                                    model.fields[index].name
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
                        let target_index = belongs_to.target.0;

                        for target_field_index in 0..self.models[target_index].fields.len() {
                            pair = match &self.models[target_index].fields[target_field_index].ty {
                                FieldTy::HasMany(has_many) if has_many.pair == field_id => {
                                    assert!(pair.is_none());
                                    Some(self.models[target_index].fields[target_field_index].id)
                                }
                                FieldTy::HasOne(has_one) if has_one.pair == field_id => {
                                    assert!(pair.is_none());
                                    Some(self.models[target_index].fields[target_field_index].id)
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
        let target_model = &self.models[target.0];

        // Find all BelongsTo relations that reference the model
        let belongs_to: Vec<_> = target_model
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
            &self.models[target.0]
        );
    }
}
