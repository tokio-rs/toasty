use super::{util, Expand};
use crate::schema::{BelongsTo, Field, FieldTy, HasMany, HasOne};

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_query_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_struct_ident = &self.model.kind.expect_root().query_struct_ident;
        let update_struct_ident = &self.model.kind.expect_root().update_struct_ident;
        let include_ty = util::ident("T");
        let filter_methods = self.expand_query_filter_methods();
        let relation_methods = self.expand_relation_methods();
        let include = self.expand_include_method(&include_ty);

        quote! {
            #vis struct #query_struct_ident {
                stmt: #toasty::stmt::Query<#toasty::List<#model_ident>>,
            }

            impl #query_struct_ident {
                #vis const fn from_stmt(stmt: #toasty::stmt::Query<#toasty::List<#model_ident>>) -> #query_struct_ident {
                    #query_struct_ident { stmt }
                }

                #filter_methods

                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<Vec<#model_ident>> {
                    use #toasty::ExecutorExt;
                    executor.all(self.stmt).await
                }

                #vis async fn first(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#toasty::Option<#model_ident>> {
                    use #toasty::ExecutorExt;
                    executor.first(self.stmt).await
                }

                #vis async fn get(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#model_ident> {
                    use #toasty::ExecutorExt;
                    executor.get(self.stmt).await
                }

                #vis fn update(self) -> #update_struct_ident {
                    #update_struct_ident::from(self)
                }

                #vis fn delete(self) -> #toasty::stmt::Delete<#toasty::List<#model_ident>> {
                    self.stmt.delete()
                }

                #vis fn paginate(self, per_page: usize) -> #toasty::stmt::Paginate<#model_ident> {
                    #toasty::stmt::Paginate::new(self.stmt, per_page)
                }

                #vis fn filter(self, expr: #toasty::stmt::Expr<bool>) -> #query_struct_ident {
                    #query_struct_ident {
                        stmt: self.stmt.and(expr),
                    }
                }

                #vis fn order_by(mut self, order_by: impl Into<#toasty::stmt::OrderBy>) -> #query_struct_ident {
                    self.stmt.order_by(order_by);
                    self
                }

                #vis fn limit(mut self, n: usize) -> #query_struct_ident {
                    self.stmt.limit(n);
                    self
                }

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

        quote! {
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

        quote! {
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

        quote! {
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
        let query_struct_ident = &self.model.kind.expect_root().query_struct_ident;

        if self.model.has_associations() {
            Some(quote! {
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
