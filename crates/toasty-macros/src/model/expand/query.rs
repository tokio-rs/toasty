use super::{Expand, util};
use crate::model::schema::{BelongsTo, Field, FieldTy, HasMany, HasOne};

use proc_macro2::TokenStream;
use quote::quote;

/// Inherent method names emitted on the generated `{Model}Query` struct.
/// A primitive field with one of these names cannot get a same-named
/// projection method without producing a duplicate-definition error.
const QUERY_STRUCT_RESERVED_METHODS: &[&str] = &[
    "from_stmt",
    "exec",
    "first",
    "one",
    "get",
    "update",
    "count",
    "select",
    "delete",
    "paginate",
    "filter",
    "order_by",
    "latest_by",
    "limit",
    "offset",
    "include",
];

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
        let primitive_field_projections = self.expand_query_primitive_field_methods();
        let include = self.expand_include_method(&include_ty);

        quote! {
            #[derive(Clone)]
            #vis struct #query_struct_ident {
                stmt: #toasty::stmt::Query<#toasty::List<#model_ident>>,
            }

            impl #query_struct_ident {
                #vis const fn from_stmt(stmt: #toasty::stmt::Query<#toasty::List<#model_ident>>) -> #query_struct_ident {
                    #query_struct_ident { stmt }
                }

                #filter_methods

                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<Vec<#model_ident>> {
                    executor.exec(self.stmt.into()).await
                }

                #vis fn first(self) -> #toasty::stmt::Query<Option<#model_ident>> {
                    self.stmt.first()
                }

                #vis fn one(self) -> #toasty::stmt::Query<#model_ident> {
                    self.stmt.one()
                }

                #vis async fn get(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#model_ident> {
                    self.one().exec(executor).await
                }

                #vis fn update(self) -> #update_struct_ident {
                    #update_struct_ident::from(self)
                }

                #vis fn count(self) -> #toasty::stmt::Query<u64> {
                    self.stmt.count()
                }

                #vis fn select<__E, __T>(
                    self,
                    projection: __E,
                ) -> #toasty::stmt::Query<#toasty::List<__T>>
                where
                    __E: #toasty::IntoExpr<__T>,
                    __T: #toasty::Load,
                {
                    self.stmt.select(projection)
                }

                #vis fn delete(self) -> #toasty::stmt::Delete<()> {
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

                #vis fn latest_by<#include_ty>(
                    mut self,
                    field: #toasty::stmt::Path<#model_ident, #include_ty>
                ) -> #query_struct_ident {
                    self.stmt.latest_by(field);
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
                #primitive_field_projections
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

            impl #toasty::stmt::IntoScope<#model_ident> for #query_struct_ident {
                fn into_scope(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::stmt::IntoScope;
                    self.stmt.into_scope()
                }
            }

            impl #toasty::stmt::IntoScope<#model_ident> for &#query_struct_ident {
                fn into_scope(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::stmt::IntoScope;
                    self.stmt.clone().into_scope()
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

    /// For each primitive field, emit a method that projects the query down
    /// to just that column — `User::all().name().exec(&mut db)` returns
    /// `Vec<String>`. Symmetric with the same method generated on `Many`,
    /// so `.filter(...).name()` chains work too.
    ///
    /// Fields whose names collide with the built-in inherent methods on the
    /// generated query struct (`first`, `one`, `count`, etc.) are skipped —
    /// users can still project them via `.select(Model::fields().foo())`.
    fn expand_query_primitive_field_methods(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;

        self.model
            .fields
            .iter()
            .filter_map(|field| {
                let FieldTy::Primitive(ty) = &field.ty else {
                    return None;
                };
                let field_ident = &field.name.ident;
                if QUERY_STRUCT_RESERVED_METHODS.contains(&field_ident.to_string().as_str()) {
                    return None;
                }

                Some(quote! {
                    #vis fn #field_ident(self) -> #toasty::stmt::Query<#toasty::List<<#ty as #toasty::Field>::ExprTarget>>
                    where
                        #ty: #toasty::Field,
                        <#ty as #toasty::Field>::Path<#model_ident>:
                            Into<#toasty::Path<#model_ident, <#ty as #toasty::Field>::ExprTarget>>,
                        <#ty as #toasty::Field>::ExprTarget: #toasty::Load,
                    {
                        let path: #toasty::Path<#model_ident, <#ty as #toasty::Field>::ExprTarget> =
                            #model_ident::fields().#field_ident().into();
                        self.stmt.select(path)
                    }
                })
            })
            .collect()
    }

    fn expand_belongs_to_method(&self, field: &Field, rel: &BelongsTo) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let ty = &rel.ty;
        let target = quote!(<#ty as #toasty::RelationOneField>::Model);
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;

        quote! {
            #vis fn #field_ident(mut self) -> <#target as #toasty::Model>::Query {
                use #toasty::IntoStatement;
                <<#target as #toasty::Model>::Query>::from_stmt(
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
        let ty = &rel.ty;
        let target = quote!(<#ty as #toasty::RelationManyField>::Model);
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;

        quote! {
            #vis fn #field_ident(mut self) -> <#target as #toasty::Model>::Query {
                use #toasty::IntoStatement;
                <<#target as #toasty::Model>::Query>::from_stmt(
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
        let ty = &rel.ty;
        let target = quote!(<#ty as #toasty::RelationOneField>::Model);
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;

        quote! {
            #vis fn #field_ident(mut self) -> <#target as #toasty::Model>::Query {
                use #toasty::IntoStatement;
                <<#target as #toasty::Model>::Query>::from_stmt(
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

        // Always emit `include()` on root models. The macro can't see through a
        // field's type to know whether an embedded type holds a deferred
        // sub-field, so a stricter gate would deny `.include(metadata().notes())`
        // on a model whose only includable thing lives inside an embed.
        Some(quote! {
                #vis fn include<#include_ty>(mut self, path: impl #toasty::Into<#toasty::Path<#model_ident, #include_ty>>) -> #query_struct_ident {
                    self.stmt.include(path.into());
                    self
                }
        })
    }
}
