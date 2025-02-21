use super::*;

use app::FieldTy;

impl Generator<'_> {
    pub(super) fn self_update_struct_name(&self) -> &syn::Ident {
        self.update_struct_name_for(self.model.id)
    }

    pub(super) fn update_struct_name_for(&self, id: app::ModelId) -> &syn::Ident {
        &self.names.models[&id].update_name
    }

    pub(super) fn gen_model_update_method_def(&self) -> TokenStream {
        let update_struct_name = self.self_update_struct_name();

        quote! {
            pub fn update(&mut self) -> builders::#update_struct_name<'_> {
                let query = builders::UpdateQuery::from(self.into_select());
                builders::#update_struct_name {
                    model: self,
                    query,
                }
            }
        }
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
                            quote!(#i => self.model.#name = value.#conv()?.map(stmt::Id::from_untyped),)
                        } else {
                            quote!(#i => self.model.#name = stmt::Id::from_untyped(value.#conv()?),)
                        }
                    } else {
                        quote!(#i => self.model.#name = value.#conv()?,)
                    }
                }
                _ => quote!(#i => todo!("should not be set; {} = {value:#?}", #i),)
            }

        });

        quote! {
            // TODO: unify with `UpdateQuery`
            #[derive(Debug)]
            pub struct #update_struct_name<'a> {
                // TODO: builder?
                pub(super) model: &'a mut #struct_name,
                pub(super) query: UpdateQuery,
            }

            #[derive(Debug)]
            pub struct UpdateQuery {
                stmt: stmt::Update<#struct_name>,
            }

            impl #update_struct_name<'_> {
                #update_methods

                pub async fn exec(self, db: &Db) -> Result<()> {
                    let mut stmt = self.query.stmt;
                    let mut result = db.exec_one(stmt.into()).await?;

                    for (field, value) in result.into_sparse_record().into_iter() {
                        match field {
                            #( #reload )*
                            _ => todo!("handle unknown field id in reload after update"),
                        }
                    }

                    Ok(())
                }
            }

            impl UpdateQuery {
                #update_query_methods

                pub async fn exec(self, db: &Db) -> Result<()> {
                    let stmt = self.stmt;
                    let mut cursor = db.exec(stmt.into()).await?;
                    Ok(())
                }
            }

            impl From<Query> for UpdateQuery {
                fn from(value: Query) -> UpdateQuery {
                    UpdateQuery { stmt: stmt::Update::new(value.stmt) }
                }
            }

            impl From<stmt::Select<#struct_name>> for UpdateQuery {
                fn from(src: stmt::Select<#struct_name>) -> UpdateQuery {
                    UpdateQuery { stmt: stmt::Update::new(src) }
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
                    let ty = self.field_ty(field, 1);

                    quote! {
                        pub fn #name(mut self, #name: impl Into<#ty>) -> Self {
                            self.query.#set_ident(#name);
                            self
                        }

                        #unset_fn
                    }
                }
                FieldTy::HasOne(rel) => {
                    let target_struct_name = self.model_struct_path(rel.target, 1);

                    quote! {
                        pub fn #name(mut self, #name: impl IntoExpr<#target_struct_name>) -> Self {
                            self.query.#set_ident(#name);
                            self
                        }

                        #unset_fn
                    }
                }
                FieldTy::HasMany(rel) => {
                    let singular = self.singular_name(field);
                    let target_struct_name = self.model_struct_path(rel.target, 1);
                    let add_ident = ident!("add_{}", singular);

                    quote! {
                        pub fn #singular(mut self, #singular: impl IntoExpr<#target_struct_name>) -> Self {
                            self.query.#add_ident(#singular);
                            self
                        }
                    }
                }
                FieldTy::BelongsTo(rel) => {
                    let target_struct_name = self.model_struct_path(rel.target, 1);

                    quote! {
                        pub fn #name(mut self, #name: impl IntoExpr<#target_struct_name>) -> Self {
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
                    let ty = self.field_ty(field, 1);

                    quote! {
                        pub fn #name(mut self, #name: impl Into<#ty>) -> Self {
                            self.#set_ident(#name);
                            self
                        }

                        pub fn #set_ident(&mut self, #name: impl Into<#ty>) -> &mut Self {
                            self.stmt.set(#index, #name.into());
                            self
                        }

                        #unset_fn
                    }
                }
                FieldTy::HasOne(rel) => {
                    let target_struct_name = self.model_struct_path(rel.target, 1);

                    quote! {
                        pub fn #name(mut self, #name: impl IntoExpr<#target_struct_name>) -> Self {
                            self.#set_ident(#name);
                            self
                        }

                        pub fn #set_ident(&mut self, #name: impl IntoExpr<#target_struct_name>) -> &mut Self {
                            self.stmt.set(#index, #name.into_expr());
                            self
                        }

                        #unset_fn
                    }
                }
                FieldTy::HasMany(rel) => {
                    let singular = self.singular_name(field);
                    let target_struct_name = self.model_struct_path(rel.target, 1);
                    let add_ident = ident!("add_{}", singular);

                    quote! {
                        pub fn #singular(mut self, #singular: impl IntoExpr<#target_struct_name>) -> Self {
                            self.#add_ident(#singular);
                            self
                        }

                        pub fn #add_ident(&mut self, #singular: impl IntoExpr<#target_struct_name>) -> &mut Self {
                            self.stmt.insert(#index, #singular.into_expr());
                            self
                        }
                    }
                }
                FieldTy::BelongsTo(rel) => {
                    let target_struct_name = self.model_struct_path(rel.target, 1);

                    quote! {
                        pub fn #name(mut self, #name: impl IntoExpr<#target_struct_name>) -> Self {
                            self.#set_ident(#name);
                            self
                        }

                        pub fn #set_ident(&mut self, #name: impl IntoExpr<#target_struct_name>) -> &mut Self {
                            self.stmt.set(#index, #name.into_expr());
                            self
                        }

                        #unset_fn
                    }
                }
            }
        }).collect()
    }
}
