use super::*;

use crate::Result;
use indexmap::IndexMap;

#[derive(Debug, Default)]
pub struct Schema {
    pub models: IndexMap<ModelId, Model>,
    pub queries: Vec<Query>,
}

#[derive(Default)]
struct Builder {
    models: IndexMap<ModelId, Model>,
    queries: Vec<Query>,
}

impl Schema {
    pub fn from_macro(models: &[Model]) -> Result<Schema> {
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

    pub fn query(&self, id: impl Into<QueryId>) -> &Query {
        let id = id.into();
        &self.queries[id.0]
    }
}

impl Builder {
    pub(crate) fn from_macro(models: &[Model]) -> Result<Schema> {
        let mut builder = Builder {
            ..Builder::default()
        };

        for model in models {
            builder.models.insert(model.id, model.clone());
        }

        builder.process_models()?;
        builder.into_schema()
    }

    fn into_schema(self) -> Result<Schema> {
        Ok(Schema {
            models: self.models,
            queries: self.queries,
        })
    }

    fn process_models(&mut self) -> Result<()> {
        // All models have been discovered and initialized at some level, now do
        // the relation linking.
        self.link_relations()?;

        // Build default queries (e.g. find_by_[index])
        self.build_queries()?;

        // Build queries on relationships
        self.build_relation_queries()?;

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

    fn build_queries(&mut self) -> Result<()> {
        for model in self.models.values_mut() {
            for index in &model.indices {
                let mut fields = index.partition_fields().to_vec();
                let mut local_fields = index.local_fields().to_vec();

                loop {
                    if index.primary_key && local_fields.is_empty() {
                        model.primary_key.query = QueryId(self.queries.len());
                    }

                    let by_fk = vec![false];

                    // Generate a query that takes BelongsTo arguments by reference
                    // and one by foreign key.
                    for by_fk in by_fk {
                        let id = QueryId(self.queries.len());
                        let mut builder = Query::find_by(id, model, by_fk);

                        for index_field in &fields {
                            builder.field(model.field(index_field.field).id());
                        }

                        /*
                        assert!(self
                            .find_by_queries
                            .insert(builder.args.clone(), id)
                            .is_none());
                        */

                        self.queries.push(builder.build());
                        model.queries.push(id);

                        // If this is a unique index, create a query that takes
                        // a batch of keys.
                        if index.unique && local_fields.is_empty() {
                            let id = QueryId(self.queries.len());
                            // TODO: do we need to generate multiple versions `by_fk`
                            // like above.
                            let mut builder = Query::find_by(id, model, by_fk);
                            builder.many();

                            for index_field in &index.fields {
                                builder.field(model.field(index_field.field).id());
                            }

                            self.queries.push(builder.build());
                            model.queries.push(id);
                        }
                    }

                    if local_fields.is_empty() {
                        break;
                    }

                    fields.push(local_fields.remove(0));
                }
            }
        }

        Ok(())
    }

    fn build_relation_queries(&mut self) -> Result<()> {
        for curr in 0..self.models.len() {
            for field_id in 0..self.models[curr].fields.len() {
                let model = &self.models[curr];

                // If this is a `HasMany`, get the target & field pair
                let Some(rel) = model.fields[field_id].ty.as_has_many() else {
                    continue;
                };
                let pair = self.models[&rel.pair.model].fields[rel.pair.index]
                    .ty
                    .expect_belongs_to();

                let target = &self.models[&rel.target];
                let query_id = QueryId(self.queries.len());

                let mut builder = Query::find_by(query_id, target, false);
                builder.scope(rel.pair);

                let mut fields: Vec<_> = target.primary_key_fields().collect();
                assert!(!fields.is_empty());

                for fk_field in &pair.foreign_key.fields[..] {
                    if fields[0].id != fk_field.source {
                        break;
                    }

                    fields.remove(0);
                }

                if fields.is_empty() {
                    todo!()
                }

                // Add all the target's primary key fields
                for field in fields {
                    // Assert the field is not part of the scope
                    builder.field(field.id());
                }

                let query = builder.build();
                let scoped_query = ScopedQuery::new(&query);

                self.models[curr].fields[field_id]
                    .ty
                    .expect_has_many_mut()
                    .queries
                    .push(scoped_query);

                self.queries.push(query);
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
