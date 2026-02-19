use super::{util, Expand};
use crate::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    /// Generate update builder for embedded structs
    pub(super) fn expand_embedded_update_builder(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let update_struct_ident = &self.model.kind.expect_embedded().update_struct_ident;
        let builder_methods = self.expand_update_field_methods(true);

        quote! {
            #vis struct #update_struct_ident<'a> {
                stmt: &'a mut #toasty::core::stmt::Update,
                projection: #toasty::stmt::Projection,
            }

            impl<'a> #update_struct_ident<'a> {
                #builder_methods
            }
        }
    }

    /// Expand all update methods for all fields.
    /// Generates both the field setter methods and the .with_field() method for each field.
    /// For embedded builders: uses self.projection.clone().push(index) and self.stmt.assignments
    /// For root builders: uses Projection::from_index(index) and self.stmt
    fn expand_update_field_methods(&self, is_embedded: bool) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;

        let stmt_method = if is_embedded {
            quote!(self.stmt.assignments)
        } else {
            quote!(self.stmt)
        };

        let stmt_for_builder = if is_embedded {
            quote!(self.stmt)
        } else {
            quote!(self.stmt.as_untyped_mut())
        };

        self.model.fields.iter().enumerate().map(|(field_index, field)| {
            let field_ident = &field.name.ident;
            let set_field_ident = &field.set_ident;
            let with_field_ident = &field.with_ident;

            let index = util::int(field_index);
            let projection = if is_embedded {
                quote! {{
                    let mut projection = self.projection.clone();
                    projection.push(#index);
                    projection
                }}
            } else {
                quote! {
                    #toasty::stmt::Projection::from_index(#index)
                }
            };

            match &field.ty {
            FieldTy::BelongsTo(rel) => {
                let ty = &rel.ty;

                quote! {
                    #vis fn #field_ident(mut self, #field_ident: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> Self {
                        self.#set_field_ident(#field_ident);
                        self
                    }

                    #vis fn #set_field_ident(&mut self, #field_ident: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> &mut Self {
                        let projection = #projection;
                        #stmt_method.set(projection, #field_ident.into_expr());
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
                        let projection = #projection;
                        #stmt_method.insert(projection, #singular.into_expr());
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
                        let projection = #projection;
                        #stmt_method.set(projection, #field_ident.into_expr());
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
                        let projection = #projection;
                        #stmt_method.set(projection, #field_ident.into_expr());
                        self
                    }

                    #vis fn #with_field_ident(
                        mut self,
                        f: impl FnOnce(<#ty as #toasty::stmt::Primitive>::UpdateBuilder<'_>)
                    ) -> Self {
                        let projection = #projection;
                        let builder = <#ty as #toasty::stmt::Primitive>::make_update_builder(#stmt_for_builder, projection);
                        f(builder);
                        self
                    }
                }
            }
            }
        }).collect()
    }

    pub(super) fn expand_update_builder(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_struct_ident = &self.model.kind.expect_root().query_struct_ident;
        let update_struct_ident = &self.model.kind.expect_root().update_struct_ident;
        let target_ty = util::ident("T");
        let builder_methods = self.expand_update_field_methods(false);

        quote! {
            // Unified update builder generic over the target type
            #vis struct #update_struct_ident<#target_ty = #toasty::Query> {
                stmt: #toasty::stmt::Update<#model_ident>,
                target: #target_ty,
            }

            // Generic builder methods work for any target type
            impl<#target_ty: #toasty::ApplyUpdate> #update_struct_ident<#target_ty> {
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
                    let value = values.into_iter()
                        .next()
                        .ok_or_else(|| #toasty::Error::record_not_found("update returned no results"))?;
                    self.reload(value)
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

    /// Generate match arms for reloading each model field from a sparse record value.
    fn expand_reload_match_arms(&self) -> TokenStream {
        let toasty = &self.toasty;

        self.model.fields.iter().enumerate().map(|(offset, field)| {
            let i = util::int(offset);
            let field_ident = &field.name.ident;

            match &field.ty {
                FieldTy::Primitive(ty) => {
                    quote!(#i => <#ty as #toasty::stmt::Primitive>::reload(&mut self.#field_ident, value)?,)
                }
                _ => {
                    // Relation fields (BelongsTo, HasMany, HasOne) are unloaded on update.
                    // Embedded fields are handled above via the Primitive arm.
                    quote!(#i => self.#field_ident.unload(),)
                }
            }

        }).collect()
    }

    /// Generate the body of the `reload` method for a root model.
    pub(super) fn expand_reload_method(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let reload_arms = self.expand_reload_match_arms();

        quote! {
            #vis fn reload(&mut self, value: #toasty::Value) -> #toasty::Result<()> {
                use #toasty::stmt::Primitive;
                for (field, value) in value.into_sparse_record().into_iter() {
                    match field {
                        #reload_arms
                        _ => todo!("handle unknown field id in reload after update"),
                    }
                }
                Ok(())
            }
        }
    }
}
