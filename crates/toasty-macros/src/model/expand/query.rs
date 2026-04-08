use super::{Expand, docs, util};
use crate::model::schema::{BelongsTo, Field, FieldTy, HasMany, HasOne};

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_query_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_struct_ident = &self.model.kind.as_root_unwrap().query_struct_ident;
        let update_struct_ident = &self.model.kind.as_root_unwrap().update_struct_ident;
        let include_ty = util::ident("T");
        let filter_methods = self.expand_query_filter_methods();
        let relation_methods = self.expand_relation_methods();
        let include = self.expand_include_method(&include_ty);

        let model_name = model_ident.to_string();
        let doc_struct = docs::query_struct(&model_name);
        let doc_from_stmt = docs::query_from_stmt(&query_struct_ident.to_string());
        let doc_exec = docs::query_exec(&model_name);
        let doc_first = docs::query_first(&model_name);
        let doc_one = docs::query_one(&model_name);
        let doc_get = docs::query_get(&model_name);
        let doc_update = docs::query_update(&model_name);
        let doc_count = docs::query_count(&model_name);
        let doc_delete = docs::query_delete(&model_name);
        let doc_paginate = docs::query_paginate(&model_name);
        let doc_filter = docs::QUERY_FILTER;
        let doc_order_by = docs::QUERY_ORDER_BY;
        let doc_limit = docs::QUERY_LIMIT;
        let doc_offset = docs::QUERY_OFFSET;

        quote! {
            #[doc = #doc_struct]
            #[derive(Clone)]
            #vis struct #query_struct_ident {
                stmt: #toasty::stmt::Query<#toasty::List<#model_ident>>,
            }

            impl #query_struct_ident {
                #[doc = #doc_from_stmt]
                #vis const fn from_stmt(stmt: #toasty::stmt::Query<#toasty::List<#model_ident>>) -> #query_struct_ident {
                    #query_struct_ident { stmt }
                }

                #filter_methods

                #[doc = #doc_exec]
                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<Vec<#model_ident>> {
                    executor.exec(self.stmt.into()).await
                }

                #[doc = #doc_first]
                #vis fn first(self) -> #toasty::stmt::Query<Option<#model_ident>> {
                    self.stmt.first()
                }

                #[doc = #doc_one]
                #vis fn one(self) -> #toasty::stmt::Query<#model_ident> {
                    self.stmt.one()
                }

                #[doc = #doc_get]
                #vis async fn get(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#model_ident> {
                    self.one().exec(executor).await
                }

                #[doc = #doc_update]
                #vis fn update(self) -> #update_struct_ident {
                    #update_struct_ident::from(self)
                }

                #[doc = #doc_count]
                #vis fn count(self) -> #toasty::stmt::Query<u64> {
                    self.stmt.count()
                }

                #[doc = #doc_delete]
                #vis fn delete(self) -> #toasty::stmt::Delete<()> {
                    self.stmt.delete()
                }

                #[doc = #doc_paginate]
                #vis fn paginate(self, per_page: usize) -> #toasty::stmt::Paginate<#model_ident> {
                    #toasty::stmt::Paginate::new(self.stmt, per_page)
                }

                #[doc = #doc_filter]
                #vis fn filter(self, expr: #toasty::stmt::Expr<bool>) -> #query_struct_ident {
                    #query_struct_ident {
                        stmt: self.stmt.and(expr),
                    }
                }

                #[doc = #doc_order_by]
                #vis fn order_by(mut self, order_by: impl Into<#toasty::stmt::OrderBy>) -> #query_struct_ident {
                    self.stmt.order_by(order_by);
                    self
                }

                #[doc = #doc_limit]
                #vis fn limit(mut self, n: usize) -> #query_struct_ident {
                    self.stmt.limit(n);
                    self
                }

                #[doc = #doc_offset]
                #vis fn offset(mut self, n: usize) -> #query_struct_ident {
                    self.stmt.offset(n);
                    self
                }

                #include
                #relation_methods
            }

            impl #toasty::IntoStatement for #query_struct_ident {
                type Returning = #toasty::List<#model_ident>;

                fn into_statement(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::IntoStatement;
                    self.stmt.into_statement()
                }
            }

            impl #toasty::IntoStatement for &#query_struct_ident {
                type Returning = #toasty::List<#model_ident>;

                fn into_statement(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::IntoStatement;
                    self.stmt.clone().into_statement()
                }
            }

            impl #toasty::Default for #query_struct_ident {
                fn default() -> #query_struct_ident {
                    #query_struct_ident { stmt: #toasty::stmt::Query::all() }
                }
            }
        }
    }

    fn expand_relation_methods(&self) -> TokenStream {
        self.model
            .fields
            .iter()
            .filter_map(|field| match &field.ty {
                FieldTy::BelongsTo(rel) => Some(self.expand_belongs_to_method(field, rel)),
                FieldTy::HasMany(rel) => Some(self.expand_has_many_method(field, rel)),
                FieldTy::HasOne(rel) => Some(self.expand_has_one_method(field, rel)),
                FieldTy::Primitive(..) => None,
            })
            .collect()
    }

    fn expand_belongs_to_method(&self, field: &Field, rel: &BelongsTo) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let target = &rel.ty;
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;

        let doc = docs::query_relation_navigate(&model_ident.to_string(), &field_ident.to_string());

        quote! {
            #[doc = #doc]
            #vis fn #field_ident(mut self) -> <#target as #toasty::Relation>::Query {
                use #toasty::IntoStatement;
                <#target as #toasty::Relation>::Query::from_stmt(
                    #toasty::stmt::Association::many_via_one(
                        self.stmt, #model_ident::fields().#field_ident().into()
                    ).into_statement().into_query().unwrap()
                )
            }
        }
    }

    fn expand_has_many_method(&self, field: &Field, rel: &HasMany) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let target = &rel.ty;
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;

        let doc = docs::query_relation_navigate(&model_ident.to_string(), &field_ident.to_string());

        quote! {
            #[doc = #doc]
            #vis fn #field_ident(mut self) -> <#target as #toasty::Relation>::Query {
                use #toasty::IntoStatement;
                <#target as #toasty::Relation>::Query::from_stmt(
                    #toasty::stmt::Association::many(
                        self.stmt, #model_ident::fields().#field_ident().into()
                    ).into_statement().into_query().unwrap()
                )
            }
        }
    }

    fn expand_has_one_method(&self, field: &Field, rel: &HasOne) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let target = &rel.ty;
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;

        let doc =
            docs::query_relation_navigate_one(&model_ident.to_string(), &field_ident.to_string());

        quote! {
            #[doc = #doc]
            #vis fn #field_ident(mut self) -> <#target as #toasty::Relation>::Query {
                use #toasty::IntoStatement;
                <#target as #toasty::Relation>::Query::from_stmt(
                    #toasty::stmt::Association::many_via_one(
                        self.stmt, #model_ident::fields().#field_ident().into()
                    ).into_statement().into_query().unwrap()
                )
            }
        }
    }

    fn expand_include_method(&self, include_ty: &syn::Ident) -> Option<TokenStream> {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_struct_ident = &self.model.kind.as_root_unwrap().query_struct_ident;

        let example_relation = self
            .model
            .fields
            .iter()
            .find(|f| f.ty.is_relation())
            .map(|f| f.name.ident.to_string())
            .unwrap_or_else(|| "relation_name".to_string());
        let doc_include = docs::query_include(&model_ident.to_string(), &example_relation);

        if self.model.has_associations() {
            Some(quote! {
                    #[doc = #doc_include]
                    #vis fn include<#include_ty>(mut self, path: impl #toasty::Into<#toasty::Path<#model_ident, #include_ty>>) -> #query_struct_ident {
                        self.stmt.include(path.into());
                        self
                    }
            })
        } else {
            None
        }
    }
}
