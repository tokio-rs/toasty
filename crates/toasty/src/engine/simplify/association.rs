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
    use crate as toasty;
    use crate::model::Register;
    use toasty_core::{
        driver::Capability,
        schema::{app, app::FieldId, app::ModelId, Builder},
        stmt::{Association, Expr, ExprInSubquery, Path, Query, SourceModel, Value},
    };

    #[allow(dead_code)]
    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: i64,

        #[has_many(pair = author)]
        posts: toasty::HasMany<Post>,
    }

    #[allow(dead_code)]
    #[derive(toasty::Model)]
    struct Post {
        #[key]
        id: i64,

        #[index]
        user_id: i64,

        #[belongs_to(key = user_id, references = id)]
        author: toasty::BelongsTo<User>,
    }

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
            let app_schema = app::Schema::from_macro(&[User::schema(), Post::schema()])
                .expect("schema should build from macro");

            let schema = Builder::new()
                .build(app_schema, &Capability::SQLITE)
                .expect("schema should build");

            let user_model = User::id();
            let post_model = Post::id();

            // Find field IDs by name from the generated schema
            let user_id = schema
                .app
                .model(user_model)
                .fields
                .iter()
                .find(|f| f.name.app_name == "id")
                .unwrap()
                .id;

            let user_posts = schema
                .app
                .model(user_model)
                .fields
                .iter()
                .find(|f| f.name.app_name == "posts")
                .unwrap()
                .id;

            let post_author = schema
                .app
                .model(post_model)
                .fields
                .iter()
                .find(|f| f.name.app_name == "author")
                .unwrap()
                .id;

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
