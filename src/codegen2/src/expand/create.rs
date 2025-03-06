use super::{util, Expand};
use crate::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_create_builder(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let create_builder_ident = &self.model.create_builder_struct_ident;
        let create_methods = self.expand_create_methods();
        /*
        let create_struct_name = self.self_create_struct_name();
        */

        quote! {
            #[derive(Debug)]
            #vis struct #create_builder_ident {
                stmt: #toasty::stmt::Insert<#model_ident>,
            }

            impl #create_builder_ident {
                #create_methods

                #vis async fn exec(self, db: &#toasty::Db) -> #toasty::Result<#model_ident> {
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

                fn by_ref(&self) -> stmt::Expr<#struct_name> {
                    todo!()
                }
            }

            impl IntoExpr<[#struct_name]> for #create_struct_name {
                fn into_expr(self) -> stmt::Expr<[#struct_name]> {
                    self.stmt.into_list_expr()
                }

                fn by_ref(&self) -> stmt::Expr<[#struct_name]> {
                    todo!()
                }
            }
            */

            impl Default for #create_builder_ident {
                fn default() -> #create_builder_ident {
                    #create_builder_ident {
                        stmt: #toasty::stmt::Insert::blank(),
                    }
                }
            }
        }
    }

    fn expand_create_methods(&self) -> TokenStream {
        let toasty = &self.toasty;

        self.model
            .fields
            .iter()
            .enumerate()
            .map(move |(index, field)| {
                let name = &field.name.ident;
                let index_tokenized = util::int(index);

                match &field.ty {
                    /*
                    FieldTy::HasOne(rel) => {
                        let target_struct_name = self.model_struct_path(rel.target, 1);

                        quote! {
                            pub fn #name(mut self, #name: impl IntoExpr<#target_struct_name>) -> Self {
                                self.stmt.set(#index, #name.into_expr());
                                self
                            }
                        }
                    }
                    */
                    FieldTy::HasMany(rel) => {
                        let singular = &rel.singular.ident;
                        let ty = &rel.ty;

                        quote! {
                            pub fn #singular(mut self, #singular: impl #toasty::IntoExpr<#ty>) -> Self {
                                self.stmt.insert(#index, #singular.into_expr());
                                self
                            }
                        }
                    }
                    FieldTy::BelongsTo(rel) => {
                        let ty = &rel.ty;

                        quote! {
                            pub fn #name(mut self, #name: impl #toasty::IntoExpr<#ty>) -> Self {
                                self.stmt.set(#index_tokenized, #name.into_expr());
                                self
                            }
                        }
                    }
                    FieldTy::Primitive(ty) => {
                        quote! {
                            pub fn #name(mut self, #name: impl #toasty::IntoExpr<#ty>) -> Self {
                                self.stmt.set(#index_tokenized, #name.into_expr());
                                self
                            }
                        }
                    }
                }
            })
            .collect()
    }
}
