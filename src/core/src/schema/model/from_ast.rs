use super::*;

struct Builder<'a> {
    ctx: &'a mut schema::Context,
    model: Model,
}

impl Model {
    pub(crate) fn from_ast(ctx: &mut schema::Context, node: &ast::Model) -> crate::Result<Model> {
        let id = ctx.resolve_model(&ast::Path::new(&node.ident));

        Ok(Builder {
            ctx,
            model: Model {
                id,
                name: Name::new(&node.ident.to_string()),
                lowering: Lowering {
                    table: table::TableId::placeholder(),
                    columns: vec![],
                    model_pk_to_table: stmt::Expr::identity(),
                    table_to_model: stmt::ExprRecord::default(),
                    model_to_table: stmt::ExprRecord::default(),
                },
                fields: vec![],
                primary_key: PrimaryKey {
                    fields: vec![],
                    query: QueryId::placeholder(),
                    index: ModelIndexId {
                        model: id,
                        index: 0,
                    },
                },
                // Queries will be populated later
                queries: vec![],
                indices: vec![ModelIndex {
                    id: ModelIndexId {
                        model: id,
                        index: 0,
                    },
                    fields: vec![],
                    unique: true,
                    primary_key: true,
                    lowering: IndexLowering {
                        index: IndexId::placeholder(),
                    },
                }],
            },
        }
        .from_ast(node))
    }
}

impl<'a> Builder<'a> {
    fn from_ast(mut self, node: &ast::Model) -> Model {
        // Process model-level attributes
        let attrs = node
            .attrs
            .iter()
            .map(attr::Model::from_ast)
            .collect::<Vec<_>>();

        // Process AST fields

        for field in &node.fields {
            let field_id = FieldId {
                model: self.model.id,
                index: self.model.fields.len(),
            };
            let name = field.ident.to_string();

            let attrs = attr::FieldSet::from_ast(&field.attrs);

            match &field.ty {
                ast::Type::Option(ty_option) => match &*ty_option.ty {
                    ast::Type::Path(path) => {
                        self.push_singular_field(field_id, name, true, path, attrs.relation());
                    }
                    _ => todo!("{:#?}", field),
                },
                ast::Type::Array(path) => {
                    match &*path.ty {
                        ast::Type::Path(path) => {
                            let target = self.ctx.resolve_ty(&path.path, self.model.id);

                            if let stmt::Type::Model(target) = target {
                                let singularize = std_util::str::singularize(&name);

                                // Make a blank HasMany type, this will be
                                // linked with the belongs_to pair later.
                                let rel = relation::HasMany {
                                    target,
                                    expr_ty: stmt::Type::List(Box::new(stmt::Type::Model(target))),
                                    singular: Name::new(&singularize),
                                    pair: FieldId::placeholder(),
                                    queries: vec![],
                                };

                                // Store a blank HasMany field
                                self.model.fields.push(Field {
                                    id: field_id,
                                    name: name.clone(),
                                    ty: FieldTy::HasMany(rel),
                                    primary_key: false,
                                    nullable: false,
                                    auto: None,
                                });
                            } else {
                                todo!()
                            }
                        }
                        _ => todo!(),
                    }
                }
                ast::Type::Path(path) => {
                    self.push_singular_field(field_id, name, false, path, attrs.relation());
                }
            };

            for attr in attrs.iter() {
                let field = self.model.fields.last_mut().unwrap();

                if attr.is_index() {
                    self.model.indices.push(ModelIndex {
                        id: ModelIndexId {
                            model: self.model.id,
                            index: self.model.indices.len(),
                        },
                        fields: vec![ModelIndexField {
                            field: field_id,
                            op: IndexOp::Eq,
                            scope: IndexScope::Partition,
                        }],
                        unique: attr.is_unique(),
                        primary_key: false,
                        lowering: IndexLowering {
                            index: IndexId::placeholder(),
                        },
                    });
                }

                if attr.is_key() {
                    field.primary_key = true;
                    self.model.primary_key.fields.push(field_id);
                    self.model.indices[0].fields.push(ModelIndexField {
                        field: field_id,
                        op: IndexOp::Eq,
                        scope: IndexScope::Partition,
                    });
                }

                if attr.is_auto() {
                    // For now, auto is only supported on ID types
                    assert!(field.ty.expect_primitive().ty.is_id());
                    field.auto = Some(Auto::Id);
                }

                if let attr::Field::Relation(attr) = attr {
                    // Store the relation for later
                    self.ctx.store_relation_attr(field.id, attr.clone());
                }
            }
        }

        // Process model-level attributes
        for attr in attrs {
            match attr {
                attr::Model::Key(attr) => {
                    assert!(self.model.primary_key.fields.is_empty());
                    assert!(self.model.indices[0].fields.is_empty());

                    for field_name in &attr.partition {
                        let field = self.model.field_by_name_mut(field_name).unwrap();
                        let field_id = field.id;

                        field.primary_key = true;

                        self.model.primary_key.fields.push(field_id);
                        self.model.indices[0].fields.push(ModelIndexField {
                            field: field_id,
                            op: IndexOp::Eq,
                            scope: IndexScope::Partition,
                        });
                    }

                    for field_name in &attr.local {
                        let field = self.model.field_by_name_mut(field_name).unwrap();
                        let field_id = field.id;

                        field.primary_key = true;

                        self.model.primary_key.fields.push(field_id);
                        self.model.indices[0].fields.push(ModelIndexField {
                            field: field_id,
                            op: IndexOp::Eq,
                            scope: IndexScope::Local,
                        });
                    }
                }
            }
        }

        assert!(
            !self.model.primary_key.fields.is_empty(),
            "no primary key set for {}",
            self.model.name.upper_camel_case()
        );

        // Track the model
        self.model
    }

    fn push_singular_field(
        &mut self,
        id: FieldId,
        name: String,
        nullable: bool,
        path: &ast::TypePath,
        relation: Option<&attr::Relation>,
    ) {
        let ty = self.ctx.resolve_ty(&path.path, self.model.id);

        if let stmt::Type::Model(target) = ty {
            let ty = if let Some(relation) = relation {
                assert_eq!(1, relation.references.len());
                assert_eq!("id", relation.references[0].as_str());

                BelongsTo {
                    target,
                    expr_ty: stmt::Type::Model(target),
                    pair: FieldId::placeholder(),
                    // This will be populated at a later step.
                    foreign_key: relation::ForeignKey::placeholder(),
                }
                .into()
            } else {
                // Use HasOne as a placeholder. This will be updated during the
                // relation linking phase
                HasOne {
                    target,
                    expr_ty: stmt::Type::Model(target),
                    pair: FieldId::placeholder(),
                }
                .into()
            };

            self.model.fields.push(Field {
                id,
                name: name.clone(),
                ty,
                primary_key: false,
                nullable,
                auto: None,
            })
        } else {
            self.model.fields.push(Field {
                id,
                name: name.clone(),
                ty: FieldTy::Primitive(FieldPrimitive {
                    column: ColumnId::placeholder(),
                    lowering: usize::MAX,
                    ty,
                }),
                primary_key: false,
                nullable,
                auto: None,
            })
        }
    }
}
