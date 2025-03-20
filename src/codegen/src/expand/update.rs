use super::{util, Expand};
use crate::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_update_builder(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_struct_ident = &self.model.query_struct_ident;
        let update_struct_ident = &self.model.update_struct_ident;
        let update_query_struct_ident = &self.model.update_query_struct_ident;
        let update_methods = self.expand_update_methods();
        let update_query_methods = self.expand_update_query_methods();
        let reload_model = self.expand_reload_model_expr();

        quote! {
            #vis struct #update_struct_ident<'a> {
                model: &'a mut #model_ident,
                query: #update_query_struct_ident,
            }

            #vis struct #update_query_struct_ident {
                stmt: #toasty::stmt::Update<#model_ident>,
            }

            impl #update_struct_ident<'_> {
                #update_methods

                #vis async fn exec(self, db: &#toasty::Db) -> #toasty::Result<()> {
                    let mut stmt = self.query.stmt;
                    let mut result = db.exec_one(stmt.into()).await?;

                    for (field, value) in result.into_sparse_record().into_iter() {
                        match field {
                            #reload_model
                            _ => todo!("handle unknown field id in reload after update"),
                        }
                    }

                    Ok(())
                }
            }

            impl #update_query_struct_ident {
                #update_query_methods

                #vis async fn exec(self, db: &#toasty::Db) -> #toasty::Result<()> {
                    let stmt = self.stmt;
                    let mut cursor = db.exec(stmt.into()).await?;
                    Ok(())
                }
            }

            impl From<#query_struct_ident> for #update_query_struct_ident {
                fn from(value: #query_struct_ident) -> #update_query_struct_ident {
                    #update_query_struct_ident { stmt: #toasty::stmt::Update::new(value.stmt) }
                }
            }

            impl From<#toasty::stmt::Select<#model_ident>> for #update_query_struct_ident {
                fn from(src: #toasty::stmt::Select<#model_ident>) -> #update_query_struct_ident {
                    #update_query_struct_ident { stmt: #toasty::stmt::Update::new(src) }
                }
            }
        }
    }

    fn expand_reload_model_expr(&self) -> TokenStream {
        let toasty = &self.toasty;

        self.model.fields.iter().enumerate().map(|(offset, field)| {
            let i = util::int(offset);
            let field_ident = &field.name.ident;

            match &field.ty {
                FieldTy::Primitive(ty) => {
                    quote!(#i => self.model.#field_ident = <#ty as #toasty::stmt::Primitive>::load(value)?,)
                }
                _ => quote!(#i => todo!("should not be set; {} = {value:#?}", #i),)
            }

        }).collect()
    }

    fn expand_update_methods(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;

        self.model.fields.iter().map(|field| {
            let field_ident = &field.name.ident;
            let set_field_ident = &field.set_ident;

            match &field.ty {
                FieldTy::BelongsTo(rel) => {
                    let ty = &rel.ty;

                    quote! {
                        #vis fn #field_ident(mut self, #field_ident: impl #toasty::IntoExpr<#ty>) -> Self {
                            self.query.#set_field_ident(#field_ident);
                            self
                        }

                        #vis fn #set_field_ident(&mut self, #field_ident: impl #toasty::IntoExpr<#ty>) -> &mut Self {
                            self.query.#set_field_ident(#field_ident);
                            self
                        }
                    }
                }
                FieldTy::HasMany(rel) => {
                    let ty = &rel.ty;
                    let singular = &rel.singular.ident;
                    let insert_ident = &rel.insert_ident;

                    quote! {
                        #vis fn #singular(mut self, #singular: impl #toasty::IntoExpr<#ty>) -> Self {
                            self.query.#insert_ident(#singular);
                            self
                        }

                        #vis fn #insert_ident(&mut self, #singular: impl #toasty::IntoExpr<#ty>) -> &mut Self {
                            self.query.#insert_ident(#singular);
                            self
                        }
                    }
                }
                FieldTy::HasOne(rel) => {
                    let ty = &rel.ty;

                    quote! {
                        #vis fn #field_ident(mut self, #field_ident: impl #toasty::IntoExpr<#ty>) -> Self {
                            self.query.#set_field_ident(#field_ident);
                            self
                        }

                        #vis fn #set_field_ident(&mut self, #field_ident: impl #toasty::IntoExpr<#ty>) -> &mut Self {
                            self.query.#set_field_ident(#field_ident);
                            self
                        }
                    }
                }
                FieldTy::Primitive(ty) => {
                    quote! {
                        #vis fn #field_ident(mut self, #field_ident: impl #toasty::IntoExpr<#ty>) -> Self {
                            self.query.#set_field_ident(#field_ident);
                            self
                        }

                        #vis fn #set_field_ident(&mut self, #field_ident: impl #toasty::IntoExpr<#ty>) -> &mut Self {
                            self.query.#set_field_ident(#field_ident);
                            self
                        }
                    }
                }
            }
        }).collect()
    }

    fn expand_update_query_methods(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;

        self.model.fields.iter().enumerate().map(|(offset, field)| {
            let index = util::int(offset);
            let field_ident = &field.name.ident;
            let set_field_ident = &field.set_ident;

            match &field.ty {
                FieldTy::BelongsTo(rel) => {
                    let ty = &rel.ty;

                    quote! {
                        #vis fn #field_ident(mut self, #field_ident: impl #toasty::IntoExpr<#ty>) -> Self {
                            self.#set_field_ident(#field_ident);
                            self
                        }

                        #vis fn #set_field_ident(&mut self, #field_ident: impl #toasty::IntoExpr<#ty>) -> &mut Self {
                            self.stmt.set(#index, #field_ident.into_expr());
                            self
                        }
                    }
                }
                FieldTy::HasMany(rel) => {
                    let ty = &rel.ty;
                    let singular = &rel.singular.ident;
                    let insert_ident = &rel.insert_ident;

                    quote! {
                        #vis fn #singular(mut self, #singular: impl #toasty::IntoExpr<#ty>) -> Self {
                            self.#insert_ident(#singular);
                            self
                        }

                        #vis fn #insert_ident(&mut self, #singular: impl #toasty::IntoExpr<#ty>) -> &mut Self {
                            self.stmt.insert(#index, #singular.into_expr());
                            self
                        }
                    }
                }
                FieldTy::HasOne(rel) => {
                    let ty = &rel.ty;

                    quote! {
                        #vis fn #field_ident(mut self, #field_ident: impl #toasty::IntoExpr<#ty>) -> Self {
                            self.#set_field_ident(#field_ident);
                            self
                        }

                        #vis fn #set_field_ident(&mut self, #field_ident: impl #toasty::IntoExpr<#ty>) -> &mut Self {
                            self.stmt.set(#index, #field_ident.into_expr());
                            self
                        }
                    }
                }
                FieldTy::Primitive(ty) => {
                    quote! {
                        #vis fn #field_ident(mut self, #field_ident: impl #toasty::IntoExpr<#ty>) -> Self {
                            self.#set_field_ident(#field_ident);
                            self
                        }

                        #vis fn #set_field_ident(&mut self, #field_ident: impl #toasty::IntoExpr<#ty>) -> &mut Self {
                            self.stmt.set(#index, #field_ident.into_expr());
                            self
                        }
                    }
                }
            }
        }).collect()
    }
}
