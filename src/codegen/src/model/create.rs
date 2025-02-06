use super::*;

impl<'a> Generator<'a> {
    pub(super) fn gen_create_struct(&self) -> TokenStream {
        let struct_name = self.self_struct_name();
        let create_struct_name = self.self_create_struct_name();
        let create_methods = self.gen_create_methods();

        quote! {
            #[derive(Debug)]
            pub struct #create_struct_name {
                pub(super) stmt: stmt::Insert<#struct_name>,
            }

            impl #create_struct_name {
                #create_methods

                pub async fn exec(self, db: &Db) -> Result<#struct_name> {
                    db.exec_insert_one(self.stmt).await
                }
            }

            /*
            impl IntoInsert for #create_struct_name {
                type Model = #struct_name;

                fn into_insert(self) -> stmt::Insert<#struct_name> {
                    self.stmt
                }
            }

            impl IntoExpr<#struct_name> for #create_struct_name {
                fn into_expr(self) -> stmt::Expr<#struct_name> {
                    self.stmt.into()
                }
            }

            impl IntoExpr<[#struct_name]> for #create_struct_name {
                fn into_expr(self) -> stmt::Expr<[#struct_name]> {
                    self.stmt.into_list_expr()
                }
            }
            */

            impl Default for #create_struct_name {
                fn default() -> #create_struct_name {
                    #create_struct_name {
                        stmt: stmt::Insert::blank(),
                    }
                }
            }
        }
    }

    pub(super) fn self_create_struct_name(&self) -> TokenStream {
        self.create_struct_path(self.model.id, 0)
    }

    pub(super) fn create_struct_path(&self, id: app::ModelId, depth: usize) -> TokenStream {
        let name = &self.names.models[&id].create_name;

        if id == self.model.id {
            quote!(#name)
        } else {
            let prefix = self.module_name(id, depth);
            quote!(#prefix::#name)
        }
    }

    fn gen_create_methods(&self) -> TokenStream {
        use app::FieldTy;

        self.model.fields.iter().map(move |field| {
            let name = self.field_name(field.id);
            let index = util::int(field.id.index);

            match &field.ty {
                FieldTy::HasMany(rel) => {
                    let singular = self.singular_name(field);
                    let target_struct_name = self.model_struct_path(rel.target, 1);

                    quote! {
                        pub fn #singular(mut self, #singular: impl IntoExpr<#target_struct_name>) -> Self {
                            self.stmt.insert(#index, #singular.into_expr());
                            self
                        }
                    }
                }
                FieldTy::HasOne(rel) => {
                    let target_struct_name = self.model_struct_path(rel.target, 1);

                    quote! {
                        pub fn #name(mut self, #name: impl IntoExpr<#target_struct_name>) -> Self {
                            self.stmt.set(#index, #name.into_expr());
                            self
                        }
                    }
                }
                FieldTy::BelongsTo(rel) => {
                    let target_struct_name = self.model_struct_path(rel.target, 1);

                    quote! {
                        pub fn #name(mut self, #name: impl IntoExpr<#target_struct_name>) -> Self {
                            self.stmt.set(#index, #name.into_expr());
                            self
                        }
                    }
                }
                FieldTy::Primitive(_) => {
                    let ty = self.field_ty(field, 1);
                    let ty = quote!(impl Into<#ty>);

                    quote! {
                        pub fn #name(mut self, #name: #ty) -> Self {
                            self.stmt.set(#index, #name.into());
                            self
                        }
                    }
                }
            }
        }).collect()
    }
}
