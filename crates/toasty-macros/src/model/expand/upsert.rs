use super::{Expand, util};
use crate::model::schema::FieldTy;
use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

impl Expand<'_> {
    pub(super) fn expand_upsert_builders(&self) -> TokenStream {
        self.model
            .indices
            .iter()
            .filter(|index| index.unique)
            .filter_map(|index| {
                if index.fields.iter().any(|field| {
                    !matches!(self.model.fields[field.field].ty, FieldTy::Primitive(_))
                }) {
                    return None;
                }

                Some(
                    self.expand_upsert_builder(
                        &index
                            .fields
                            .iter()
                            .map(|field| field.field)
                            .collect::<Vec<_>>(),
                    ),
                )
            })
            .collect()
    }

    fn expand_upsert_builder(&self, target_fields: &[usize]) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let suffix = target_fields
            .iter()
            .map(|&field| {
                self.model.fields[field]
                    .name
                    .ident
                    .to_string()
                    .to_upper_camel_case()
            })
            .collect::<Vec<_>>()
            .join("And");
        let method_name = format_ident!(
            "upsert_by_{}",
            target_fields
                .iter()
                .map(|&field| self.model.fields[field].name.ident.to_string())
                .collect::<Vec<_>>()
                .join("_and_")
        );
        let target_name = target_fields
            .iter()
            .map(|&field| self.model.fields[field].name.ident.to_string())
            .collect::<Vec<_>>()
            .join("_and_");
        let method_doc = format!(
            "Returns a builder that creates `{model_ident}` or updates the record whose `{target_name}` constraint matches.\n\n\
             The target arguments initialize a new record and remain unchanged when an existing record is updated."
        );
        let builder_doc = format!(
            "Builds one atomic create-or-update operation for `{model_ident}` using the `{target_name}` conflict target."
        );
        let create_doc =
            format!("Sets values used only when the `{model_ident}` record is created.");
        let update_doc = format!(
            "Assigns fields only when the `{target_name}` target matches an existing `{model_ident}` record."
        );
        let incoming_doc = format!(
            "References values proposed for the new `{model_ident}` record from `on_update`."
        );
        let ignore_doc = format!(
            "Builds an insert-or-ignore operation for `{model_ident}` using the `{target_name}` conflict target."
        );
        let builder_ident = format_ident!("{}UpsertBy{}", model_ident, suffix);
        let create_ident = format_ident!("{}UpsertBy{}Create", model_ident, suffix);
        let update_ident = format_ident!("{}UpsertBy{}Update", model_ident, suffix);
        let incoming_ident = format_ident!("{}UpsertBy{}Incoming", model_ident, suffix);
        let ignore_ident = format_ident!("{}UpsertBy{}OrIgnore", model_ident, suffix);

        let target_args = target_fields.iter().map(|&field_index| {
            let field = &self.model.fields[field_index];
            let name = &field.name.ident;
            let FieldTy::Primitive(ty) = &field.ty else {
                unreachable!()
            };
            quote!(#name: impl IntoExpr<FieldExprTarget<#ty>>)
        });
        let target_indices = target_fields.iter().map(|&index| util::int(index));
        let target_sets = target_fields.iter().map(|&field_index| {
            let field = &self.model.fields[field_index];
            let name = &field.name.ident;
            let FieldTy::Primitive(ty) = &field.ty else {
                unreachable!()
            };
            let index = util::int(field_index);
            quote! {
                stmt.untyped_mut()
                    .source
                    .body
                    .as_values_mut_unwrap()
                    .rows
                    .last_mut()
                    .unwrap()
                    .as_record_mut_unwrap()
                    .fields[#index] =
                        #toasty::into_untyped_expr::<<#ty as #toasty::Field>::ExprTarget, _>(#name);
            }
        });

        let defaults = self
            .model
            .fields
            .iter()
            .enumerate()
            .filter_map(|(field_index, field)| {
                if target_fields.contains(&field_index) {
                    return None;
                }
                let FieldTy::Primitive(ty) = &field.ty else {
                    return None;
                };
                let index = util::int(field_index);
                let default = field.attrs.default_expr.as_ref()?;
                Some(quote! {
                    upsert.defaults.set(
                        [#index],
                        #toasty::into_untyped_expr::<<#ty as #toasty::Field>::ExprTarget, _>(#default),
                    );
                })
            });
        let update_defaults = self
            .model
            .fields
            .iter()
            .enumerate()
            .filter_map(|(field_index, field)| {
                if target_fields.contains(&field_index) {
                    return None;
                }
                let FieldTy::Primitive(ty) = &field.ty else {
                    return None;
                };
                let index = util::int(field_index);
                let update = field.attrs.update_expr.as_ref()?;
                Some(quote! {
                    upsert.update_defaults.set(
                        [#index],
                        #toasty::into_untyped_expr::<<#ty as #toasty::Field>::ExprTarget, _>(#update),
                    );
                })
            });

        let shared_methods = self.expand_upsert_shared_methods(target_fields);
        let create_methods = self.expand_upsert_create_methods(target_fields);
        let update_methods = self.expand_upsert_update_methods(target_fields);
        let incoming_methods = self.expand_upsert_incoming_methods();

        quote! {
            impl #model_ident {
                #[doc = #method_doc]
                #vis fn #method_name(#(#target_args),*) -> #builder_ident {
                    let mut stmt = #toasty::stmt::Upsert::<#model_ident>::blank([#(#target_indices),*]);
                    #(#target_sets)*
                    {
                        let upsert = stmt.untyped_mut().upsert.as_mut().unwrap();
                        #(#defaults)*
                        #(#update_defaults)*
                    }
                    #builder_ident { stmt }
                }
            }

            #[derive(Clone)]
            #[doc = #builder_doc]
            #vis struct #builder_ident {
                stmt: #toasty::stmt::Upsert<#model_ident>,
            }

            impl #builder_ident {
                #shared_methods

                #[doc = "Adds assignments used only when the record is created.\n\nBackend support depends on whether the assignment can be applied atomically without changing an existing record."]
                #vis fn on_create(
                    mut self,
                    f: impl for<'a> FnOnce(#create_ident<'a>) -> #create_ident<'a>,
                ) -> Self {
                    let branch = #create_ident { stmt: &mut self.stmt };
                    let _ = f(branch);
                    self
                }

                #[doc = "Adds assignments used only when the conflict target matches an existing record.\n\nThe closure accepts the same assignment operators as a normal update. Database drivers that do not support branch-specific upsert assignments return `unsupported_feature` at execution."]
                #vis fn on_update(
                    mut self,
                    f: impl for<'a> FnOnce(#update_ident<'a>) -> #update_ident<'a>,
                ) -> Self {
                    let branch = #update_ident { stmt: &mut self.stmt };
                    let _ = f(branch);
                    self
                }

                #[doc = "Leaves a record unchanged when the selected conflict target matches.\n\nThe returned builder's `exec` method produces `Some(model)` after an insert and `None` after a conflict."]
                #vis fn or_ignore(mut self) -> #ignore_ident {
                    self.stmt.untyped_mut().upsert.as_mut().unwrap().action =
                        #toasty::core::stmt::UpsertAction::Ignore;
                    #ignore_ident { stmt: self.stmt }
                }

                #[doc = "Executes the upsert and returns the record stored by the database."]
                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#model_ident> {
                    executor.exec(self.stmt.into()).await
                }
            }

            #[doc = #create_doc]
            #vis struct #create_ident<'a> {
                stmt: &'a mut #toasty::stmt::Upsert<#model_ident>,
            }

            impl<'a> #create_ident<'a> {
                #create_methods
            }

            #[doc = #update_doc]
            #vis struct #update_ident<'a> {
                stmt: &'a mut #toasty::stmt::Upsert<#model_ident>,
            }

            impl<'a> #update_ident<'a> {
                #[doc = "Returns field expressions for values proposed by the create branch."]
                #vis fn incoming(&self) -> #incoming_ident {
                    #incoming_ident
                }

                #update_methods
            }

            #[derive(Clone, Copy)]
            #[doc = #incoming_doc]
            #vis struct #incoming_ident;

            impl #incoming_ident {
                #incoming_methods
            }

            #[doc = #ignore_doc]
            #vis struct #ignore_ident {
                stmt: #toasty::stmt::Upsert<#model_ident>,
            }

            impl #ignore_ident {
                #[doc = "Executes the insert-or-ignore operation.\n\nReturns `Some(model)` when the record is inserted and `None` when the selected target conflicts."]
                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<Option<#model_ident>> {
                    let stmt = #toasty::Statement::<Option<#model_ident>>::from_untyped_stmt(
                        self.stmt.into_untyped(),
                    );
                    executor.exec(stmt).await
                }
            }
        }
    }

    fn expand_upsert_shared_methods(&self, target_fields: &[usize]) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        self.model
            .fields
            .iter()
            .enumerate()
            .filter_map(|(field_index, field)| {
                if target_fields.contains(&field_index) {
                    return None;
                }
                let FieldTy::Primitive(ty) = &field.ty else {
                    return None;
                };
                let name = &field.name.ident;
                let index = util::int(field_index);
                let doc = format!("Assigns `{name}` on both the create and update branches.");
                Some(quote! {
                    #[doc = #doc]
                    #vis fn #name(mut self, #name: impl Assign<FieldExprTarget<#ty>>) -> Self {
                        let upsert = self.stmt.untyped_mut().upsert.as_mut().unwrap();
                        #name.assign(
                            &mut upsert.shared,
                            #toasty::stmt::Projection::from_index(#index),
                        );
                        self
                    }
                })
            })
            .collect()
    }

    fn expand_upsert_create_methods(&self, target_fields: &[usize]) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        self.model.fields.iter().enumerate().filter_map(|(field_index, field)| {
            if target_fields.contains(&field_index) { return None }
            let FieldTy::Primitive(ty) = &field.ty else { return None };
            let name = &field.name.ident;
            let index = util::int(field_index);
            let doc = format!("Sets `{name}` only when the record is created.");
            Some(quote! {
                #[doc = #doc]
                #vis fn #name(mut self, #name: impl IntoExpr<FieldExprTarget<#ty>>) -> Self {
                    self.stmt
                        .untyped_mut()
                        .upsert
                        .as_mut()
                        .unwrap()
                        .create
                        .set(
                            #toasty::stmt::Projection::from_index(#index),
                            #toasty::into_untyped_expr::<<#ty as #toasty::Field>::ExprTarget, _>(#name),
                        );
                    self
                }
            })
        }).collect()
    }

    fn expand_upsert_update_methods(&self, target_fields: &[usize]) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        self.model
            .fields
            .iter()
            .enumerate()
            .filter_map(|(field_index, field)| {
                if target_fields.contains(&field_index) {
                    return None;
                }
                let FieldTy::Primitive(ty) = &field.ty else {
                    return None;
                };
                let name = &field.name.ident;
                let index = util::int(field_index);
                let doc = format!(
                    "Assigns `{name}` only when the conflict target matches an existing record."
                );
                Some(quote! {
                    #[doc = #doc]
                    #vis fn #name(mut self, #name: impl Assign<FieldExprTarget<#ty>>) -> Self {
                        let upsert = self.stmt.untyped_mut().upsert.as_mut().unwrap();
                        #name.assign(
                            &mut upsert.update,
                            #toasty::stmt::Projection::from_index(#index),
                        );
                        self
                    }
                })
            })
            .collect()
    }

    fn expand_upsert_incoming_methods(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        self.model
            .fields
            .iter()
            .enumerate()
            .filter_map(|(field_index, field)| {
                let FieldTy::Primitive(ty) = &field.ty else {
                    return None;
                };
                let name = &field.name.ident;
                let index = util::int(field_index);
                let doc = format!("Returns an expression referencing the proposed `{name}` value.");
                Some(quote! {
                    #[doc = #doc]
                    #vis fn #name(self) -> #toasty::stmt::Expr<FieldExprTarget<#ty>> {
                        #toasty::stmt::Expr::from_untyped(
                            #toasty::core::stmt::Expr::project(
                                #toasty::core::stmt::ExprIncoming::model(
                                    <#model_ident as #toasty::Model>::id(),
                                ),
                                #toasty::stmt::Projection::from_index(#index),
                            ),
                        )
                    }
                })
            })
            .collect()
    }
}
