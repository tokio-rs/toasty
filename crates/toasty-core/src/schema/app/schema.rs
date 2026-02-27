use super::{Field, FieldId, FieldTy, Model, ModelId};

use crate::{
    stmt::{self, Step},
    Result,
};
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
        let fields = match self.model(id.model) {
            Model::Root(root) => &root.fields,
            Model::EmbeddedStruct(embedded) => &embedded.fields,
            Model::EmbeddedEnum(_) => panic!("embedded enum has no fields"),
        };
        fields.get(id.index).expect("invalid field ID")
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
        let [Step::Field(first), rest @ ..] = projection.as_slice() else {
            return None;
        };

        // Get the first field from the root model
        let mut current_field = root.expect_root().fields.get(*first)?;

        // Walk through remaining steps
        for step in rest {
            current_field = match (&current_field.ty, step) {
                (FieldTy::Primitive(..), _) => {
                    // Cannot project through primitive fields
                    return None;
                }
                (FieldTy::Embedded(embedded), _) => match (self.model(embedded.target), step) {
                    (Model::EmbeddedStruct(embedded_struct), Step::Field(step)) => {
                        embedded_struct.fields.get(*step)?
                    }
                    (Model::EmbeddedEnum(embedded_enum), Step::Variant(_)) => {
                        todo!("resolve variant; embedded_enum={embedded_enum:#?}")
                    }
                    _ => return None,
                },
                (FieldTy::BelongsTo(belongs_to), Step::Field(step)) => {
                    belongs_to.target(self).expect_root().fields.get(*step)?
                }
                (FieldTy::HasMany(has_many), Step::Field(step)) => {
                    has_many.target(self).expect_root().fields.get(*step)?
                }
                (FieldTy::HasOne(has_one), Step::Field(step)) => {
                    has_one.target(self).expect_root().fields.get(*step)?
                }
                _ => return None,
            };
        }

        Some(current_field)
    }

    pub fn resolve_field_path<'a>(&'a self, path: &stmt::Path) -> Option<&'a Field> {
        let model = self.model(path.root);
        self.resolve_field(model, &path.projection)
    }
}

impl Builder {
    pub(crate) fn from_macro(models: &[Model]) -> Result<Schema> {
        let mut builder = Self { ..Self::default() };

        for model in models {
            builder.models.insert(model.id(), model.clone());
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
            for index in 0..self.models[curr].expect_root().fields.len() {
                let model = &self.models[curr];
                let src = model.id();
                let field = &model.expect_root().fields[index];

                if let FieldTy::HasMany(has_many) = &field.ty {
                    let target = has_many.target;
                    let field_name = field.name.app_name.clone();
                    let pair = self.find_has_many_pair(src, target, &field_name)?;
                    self.models[curr].expect_root_mut().fields[index]
                        .ty
                        .expect_has_many_mut()
                        .pair = pair;
                }
            }
        }

        // Link HasOne relations and compute BelongsTo foreign keys
        for curr in 0..self.models.len() {
            if self.models[curr].is_embedded() {
                continue;
            }
            for index in 0..self.models[curr].expect_root().fields.len() {
                let model = &self.models[curr];
                let src = model.id();
                let field = &model.expect_root().fields[index];

                match &field.ty {
                    FieldTy::HasOne(has_one) => {
                        let target = has_one.target;
                        let field_name = field.name.app_name.clone();
                        let pair = match self.find_belongs_to_pair(src, target, &field_name)? {
                            Some(pair) => pair,
                            None => {
                                return Err(crate::Error::invalid_schema(format!(
                                    "field `{}::{}` has no matching `BelongsTo` relation on the target model",
                                    self.models[curr].name().upper_camel_case(),
                                    field_name,
                                )));
                            }
                        };

                        self.models[curr].expect_root_mut().fields[index]
                            .ty
                            .expect_has_one_mut()
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
            for index in 0..self.models[curr].expect_root().fields.len() {
                let model = &self.models[curr];
                let field_id = model.expect_root().fields[index].id;

                let pair = match &self.models[curr].expect_root().fields[index].ty {
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
                                    model.expect_root().fields[index].name.app_name,
                                )));
                            }
                        };

                        for target_index in 0..self.models[target].expect_root().fields.len() {
                            pair = match &self.models[target].expect_root().fields[target_index].ty
                            {
                                FieldTy::HasMany(has_many) if has_many.pair == field_id => {
                                    assert!(pair.is_none());
                                    Some(self.models[target].expect_root().fields[target_index].id)
                                }
                                FieldTy::HasOne(has_one) if has_one.pair == field_id => {
                                    assert!(pair.is_none());
                                    Some(self.models[target].expect_root().fields[target_index].id)
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

                self.models[curr].expect_root_mut().fields[index]
                    .ty
                    .expect_belongs_to_mut()
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
            .expect_root()
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
                "model `{}` has more than one `BelongsTo` relation targeting `{}`",
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
}
