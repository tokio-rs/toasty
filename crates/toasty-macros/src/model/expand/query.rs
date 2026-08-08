use super::{Expand, util};
use crate::model::schema::{BelongsTo, Field, FieldTy, HasMany, HasOne};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

const QUERY_LIST_RESERVED_METHODS: &[&str] = &[
    "from_assoc_many",
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
    "insert",
    "remove",
    "include",
    "exec",
    "create",
];

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
        let filter_methods = self.expand_relation_filter_methods();
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

                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<__T::Output>
                where
                    __T: #toasty::Load,
                {
                    use #toasty::IntoStatement;
                    executor.exec(self.stmt.into_statement()).await
                }
            }

            // ----- Shared `create` (any `T` whose query can scope an insert) -----
            impl<__T> #query_struct_ident<__T>
            where
                #toasty::stmt::Query<__T>: #toasty::stmt::IntoScope<#model_ident>,
            {
                #vis fn create(self) -> #create_builder_ident {
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt);
                    builder
                }
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
                        stmt: self.stmt.filter(expr),
                    }
                }

                #vis fn order_by(self, order_by: impl Into<#toasty::stmt::OrderBy>) -> Self {
                    Self {
                        stmt: self.stmt.order_by(order_by),
                    }
                }

                #vis fn latest_by<#include_ty>(
                    self,
                    field: #toasty::stmt::Path<#model_ident, #include_ty>,
                ) -> Self {
                    Self {
                        stmt: self.stmt.latest_by(field),
                    }
                }

                #vis fn limit(self, n: usize) -> Self {
                    Self {
                        stmt: self.stmt.limit(n),
                    }
                }

                #vis fn offset(self, n: usize) -> Self {
                    Self {
                        stmt: self.stmt.offset(n),
                    }
                }

                /// Returns a builder that will add an item to the relation this query was scoped
                /// from only on execution.
                #vis fn insert(
                    self,
                    item: impl #toasty::IntoExpr<#model_ident>,
                ) -> #toasty::stmt::RelationInsert<#model_ident> {
                    self.stmt.insert(item)
                }

                /// Returns a builder that will remove an item from the relation this query was scoped
                /// from only on execution.
                #vis fn remove(
                    self,
                    item: impl #toasty::IntoExpr<#model_ident>,
                ) -> #toasty::stmt::RelationRemove<#model_ident> {
                    self.stmt.remove(item)
                }

                #relation_methods
            }

            // ----- IntoStatement / IntoScope -----
            impl<__T> #toasty::IntoStatement for #query_struct_ident<__T> {
                type Returning = __T;

                fn into_statement(self) -> #toasty::Statement<__T> {
                    use #toasty::IntoStatement;
                    self.stmt.into_statement()
                }
            }

            impl<__T> #toasty::IntoStatement for &#query_struct_ident<__T> {
                type Returning = __T;

                fn into_statement(self) -> #toasty::Statement<__T> {
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
            .filter(|field| {
                !util::ident_is_reserved(&field.name.ident, QUERY_LIST_RESERVED_METHODS)
            })
            .filter_map(|field| match &field.ty {
                FieldTy::BelongsTo(rel) => Some(self.expand_belongs_to_method(field, rel)),
                FieldTy::HasMany(rel) if rel.via.is_some() => {
                    Some(self.expand_has_many_via_method(field, rel))
                }
                FieldTy::HasMany(rel) => Some(self.expand_has_many_method(field, rel)),
                FieldTy::HasOne(rel) => Some(self.expand_has_one_method(field, rel)),
                FieldTy::Primitive(..) => None,
            })
            .collect()
    }

    /// Shared body for the per-relation accessor methods on `Query<List<M>>`.
    /// The three direct relation kinds emit the same `QueryMany<Target>`
    /// accessor and differ only along two axes: which projection trait names
    /// the target model (`RelationOneField` for the singular `belongs_to` /
    /// `has_one`, `RelationManyField` for `has_many`) and which
    /// `chain_or_build_*` helper threads the association. `has_many(via = …)`
    /// takes a different shape and is handled by
    /// [`Self::expand_has_many_via_method`].
    fn expand_relation_query_method(
        &self,
        field: &Field,
        ty: &syn::Type,
        target_field: &str,
        chain_builder: &str,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let target_field = format_ident!("{}", target_field);
        let chain_builder = format_ident!("{}", chain_builder);
        let target = quote!(<#ty as #toasty::#target_field>::Target);
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;
        let field_offset = util::int(field.id);

        quote! {
            #vis fn #field_ident(self) -> #toasty::QueryMany<#target> {
                let assoc = #toasty::#chain_builder::<#model_ident, #target>(
                    self.stmt,
                    #field_offset,
                    #model_ident::fields().#field_ident().into(),
                );
                <#toasty::QueryMany<#target>>::from_assoc_many(assoc)
            }
        }
    }

    fn expand_belongs_to_method(&self, field: &Field, rel: &BelongsTo) -> TokenStream {
        // Belongs-to via the source side is always singular and may be
        // nullable; both cases collapse to `QueryMany<Target>` here because we
        // are starting from a list of source rows.
        self.expand_relation_query_method(
            field,
            &rel.ty,
            "RelationOneField",
            "chain_or_build_many_via_one",
        )
    }

    /// Chained accessor for a `#[has_many(via = …)]` field. Builds an
    /// association from the current query following the via field, then routes
    /// through [`ViaTarget`] — keyed on the terminal type — so it works for a
    /// model terminal (`QueryMany<M>`) and a scalar terminal
    /// (`Query<List<scalar>>`) alike. Mirrors the instance accessor in
    /// `relation.rs`; here the source query is `self.stmt` directly.
    fn expand_has_many_via_method(&self, field: &Field, rel: &HasMany) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let ty = &rel.ty;
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;

        quote! {
            #vis fn #field_ident(self) -> #toasty::ViaMany<#ty> {
                // The accessor returns the terminal's `ViaTarget::Path` (a
                // `ManyField` for a model terminal, a `Path` for a scalar);
                // `.into()` collapses either into the `Path` the association
                // needs.
                let __assoc = #toasty::stmt::Association::from_source_and_path(
                    self.stmt,
                    #model_ident::fields().#field_ident().into(),
                );
                <<#ty as #toasty::ViaManyField>::Target as #toasty::ViaTarget>::make_via_query(__assoc)
            }
        }
    }

    fn expand_has_many_method(&self, field: &Field, rel: &HasMany) -> TokenStream {
        self.expand_relation_query_method(
            field,
            &rel.ty,
            "RelationManyField",
            "chain_or_build_many",
        )
    }

    fn expand_has_one_method(&self, field: &Field, rel: &HasOne) -> TokenStream {
        self.expand_relation_query_method(
            field,
            &rel.ty,
            "RelationOneField",
            "chain_or_build_many_via_one",
        )
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
                self,
                include: impl #toasty::Into<#toasty::stmt::Include<#model_ident, #include_ty>>,
            ) -> Self {
                Self {
                    stmt: self.stmt.include(include.into()),
                }
            }
        })
    }
}
