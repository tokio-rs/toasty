use super::*;

impl<'a> Generator<'a> {
    pub(super) fn self_update_struct_name(&self) -> &syn::Ident {
        self.update_struct_name_for(self.model.id)
    }

    pub(super) fn update_struct_name_for(&self, id: ModelId) -> &syn::Ident {
        &self.names.models[&id].update_name
    }

    pub(super) fn gen_update_struct_def(&self) -> TokenStream {
        let struct_name = self.self_struct_name();
        let update_struct_name = self.self_update_struct_name();
        let update_methods = self.gen_update_methods();
        let update_query_methods = self.gen_update_query_methods();

        let reload = self.model.fields.iter().map(|field| {
            let i = util::int(field.id.index);
            let name = self.field_name(field.id);

            match &field.ty {
                FieldTy::Primitive(primitive) => {
                    let conv = self.value_to_ty_fn(&primitive.ty, field.nullable);

                    if let stmt::Type::Id(_) = primitive.ty {
                        if field.nullable {
                            quote!(#i => self.model.#name = into_iter.next().unwrap().#conv()?.map(stmt::Id::from_untyped),)
                        } else {
                            quote!(#i => self.model.#name = stmt::Id::from_untyped(into_iter.next().unwrap().#conv()?),)
                        }
                    } else {
                        quote!(#i => self.model.#name = into_iter.next().unwrap().#conv()?,)
                    }
                }
                FieldTy::BelongsTo(rel) => {
                    match &rel.foreign_key.fields[..] {
                        [fk_field] => {
                            let source = self.schema.field(fk_field.source);
                            let primitive = source.ty.expect_primitive();
                            let name = self.field_name(fk_field.source);
                            let conv = self.value_to_ty_fn(&primitive.ty, source.nullable);

                            if let stmt::Type::Id(_) = primitive.ty {
                                if field.nullable {
                                    quote!(#i => self.model.#name = into_iter.next().unwrap().#conv()?.map(stmt::Id::from_untyped),)
                                } else {
                                    quote!(#i => self.model.#name = stmt::Id::from_untyped(into_iter.next().unwrap().#conv()?),)
                                }
                            } else {
                                quote!(#i => self.model.#name = into_iter.next().unwrap().#conv()?,)
                            }
                        }
                        _ => todo!(),
                    }
                }
                FieldTy::HasMany(..) => {
                    // TODO: something to do here?
                    quote!(#i => {})
                }
                FieldTy::HasOne(..) => {
                    // TODO: something to do here?
                    quote!(#i => {})
                }
            }

        });

        quote! {
            // TODO: unify with `UpdateQuery`
            #[derive(Debug)]
            pub struct #update_struct_name<'a> {
                model: &'a mut #struct_name,
                query: UpdateQuery<'a>,
            }

            #[derive(Debug)]
            pub struct UpdateQuery<'a> {
                stmt: stmt::Update<'a, #struct_name>,
            }

            impl<'a> #update_struct_name<'a> {
                #update_methods

                pub async fn exec(self, db: &Db) -> Result<()> {
                    let fields;
                    let mut into_iter;

                    {
                        let mut stmt = self.query.stmt;
                        fields = stmt.fields().clone();

                        stmt.set_selection(&*self.model);

                        let mut records = db.exec::<#struct_name>(stmt.into()).await?;
                        // TODO: try to avoid the vec clone from turning a record static
                        into_iter = records.next().await.unwrap()?.into_record().into_owned().into_iter();
                    }

                    for field in fields.iter() {
                        match field.into_usize() {
                            #( #reload )*
                            _ => todo!("handle unknown field id in reload after update"),
                        }
                    }

                    Ok(())
                }
            }

            impl<'a> UpdateQuery<'a> {
                #update_query_methods

                pub async fn exec(self, db: &Db) -> Result<()> {
                    let stmt = self.stmt;
                    let mut cursor = db.exec(stmt.into()).await?;
                    Ok(())
                }
            }

            impl<'a> From<Query<'a>> for UpdateQuery<'a> {
                fn from(value: Query<'a>) -> UpdateQuery<'a> {
                    UpdateQuery { stmt: stmt::Update::new(value) }
                }
            }

            impl<'a> From<stmt::Select<'a, #struct_name>> for UpdateQuery<'a> {
                fn from(src: stmt::Select<'a, #struct_name>) -> UpdateQuery<'a> {
                    UpdateQuery { stmt: stmt::Update::new(src) }
                }
            }
        }
    }

    pub(super) fn gen_model_update_method_def(&self) -> TokenStream {
        let update_struct_name = self.self_update_struct_name();

        quote! {
            pub fn update<'a>(&'a mut self) -> #update_struct_name<'a> {
                #update_struct_name {
                    model: self,
                    query: UpdateQuery {
                        stmt: stmt::Update::default(),
                    },
                }
            }
        }
    }

    fn gen_update_methods(&self) -> TokenStream {
        self.model.fields.iter().map(|field| {
            let name = self.field_name(field.id);
            let set_ident = ident!("set_{}", name);
            let unset_ident = ident!("unset_{}", name);

            let unset_fn = if field.nullable {
                Some(quote! {
                    pub fn #unset_ident(&mut self) -> &mut Self {
                        self.query.#unset_ident();
                        self
                    }
                })
            } else {
                None
            };

            match &field.ty {
                FieldTy::Primitive(_) => {
                    let ty = self.field_ty(&field, 0);

                    quote! {
                        pub fn #name(mut self, #name: impl Into<#ty>) -> Self {
                            self.query.#set_ident(#name);
                            self
                        }

                        #unset_fn
                    }
                }
                FieldTy::HasOne(rel) => {
                    let target_struct_name = self.model_struct_path(rel.target, 0);

                    quote! {
                        pub fn #name(mut self, #name: impl IntoExpr<'a, #target_struct_name>) -> Self {
                            self.query.#set_ident(#name);
                            self
                        }

                        #unset_fn
                    }
                }
                FieldTy::HasMany(rel) => {
                    let singular = self.singular_name(field);
                    let target_struct_name = self.model_struct_path(rel.target, 0);
                    let add_ident = ident!("add_{}", singular);

                    quote! {
                        pub fn #singular(mut self, #singular: impl IntoExpr<'a, #target_struct_name>) -> Self {
                            self.query.#add_ident(#singular);
                            self
                        }
                    }
                }
                FieldTy::BelongsTo(_) => {
                    let relation_struct_path = self.field_ty(field, 0);

                    quote! {
                        pub fn #name<'b>(mut self, #name: impl IntoExpr<'a, #relation_struct_path<'b>>) -> Self {
                            self.query.#set_ident(#name);
                            self
                        }

                        #unset_fn
                    }
                }
            }
        }).collect()
    }

    fn gen_update_query_methods(&self) -> TokenStream {
        self.model.fields.iter().map(|field| {
            let name = self.field_name(field.id);
            let index = util::int(field.id.index);
            let set_ident = ident!("set_{}", name);
            let unset_ident = ident!("unset_{}", name);

            let unset_fn = if field.nullable {
                Some(quote! {
                    pub fn #unset_ident(&mut self) -> &mut Self {
                        self.stmt.set(#index, Value::Null);
                        self
                    }
                })
            } else {
                None
            };

            match &field.ty {
                FieldTy::Primitive(_) => {
                    let ty = self.field_ty(&field, 0);

                    quote! {
                        pub fn #name(mut self, #name: impl Into<#ty>) -> Self {
                            self.#set_ident(#name);
                            self
                        }

                        pub fn #set_ident(&mut self, #name: impl Into<#ty>) -> &mut Self {
                            self.stmt.set_expr(#index, #name.into());
                            self
                        }

                        #unset_fn
                    }
                }
                FieldTy::HasOne(rel) => {
                    let target_struct_name = self.model_struct_path(rel.target, 0);

                    quote! {
                        pub fn #name(mut self, #name: impl IntoExpr<'a, #target_struct_name>) -> Self {
                            self.#set_ident(#name);
                            self
                        }

                        pub fn #set_ident(&mut self, #name: impl IntoExpr<'a, #target_struct_name>) -> &mut Self {
                            self.stmt.set_expr(#index, #name.into_expr());
                            self
                        }

                        #unset_fn
                    }
                }
                FieldTy::HasMany(rel) => {
                    let singular = self.singular_name(field);
                    let target_struct_name = self.model_struct_path(rel.target, 0);
                    let add_ident = ident!("add_{}", singular);

                    quote! {
                        pub fn #singular(mut self, #singular: impl IntoExpr<'a, #target_struct_name>) -> Self {
                            self.#add_ident(#singular);
                            self
                        }

                        pub fn #add_ident(&mut self, #singular: impl IntoExpr<'a, #target_struct_name>) -> &mut Self {
                            self.stmt.push_expr(#index, #singular.into_expr());
                            self
                        }
                    }
                }
                FieldTy::BelongsTo(_) => {
                    let relation_struct_path = self.field_ty(field, 0);

                    quote! {
                        pub fn #name<'b>(mut self, #name: impl IntoExpr<'a, #relation_struct_path<'b>>) -> Self {
                            self.#set_ident(#name);
                            self
                        }

                        pub fn #set_ident<'b>(&mut self, #name: impl IntoExpr<'a, #relation_struct_path<'b>>) -> &mut Self {
                            self.stmt.set_expr(#index, #name.into_expr());
                            self
                        }

                        #unset_fn
                    }
                }
            }
        }).collect()
    }
}
