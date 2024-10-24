use super::*;

impl<'a> Generator<'a> {
    pub(super) fn gen_relation_structs(&self) -> TokenStream {
        self.model
            .fields
            .iter()
            .filter_map(|field| match &field.ty {
                FieldTy::HasMany(rel) => Some(self.gen_has_many_struct(rel, field.id)),
                FieldTy::HasOne(rel) => Some(self.gen_has_one_struct(rel, field)),
                FieldTy::BelongsTo(rel) => Some(self.gen_belongs_to_struct(rel, field)),
                FieldTy::Primitive(..) => None,
            })
            .collect()
    }

    pub(super) fn gen_relation_fields(&self) -> TokenStream {
        self.model
            .fields
            .iter()
            .filter_map(|field| match &field.ty {
                FieldTy::HasMany(rel) => Some(self.gen_relation_field(field, rel.target)),
                FieldTy::HasOne(rel) => Some(self.gen_relation_field(field, rel.target)),
                FieldTy::BelongsTo(rel) => Some(self.gen_relation_field(field, rel.target)),
                FieldTy::Primitive(..) => None,
            })
            .collect()
    }

    fn gen_has_many_struct(&self, rel: &HasMany, field: FieldId) -> TokenStream {
        let field_name = self.field_name(field);
        let field_const_name = self.field_const_name(field);
        let pair_field_const_name = self.field_const_name(rel.pair);
        let model_struct_name = self.self_struct_name();
        let relation_struct_name = self.relation_struct_name(field);
        let target_mod_name = self.module_name(rel.target, 2);
        let target_struct_name = self.model_struct_path(rel.target, 2);
        let target_create_struct_path = self.create_struct_path(rel.target, 2);

        let scoped_query_method_defs = rel
            .queries
            .iter()
            .map(|scoped_query| self.gen_scoped_find_by_method(scoped_query))
            .collect::<TokenStream>();

        let scoped_query_struct_defs = rel
            .queries
            .iter()
            .map(|scoped_query| self.gen_scoped_find_by_struct(scoped_query, 2))
            .collect::<TokenStream>();

        quote! {
            pub mod #field_name {
                use super::*;

                #[derive(Debug)]
                pub struct #relation_struct_name<'a> {
                    scope: &'a #model_struct_name,
                }

                #[derive(Debug)]
                pub struct Query<'a> {
                    pub(super) scope: super::Query<'a>,
                }

                #[derive(Debug)]
                pub struct Remove<'a> {
                    stmt: stmt::Unlink<'a, super::#model_struct_name>,
                }

                #[derive(Debug)]
                pub struct Add<'a> {
                    stmt: stmt::Link<'a, super::#model_struct_name>,
                }

                impl super::#model_struct_name {
                    pub fn #field_name(&self) -> #relation_struct_name<'_> {
                        #relation_struct_name { scope: self }
                    }
                }

                impl<'a> super::Query<'a> {
                    pub fn #field_name(self) -> Query<'a> {
                        Query::with_scope(self)
                    }
                }

                impl<'a> #relation_struct_name<'a> {
                    pub fn get(&self) -> &[#target_struct_name] {
                        self.scope.#field_name.get()
                    }

                    /// Iterate all entries in the relation
                    pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, #target_struct_name>> {
                        db.all(self.into_select()).await
                    }

                    pub async fn collect<A>(self, db: &'a Db) -> Result<A>
                    where
                        A: FromCursor<#target_struct_name>
                    {
                        self.all(db).await?.collect().await
                    }

                    /// Create a new associated record
                    pub fn create(self) -> #target_create_struct_path<'a> {
                        let mut builder = #target_create_struct_path::default();
                        builder.stmt.set_scope(self);
                        builder
                    }

                    pub fn query(
                        self,
                        filter: stmt::Expr<'a, bool>
                    ) -> #target_mod_name::Query<'a> {
                        let query = self.into_select();
                        #target_mod_name::Query::from_stmt(query.and(filter))
                    }

                    /// Add an item to the association
                    pub fn add(self, #field_name: impl IntoSelect<'a, Model = #target_struct_name>) -> Add<'a> {
                        Add {
                            stmt: stmt::Link::new(self.scope,
                                super::#model_struct_name::#field_const_name,
                                #field_name,
                            ),
                        }
                    }

                    /// Remove items from the association
                    pub fn remove(self, #field_name: impl IntoSelect<'a, Model = #target_struct_name>) -> Remove<'a> {
                        Remove {
                            stmt: stmt::Unlink::new(
                                self.scope,
                                super::#model_struct_name::#field_const_name,
                                #field_name,
                            ),
                        }
                    }

                    #scoped_query_method_defs
                }

                impl<'a> stmt::IntoSelect<'a> for #relation_struct_name<'a> {
                    type Model = #target_struct_name;

                    fn into_select(self) -> stmt::Select<'a, #target_struct_name> {
                        #target_struct_name::filter(
                            #target_struct_name::#pair_field_const_name.in_query(self.scope)
                        ).into_select()
                    }
                }

                impl<'a> Query<'a> {
                    // TODO: rename `from_stmt`?
                    pub fn with_scope<S>(scope: S) -> Query<'a>
                    where
                        S: IntoSelect<'a, Model = #model_struct_name>,
                    {
                        Query { scope: super::Query::from_stmt(scope.into_select()) }
                    }

                    #scoped_query_method_defs
                }

                impl<'a> Add<'a> {
                    pub async fn exec(self, db: &'a Db) -> Result<()> {
                        let mut cursor = db.exec(self.stmt.into()).await?;
                        assert!(cursor.next().await.is_none());
                        Ok(())
                    }
                }

                impl<'a> Remove<'a> {
                    pub async fn exec(self, db: &'a Db) -> Result<()> {
                        let mut cursor = db.exec(self.stmt.into()).await?;
                        assert!(cursor.next().await.is_none());
                        Ok(())
                    }
                }

                #scoped_query_struct_defs
            }

            pub use #field_name::#relation_struct_name;
        }
    }

    fn gen_relation_field(&self, field: &Field, target: ModelId) -> TokenStream {
        let field_name = self.field_name(field.id);
        let relation_struct_name = self.relation_struct_name(field);
        let target_struct_name = self.model_struct_path(target, 1);

        let path_methods = self.gen_path_methods(self.schema.model(target), 1);

        let target_ty = if field.ty.is_has_many() {
            quote!([#target_struct_name])
        } else {
            quote!(#target_struct_name)
        };

        let relation_struct = if field.ty.is_belongs_to() {
            quote!(#relation_struct_name<'stmt>)
        } else {
            quote!(#relation_struct_name<'stmt>)
        };

        let op_methods = if field.ty.is_belongs_to() {
            quote! {
                pub fn eq<'a, 'b, T>(
                    self,
                    rhs: T
                ) -> stmt::Expr<'a, bool>
                where
                    T: toasty::stmt::IntoExpr<'a, super::relation::#field_name::#relation_struct_name<'b>>,
                {
                    self.path.eq(rhs.into_expr().cast())
                }

                pub fn in_query<'a, Q>(self, rhs: Q) -> toasty::stmt::Expr<'a, bool>
                where
                    Q: stmt::IntoSelect<'a, Model = #target_struct_name>,
                {
                    self.path.in_query(rhs)
                }
            }
        } else {
            quote!()
        };

        quote! {
            pub struct #relation_struct_name {
                pub(super) path: Path<#target_ty>,
            }

            impl #relation_struct_name {
                pub const fn from_path(path: Path<#target_ty>) -> #relation_struct_name {
                    #relation_struct_name { path }
                }

                #path_methods

                #op_methods
            }

            impl From<#relation_struct_name> for Path<#target_ty> {
                fn from(val: #relation_struct_name) -> Path<#target_ty> {
                    val.path
                }
            }

            impl<'stmt> stmt::IntoExpr<'stmt, super::relation::#field_name::#relation_struct> for #relation_struct_name {
                fn into_expr(self) -> stmt::Expr<'stmt, super::relation::#field_name::#relation_struct> {
                    todo!("into_expr for {} (field path struct)", stringify!(#relation_struct_name));
                }
            }
        }
    }

    fn gen_has_one_struct(&self, rel: &HasOne, field: &Field) -> TokenStream {
        let field_name = self.field_name(field);
        let pair_field_const_name = self.field_const_name(rel.pair);
        let model_struct_name = self.self_struct_name();
        let relation_struct_name = self.relation_struct_name(field);
        let target_struct_name = self.model_struct_path(rel.target, 2);
        let target_create_struct_path = self.create_struct_path(rel.target, 2);
        let get_ret_ty;
        let get_db_fn;

        if field.nullable {
            get_ret_ty = quote!(Option<#target_struct_name>);
            get_db_fn = quote!(first);
        } else {
            get_ret_ty = quote!(#target_struct_name);
            get_db_fn = quote!(get);
        }

        quote! {
            pub mod #field_name {
                use super::*;

                #[derive(Debug)]
                pub struct #relation_struct_name<'a> {
                    scope: &'a #model_struct_name,
                }

                #[derive(Debug)]
                pub struct Query<'a> {
                    pub(super) scope: super::Query<'a>,
                }

                impl super::#model_struct_name {
                    pub fn #field_name(&self) -> #relation_struct_name<'_> {
                        #relation_struct_name { scope: self }
                    }
                }

                impl<'a> super::Query<'a> {
                    pub fn #field_name(self) -> Query<'a> {
                        Query::with_scope(self)
                    }
                }

                impl<'a> #relation_struct_name<'a> {
                    /// Get the relation
                    pub async fn get(self, db: &'a Db) -> Result<#get_ret_ty> {
                        db.#get_db_fn(self.into_select()).await
                    }

                    /// Create a new associated record
                    pub fn create(self) -> #target_create_struct_path<'a> {
                        let mut builder = #target_create_struct_path::default();
                        builder.stmt.set_scope(self);
                        builder
                    }
                }

                impl<'a> stmt::IntoSelect<'a> for #relation_struct_name<'a> {
                    type Model = #target_struct_name;

                    fn into_select(self) -> stmt::Select<'a, #target_struct_name> {
                        #target_struct_name::filter(
                            #target_struct_name::#pair_field_const_name.in_query(self.scope)
                        ).into_select()
                    }
                }

                impl<'stmt> Query<'stmt> {
                    // TODO: rename `from_stmt`?
                    pub fn with_scope<S>(scope: S) -> Query<'stmt>
                    where
                        S: IntoSelect<'stmt, Model = #model_struct_name>,
                    {
                        Query { scope: super::Query::from_stmt(scope.into_select()) }
                    }
                }
            }

            pub use #field_name::#relation_struct_name;
        }
    }

    pub(crate) fn gen_belongs_to_struct(&self, rel: &BelongsTo, field: &Field) -> TokenStream {
        let field_name = self.field_name(field.id);
        let model_struct_name = self.self_struct_name();
        let relation_struct_name = self.relation_struct_name(field.id);
        let target_struct_name = self.model_struct_path(rel.target, 2);
        let target_create_struct_path = self.create_struct_path(rel.target, 2);

        let find_ret_ty;
        let find_db_fn;

        let find_by_pk = self.model_pk_query_method_name(rel.target);

        let find_by_pk_args = rel.foreign_key.fields.iter().map(|fk_field| {
            let field = self.schema.field(fk_field.source);
            let name = self.field_name(fk_field.source);

            if field.nullable {
                quote!(self.scope.#name.as_ref().expect("TODO: handle null fk fields"))
            } else {
                quote!(&self.scope.#name)
            }
        });

        let target_struct_ref_into_expr_impl;

        match &rel.foreign_key.fields[..] {
            [fk_field] => {
                let name = self.field_name(fk_field.target);
                target_struct_ref_into_expr_impl = quote! {
                    stmt::Expr::from_untyped(&self.#name)
                };
            }
            _ => {
                todo!()
            }
        }

        if field.nullable {
            find_ret_ty = quote!(Option<#target_struct_name>);
            find_db_fn = quote!(first);
        } else {
            find_ret_ty = quote!(#target_struct_name);
            find_db_fn = quote!(get);
        }

        let rel_struct_into_select_impl = quote! {
                    #target_struct_name::#find_by_pk(
                        #( #find_by_pk_args )*
                    ).into_select()
        };

        quote! {
            pub mod #field_name {
                use super::*;

                #[derive(Debug)]
                pub struct #relation_struct_name<'a> {
                    scope: &'a #model_struct_name,
                }

                impl super::#model_struct_name {
                    pub fn #field_name(&self) -> #relation_struct_name<'_> {
                        #relation_struct_name { scope: self }
                    }
                }

                impl<'a> #relation_struct_name<'a> {
                    pub fn get(&self) -> &#target_struct_name {
                        self.scope.#field_name.get()
                    }
                }

                impl<'a> stmt::IntoSelect<'a> for &'a #relation_struct_name<'_> {
                    type Model = #target_struct_name;

                    fn into_select(self) -> stmt::Select<'a, Self::Model> {
                        #rel_struct_into_select_impl
                    }
                }

                impl<'stmt, 'a> stmt::IntoExpr<'stmt, #relation_struct_name<'a>> for #relation_struct_name<'a> {
                    fn into_expr(self) -> stmt::Expr<'stmt, #relation_struct_name<'a>> {
                        // #rel_struct_into_expr_impl
                        todo!("stmt::IntoExpr for {} (belongs_to Fk struct) - self = {:#?}", stringify!(#relation_struct_name), self);
                    }
                }

                impl<'stmt, 'a> stmt::IntoExpr<'stmt, #relation_struct_name<'a>> for &'stmt #relation_struct_name<'a> {
                    fn into_expr(self) -> stmt::Expr<'stmt, #relation_struct_name<'a>> {
                        todo!("stmt::IntoExpr for &'a {} (belongs_to Fk struct) - self = {:#?}", stringify!(#relation_struct_name), self);
                    }
                }

                impl<'stmt, 'a> stmt::IntoExpr<'stmt, #relation_struct_name<'a>> for &'stmt #target_struct_name {
                    fn into_expr(self) -> stmt::Expr<'stmt, #relation_struct_name<'a>> {
                        #target_struct_ref_into_expr_impl
                    }
                }

                impl<'stmt, 'a> stmt::IntoExpr<'stmt, #relation_struct_name<'a>> for #target_create_struct_path<'stmt> {
                    fn into_expr(self) -> stmt::Expr<'stmt, #relation_struct_name<'a>> {
                        let expr: stmt::Expr<'stmt, #target_struct_name> = self.stmt.into();
                        expr.cast()
                    }
                }

                // #field_into_expr

                impl<'a> #relation_struct_name<'a> {
                    // TODO: make this return a query type?
                    pub async fn find(&self, db: &Db) -> Result<#find_ret_ty> {
                        db.#find_db_fn(self.into_select()).await
                    }
                }
            }

            pub use #field_name::#relation_struct_name;
        }
    }
}
