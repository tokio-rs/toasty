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
            FieldTy::Primitive(ty) if field.attrs.serialize.is_some() => {
                let serialize_attr = field.attrs.serialize.as_ref().unwrap();
                if serialize_attr.nullable {
                    quote! {
                        #vis fn #field_ident(mut self, #field_ident: #ty) -> Self {
                            self.#set_field_ident(#field_ident);
                            self
                        }

                        #vis fn #set_field_ident(&mut self, #field_ident: #ty) -> &mut Self {
                            let projection = #projection;
                            match &#field_ident {
                                Some(v) => {
                                    let json = #toasty::serde_json::to_string(v).expect("failed to serialize");
                                    #stmt_method.set(projection, <String as #toasty::IntoExpr<String>>::into_expr(json));
                                }
                                None => {
                                    #stmt_method.set(projection, #toasty::stmt::Expr::<String>::from_untyped(#toasty::core::stmt::Expr::Value(#toasty::core::stmt::Value::Null)));
                                }
                            }
                            self
                        }
                    }
                } else {
                    quote! {
                        #vis fn #field_ident(mut self, #field_ident: #ty) -> Self {
                            self.#set_field_ident(#field_ident);
                            self
                        }

                        #vis fn #set_field_ident(&mut self, #field_ident: #ty) -> &mut Self {
                            let projection = #projection;
                            let json = #toasty::serde_json::to_string(&#field_ident).expect("failed to serialize");
                            #stmt_method.set(projection, <String as #toasty::IntoExpr<String>>::into_expr(json));
                            self
                        }
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
                        f: impl FnOnce(<#ty as #toasty::Field>::UpdateBuilder<'_>)
                    ) -> Self {
                        let projection = #projection;
                        let builder = <#ty as #toasty::Field>::make_update_builder(#stmt_for_builder, projection);
                        f(builder);
                        self
                    }
                }
            }
            }
        }).collect()
    }

    fn expand_update_default_stmts(&self) -> TokenStream {
        let toasty = &self.toasty;

        self.model
            .fields
            .iter()
            .enumerate()
            .filter_map(|(index, field)| {
                let expr = field.attrs.update_expr.as_ref()?;
                let FieldTy::Primitive(ty) = &field.ty else {
                    return None;
                };
                let index_tokenized = util::int(index);
                Some(quote! {
                    self.stmt.set(
                        #toasty::stmt::Projection::from_index(#index_tokenized),
                        <#ty as #toasty::IntoExpr<#ty>>::into_expr(#expr),
                    );
                })
            })
            .collect()
    }

    pub(super) fn expand_update_builder(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_struct_ident = &self.model.kind.expect_root().query_struct_ident;
        let update_struct_ident = &self.model.kind.expect_root().update_struct_ident;
        let target_ty = util::ident("T");
        let builder_methods = self.expand_update_field_methods(false);
        let update_default_stmts = self.expand_update_default_stmts();

        quote! {
            // Unified update builder generic over the update target.
            //
            // The target's `Returning` associated type determines the statement
            // return type:
            // - `T = Query<Model>`: query-based update, `Returning = List<Model>`
            // - `T = &mut Model`: instance update, `Returning = Model`
            #vis struct #update_struct_ident<#target_ty: #toasty::UpdateTarget = #toasty::Query<#model_ident>> {
                stmt: #toasty::stmt::Update<<#target_ty as #toasty::UpdateTarget>::Returning>,
                target: #target_ty,
            }

            impl<#target_ty: #toasty::UpdateTarget> #update_struct_ident<#target_ty> {
                fn apply_update_defaults(&mut self) {
                    #update_default_stmts
                }

                #builder_methods

                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<()> {
                    use #toasty::ExecutorExt;
                    let stream = executor.exec(self.stmt.into()).await?;
                    let values = stream.collect().await?;
                    self.target.apply_result(values)?;
                    Ok(())
                }
            }

            // Implement UpdateTarget for &mut Model to enable reloading
            impl #toasty::UpdateTarget for &mut #model_ident {
                type Returning = #model_ident;

                fn apply_result(self, mut values: ::std::vec::Vec<#toasty::core::stmt::Value>) -> #toasty::Result<()> {
                    let value = values.into_iter()
                        .next()
                        .ok_or_else(|| #toasty::Error::record_not_found("update returned no results"))?;
                    self.reload(value)
                }
            }

            // Convert from query to update builder (list return type)
            impl From<#query_struct_ident> for #update_struct_ident {
                fn from(value: #query_struct_ident) -> #update_struct_ident {
                    let mut s = #update_struct_ident {
                        stmt: #toasty::stmt::Update::new(value.stmt),
                        target: #toasty::Query::new(),
                    };
                    s.apply_update_defaults();
                    s
                }
            }

            impl From<#toasty::stmt::Query<#model_ident>> for #update_struct_ident {
                fn from(src: #toasty::stmt::Query<#model_ident>) -> #update_struct_ident {
                    let mut s = #update_struct_ident {
                        stmt: #toasty::stmt::Update::new(src),
                        target: #toasty::Query::new(),
                    };
                    s.apply_update_defaults();
                    s
                }
            }

            impl #toasty::IntoStatement for #update_struct_ident {
                type Returning = ();

                fn into_statement(self) -> #toasty::Statement<()> {
                    #toasty::Statement::from_untyped_stmt(self.stmt.into_untyped_stmt())
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
            let field_name_str = field.name.ident.to_string();

            match &field.ty {
                FieldTy::Primitive(_ty) if field.attrs.serialize.is_some() => {
                    let serialize_attr = field.attrs.serialize.as_ref().unwrap();

                    let json_deserialize = quote! {
                        let json_str = <String as #toasty::Field>::load(value)?;
                        #toasty::serde_json::from_str(&json_str)
                            .map_err(|e| #toasty::Error::from_args(
                                format_args!("failed to deserialize field '{}': {}", #field_name_str, e)
                            ))?
                    };

                    let assign = if serialize_attr.nullable {
                        quote! {
                            if value.is_null() { None } else { Some({ #json_deserialize }) }
                        }
                    } else {
                        quote! { { #json_deserialize } }
                    };

                    quote! {
                        #i => {
                            self.#field_ident = #assign;
                        }
                    }
                }
                FieldTy::Primitive(ty) => {
                    quote!(#i => <#ty as #toasty::Field>::reload(&mut self.#field_ident, value)?,)
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
            #vis fn reload(&mut self, value: #toasty::core::stmt::Value) -> #toasty::Result<()> {
                use #toasty::Field;
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
