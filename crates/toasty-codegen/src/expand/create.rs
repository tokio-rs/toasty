use super::{util, Expand};
use crate::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_create_builder(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let create_struct_ident = &self.model.kind.as_root_unwrap().create_struct_ident;
        let create_methods = self.expand_create_methods();
        let default_stmts = self.expand_create_default_stmts();

        quote! {
            #vis struct #create_struct_ident {
                stmt: #toasty::stmt::Insert<#model_ident>,
            }

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
                Some(quote! {
                    s.stmt.set(#index_tokenized, <#ty as #toasty::IntoExpr<#ty>>::into_expr(#expr));
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
                let with_ident = &field.with_ident;
                let index_tokenized = util::int(index);

                match &field.ty {
                    FieldTy::BelongsTo(rel) => {
                        let ty = &rel.ty;
                        let rel_create = quote!(<#ty as #toasty::Relation>::Create);

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

                            /// Closure-based variant: build the related model inline using its create builder.
                            #vis fn #with_ident(mut self, f: impl ::std::ops::FnOnce(#rel_create) -> #rel_create) -> Self {
                                let create = f(::std::default::Default::default());
                                let expr: #toasty::stmt::Expr<<#ty as #toasty::Relation>::Model> = #toasty::IntoExpr::into_expr(create);
                                self.stmt.set(#index_tokenized, expr);
                                self
                            }
                        }
                    }
                    FieldTy::HasMany(rel) => {
                        let singular = &rel.singular.ident;
                        let plural = name;
                        let ty = &rel.ty;
                        let rel_model = quote!(<#ty as #toasty::Relation>::Model);

                        quote! {
                            #vis fn #singular(mut self, #singular: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> Self {
                                self.stmt.insert(#index_tokenized, #singular.into_expr());
                                self
                            }

                            #vis fn #plural(mut self, #plural: impl #toasty::IntoExpr<#toasty::List<<#ty as #toasty::Relation>::Model>>) -> Self {
                                self.stmt.insert_all(#index_tokenized, #plural.into_expr());
                                self
                            }

                            /// Closure-based variant: build a collection of nested items using `CreateMany::with_item`.
                            #vis fn #with_ident(mut self, f: impl ::std::ops::FnOnce(#toasty::stmt::CreateMany<#rel_model>) -> #toasty::stmt::CreateMany<#rel_model>) -> Self {
                                let many = f(#toasty::stmt::CreateMany::new());
                                self.stmt.insert_all(#index_tokenized, many.into_expr());
                                self
                            }
                        }
                    }
                    FieldTy::HasOne(rel) => {
                        let ty = &rel.ty;
                        let rel_create = quote!(<#ty as #toasty::Relation>::Create);

                        quote! {
                            #vis fn #name(mut self, #name: impl #toasty::IntoExpr<<#ty as #toasty::Relation>::Expr>) -> Self {
                                self.stmt.set(#index_tokenized, #name.into_expr());
                                self
                            }

                            /// Closure-based variant: build the related model inline using its create builder.
                            #vis fn #with_ident(mut self, f: impl ::std::ops::FnOnce(#rel_create) -> #rel_create) -> Self {
                                let create = f(::std::default::Default::default());
                                let expr: #toasty::stmt::Expr<<#ty as #toasty::Relation>::Model> = #toasty::IntoExpr::into_expr(create);
                                self.stmt.set(#index_tokenized, expr);
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
                    FieldTy::Primitive(ty) => {
                        quote! {
                            #vis fn #name(mut self, #name: impl #toasty::IntoExpr<#ty>) -> Self {
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
