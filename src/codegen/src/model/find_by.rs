use super::*;

impl Generator<'_> {
    pub(super) fn gen_query_structs(&mut self) -> TokenStream {
        self.model
            .queries
            .iter()
            .copied()
            .map(|query| self.find_by_query(self.query(query)))
            .collect()
    }

    pub(crate) fn gen_scoped_find_by_method(&self, scoped: &app::ScopedQuery) -> TokenStream {
        let query_method_name = self.scoped_query_method_name(scoped.id);
        let query_struct_name = self.query_struct_name(scoped.id);

        let caller_idents: Vec<_> = scoped
            .caller_args
            .iter()
            .map(|arg| {
                let ident = crate::util::ident(&arg.name);
                quote!(#ident)
            })
            .collect();

        let mut idents = vec![];

        for _ in &scoped.scope_args {
            idents.push(quote!(self.scope));
        }

        idents.extend(caller_idents.clone());

        let query_method_args = self.gen_method_args(&scoped.caller_args, &caller_idents, 2);

        let query = self.query(scoped.id);
        let filter = &query.stmt.body.as_select().filter;
        let body = self.gen_expr_from_stmt(query.ret, &idents, filter, 2);

        quote! {
            // TODO: should this borrow more?
            pub fn #query_method_name(self, #query_method_args) ->  #query_struct_name {
                #query_struct_name {
                    stmt: stmt::Select::filter(#body)
                }
            }
        }
    }

    // TODO: split this up and unify with other fns
    pub(crate) fn gen_scoped_find_by_struct(
        &self,
        scoped: &app::ScopedQuery,
        depth: usize,
    ) -> TokenStream {
        let query = self.query(scoped.id);
        self.gen_find_by_struct(query, depth)
    }

    fn find_by_query(&self, query: &app::Query) -> TokenStream {
        if !query.many {
            self.find_basic_by_query(query)
        } else {
            self.find_many_by_query(query)
        }
    }

    fn find_basic_by_query(&self, query: &app::Query) -> TokenStream {
        use app::FieldTy;

        let query_method_name = self.query_method_name(query.id);
        let model_struct_name = self.self_struct_name();
        let query_struct_name = self.query_struct_name(query.id);

        let arg_idents: Vec<_> = query
            .args
            .iter()
            .map(|arg| {
                let ident = crate::util::ident(&arg.name);
                quote!(#ident)
            })
            .collect();

        let query_method_args = self.gen_method_args(&query.args, &arg_idents, 1);

        let filter = &query.stmt.body.as_select().filter;
        let query = self.gen_expr_from_stmt(query.ret, &arg_idents, filter, 1);

        let relation_methods = self
            .model
            .fields
            .iter()
            .filter_map(|field| match &field.ty {
                FieldTy::Primitive(..) => None,
                FieldTy::HasMany(_) | FieldTy::HasOne(_) => {
                    let name = self.field_name(field.id);
                    let relation_query_struct_path = self.relation_query_struct_path(field, 0);

                    Some(quote! {
                        pub fn #name(mut self) -> #relation_query_struct_path {
                            #relation_query_struct_path::with_scope(self)
                        }
                    })
                }
                FieldTy::BelongsTo(_) => None,
            });

        quote! {
            impl super::#model_struct_name {
                // TODO: should this borrow more?
                pub fn #query_method_name(#query_method_args) ->  #query_struct_name {
                    #query_struct_name {
                        query: Query::from_stmt(stmt::Select::filter(#query))
                    }
                }
            }

            pub struct #query_struct_name {
                query: Query,
            }

            impl #query_struct_name {
                pub async fn all(self, db: &Db) -> Result<Cursor<super::#model_struct_name>> {
                    self.query.all(db).await
                }

                pub async fn first(self, db: &Db) -> Result<Option<super::#model_struct_name>> {
                    self.query.first(db).await
                }

                pub async fn get(self, db: &Db) -> Result<super::#model_struct_name> {
                    self.query.get(db).await
                }

                pub fn update(self) -> super::UpdateQuery {
                    super::UpdateQuery::from(self.query)
                }

                pub async fn delete(self, db: &Db) -> Result<()> {
                    self.query.delete(db).await
                }

                pub fn include<T: ?Sized>(mut self, path: impl Into<Path<T>>) -> #query_struct_name {
                    let path = path.into();
                    self.query.stmt.include(path);
                    self
                }

                pub fn filter(self, filter: stmt::Expr<bool>) -> Query {
                    let stmt = self.into_select();
                    Query::from_stmt(stmt.and(filter))
                }

                pub async fn collect<A>(self, db: &Db) -> Result<A>
                where
                    A: FromCursor<super::#model_struct_name>
                {
                    self.all(db).await?.collect().await
                }

                #( #relation_methods )*
            }

            impl stmt::IntoSelect for #query_struct_name {
                type Model = super::#model_struct_name;

                fn into_select(self) -> stmt::Select<Self::Model> {
                    self.query.into_select()
                }
            }
        }
    }

    fn find_many_by_query(&self, query: &app::Query) -> TokenStream {
        let query_method_name = self.query_method_name(query.id);
        let model_struct_name = self.self_struct_name();
        let query_struct_name = self.query_struct_name(query.id);

        let arg_idents: Vec<_> = query
            .args
            .iter()
            .map(|arg| {
                let ident = crate::util::ident(&arg.name);
                quote!(#ident)
            })
            .collect();

        let item_ty = self.gen_item_arg_tys(&query.args);

        let query_push_item = if query.args.len() == 1 {
            let ident = &arg_idents[0];
            quote!( #ident.into_expr() )
        } else {
            quote!(( #( #arg_idents .into_expr() ),* ).into_expr())
        };

        let query_method_args = self.gen_method_args(&query.args, &arg_idents, 1);

        let filter = &query.stmt.body.as_select().filter;
        let arg_idents = vec![quote!(self.items)];
        let query = self.gen_expr_from_stmt(query.ret, &arg_idents, filter, 1);

        quote! {
            impl super::#model_struct_name {
                // TODO: should this borrow more?
                pub fn #query_method_name() ->  #query_struct_name {
                    #query_struct_name { items: vec![] }
                }
            }

            pub struct #query_struct_name {
                items: Vec<stmt::Expr<#item_ty>>,
            }

            impl #query_struct_name {
                pub fn item(mut self, #query_method_args ) -> Self {
                    self.items.push( #query_push_item );
                    self
                }

                pub async fn all(self, db: &Db) -> Result<Cursor<super::#model_struct_name>> {
                    db.all(self.into_select()).await
                }

                pub async fn first(self, db: &Db) -> Result<Option<super::#model_struct_name>> {
                    db.first(self.into_select()).await
                }

                pub async fn get(self, db: &Db) -> Result<super::#model_struct_name> {
                    db.get(self.into_select()).await
                }

                pub fn update(self) -> super::UpdateQuery {
                    super::UpdateQuery::from(self.into_select())
                }

                pub async fn delete(self, db: &Db) -> Result<()> {
                    db.delete(self.into_select()).await
                }

                pub fn filter(self, filter: stmt::Expr<bool>) -> Query {
                    let stmt = self.into_select();
                    Query::from_stmt(stmt.and(filter))
                }

                pub async fn collect<A>(self, db: &Db) -> Result<A>
                where
                    A: FromCursor<super::#model_struct_name>
                {
                    self.all(db).await?.collect().await
                }

                // #( #relation_methods )*
            }

            impl stmt::IntoSelect for #query_struct_name {
                type Model = super::#model_struct_name;

                fn into_select(self) -> stmt::Select<Self::Model> {
                    stmt::Select::filter(#query)
                }
            }
        }
    }

    fn gen_method_args(
        &self,
        args: &[app::Arg],
        arg_idents: &[TokenStream],
        depth: usize,
    ) -> TokenStream {
        let args = args.iter().enumerate().map(move |(i, arg)| {
            let name = &arg_idents[i];

            match &arg.ty {
                stmt::Type::Model(model_id) => {
                    let target_struct_name = self.model_struct_path(*model_id, depth);

                    quote!(
                        #name: impl stmt::IntoExpr<#target_struct_name>
                    )
                }
                stmt::Type::ForeignKey(field_id) => {
                    let field_name = self.field_name(field_id);
                    let relation_struct_name = self.relation_struct_name(field_id);

                    quote!(
                        #name: impl stmt::IntoExpr<super::relation::#field_name::#relation_struct_name>
                    )
                }
                ty => {
                    let ty = self.ty(ty, depth);

                    quote! {
                        #name: impl stmt::IntoExpr<#ty>
                    }
                }
            }
        });

        quote!( #( #args ),* )
    }

    fn gen_item_arg_tys(&self, args: &[app::Arg]) -> TokenStream {
        let mut tys = args.iter().map(move |arg| match &arg.ty {
            stmt::Type::Model(model) => {
                let target_struct_name = self.model_struct_path(*model, 1);
                quote!( #target_struct_name )
            }
            stmt::Type::ForeignKey(field_id) => {
                let field_name = self.field_name(field_id);
                let relation_struct_name = self.relation_struct_name(field_id);

                quote!( super::relation::#field_name::#relation_struct_name )
            }
            ty => {
                let ty = self.ty(ty, 1);
                quote!(#ty)
            }
        });

        if tys.len() == 1 {
            let ty = tys.next().unwrap();
            quote!(#ty)
        } else {
            quote!( ( #( #tys ),* ) )
        }
    }

    fn gen_find_by_struct(&self, query: &app::Query, depth: usize) -> TokenStream {
        let path = self.module_path(query.ret, depth);
        let model_struct_name = self.model_struct_path(query.ret, depth);
        let query_struct_name = self.query_struct_name(query.id);

        quote! {
            pub struct #query_struct_name {
                stmt: stmt::Select<#model_struct_name>,
            }

            impl #query_struct_name {
                pub async fn all(self, db: &Db) -> Result<Cursor<#model_struct_name>> {
                    db.all(self.stmt).await
                }

                pub async fn first(self, db: &Db) -> Result<Option<#model_struct_name>> {
                    db.first(self.stmt).await
                }

                pub async fn get(self, db: &Db) -> Result<#model_struct_name> {
                    db.get(self.stmt).await
                }

                pub fn update(self) -> #path UpdateQuery {
                    #path UpdateQuery::from(self.stmt)
                }

                pub async fn delete(self, db: &Db) -> Result<()> {
                    db.exec(self.stmt.delete()).await?;
                    Ok(())
                }
            }

            impl stmt::IntoSelect for #query_struct_name {
                type Model = #model_struct_name;

                fn into_select(self) -> stmt::Select<Self::Model> {
                    self.stmt
                }
            }
        }
    }

    fn gen_expr_from_stmt(
        &self,
        mid: app::ModelId,
        args: &[TokenStream],
        filter: &stmt::Expr,
        depth: usize,
    ) -> TokenStream {
        let struct_name = self.model_struct_path(mid, depth);

        match filter {
            stmt::Expr::And(exprs) => self.gen_expr_chain(mid, args, exprs, quote!(and), depth),
            stmt::Expr::Or(exprs) => self.gen_expr_chain(mid, args, exprs, quote!(or), depth),
            stmt::Expr::Field(expr_field) => {
                let base = quote!(#struct_name);
                let field = self.field_const_name(expr_field.field);
                quote!( #base :: #field )
            }
            stmt::Expr::Arg(arg) => {
                let arg = &args[arg.position];
                quote!(#arg)
            }
            stmt::Expr::Value(value) => match value {
                stmt::Value::Bool(v) => quote!(#v),
                stmt::Value::String(s) => quote!(#s),
                _ => todo!(),
            },
            stmt::Expr::Record(exprs) => {
                let exprs = exprs
                    .iter()
                    .map(|expr| self.gen_expr_from_stmt(mid, args, expr, depth));
                quote!(( #( #exprs ),* ))
            }
            stmt::Expr::List(exprs) => {
                let exprs = exprs
                    .items
                    .iter()
                    .map(|expr| self.gen_expr_from_stmt(mid, args, expr, depth));
                quote!(vec![ #( #exprs ),* ])
            }
            stmt::Expr::BinaryOp(expr_binary_op) => {
                let lhs = self.gen_expr_from_stmt(mid, args, &expr_binary_op.lhs, depth);
                let rhs = self.gen_expr_from_stmt(mid, args, &expr_binary_op.rhs, depth);

                let op = match expr_binary_op.op {
                    stmt::BinaryOp::Eq => quote!(eq),
                    _ => todo!(),
                };

                quote!(#lhs . #op ( #rhs ))
            }
            stmt::Expr::InList(expr_in_list) => {
                let lhs = self.gen_expr_from_stmt(mid, args, &expr_in_list.expr, depth);
                let rhs = self.gen_expr_from_stmt(mid, args, &expr_in_list.list, depth);
                quote!(stmt::in_set( #lhs, #rhs ))
            }
            stmt::Expr::InSubquery(expr_in_subquery) => {
                let lhs = self.gen_expr_from_stmt(mid, args, &expr_in_subquery.expr, depth);
                let filter = &expr_in_subquery.query.body.as_select().filter;

                let subquery = match filter {
                    stmt::Expr::Arg(arg) => {
                        let arg = &args[arg.position];
                        quote!(#arg)
                    }
                    _ => todo!("expr = {:#?}", filter),
                };

                quote!(#lhs . in_query ( #subquery ))
            }
            expr => todo!("expr={expr:#?}"),
        }
    }

    fn gen_expr_chain(
        &self,
        model_id: app::ModelId,
        args: &[TokenStream],
        exprs: &[stmt::Expr],
        f: TokenStream,
        depth: usize,
    ) -> TokenStream {
        assert!(exprs.len() > 1);

        let [head, rest @ ..] = exprs else {
            panic!()
        };

        let mut expr = self.gen_expr_from_stmt(model_id, args, head, depth);

        for next in rest {
            let next = self.gen_expr_from_stmt(model_id, args, next, depth);
            expr = quote!( #expr . #f ( #next ));
        }

        expr
    }
}
