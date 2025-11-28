use super::Simplify;
use toasty_core::{schema::app, stmt};

impl Simplify<'_> {
    pub(super) fn simplify_via_association_for_delete(&mut self, stmt: &mut stmt::Delete) {
        if let stmt::Source::Model(model) = &mut stmt.from {
            if let Some(via) = model.via.take() {
                // Create a new scope to indicate we are operating in the
                // context of stmt.from
                let mut s = self.scope(&stmt.from);

                let filter = s.rewrite_association_as_filter(via);
                stmt.filter = stmt::Filter::and(stmt.filter.take(), filter);
            }
        }
    }

    pub(super) fn simplify_via_association_for_insert(&mut self, stmt: &mut stmt::Insert) {
        if let stmt::InsertTarget::Scope(scope) = &mut stmt.target {
            self.simplify_via_association_for_query(scope);
        }
    }

    pub(super) fn simplify_via_association_for_query(&mut self, stmt: &mut stmt::Query) {
        if let stmt::ExprSet::Select(select) = &mut stmt.body {
            if let stmt::Source::Model(model) = &mut select.source {
                if let Some(via) = model.via.take() {
                    // Create a new scope to indicate we are operating in the
                    // context of stmt.target
                    let mut s = self.scope(&select.source);

                    let filter = s.rewrite_association_as_filter(via);
                    select.filter = stmt::Filter::and(select.filter.take(), filter);
                }
            }
        }
    }

    fn rewrite_association_as_filter(
        &mut self,
        mut association: stmt::Association,
    ) -> stmt::Filter {
        // First, we want to simplify the association source.
        stmt::visit_mut::visit_stmt_query_mut(self, &mut association.source);

        // For now, we only support paths with a single step
        assert!(association.path.len() == 1, "TODO");

        let field = association.path.resolve_field(&self.schema().app);

        match &field.ty {
            app::FieldTy::BelongsTo(rel) => {
                self.rewrite_association_belongs_to_as_filter(rel, association)
            }
            app::FieldTy::HasOne(rel) => {
                stmt::Expr::in_subquery(stmt::Expr::ref_self_field(rel.pair), *association.source)
                    .into()
            }
            app::FieldTy::HasMany(rel) => {
                stmt::Expr::in_subquery(stmt::Expr::ref_self_field(rel.pair), *association.source)
                    .into()
            }
            _ => todo!("field={field:#?}"),
        }
    }

    fn rewrite_association_belongs_to_as_filter(
        &mut self,
        rel: &app::BelongsTo,
        association: stmt::Association,
    ) -> stmt::Filter {
        todo!("rel={rel:#?}, association={association:#?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toasty_core::{
        driver::Capability,
        schema::{
            app::{
                BelongsTo, Field, FieldId, FieldName, FieldPrimitive, FieldTy, ForeignKey,
                ForeignKeyField, HasMany, Index, IndexField, IndexId, Model, ModelId, PrimaryKey,
            },
            db::{IndexOp, IndexScope},
            Builder, Name,
        },
        stmt::{Association, Expr, ExprInSubquery, Path, Query, SourceModel, Type, Value},
    };

    struct UserPostSchema {
        schema: toasty_core::Schema,
        user_model: ModelId,
        user_id: FieldId,
        user_posts: FieldId,
        post_model: ModelId,
        post_author: FieldId,
    }

    impl UserPostSchema {
        fn new() -> Self {
            let user_model = ModelId(0);
            let post_model = ModelId(1);

            let user_id = FieldId {
                model: user_model,
                index: 0,
            };
            let user_posts = FieldId {
                model: user_model,
                index: 1,
            };

            let post_id = FieldId {
                model: post_model,
                index: 0,
            };
            let post_user_id = FieldId {
                model: post_model,
                index: 1,
            };
            let post_author = FieldId {
                model: post_model,
                index: 2,
            };

            let user_model_def = Model {
                id: user_model,
                name: Name::new("User"),
                fields: vec![
                    Field {
                        id: user_id,
                        name: FieldName {
                            app_name: "id".to_string(),
                            storage_name: None,
                        },
                        ty: FieldTy::Primitive(FieldPrimitive {
                            ty: Type::I64,
                            storage_ty: None,
                        }),
                        nullable: false,
                        primary_key: true,
                        auto: None,
                        constraints: vec![],
                    },
                    Field {
                        id: user_posts,
                        name: FieldName {
                            app_name: "posts".to_string(),
                            storage_name: None,
                        },
                        ty: FieldTy::HasMany(HasMany {
                            target: post_model,
                            expr_ty: Type::List(Box::new(Type::Model(post_model))),
                            singular: Name::new("post"),
                            pair: post_author,
                        }),
                        nullable: false,
                        primary_key: false,
                        auto: None,
                        constraints: vec![],
                    },
                ],
                primary_key: PrimaryKey {
                    fields: vec![user_id],
                    index: IndexId {
                        model: user_model,
                        index: 0,
                    },
                },
                indices: vec![Index {
                    id: IndexId {
                        model: user_model,
                        index: 0,
                    },
                    fields: vec![IndexField {
                        field: user_id,
                        op: IndexOp::Eq,
                        scope: IndexScope::Local,
                    }],
                    unique: true,
                    primary_key: true,
                }],
                table_name: None,
            };

            let post_model_def = Model {
                id: post_model,
                name: Name::new("Post"),
                fields: vec![
                    Field {
                        id: post_id,
                        name: FieldName {
                            app_name: "id".to_string(),
                            storage_name: None,
                        },
                        ty: FieldTy::Primitive(FieldPrimitive {
                            ty: Type::I64,
                            storage_ty: None,
                        }),
                        nullable: false,
                        primary_key: true,
                        auto: None,
                        constraints: vec![],
                    },
                    Field {
                        id: post_user_id,
                        name: FieldName {
                            app_name: "user_id".to_string(),
                            storage_name: None,
                        },
                        ty: FieldTy::Primitive(FieldPrimitive {
                            ty: Type::I64,
                            storage_ty: None,
                        }),
                        nullable: false,
                        primary_key: false,
                        auto: None,
                        constraints: vec![],
                    },
                    Field {
                        id: post_author,
                        name: FieldName {
                            app_name: "author".to_string(),
                            storage_name: None,
                        },
                        ty: FieldTy::BelongsTo(BelongsTo {
                            target: user_model,
                            expr_ty: Type::Model(user_model),
                            pair: Some(user_posts),
                            foreign_key: ForeignKey {
                                fields: vec![ForeignKeyField {
                                    source: post_user_id,
                                    target: user_id,
                                }],
                            },
                        }),
                        nullable: false,
                        primary_key: false,
                        auto: None,
                        constraints: vec![],
                    },
                ],
                primary_key: PrimaryKey {
                    fields: vec![post_id],
                    index: IndexId {
                        model: post_model,
                        index: 0,
                    },
                },
                indices: vec![
                    Index {
                        id: IndexId {
                            model: post_model,
                            index: 0,
                        },
                        fields: vec![IndexField {
                            field: post_id,
                            op: IndexOp::Eq,
                            scope: IndexScope::Local,
                        }],
                        unique: true,
                        primary_key: true,
                    },
                    Index {
                        id: IndexId {
                            model: post_model,
                            index: 1,
                        },
                        fields: vec![IndexField {
                            field: post_user_id,
                            op: IndexOp::Eq,
                            scope: IndexScope::Local,
                        }],
                        unique: false,
                        primary_key: false,
                    },
                ],
                table_name: None,
            };

            let mut app_schema = toasty_core::schema::app::Schema::default();
            app_schema.models.insert(user_model, user_model_def);
            app_schema.models.insert(post_model, post_model_def);

            let schema = Builder::new()
                .build(app_schema, &Capability::SQLITE)
                .expect("schema should build");

            Self {
                schema,
                user_model,
                user_id,
                user_posts,
                post_model,
                post_author,
            }
        }
    }

    #[test]
    fn has_many_via_becomes_in_subquery() {
        // `select(Post, via(User.posts)) → select(Post, in_subquery(author, user_query))`
        let s = UserPostSchema::new();
        let mut simplify = Simplify::new(&s.schema);

        let user_filter = Expr::eq(
            Expr::ref_self_field(s.user_id),
            Expr::Value(Value::from(42i64)),
        );
        let user_query = Query::new_select(s.user_model, user_filter);

        let association = Association {
            source: Box::new(user_query),
            path: Path::field(s.user_model, s.user_posts.index),
        };

        let mut query = Query::new_select(s.post_model, Expr::Value(Value::Bool(true)));
        if let stmt::ExprSet::Select(select) = &mut query.body {
            if let stmt::Source::Model(model) = &mut select.source {
                model.via = Some(association);
            }
        }

        simplify.simplify_via_association_for_query(&mut query);

        let stmt::ExprSet::Select(select) = &query.body else {
            panic!("expected Select");
        };
        assert!(matches!(
            &select.source,
            stmt::Source::Model(SourceModel { via: None, .. })
        ));

        let filter_expr = select.filter.as_expr();
        let Expr::InSubquery(ExprInSubquery {
            expr,
            query: subquery,
        }) = filter_expr
        else {
            panic!("expected filter expression to be an `Expr::InSubquery`");
        };

        // The expression should reference the pair field (`post_author`).
        assert!(matches!(
            &**expr,
            Expr::Reference(stmt::ExprReference::Field { index, .. }) if *index == s.post_author.index
        ));

        // The subquery should be the user query.
        let stmt::ExprSet::Select(select) = &subquery.body else {
            panic!("expected subquery body to be a `ExprSet::Select`");
        };

        // Ensure the source of the subquery is the user model.
        assert!(matches!(
            &select.source,
            stmt::Source::Model(SourceModel { model, .. }) if *model == s.user_model
        ));
    }
}
