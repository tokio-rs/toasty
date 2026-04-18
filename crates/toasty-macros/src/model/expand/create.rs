use super::{Expand, util};
use crate::model::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use std::collections::HashSet;

impl Expand<'_> {
    /// Build the set of field indices referenced as `key = ...` by any
    /// `#[belongs_to]` attribute on this model.
    ///
    /// These fields are excluded from [`CreateMeta`] because they are set
    /// implicitly by the relation.
    fn fk_source_field_indices(&self) -> HashSet<usize> {
        let mut set = HashSet::new();
        for field in &self.model.fields {
            if let FieldTy::BelongsTo(rel) = &field.ty {
                for fk in &rel.foreign_key {
                    set.insert(fk.source);
                }
            }
        }
        set
    }

    /// Generate the `CREATE_META` const value for this model.
    ///
    /// Includes every primitive field that is not `#[auto]`, not
    /// `#[default(...)]`, not `#[update(...)]`, not `#[serialize]`, and not
    /// a foreign-key source for a `#[belongs_to]` relation on the same
    /// model. Relation fields are always excluded.
    ///
    /// Each included field's `required` flag is computed at compile time
    /// from `<T as Field>::NULLABLE`.
    pub(super) fn expand_create_meta_value(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_name = self.model.ident.to_string();
        let fk_sources = self.fk_source_field_indices();

        let fields = self
            .model
            .fields
            .iter()
            .enumerate()
            .filter_map(|(index, field)| {
                let FieldTy::Primitive(ty) = &field.ty else {
                    return None;
                };

                // Skip fields that Toasty fills in automatically or via FK
                // resolution.
                if field.attrs.auto.is_some()
                    || field.attrs.default_expr.is_some()
                    || field.attrs.update_expr.is_some()
                    || field.attrs.serialize.is_some()
                    || fk_sources.contains(&index)
                {
                    return None;
                }

                let name = field.name.ident.to_string();
                let missing_message =
                    format!("missing required field `{name}` in create! for `{model_name}`");
                Some(quote! {
                    #toasty::CreateField {
                        name: #name,
                        required: !<#ty as #toasty::Field>::NULLABLE,
                        missing_message: #missing_message,
                    }
                })
            });

        quote! {
            #toasty::CreateMeta {
                fields: &[ #( #fields ),* ],
                model_name: #model_name,
            }
        }
    }

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
