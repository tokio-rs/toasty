use super::{util, Expand};
use crate::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_update_builder(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_struct_ident = &self.model.kind.expect_root().query_struct_ident;
        let update_struct_ident = &self.model.kind.expect_root().update_struct_ident;
        let builder_methods = self.expand_builder_methods();
        let reload_model = self.expand_reload_model_expr();

        quote! {
            // Unified update builder generic over the target type
            #vis struct #update_struct_ident<T = #toasty::Query> {
                stmt: #toasty::stmt::Update<#model_ident>,
                target: T,
            }

            // Generic builder methods work for any target type
            impl<T: #toasty::ApplyUpdate> #update_struct_ident<T> {
                #builder_methods

                #vis async fn exec(self, db: &#toasty::Db) -> #toasty::Result<()> {
                    let stream = db.exec(self.stmt.into()).await?;
                    let values = stream.collect().await?;
                    self.target.apply_result(values)?;
                    Ok(())
                }
            }

            // Implement ApplyUpdate for &mut Model to enable reloading
            impl #toasty::ApplyUpdate for &mut #model_ident {
                fn apply_result(self, mut values: ::std::vec::Vec<#toasty::Value>) -> #toasty::Result<()> {
                    use #toasty::stmt::Primitive;

                    // Read the first value from the results
                    let value = values.into_iter()
                        .next()
                        .ok_or_else(|| #toasty::Error::record_not_found("update returned no results"))?;

                    // Reload model fields from the returned value
                    for (field, value) in value.into_sparse_record().into_iter() {
                        match field {
                            #reload_model
                            _ => todo!("handle unknown field id in reload after update"),
                        }
                    }

                    Ok(())
                }
            }

            // Convert from query to update builder
            impl From<#query_struct_ident> for #update_struct_ident {
                fn from(value: #query_struct_ident) -> #update_struct_ident {
                    #update_struct_ident {
                        stmt: #toasty::stmt::Update::new(value.stmt),
                        target: #toasty::Query,
                    }
                }
            }

            impl From<#toasty::stmt::Select<#model_ident>> for #update_struct_ident {
                fn from(src: #toasty::stmt::Select<#model_ident>) -> #update_struct_ident {
                    #update_struct_ident {
                        stmt: #toasty::stmt::Update::new(src),
                        target: #toasty::Query,
                    }
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
                    quote!(#i => self.#field_ident = <#ty as #toasty::stmt::Primitive>::load(value)?,)
                }
                _ => {
                    // TODO: Actually implement this
                    quote!(#i => self.#field_ident.unload(),)
                }
            }

        }).collect()
    }

    fn expand_builder_methods(&self) -> TokenStream {
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
                        #vis fn #field_ident(mut self, #field_ident: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> Self {
                            self.#set_field_ident(#field_ident);
                            self
                        }

                        #vis fn #set_field_ident(&mut self, #field_ident: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> &mut Self {
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
                        #vis fn #singular(mut self, #singular: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> Self {
                            self.#insert_ident(#singular);
                            self
                        }

                        #vis fn #insert_ident(&mut self, #singular: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> &mut Self {
                            self.stmt.insert(#index, #singular.into_expr());
                            self
                        }
                    }
                }
                FieldTy::HasOne(rel) => {
                    let ty = &rel.ty;

                    quote! {
                        #vis fn #field_ident(mut self, #field_ident: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> Self {
                            self.#set_field_ident(#field_ident);
                            self
                        }

                        #vis fn #set_field_ident(&mut self, #field_ident: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> &mut Self {
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
