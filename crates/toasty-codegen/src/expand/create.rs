use super::{util, Expand};
use crate::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_create_builder(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let create_struct_ident = &self.model.create_struct_ident;
        let create_methods = self.expand_create_methods();

        quote! {
            #vis struct #create_struct_ident {
                stmt: #toasty::stmt::Insert<#model_ident>,
            }

            impl #create_struct_ident {
                #create_methods

                #vis async fn exec(self, db: &#toasty::Db) -> #toasty::Result<#model_ident> {
                    db.exec_insert_one(self.stmt).await
                }
            }

            impl #toasty::IntoInsert for #create_struct_ident {
                type Model = #model_ident;

                fn into_insert(self) -> #toasty::stmt::Insert<#model_ident> {
                    self.stmt
                }
            }

            impl #toasty::IntoExpr<#model_ident> for #create_struct_ident {
                fn into_expr(self) -> #toasty::stmt::Expr<#model_ident> {
                    self.stmt.into()
                }

                fn by_ref(&self) -> #toasty::stmt::Expr<#model_ident> {
                    todo!()
                }
            }

            impl #toasty::IntoExpr<Option<#model_ident>> for #create_struct_ident {
                fn into_expr(self) -> #toasty::stmt::Expr<Option<#model_ident>> {
                    self.stmt.into()
                }

                fn by_ref(&self) -> #toasty::stmt::Expr<Option<#model_ident>> {
                    todo!()
                }
            }

            impl #toasty::IntoExpr<[#model_ident]> for #create_struct_ident {
                fn into_expr(self) -> #toasty::stmt::Expr<[#model_ident]> {
                    self.stmt.into_list_expr()
                }

                fn by_ref(&self) -> #toasty::stmt::Expr<[#model_ident]> {
                    todo!()
                }
            }

            impl Default for #create_struct_ident {
                fn default() -> #create_struct_ident {
                    #create_struct_ident {
                        stmt: #toasty::stmt::Insert::blank_single(),
                    }
                }
            }
        }
    }

    fn expand_create_methods(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;

        self.model
            .fields
            .iter()
            .enumerate()
            .map(move |(index, field)| {
                let name = &field.name.ident;
                let index_tokenized = util::int(index);

                match &field.ty {
                    FieldTy::BelongsTo(rel) => {
                        let ty = &rel.ty;

                        quote! {
                            pub fn #name(mut self, #name: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> Self {
                                // Silences unused field warning when the field is set on creation.
                                if false {
                                    let m = <#model_ident as #toasty::Model>::load(Default::default()).unwrap();
                                    let _ = &m.#name;
                                }

                                self.stmt.set(#index_tokenized, #name.into_expr());
                                self
                            }
                        }
                    }
                    FieldTy::HasMany(rel) => {
                        let singular = &rel.singular.ident;
                        let ty = &rel.ty;

                        quote! {
                            pub fn #singular(mut self, #singular: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> Self {
                                self.stmt.insert(#index, #singular.into_expr());
                                self
                            }
                        }
                    }
                    FieldTy::HasOne(rel) => {
                        let ty = &rel.ty;

                        quote! {
                            pub fn #name(mut self, #name: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> Self {
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
