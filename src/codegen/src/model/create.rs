use super::*;

impl<'a> Generator<'a> {
    pub(super) fn gen_create_struct(&self) -> TokenStream {
        let struct_name = self.self_struct_name();
        let create_struct_name = self.self_create_struct_name();
        let create_methods = self.gen_create_methods();

        quote! {
            #[derive(Debug)]
            pub struct #create_struct_name<'a> {
                pub(super) stmt: stmt::Insert<'a, #struct_name>,
            }

            impl<'a> #create_struct_name<'a> {
                #create_methods

                pub async fn exec(self, db: &'a Db) -> Result<#struct_name> {
                    db.exec_insert_one::<#struct_name>(self.stmt).await
                }
            }

            impl<'a> IntoInsert<'a> for #create_struct_name<'a> {
                type Model = #struct_name;

                fn into_insert(self) -> stmt::Insert<'a, #struct_name> {
                    self.stmt
                }
            }

            impl<'a> IntoExpr<'a, #struct_name> for #create_struct_name<'a> {
                fn into_expr(self) -> stmt::Expr<'a, #struct_name> {
                    self.stmt.into()
                }
            }

            impl<'a> IntoExpr<'a, [#struct_name]> for #create_struct_name<'a> {
                fn into_expr(self) -> stmt::Expr<'a, [#struct_name]> {
                    self.stmt.into_list_expr()
                }
            }

            impl<'a> Default for #create_struct_name<'a> {
                fn default() -> #create_struct_name<'a> {
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

    pub(super) fn create_struct_path(&self, id: ModelId, depth: usize) -> TokenStream {
        let name = &self.names.models[&id].create_name;

        if id == self.model.id {
            quote!(#name)
        } else {
            let prefix = self.module_name(id, depth);
            quote!(#prefix::#name)
        }
    }

    fn gen_create_methods(&self) -> TokenStream {
        self.model.fields.iter().map(move |field| {
            let name = self.field_name(field.id);
            let index = util::int(field.id.index);

            match &field.ty {
                FieldTy::HasMany(rel) => {
                    let singular = self.singular_name(field);
                    let target_struct_name = self.model_struct_path(rel.target, 0);

                    quote! {
                        pub fn #singular(mut self, #singular: impl IntoExpr<'a, #target_struct_name>) -> Self {
                            self.stmt.push_expr(#index, #singular.into_expr());
                            self
                        }
                    }
                }
                FieldTy::HasOne(rel) => {
                    let target_struct_name = self.model_struct_path(rel.target, 0);

                    quote! {
                        pub fn #name(mut self, #name: impl IntoExpr<'a, #target_struct_name>) -> Self {
                            self.stmt.set_expr(#index, #name.into_expr());
                            self
                        }
                    }
                }
                FieldTy::BelongsTo(_) => {
                    let relation_struct_path = self.field_ty(field, 0);

                    quote! {
                        pub fn #name<'b>(mut self, #name: impl IntoExpr<'a, #relation_struct_path<'b>>) -> Self {
                            self.stmt.set_expr(#index, #name.into_expr());
                            self
                        }
                    }
                }
                FieldTy::Primitive(_) => {
                    let ty = self.field_ty(field, 0);
                    let ty = quote!(impl Into<#ty>);

                    quote! {
                        pub fn #name(mut self, #name: #ty) -> Self {
                            self.stmt.set_value(#index, #name.into());
                            self
                        }
                    }
                }
            }
        }).collect()
    }
}
