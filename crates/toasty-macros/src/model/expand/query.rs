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
        let create_builder_ident = &self.model.kind.as_root_unwrap().create_struct_ident;
        let field_struct_ident = &self.model.kind.as_root_unwrap().field_struct_ident;
        let field_list_struct_ident = &self.model.kind.as_root_unwrap().field_list_struct_ident;

        let include_ty = util::ident("T");
        let filter_methods = self.expand_query_list_filter_methods();
        let relation_methods = self.expand_query_list_relation_methods();
        let include = self.expand_include_method(&include_ty);

        quote! {
            #vis struct #query_struct_ident<__T = #toasty::List<#model_ident>> {
                stmt: #toasty::stmt::Query<__T>,
            }

            impl<__T> #toasty::Clone for #query_struct_ident<__T> {
                fn clone(&self) -> Self {
                    Self {
                        stmt: self.stmt.clone(),
                    }
                }
            }

            // ----- Shared methods (all `T`) -----
            impl<__T> #query_struct_ident<__T> {
                #vis const fn from_stmt(stmt: #toasty::stmt::Query<__T>) -> Self {
                    Self { stmt }
                }

                #include
            }

            // ----- List<M>-specific methods -----
            impl #query_struct_ident<#toasty::List<#model_ident>> {
                /// Construct a list query from a many-style association.
                #[doc(hidden)]
                #vis fn from_assoc_many(
                    assoc: #toasty::stmt::Association<#toasty::List<#model_ident>>,
                ) -> Self {
                    use #toasty::IntoStatement;
                    let stmt = assoc.into_statement().into_query().unwrap();
                    Self { stmt }
                }

                #filter_methods

                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<Vec<#model_ident>> {
                    executor.exec(self.stmt.into()).await
                }

                #vis fn first(self) -> #query_struct_ident<#toasty::Option<#model_ident>> {
                    #query_struct_ident {
                        stmt: self.stmt.first(),
                    }
                }

                #vis fn one(self) -> #query_struct_ident<#model_ident> {
                    #query_struct_ident {
                        stmt: self.stmt.one(),
                    }
                }

                #vis async fn get(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#model_ident> {
                    self.one().exec(executor).await
                }

                #vis fn update(self) -> #update_struct_ident {
                    #update_struct_ident::from(self.stmt)
                }

                #vis fn count(self) -> #toasty::stmt::Query<u64> {
                    self.stmt.count()
                }

                #vis fn select<__E, __U>(
                    self,
                    projection: __E,
                ) -> #toasty::stmt::Query<#toasty::List<__U>>
                where
                    __E: #toasty::IntoExpr<__U>,
                    __U: #toasty::Load,
                {
                    self.stmt.select(projection)
                }

                #vis fn delete(self) -> #toasty::stmt::Delete<()> {
                    self.stmt.delete()
                }

                #vis fn paginate(self, per_page: usize) -> #toasty::stmt::Paginate<#model_ident> {
                    #toasty::stmt::Paginate::new(self.stmt, per_page)
                }

                #vis fn filter(self, expr: #toasty::stmt::Expr<bool>) -> Self {
                    Self {
                        stmt: self.stmt.and(expr),
                    }
                }

                #vis fn order_by(mut self, order_by: impl Into<#toasty::stmt::OrderBy>) -> Self {
                    self.stmt.order_by(order_by);
                    self
                }

                #vis fn latest_by<#include_ty>(
                    mut self,
                    field: #toasty::stmt::Path<#model_ident, #include_ty>,
                ) -> Self {
                    self.stmt.latest_by(field);
                    self
                }

                #vis fn limit(mut self, n: usize) -> Self {
                    self.stmt.limit(n);
                    self
                }

                #vis fn offset(mut self, n: usize) -> Self {
                    self.stmt.offset(n);
                    self
                }

                /// Add an item to the relation this query was scoped from.
                ///
                /// Returns an error at exec time if the query is not scoped to
                /// a single-step relation traversal.
                #vis async fn insert(
                    self,
                    executor: &mut dyn #toasty::Executor,
                    item: impl #toasty::IntoExpr<#model_ident>,
                ) -> #toasty::Result<()> {
                    #toasty::relation_insert(self.stmt, executor, item).await
                }

                /// Remove an item from the relation this query was scoped from.
                ///
                /// Returns an error at exec time if the query is not scoped to
                /// a single-step relation traversal.
                #vis async fn remove(
                    self,
                    executor: &mut dyn #toasty::Executor,
                    item: impl #toasty::IntoExpr<#model_ident>,
                ) -> #toasty::Result<()> {
                    #toasty::relation_remove(self.stmt, executor, item).await
                }

                #vis fn create(self) -> #create_builder_ident {
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt);
                    builder
                }

                #relation_methods
            }

            // ----- Single-row impl -----
            impl #query_struct_ident<#model_ident> {
                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#model_ident> {
                    use #toasty::IntoStatement;
                    let stmt: #toasty::Statement<#model_ident> = self.stmt.into_statement();
                    executor.exec(stmt).await
                }

                #vis fn create(self) -> #create_builder_ident {
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt);
                    builder
                }
            }

            // ----- Optional single-row impl -----
            impl #query_struct_ident<#toasty::Option<#model_ident>> {
                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#toasty::Option<#model_ident>> {
                    use #toasty::IntoStatement;
                    let stmt: #toasty::Statement<#toasty::Option<#model_ident>> = self.stmt.into_statement();
                    executor.exec(stmt).await
                }

                #vis fn create(self) -> #create_builder_ident {
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt);
                    builder
                }
            }

            // ----- IntoStatement / IntoScope -----
            impl #toasty::IntoStatement for #query_struct_ident<#toasty::List<#model_ident>> {
                type Returning = #toasty::List<#model_ident>;

                fn into_statement(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::IntoStatement;
                    self.stmt.into_statement()
                }
            }

            impl #toasty::IntoStatement for &#query_struct_ident<#toasty::List<#model_ident>> {
                type Returning = #toasty::List<#model_ident>;

                fn into_statement(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::IntoStatement;
                    self.stmt.clone().into_statement()
                }
            }

            impl #toasty::IntoStatement for #query_struct_ident<#model_ident> {
                type Returning = #model_ident;

                fn into_statement(self) -> #toasty::Statement<#model_ident> {
                    use #toasty::IntoStatement;
                    self.stmt.into_statement()
                }
            }

            impl #toasty::IntoStatement for &#query_struct_ident<#model_ident> {
                type Returning = #model_ident;

                fn into_statement(self) -> #toasty::Statement<#model_ident> {
                    use #toasty::IntoStatement;
                    self.stmt.clone().into_statement()
                }
            }

            impl #toasty::IntoStatement for #query_struct_ident<#toasty::Option<#model_ident>> {
                type Returning = #toasty::Option<#model_ident>;

                fn into_statement(self) -> #toasty::Statement<#toasty::Option<#model_ident>> {
                    use #toasty::IntoStatement;
                    self.stmt.into_statement()
                }
            }

            impl #toasty::IntoStatement for &#query_struct_ident<#toasty::Option<#model_ident>> {
                type Returning = #toasty::Option<#model_ident>;

                fn into_statement(self) -> #toasty::Statement<#toasty::Option<#model_ident>> {
                    use #toasty::IntoStatement;
                    self.stmt.clone().into_statement()
                }
            }

            impl #toasty::stmt::IntoScope<#model_ident> for #query_struct_ident<#toasty::List<#model_ident>> {
                fn into_scope(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::stmt::IntoScope;
                    self.stmt.into_scope()
                }
            }

            impl #toasty::stmt::IntoScope<#model_ident> for &#query_struct_ident<#toasty::List<#model_ident>> {
                fn into_scope(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::stmt::IntoScope;
                    self.stmt.clone().into_scope()
                }
            }

            impl #toasty::stmt::IntoScope<#model_ident> for #query_struct_ident<#model_ident> {
                fn into_scope(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::stmt::IntoScope;
                    self.stmt.into_scope()
                }
            }

            impl #toasty::stmt::IntoScope<#model_ident> for #query_struct_ident<#toasty::Option<#model_ident>> {
                fn into_scope(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::stmt::IntoScope;
                    self.stmt.into_scope()
                }
            }

            impl #toasty::Default for #query_struct_ident<#toasty::List<#model_ident>> {
                fn default() -> Self {
                    Self {
                        stmt: #toasty::stmt::Query::all(),
                    }
                }
            }

            // ----- Scope trait impls (used by `create!` macro and field path builder) -----
            #[diagnostic::do_not_recommend]
            impl #toasty::Scope for #query_struct_ident<#toasty::List<#model_ident>> {
                type Item = #toasty::List<#model_ident>;
                type Path<__Origin> = #field_list_struct_ident<__Origin>;
                type Create = #create_builder_ident;

                fn new_path<__Origin>(path: #toasty::Path<__Origin, Self::Item>) -> Self::Path<__Origin> {
                    #field_list_struct_ident::from_path(path)
                }

                fn new_create() -> Self::Create {
                    #create_builder_ident::default()
                }

                fn new_path_root() -> Self::Path<Self::Item> {
                    #field_list_struct_ident::from_path(<#model_ident as #toasty::Model>::path_model_list())
                }

                fn create_in_scope(self) -> Self::Create {
                    Self::create(self)
                }
            }

            #[diagnostic::do_not_recommend]
            impl #toasty::Scope for #query_struct_ident<#model_ident> {
                type Item = #model_ident;
                type Path<__Origin> = #field_struct_ident<__Origin>;
                type Create = #create_builder_ident;

                fn new_path<__Origin>(path: #toasty::Path<__Origin, Self::Item>) -> Self::Path<__Origin> {
                    #field_struct_ident::from_path(path)
                }

                fn new_create() -> Self::Create {
                    #create_builder_ident::default()
                }

                fn new_path_root() -> Self::Path<Self::Item> {
                    #field_struct_ident::from_path(<#model_ident as #toasty::Model>::path_root())
                }

                fn create_in_scope(self) -> Self::Create {
                    Self::create(self)
                }
            }

            #[diagnostic::do_not_recommend]
            impl #toasty::Scope for #query_struct_ident<#toasty::Option<#model_ident>> {
                type Item = #model_ident;
                type Path<__Origin> = #field_struct_ident<__Origin>;
                type Create = #create_builder_ident;

                fn new_path<__Origin>(path: #toasty::Path<__Origin, Self::Item>) -> Self::Path<__Origin> {
                    #field_struct_ident::from_path(path)
                }

                fn new_create() -> Self::Create {
                    #create_builder_ident::default()
                }

                fn new_path_root() -> Self::Path<Self::Item> {
                    #field_struct_ident::from_path(<#model_ident as #toasty::Model>::path_root())
                }

                fn create_in_scope(self) -> Self::Create {
                    Self::create(self)
                }
            }
        }
    }

    /// Filter methods on `Query<List<M>>` — merge of `expand_query_filter_methods`
    /// and `expand_relation_filter_methods`. Both forms went into different
    /// structs before unification; they share the same method shape (take
    /// `self`, return `Query<List<M>>`).
    fn expand_query_list_filter_methods(&self) -> TokenStream {
        self.expand_relation_filter_methods()
    }

    /// Per-relation-field accessor methods on `Query<List<M>>`. Replaces the
    /// previous methods that lived on the old `UserQuery` and `Many` structs,
    /// unifying their behaviour. Dispatches to `chain_or_build_many` /
    /// `chain_or_build_many_via_one` which handle both the "already scoped
    /// from a traversal — extend the path" and "fresh query — start a new
    /// association" branches.
    fn expand_query_list_relation_methods(&self) -> TokenStream {
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
        let ty = &rel.ty;
        let target = quote!(<#ty as #toasty::RelationOneField>::Model);
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;
        let field_offset = util::int(field.id);

        // Belongs-to via the source side is always singular and may be
        // nullable; both cases collapse to `QueryMany<Target>` here because we
        // are starting from a list of source rows.
        quote! {
            #vis fn #field_ident(self) -> #toasty::QueryMany<#target> {
                let assoc = #toasty::chain_or_build_many_via_one::<#model_ident, #target>(
                    self.stmt,
                    #field_offset,
                    #model_ident::fields().#field_ident().into(),
                );
                <#toasty::QueryMany<#target>>::from_assoc_many(assoc)
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
        let field_offset = util::int(field.id);

        quote! {
            #vis fn #field_ident(self) -> #toasty::QueryMany<#target> {
                let assoc = #toasty::chain_or_build_many::<#model_ident, #target>(
                    self.stmt,
                    #field_offset,
                    #model_ident::fields().#field_ident().into(),
                );
                <#toasty::QueryMany<#target>>::from_assoc_many(assoc)
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
        let field_offset = util::int(field.id);

        quote! {
            #vis fn #field_ident(self) -> #toasty::QueryMany<#target> {
                let assoc = #toasty::chain_or_build_many_via_one::<#model_ident, #target>(
                    self.stmt,
                    #field_offset,
                    #model_ident::fields().#field_ident().into(),
                );
                <#toasty::QueryMany<#target>>::from_assoc_many(assoc)
            }
        }
    }

    fn expand_include_method(&self, include_ty: &syn::Ident) -> Option<TokenStream> {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;

        // Always emit `include()` on root models. The macro can't see through a
        // field's type to know whether an embedded type holds a deferred
        // sub-field, so a stricter gate would deny `.include(metadata().notes())`
        // on a model whose only includable thing lives inside an embed.
        Some(quote! {
            #vis fn include<#include_ty>(
                mut self,
                path: impl #toasty::Into<#toasty::Path<#model_ident, #include_ty>>,
            ) -> Self {
                self.stmt.include(path.into());
                self
            }
        })
    }
}
