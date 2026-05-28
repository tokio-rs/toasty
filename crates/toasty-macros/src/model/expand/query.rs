use super::{Expand, util};
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
        let (projection_methods, projection_warnings) = self.expand_query_projection_methods();
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

                #vis fn count_rows(self) -> #toasty::stmt::Query<u64> {
                    self.stmt.count_rows()
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
                #projection_methods
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

            #projection_warnings
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

    /// Emit a projection method for each non-relation field on the model's
    /// query struct: `UserQuery::name(self) -> Query<List<String>>`, etc.
    ///
    /// The return type uses `<Ty as Field>::ExprTarget`, which delegates
    /// through wrappers (`Deferred<T>` strips to `T::ExprTarget`).
    ///
    /// Returns `(in_impl, outside_impl)`: methods placed inside `impl Query`
    /// and a sibling set of `#[deprecated]` markers for fields whose names
    /// collide with a built-in wrapper method. One marker per field — the
    /// trigger constant references the deprecated `fn`, which fires the
    /// warning at compile time. Markers cover collisions on `{Model}Query`,
    /// `Many`, `One`, and `OptionOne` collectively.
    fn expand_query_projection_methods(&self) -> (TokenStream, TokenStream) {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;

        let mut methods = TokenStream::new();
        let mut warnings = TokenStream::new();

        for field in &self.model.fields {
            let ty = match &field.ty {
                FieldTy::Primitive(ty) => ty,
                _ => continue,
            };
            let field_ident = &field.name.ident;

            if super::projection_method_collides(field_ident) {
                warnings.extend(super::projection_collision_warning(
                    field_ident,
                    model_ident,
                ));
                // Still skip the in-impl emission if it collides on this
                // wrapper specifically; otherwise the method generates fine
                // and the marker only documents collisions on the other
                // wrappers (Many / One / OptionOne).
                if super::QUERY_RESERVED_METHODS.contains(&field_ident.to_string().as_str()) {
                    continue;
                }
            }

            methods.extend(quote! {
                #vis fn #field_ident(self) -> #toasty::stmt::Query<
                    #toasty::List<<#ty as #toasty::Field>::ExprTarget>
                > {
                    self.stmt.select(#model_ident::fields().#field_ident())
                }
            });
        }

        (methods, warnings)
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
