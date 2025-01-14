use super::*;

use crate::Result;

#[derive(Debug, Default)]
pub struct Schema {
    pub models: Vec<Model>,
    pub queries: Vec<Query>,
}

#[derive(Default)]
struct Builder {
    models: Vec<Model>,
    queries: Vec<Query>,
    cx: Context,
}

impl Schema {
    pub(crate) fn from_ast(ast: &ast::Schema) -> Result<Schema> {
        let builder = Builder::default();
        builder.from_ast(ast)
    }
}

impl Builder {
    fn from_ast(mut self, ast: &ast::Schema) -> Result<Schema> {
        // First, register all defined types with the resolver.
        for node in ast.models() {
            self.cx.register_model(&node.ident);
        }

        for item in &ast.items {
            match item {
                ast::SchemaItem::Model(node) => {
                    let model = Model::from_ast(&mut self.cx, node)?;
                    assert_eq!(self.models.len(), model.id.0);
                    self.models.push(model);
                }
            }
        }

        // All models have been discovered and initialized at some level, now do
        // the relation linking.
        self.link_relations()?;

        // Build default queries (e.g. find_by_[index])
        self.build_queries()?;

        // Build queries on relationships
        self.build_relation_queries()?;

        Ok(Schema {
            models: self.models,
            queries: self.queries,
        })
    }

    /// Go through all relations and link them to their pairs
    pub(crate) fn link_relations(&mut self) -> crate::Result<()> {
        // Because arbitrary models will be mutated throughout the linking
        // process, models cannot be iterated as that would hold a reference to
        // `self`. Instead, we use index based iteration.

        // First, link all HasMany relations. HasManys are linked first because
        // linking them may result in converting HasOne relations to BelongTo.
        // We need this conversion to happen before any of the other processing.
        for src in 0..self.models.len() {
            for index in 0..self.models[src].fields.len() {
                let field = &self.models[src].fields[index];

                if let FieldTy::HasMany(has_many) = &field.ty {
                    let pair = self.find_has_many_pair(src, has_many.target);
                    self.models[src].fields[index].ty.expect_has_many_mut().pair = pair;
                }
            }
        }

        // Link HasOne relations and compute BelongsTo foreign keys
        for src in 0..self.models.len() {
            for index in 0..self.models[src].fields.len() {
                let model = &self.models[src];
                let field = &model.fields[index];

                match &field.ty {
                    FieldTy::HasOne(has_one) => {
                        let pair = match self.find_belongs_to_pair(src, has_one.target) {
                            Some(pair) => pair,
                            None => {
                                let model = &self.models[src];
                                panic!(
                                    "no relation pair for {}::{}",
                                    model.name.upper_camel_case(),
                                    model.fields[index].name
                                );
                            }
                        };

                        self.models[src].fields[index].ty.expect_has_one_mut().pair = pair;
                    }
                    FieldTy::BelongsTo(belongs_to) => {
                        assert!(belongs_to.foreign_key.is_placeholder());

                        // Compute foreign key fields.
                        let foreign_key = self.foreign_key_for(model, field, belongs_to.target);

                        self.models[src].fields[index]
                            .ty
                            .expect_belongs_to_mut()
                            .foreign_key = foreign_key;
                    }
                    _ => {}
                }
            }
        }

        // Finally, link BelongsTo relations with their pairs
        for src in 0..self.models.len() {
            for index in 0..self.models[src].fields.len() {
                let field_id = self.models[src].fields[index].id;

                let pair = match &self.models[src].fields[index].ty {
                    FieldTy::BelongsTo(belongs_to) => {
                        let mut pair = None;

                        for target_index in 0..self.models[belongs_to.target.0].fields.len() {
                            pair = match &self.models[belongs_to.target.0].fields[target_index].ty {
                                FieldTy::HasMany(has_many) if has_many.pair == field_id => {
                                    Some(self.models[belongs_to.target.0].fields[target_index].id)
                                }
                                FieldTy::HasOne(has_one) if has_one.pair == field_id => {
                                    Some(self.models[belongs_to.target.0].fields[target_index].id)
                                }
                                _ => continue,
                            }
                        }

                        match pair {
                            Some(pair) => pair,
                            None => continue,
                        }
                    }
                    _ => continue,
                };

                self.models[src].fields[index]
                    .ty
                    .expect_belongs_to_mut()
                    .pair = pair;
            }
        }

        Ok(())
    }

    fn build_queries(&mut self) -> Result<()> {
        for model in &mut self.models {
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
                            builder.field(model.field(index_field));
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
                                builder.field(model.field(index_field));
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
        for model_id in 0..self.models.len() {
            for field_id in 0..self.models[model_id].fields.len() {
                let model = &self.models[model_id];

                // If this is a `HasMany`, get the target & field pair
                let Some(rel) = model.fields[field_id].ty.as_has_many() else {
                    continue;
                };
                let pair = self.models[rel.pair.model.0].fields[rel.pair.index]
                    .ty
                    .expect_belongs_to();

                let target = &self.models[rel.target.0];
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
                    builder.field(field);
                }

                /*
                assert!(self
                    .find_by_queries
                    .insert(builder.args.clone(), query_id)
                    .is_none());
                */

                let query = builder.build();
                let scoped_query = ScopedQuery::new(&query);

                self.models[model_id].fields[field_id]
                    .ty
                    .expect_has_many_mut()
                    .queries
                    .push(scoped_query);

                self.queries.push(query);
            }
        }

        Ok(())
    }

    fn find_belongs_to_pair(&self, src: usize, target: ModelId) -> Option<FieldId> {
        let target = match self.models.get(target.0) {
            Some(target) => target,
            None => todo!("lol no"),
        };

        // Find all BelongsTo relations that reference the model
        let belongs_to: Vec<_> = target
            .fields
            .iter()
            .filter(|field| match &field.ty {
                FieldTy::BelongsTo(rel) => rel.target == ModelId(src),
                _ => false,
            })
            .collect();

        match &belongs_to[..] {
            [field] => Some(field.id),
            [] => None,
            _ => todo!("more than one belongs_to"),
        }
    }

    fn find_has_many_pair(&mut self, src: usize, target: ModelId) -> FieldId {
        if let Some(field_id) = self.find_belongs_to_pair(src, target) {
            return field_id;
        }

        // Try to convert a HasOne. During the initial pass, if a relation is
        // not obviously a BelongsTo, we start by assuming it is a HasOne. At
        // this point, we might consider it a BelongsTo as it is paired with a
        // HasMany.
        //
        // The key difference between a HasOne and a BelongsTo is BelongsTo
        // holds the foreign key.

        let target = &mut self.models[target.0];

        let mut has_one: Vec<_> = target
            .fields
            .iter_mut()
            .filter(|field| match &field.ty {
                FieldTy::HasOne(rel) => rel.target == ModelId(src),
                _ => false,
            })
            .collect();

        match &mut has_one[..] {
            [field] => {
                let HasOne {
                    target, expr_ty, ..
                } = field.ty.expect_has_one();

                // Convert the HasOne to a BelongsTo
                field.ty = BelongsTo {
                    target: *target,
                    expr_ty: expr_ty.clone(),
                    pair: FieldId::placeholder(),
                    foreign_key: relation::ForeignKey::placeholder(),
                }
                .into();

                field.id
            }
            [] => todo!(),
            _ => todo!(),
        }
    }

    fn foreign_key_for(
        &self,
        source: &Model,
        source_field: &Field,
        target: ModelId,
    ) -> relation::ForeignKey {
        let attr = self.cx.get_relation_attr(source_field.id);

        assert_eq!(
            attr.key.len(),
            attr.references.len(),
            "unbalanced relation attribute {attr:#?}"
        );

        let target = &self.models[target.0];
        let mut fields = vec![];

        for (key, references) in attr.key.iter().zip(attr.references.iter()) {
            let field_source = source.field_by_name(key.as_str()).expect("missing field");
            let field_target = target
                .field_by_name(references.as_str())
                .expect("missing filed");

            fields.push(relation::ForeignKeyField {
                source: field_source.id,
                target: field_target.id,
            });
        }

        relation::ForeignKey { fields }
    }
}
