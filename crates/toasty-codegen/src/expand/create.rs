use super::{util, Expand};
use crate::schema::FieldTy;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

impl Expand<'_> {
    pub(super) fn expand_create_builder(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let create_struct_ident = &self.model.kind.expect_root().create_struct_ident;
        let create_methods = self.expand_create_methods();
        let default_stmts = self.expand_create_default_stmts();

        quote! {
            #vis struct #create_struct_ident {
                stmt: #toasty::stmt::Insert<#model_ident>,
            }

            impl #create_struct_ident {
                #create_methods

                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#model_ident> {
                    use #toasty::ExecutorExt;
                    executor.exec_insert_one(self.stmt).await
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

            impl #toasty::IntoExpr<#toasty::List<#model_ident>> for #create_struct_ident {
                fn into_expr(self) -> #toasty::stmt::Expr<#toasty::List<#model_ident>> {
                    self.stmt.into_list_expr()
                }

                fn by_ref(&self) -> #toasty::stmt::Expr<#toasty::List<#model_ident>> {
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
                            #vis fn #with_ident(mut self, f: impl ::std::ops::FnOnce(#toasty::CreateMany<#rel_model>) -> #toasty::CreateMany<#rel_model>) -> Self {
                                let many = f(#toasty::CreateMany::new());
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
                                            self.stmt.set(#index_tokenized, #toasty::stmt::Expr::<String>::from_untyped(#toasty::core::stmt::Expr::Value(#toasty::Value::Null)));
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

    /// Generate the compile-time create verification infrastructure:
    ///
    /// - One trait per required field with `#[diagnostic::on_unimplemented]`
    /// - A ZST verifier struct with typestate type parameters
    /// - Field methods that transition required fields from `NotSet` to `Set`
    /// - A `check()` method gated on all required-field traits being satisfied
    /// - A `__verify_create()` constructor on the model
    pub(super) fn expand_create_verifier(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let model_name = model_ident.to_string();

        // Collect FK source field indices — these are implicitly set when
        // the corresponding BelongsTo relation is set, so they should not be
        // required in the verifier.
        let fk_source_ids: std::collections::HashSet<usize> = self
            .model
            .fields
            .iter()
            .filter_map(|f| match &f.ty {
                FieldTy::BelongsTo(rel) => Some(rel.foreign_key.iter().map(|fk| fk.source)),
                _ => None,
            })
            .flatten()
            .collect();

        // Collect required fields, excluding FK source fields
        let required_fields: Vec<_> = self
            .model
            .fields
            .iter()
            .filter(|f| f.is_required_on_create() && !fk_source_ids.contains(&f.id))
            .collect();

        // Names for generated items
        let verify_struct =
            format_ident!("__{}CreateVerify", model_ident, span = Span::mixed_site());

        // --- Per-required-field: trait + impl Set ---
        let trait_defs: Vec<_> = required_fields
            .iter()
            .map(|field| {
                let field_name = field.name.ident.to_string();
                let trait_name = format_ident!(
                    "__{}_create_has_{}",
                    model_name.to_lowercase(),
                    field_name,
                    span = Span::mixed_site()
                );
                let msg = format!("missing field `{field_name}` in create! for `{model_name}`");
                let label = format!("missing `{field_name}`");

                quote! {
                    #[doc(hidden)]
                    #[diagnostic::on_unimplemented(
                        message = #msg,
                        label = #label
                    )]
                    #vis trait #trait_name {}
                    #[diagnostic::do_not_recommend]
                    impl #trait_name for #toasty::Set {}
                }
            })
            .collect();

        // --- Type parameter idents for the verifier struct ---
        let type_params: Vec<_> = required_fields
            .iter()
            .map(|field| {
                let name = field.name.ident.to_string();
                let mut upper = String::with_capacity(name.len() + 2);
                upper.push_str("__");
                let mut chars = name.chars();
                if let Some(c) = chars.next() {
                    upper.extend(c.to_uppercase());
                }
                upper.extend(chars);
                format_ident!("{}", upper, span = Span::mixed_site())
            })
            .collect();

        // Trait names (same order as type_params)
        let trait_names: Vec<_> = required_fields
            .iter()
            .map(|field| {
                format_ident!(
                    "__{}_create_has_{}",
                    model_name.to_lowercase(),
                    field.name.ident,
                    span = Span::mixed_site()
                )
            })
            .collect();

        // --- Struct definition with defaults ---
        let default_params: Vec<_> = type_params
            .iter()
            .map(|p| quote! { #p = #toasty::NotSet })
            .collect();

        let struct_def = quote! {
            #[doc(hidden)]
            #vis struct #verify_struct < #( #default_params ),* >(
                ::std::marker::PhantomData<( #( #type_params ),* )>,
            );
        };

        // --- new() ---
        let new_impl = quote! {
            impl #verify_struct {
                #vis fn new() -> Self {
                    #verify_struct(::std::marker::PhantomData)
                }
            }
        };

        // --- Field methods ---
        let field_methods: Vec<_> = self
            .model
            .fields
            .iter()
            .map(|field| {
                let method_name = &field.name.ident;
                let with_method = &field.with_ident;

                if let Some(req_idx) = required_fields.iter().position(|f| f.id == field.id) {
                    // Required field: transition the corresponding type param to Set
                    let mut result_params: Vec<TokenStream> =
                        type_params.iter().map(|p| quote! { #p }).collect();
                    result_params[req_idx] = quote! { #toasty::Set };

                    let is_relation = field.ty.is_relation();
                    let with_variant = if is_relation {
                        // Relations also need with_ variant for nested struct syntax
                        quote! {
                            #vis fn #with_method(self) -> #verify_struct< #( #result_params ),* > {
                                #verify_struct(::std::marker::PhantomData)
                            }
                        }
                    } else {
                        quote! {}
                    };

                    quote! {
                        #vis fn #method_name(self) -> #verify_struct< #( #result_params ),* > {
                            #verify_struct(::std::marker::PhantomData)
                        }
                        #with_variant
                    }
                } else {
                    // Optional / auto / relation field: identity (no type transition)
                    let is_relation = field.ty.is_relation();
                    if is_relation {
                        quote! {
                            #vis fn #method_name(self) -> Self { self }
                            #vis fn #with_method(self) -> Self { self }
                        }
                    } else {
                        quote! {
                            #vis fn #method_name(self) -> Self { self }
                        }
                    }
                }
            })
            .collect();

        let methods_impl = quote! {
            impl< #( #type_params ),* > #verify_struct< #( #type_params ),* > {
                #( #field_methods )*
            }
        };

        // --- Trait bounds for required fields ---
        let where_clauses: Vec<_> = type_params
            .iter()
            .zip(trait_names.iter())
            .map(|(param, trait_name)| quote! { #param: #trait_name })
            .collect();

        // --- __verify_create() and __check_create() on the model ---
        //
        // We use a free function (__check_create) whose type parameters carry
        // the trait bounds instead of a bounded method on the verifier struct.
        // This makes the compiler emit E0277 (trait bound not satisfied) rather
        // than E0599 (method exists but bounds not satisfied), which is the
        // error code that `#[diagnostic::on_unimplemented]` customizes.
        let model_method = quote! {
            impl #model_ident {
                #[doc(hidden)]
                #vis fn __verify_create() -> #verify_struct {
                    #verify_struct::new()
                }

                #[doc(hidden)]
                #vis fn __check_create< #( #type_params ),* >(_: #verify_struct< #( #type_params ),* >)
                where
                    #( #where_clauses ),*
                {}
            }
        };

        quote! {
            #( #trait_defs )*
            #struct_def
            #new_impl
            #methods_impl
            #model_method
        }
    }
}
