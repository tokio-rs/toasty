use super::{Expand, util};
use crate::model::schema::{BelongsTo, Field, FieldTy, HasMany, HasOne};

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_relation_structs(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let root = self.model.kind.as_root_unwrap();
        let query_ident = &root.query_struct_ident;
        let create_builder_ident = &root.create_struct_ident;
        let field_struct_ident = &root.field_struct_ident;
        let field_list_struct_ident = &root.field_list_struct_ident;
        let filter_methods = self.expand_relation_filter_methods();

        quote! {
            #vis struct Many {
                stmt: #toasty::stmt::Association<#toasty::List<#model_ident>>,
            }

            #vis struct One {
                stmt: #toasty::stmt::Query<#toasty::List<#model_ident>>,
            }

            #vis struct OptionOne {
                stmt: #toasty::stmt::Query<#toasty::Option<#model_ident>>,
            }

            impl Many {
                pub fn from_stmt(stmt: #toasty::stmt::Association<#toasty::List<#model_ident>>) -> Many {
                    Many { stmt }
                }

                #filter_methods

                /// Iterate all entries in the relation
                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<Vec<#model_ident>> {
                    use #toasty::IntoStatement;
                    self.into_statement().exec(executor).await
                }

                #vis fn query(
                    self,
                    filter: #toasty::stmt::Expr<bool>
                ) -> #query_ident {
                    use #toasty::IntoStatement;
                    let select = self.into_statement().into_query().unwrap();
                    #query_ident::from_stmt(select.and(filter))
                }

                #vis fn create(self) -> #create_builder_ident {
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt);
                    builder
                }

                /// Add an item to the association
                #vis async fn insert(self, executor: &mut dyn #toasty::Executor, item: impl #toasty::IntoExpr<#model_ident>) -> #toasty::Result<()> {
                    executor.exec(self.stmt.insert(item)).await
                }

                /// Remove items from the association
                #vis async fn remove(self, executor: &mut dyn #toasty::Executor, item: impl #toasty::IntoExpr<#model_ident>) -> #toasty::Result<()> {
                    executor.exec(self.stmt.remove(item)).await
                }
            }

            impl #toasty::IntoStatement for Many {
                type Returning = #toasty::List<#model_ident>;

                fn into_statement(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::IntoStatement;
                    self.stmt.into_statement()
                }
            }

            impl One {
                #vis fn from_stmt(stmt: #toasty::stmt::Query<#toasty::List<#model_ident>>) -> One {
                    One { stmt }
                }

                /// Create a new associated record
                #vis fn create(self) -> #create_builder_ident {
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt);
                    builder
                }

                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#model_ident> {
                    self.stmt.one().exec(executor).await
                }
            }

            impl #toasty::IntoStatement for One {
                type Returning = #toasty::List<#model_ident>;

                fn into_statement(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::IntoStatement;
                    self.stmt.into_statement()
                }
            }

            impl OptionOne {
                pub fn from_stmt(stmt: #toasty::stmt::Query<#toasty::List<#model_ident>>) -> OptionOne {
                    OptionOne { stmt: stmt.first() }
                }

                /// Create a new associated record
                #vis fn create(self) -> #create_builder_ident {
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt);
                    builder
                }

                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#toasty::Option<#model_ident>> {
                    self.stmt.exec(executor).await
                }
            }

            #[diagnostic::do_not_recommend]
            impl #toasty::Scope for Many {
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
                    #field_list_struct_ident::from_path(#toasty::Path::from_model_list())
                }
            }

            #[diagnostic::do_not_recommend]
            impl #toasty::Scope for One {
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
                    #field_struct_ident::from_path(#toasty::Path::root())
                }
            }

            #[diagnostic::do_not_recommend]
            impl #toasty::Scope for OptionOne {
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
                    #field_struct_ident::from_path(#toasty::Path::root())
                }
            }

            #[diagnostic::do_not_recommend]
            impl #toasty::ValidateCreate for Many {
                const CREATE_META: &'static #toasty::CreateMeta =
                    &<#model_ident as #toasty::Model>::CREATE_META;
            }

            #[diagnostic::do_not_recommend]
            impl #toasty::ValidateCreate for One {
                const CREATE_META: &'static #toasty::CreateMeta =
                    &<#model_ident as #toasty::Model>::CREATE_META;
            }

            #[diagnostic::do_not_recommend]
            impl #toasty::ValidateCreate for OptionOne {
                const CREATE_META: &'static #toasty::CreateMeta =
                    &<#model_ident as #toasty::Model>::CREATE_META;
            }
        }
    }

    pub(super) fn expand_model_relation_methods(&self) -> TokenStream {
        self.model
            .fields
            .iter()
            .filter_map(|field| match &field.ty {
                FieldTy::BelongsTo(rel) => {
                    Some(self.expand_model_relation_belongs_to_method(rel, field))
                }
                FieldTy::HasMany(rel) => {
                    Some(self.expand_model_relation_has_many_method(rel, field))
                }
                FieldTy::HasOne(rel) => Some(self.expand_model_relation_has_one_method(rel, field)),
                FieldTy::Primitive(ty) if field.attrs.deferred => {
                    Some(self.expand_model_deferred_load_method(ty, field))
                }
                FieldTy::Primitive(_) => None,
            })
            .collect()
    }

    fn expand_model_deferred_load_method(&self, ty: &syn::Type, field: &Field) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;
        let field_index = util::int(field.id);
        let inner = quote!(<#ty as #toasty::Defer>::Inner);

        let pk_filter = self.primary_key_filter();
        let filter_method_ident = &pk_filter.filter_method_ident;
        let arg_idents: Vec<_> = self.expand_filter_arg_idents(pk_filter).collect();

        quote! {
            #vis fn #field_ident(&self) -> #toasty::Statement<#inner> {
                // Suppress unused field warning: a never-loaded #[deferred]
                // field still compiles cleanly even if no other code reads it.
                if false {
                    let _ = &self.#field_ident;
                }

                #toasty::build_deferred_load(
                    #model_ident::#filter_method_ident( #( & self.#arg_idents ),* ).one(),
                    #field_index,
                )
            }
        }
    }

    fn expand_model_relation_belongs_to_method(
        &self,
        rel: &BelongsTo,
        field: &Field,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_ident = &field.name.ident;
        let ty = &rel.ty;

        let operands = rel.foreign_key.iter().map(|fk_field| {
            let source = &self.model.fields[fk_field.source];
            let source_field_ident = &source.name.ident;
            let target = &fk_field.target;

            quote! {
                #toasty::Field::key_constraint(
                    &self.#source_field_ident,
                    <#ty as #toasty::Relation>::Model::fields().#target(),
                )
            }
        });

        let suppress_unused_field_warnings = rel.foreign_key.iter().map(|fk_field| {
            let source = &self.model.fields[fk_field.source];
            let source_field_ident = &source.name.ident;

            quote! {
                let _ = &self.#source_field_ident;
            }
        });

        let filter = if rel.foreign_key.len() == 1 {
            quote!( #( #operands )* )
        } else {
            quote!( #toasty::stmt::Expr::and_all([ #(#operands),* ]) )
        };

        let verify_pair_belongs_to_exists = syn::Ident::new(
            &format!("verify_pair_belongs_to_exists_for_{field_ident}"),
            field_ident.span(),
        );

        quote! {
            #vis fn #field_ident(&self) -> <#ty as #toasty::Relation>::One {
                // Suppress the unused field warning
                if false {
                    let _ = &self.#field_ident;
                }

                {
                    use #toasty::IntoStatement;
                    <#ty as #toasty::Relation>::One::from_stmt(
                        <#ty as #toasty::Relation>::Model::filter(#filter).into_statement().into_query().unwrap()
                    )
                }
            }

            #[doc(hidden)]
            #vis fn #verify_pair_belongs_to_exists(&self) -> &#ty {
                #(
                    #suppress_unused_field_warnings
                )*
                &self.#field_ident
            }
        }
    }

    fn expand_model_relation_has_many_method(&self, rel: &HasMany, field: &Field) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_ident = &field.name.ident;
        let ty = &rel.ty;

        // A `via` relation reaches its target through a path of existing
        // relations; it has no paired `BelongsTo`, so skip the back-reference
        // check that direct has-many relations emit.
        let pair_check = if rel.via.is_some() {
            quote! {}
        } else {
            let pair_ident = rel.pair.clone().unwrap_or(syn::Ident::new(
                &self.model.name.ident.to_string(),
                rel.span,
            ));
            self.expand_pair_belongs_to_check(
                &pair_ident,
                field_ident,
                ty,
                rel.span,
                "HasMany",
                "Has many associations require the target to include a back-reference",
            )
        };

        quote! {
            #vis fn #field_ident(&self) -> <#ty as #toasty::Relation>::Many {
                // Suppress the unused field warning
                if false {
                    let _ = &self.#field_ident;
                }

                #pair_check

                {
                    use #toasty::IntoStatement;
                    <#ty as #toasty::Relation>::Many::from_stmt(
                        #toasty::stmt::Association::many(
                            self.into_statement().into_query().unwrap().to_list(),
                            Self::fields().#field_ident().into()
                        )
                    )
                }
            }
        }
    }

    fn expand_model_relation_has_one_method(&self, rel: &HasOne, field: &Field) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_ident = &field.name.ident;
        let ty = &rel.ty;

        // A `via` relation reaches its target through a path of existing
        // relations; it has no paired `BelongsTo`, so skip the back-reference
        // check that direct has-one relations emit.
        let pair_check = if rel.via.is_some() {
            quote! {}
        } else {
            let pair_ident = syn::Ident::new(&self.model.name.ident.to_string(), rel.span);
            self.expand_pair_belongs_to_check(
                &pair_ident,
                field_ident,
                ty,
                rel.span,
                "HasOne",
                "Has one associations require the target to include a back-reference",
            )
        };

        quote! {
            #vis fn #field_ident(&self) -> <#ty as #toasty::Relation>::One {
                // Suppress the unused field warning
                if false {
                    let _ = &self.#field_ident;
                }

                #pair_check

                {
                    use #toasty::IntoStatement;
                    <#ty as #toasty::Relation>::One::from_stmt(
                        #toasty::stmt::Association::one(
                            self.into_statement().into_query().unwrap().to_list(),
                            Self::fields().#field_ident().into()
                        ).into_statement().into_query().unwrap()
                    )
                }
            }
        }
    }

    /// Emit a compile-time check that the target model has a `BelongsTo<Self>`
    /// (or `BelongsTo<Option<Self>>`) field named `pair_ident`. Shared by the
    /// has-many and has-one accessor expansions; the relation kind ("HasMany"
    /// / "HasOne") and `label` are woven into the `on_unimplemented` diagnostic.
    fn expand_pair_belongs_to_check(
        &self,
        pair_ident: &syn::Ident,
        field_ident: &syn::Ident,
        ty: &syn::Type,
        rel_span: proc_macro2::Span,
        relation_kind: &str,
        label: &str,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;

        let verify_pair_belongs_to_exists_for_field = syn::Ident::new(
            &format!("verify_pair_belongs_to_exists_for_{pair_ident}"),
            field_ident.span(),
        );

        let verify_a = util::ident("A");
        let verify_t = util::ident("T");

        let msg = format!(
            "{relation_kind} requires the {{{verify_a}}}::{pair_ident} field to be of type `BelongsTo<Self>`, but it was `{{Self}}` instead"
        );

        quote::quote_spanned! {rel_span=>
            // Reference the field to generate a compiler error if it is missing.
            #[allow(unreachable_code)]
            if false {
                fn load<#verify_t: #toasty::Model>() -> #verify_t {
                    #verify_t::load(todo!()).unwrap()
                }

                #[diagnostic::on_unimplemented(
                    message = #msg,
                    label = #label,
                    note = "Note 1",
                    // note = "Note 2"
                )]
                trait Verify<#verify_a> {
                }

                #[diagnostic::do_not_recommend]
                impl<#verify_a> Verify<#verify_a> for #toasty::BelongsTo<#model_ident> {
                }

                #[diagnostic::do_not_recommend]
                impl<#verify_a> Verify<#verify_a> for #toasty::BelongsTo<Option<#model_ident>> {
                }

                fn verify<#verify_t: Verify<#verify_a>, #verify_a>(_: &#verify_t) {
                }

                let instance = load::<<#ty as #toasty::Relation>::Model>();
                verify::<_, <#ty as #toasty::Relation>::Model>(instance.#verify_pair_belongs_to_exists_for_field());
            }
        }
    }
}
