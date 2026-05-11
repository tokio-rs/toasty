use super::{Expand, util};
use crate::model::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

impl Expand<'_> {
    pub(super) fn expand_create_builder(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let model_span = model_ident.span();
        let create_struct_ident = &self.model.kind.as_root_unwrap().create_struct_ident;
        let create_methods = self.expand_create_methods();
        let default_stmts = self.expand_create_default_stmts();

        // Span the struct definition to the model ident so that "method not
        // found for this struct" errors point at `struct User`, not the derive
        // attribute.
        let struct_def = quote_spanned! { model_span=>
            #[derive(Clone)]
            #vis struct #create_struct_ident {
                stmt: #toasty::stmt::Insert<#model_ident>,
            }
        };

        quote! {
            #struct_def

            impl #create_struct_ident {
                #create_methods

                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#model_ident> {
                    executor.exec(self.stmt.into()).await
                }
            }

            impl #toasty::IntoInsert for #create_struct_ident {
                type Model = #model_ident;

                fn into_insert(self) -> #toasty::stmt::Insert<#model_ident> {
                    self.stmt
                }
            }

            impl #toasty::IntoStatement for #create_struct_ident {
                type Returning = #model_ident;

                fn into_statement(self) -> #toasty::Statement<#model_ident> {
                    self.stmt.into()
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

            impl #toasty::Assign<#model_ident> for #create_struct_ident {
                fn into_assignment(self) -> #toasty::stmt::Assignment<#model_ident> {
                    #toasty::stmt::set(
                        <Self as #toasty::IntoExpr<#model_ident>>::into_expr(self)
                    )
                }
            }

            impl #toasty::Assign<Option<#model_ident>> for #create_struct_ident {
                fn into_assignment(self) -> #toasty::stmt::Assignment<Option<#model_ident>> {
                    #toasty::stmt::set(
                        <Self as #toasty::IntoExpr<Option<#model_ident>>>::into_expr(self)
                    )
                }
            }

            impl Default for #create_struct_ident {
                fn default() -> #create_struct_ident {
                    let mut s = #create_struct_ident {
                        stmt: #toasty::stmt::Insert::blank_single(),
                    };
                    #default_stmts
                    s
                }
            }
        }
    }

    fn expand_create_default_stmts(&self) -> TokenStream {
        let toasty = &self.toasty;

        self.model
            .fields
            .iter()
            .enumerate()
            .filter_map(|(index, field)| {
                // #[default] takes priority over #[update] on create
                let expr = field
                    .attrs
                    .default_expr
                    .as_ref()
                    .or(field.attrs.update_expr.as_ref())?;
                let FieldTy::Primitive(ty) = &field.ty else {
                    return None;
                };
                let index_tokenized = util::int(index);
                // Mirror the setter logic: `Vec<scalar>` defaults bind
                // through `IntoExpr<List<T>>` so they line up with the
                // rest of the expression API.
                let target = match util::vec_scalar_inner(ty) {
                    Some(inner) => quote!(#toasty::List<#inner>),
                    None => quote!(#ty),
                };
                Some(quote! {
                    s.stmt.set(#index_tokenized, <#ty as #toasty::IntoExpr<#target>>::into_expr(#expr));
                })
            })
            .collect()
    }

    fn expand_create_methods(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
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
                            #vis fn #name(mut self, #name: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> Self {
                                // Silences unused field warning when the field is set on creation.
                                if false {
                                    let m = <#model_ident as #toasty::Load>::load(Default::default()).unwrap();
                                    let _ = &m.#name;
                                }

                                self.stmt.set(#index_tokenized, #name.into_expr());
                                self
                            }
                        }
                    }
                    FieldTy::HasMany(rel) => {
                        let singular = &rel.singular.ident;
                        let plural = name;
                        let ty = &rel.ty;

                        quote! {
                            #vis fn #singular(mut self, #singular: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> Self {
                                self.stmt.insert(#index_tokenized, #singular.into_expr());
                                self
                            }

                            #vis fn #plural(mut self, #plural: impl #toasty::IntoExpr<#toasty::List<<#ty as #toasty::Relation>::Model>>) -> Self {
                                self.stmt.insert_all(#index_tokenized, #plural.into_expr());
                                self
                            }
                        }
                    }
                    FieldTy::HasOne(rel) => {
                        let ty = &rel.ty;

                        quote! {
                            #vis fn #name(mut self, #name: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> Self {
                                self.stmt.set(#index_tokenized, #name.into_expr());
                                self
                            }
                        }
                    }
                    FieldTy::Primitive(ty) if field.attrs.serialize.is_some() => {
                        let serialize_attr = field.attrs.serialize.as_ref().unwrap();
                        if serialize_attr.nullable {
                            // For nullable serialized fields, extract the inner type from Option<T>
                            // Accept Option<InnerType>, serialize Some(v) as JSON, None as NULL
                            quote! {
                                #vis fn #name(mut self, #name: #ty) -> Self {
                                    match &#name {
                                        Some(v) => {
                                            let json = #toasty::serde_json::to_string(v).expect("failed to serialize");
                                            self.stmt.set(#index_tokenized, <String as #toasty::IntoExpr<String>>::into_expr(json));
                                        }
                                        None => {
                                            self.stmt.set(#index_tokenized, #toasty::stmt::Expr::<String>::from_untyped(#toasty::core::stmt::Expr::Value(#toasty::core::stmt::Value::Null)));
                                        }
                                    }
                                    self
                                }
                            }
                        } else {
                            // Non-nullable serialized field: accept T directly, serialize to JSON
                            quote! {
                                #vis fn #name(mut self, #name: #ty) -> Self {
                                    let json = #toasty::serde_json::to_string(&#name).expect("failed to serialize");
                                    self.stmt.set(#index_tokenized, <String as #toasty::IntoExpr<String>>::into_expr(json));
                                    self
                                }
                            }
                        }
                    }
                    FieldTy::Primitive(ty) if field.attrs.deferred => {
                        let inner = quote!(<#ty as #toasty::Defer>::Inner);
                        quote! {
                            #vis fn #name(mut self, #name: impl #toasty::IntoExpr<#inner>) -> Self {
                                self.stmt.set(#index_tokenized, #name.into_expr());
                                self
                            }
                        }
                    }
                    FieldTy::Primitive(ty) => {
                        // For `Vec<scalar>` model fields, drive the setter
                        // bound through the `List<T>` marker so it lines up
                        // with the rest of the expression API.
                        // `Vec<u8>` stays a scalar (bytes) — handled by
                        // `vec_scalar_inner` returning `None`.
                        let target = match util::vec_scalar_inner(ty) {
                            Some(inner) => quote!(#toasty::List<#inner>),
                            None => quote!(#ty),
                        };
                        quote! {
                            #vis fn #name(mut self, #name: impl #toasty::IntoExpr<#target>) -> Self {
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
