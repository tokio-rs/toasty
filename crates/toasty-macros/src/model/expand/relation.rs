use super::{Expand, util};
use crate::model::schema::{BelongsTo, Field, FieldTy, HasMany, HasOne, ItemParent};

use proc_macro2::TokenStream;
use quote::quote;

struct PairBelongsToCheck<'a> {
    pair_ident: &'a syn::Ident,
    field_ident: &'a syn::Ident,
    ty: &'a syn::Type,
    field_trait: TokenStream,
    rel_span: proc_macro2::Span,
    relation_kind: &'static str,
    label: &'static str,
}

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
        let chain_methods = self.expand_many_chain_methods();

        quote! {
            #vis struct Many<Kind = #toasty::Direct> {
                stmt: #toasty::stmt::Association<#toasty::List<#model_ident>>,
                _kind: std::marker::PhantomData<Kind>,
            }

            #vis struct One<Kind = #toasty::Direct> {
                stmt: #toasty::stmt::Query<#model_ident>,
                _kind: std::marker::PhantomData<Kind>,
            }

            #vis struct OptionOne<Kind = #toasty::Direct> {
                stmt: #toasty::stmt::Query<#toasty::Option<#model_ident>>,
                _kind: std::marker::PhantomData<Kind>,
            }

            impl<Kind> Many<Kind> {
                pub fn from_stmt(stmt: #toasty::stmt::Association<#toasty::List<#model_ident>>) -> Many<Kind> {
                    Many {
                        stmt,
                        _kind: std::marker::PhantomData,
                    }
                }

                #filter_methods

                #chain_methods

                /// Iterate all entries in the relation
                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<Vec<#model_ident>> {
                    use #toasty::IntoStatement;
                    self.into_statement().exec(executor).await
                }

                #vis fn filter(
                    self,
                    filter: #toasty::stmt::Expr<bool>
                ) -> #query_ident {
                    use #toasty::IntoStatement;
                    let select = self.into_statement().into_query().unwrap();
                    #query_ident::from_stmt(select.and(filter))
                }

                #vis fn create(self) -> <Self as #toasty::Scope>::Create
                where
                    Self: #toasty::CreateScope,
                {
                    <Self as #toasty::CreateScope>::create_in_scope(self)
                }
            }

            impl Many<#toasty::Direct> {
                /// Add an item to the association
                #vis async fn insert(self, executor: &mut dyn #toasty::Executor, item: impl #toasty::IntoExpr<#model_ident>) -> #toasty::Result<()> {
                    executor.exec(self.stmt.insert(item)).await
                }

                /// Remove items from the association
                #vis async fn remove(self, executor: &mut dyn #toasty::Executor, item: impl #toasty::IntoExpr<#model_ident>) -> #toasty::Result<()> {
                    executor.exec(self.stmt.remove(item)).await
                }
            }

            impl<Kind> #toasty::IntoStatement for Many<Kind> {
                type Returning = #toasty::List<#model_ident>;

                fn into_statement(self) -> #toasty::Statement<#toasty::List<#model_ident>> {
                    use #toasty::IntoStatement;
                    self.stmt.into_statement()
                }
            }

            impl<Kind> One<Kind> {
                #vis fn from_stmt(stmt: #toasty::stmt::Query<#toasty::List<#model_ident>>) -> One<Kind> {
                    One {
                        stmt: stmt.one(),
                        _kind: std::marker::PhantomData,
                    }
                }

                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#model_ident> {
                    self.stmt.exec(executor).await
                }

                /// Create a new associated record.
                #vis fn create(self) -> <Self as #toasty::Scope>::Create
                where
                    Self: #toasty::CreateScope,
                {
                    <Self as #toasty::CreateScope>::create_in_scope(self)
                }
            }

            impl<Kind> #toasty::IntoStatement for One<Kind> {
                type Returning = #model_ident;

                fn into_statement(self) -> #toasty::Statement<#model_ident> {
                    use #toasty::IntoStatement;
                    self.stmt.into_statement()
                }
            }

            impl<Kind> OptionOne<Kind> {
                pub fn from_stmt(stmt: #toasty::stmt::Query<#toasty::List<#model_ident>>) -> OptionOne<Kind> {
                    OptionOne {
                        stmt: stmt.first(),
                        _kind: std::marker::PhantomData,
                    }
                }

                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#toasty::Option<#model_ident>> {
                    self.stmt.exec(executor).await
                }

                /// Create a new associated record.
                #vis fn create(self) -> <Self as #toasty::Scope>::Create
                where
                    Self: #toasty::CreateScope,
                {
                    <Self as #toasty::CreateScope>::create_in_scope(self)
                }
            }

            #[diagnostic::do_not_recommend]
            impl<Kind> #toasty::Scope for Many<Kind> {
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
            impl<Kind> #toasty::Scope for One<Kind> {
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
            impl<Kind> #toasty::Scope for OptionOne<Kind> {
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
            impl<Kind> #toasty::ValidateCreate for Many<Kind> {
                const CREATE_META: &'static #toasty::CreateMeta =
                    &<#model_ident as #toasty::Model>::CREATE_META;
            }

            #[diagnostic::do_not_recommend]
            impl #toasty::CreateScope for Many<#toasty::Direct> {
                fn create_in_scope(self) -> <Self as #toasty::Scope>::Create {
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt);
                    builder
                }
            }

            #[diagnostic::do_not_recommend]
            impl<Kind> #toasty::ValidateCreate for One<Kind> {
                const CREATE_META: &'static #toasty::CreateMeta =
                    &<#model_ident as #toasty::Model>::CREATE_META;
            }

            #[diagnostic::do_not_recommend]
            impl #toasty::CreateScope for One<#toasty::Direct> {
                fn create_in_scope(self) -> <Self as #toasty::Scope>::Create {
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt);
                    builder
                }
            }

            #[diagnostic::do_not_recommend]
            impl<Kind> #toasty::ValidateCreate for OptionOne<Kind> {
                const CREATE_META: &'static #toasty::CreateMeta =
                    &<#model_ident as #toasty::Model>::CREATE_META;
            }

            #[diagnostic::do_not_recommend]
            impl #toasty::CreateScope for OptionOne<#toasty::Direct> {
                fn create_in_scope(self) -> <Self as #toasty::Scope>::Create {
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt);
                    builder
                }
            }
        }
    }

    /// For each relation field on this model, emit a method on `Many` that
    /// chains the field as the next path step and returns the target's
    /// `ViaMany`.
    /// This is the runtime analog of [`expand_list_relation_field_method`] on
    /// the field-list builder — a list-context traversal always yields a list,
    /// so all relation kinds (HasMany / HasOne / BelongsTo) flatten to the
    /// target's `ViaMany`.
    pub(super) fn expand_many_chain_methods(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;

        self.model
            .fields
            .iter()
            .filter_map(|field| {
                let (ty, field_trait) = match &field.ty {
                    FieldTy::BelongsTo(rel) => (&rel.ty, quote!(#toasty::RelationOneField)),
                    FieldTy::HasMany(rel) => (&rel.ty, quote!(#toasty::RelationManyField)),
                    FieldTy::HasOne(rel) => (&rel.ty, quote!(#toasty::RelationOneField)),
                    // ItemParent traversal yields a single parent (one-to-one
                    // membership), so it slots into the `RelationOneField`
                    // chain shape. Schema-build promotes the parent's `Has`
                    // to `HasItems` and lowering picks the right path; the
                    // macro just lifts the field through `Many`.
                    FieldTy::ItemParent(rel) => (&rel.ty, quote!(#toasty::RelationOneField)),
                    FieldTy::Primitive(_) => return None,
                };
                let field_ident = &field.name.ident;
                let field_offset = util::int(field.id);

                Some(quote! {
                    #vis fn #field_ident(self) -> <<#ty as #field_trait>::Model as #toasty::Model>::ViaMany {
                        <<<#ty as #field_trait>::Model as #toasty::Model>::ViaMany>::from_stmt(
                            self.stmt.chain_field(#field_offset)
                        )
                    }
                })
            })
            .collect()
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
                FieldTy::ItemParent(rel) => {
                    Some(self.expand_model_relation_item_parent_method(rel, field))
                }
                FieldTy::Primitive(_) => None,
            })
            .collect()
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
        let target_ty = quote!(<#ty as #toasty::RelationOneField>::Target);

        let operands = rel.foreign_key.iter().map(|fk_field| {
            let source = &self.model.fields[fk_field.source];
            let source_field_ident = &source.name.ident;
            let target_field = &fk_field.target;

            // `fields().#target_field()` returns the target field's
            // `<Field>::Path<Origin>` — `Path<Origin, T>` for primitives and a
            // wrapping `{Embed}Fields<Origin>` for embedded types. Both
            // convert into `Path<Origin, T>` via `Into`, which is what
            // `key_constraint` expects.
            quote! {
                #toasty::Field::key_constraint(
                    &self.#source_field_ident,
                    #target_ty::fields().#target_field().into(),
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
            #vis fn #field_ident(&self) -> <#ty as #toasty::RelationOneField>::One {
                // Suppress the unused field warning
                if false {
                    let _ = &self.#field_ident;
                }

                <#ty as #toasty::RelationOneField>::make_one(#target_ty::filter(#filter))
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

    /// Emit the `child.parent()` accessor for an `#[item_parent]` field.
    ///
    /// Unlike [`expand_model_relation_belongs_to_method`], item-parent
    /// navigation does not lower to a value-equality FK join — an
    /// item-collection child stores its parent's identity in its own
    /// partition + sort keys (R2.9). The generated method runs a
    /// partition-scoped query against the parent type, scoped by the
    /// child's own partition value and a sort-key prefix on the parent's
    /// upper-camel-case name (e.g. `"Tenant#"`).
    ///
    /// ```ignore
    /// // For:  user: Deferred<Tenant>  (with `#[key(account, sk)]`)
    /// // emits:
    /// pub fn tenant(&self) -> One<Tenant> {
    ///     Tenant::filter(
    ///         Tenant::fields().account().eq(&self.account)
    ///             .and(Tenant::fields().sk().starts_with("Tenant#".to_string()))
    ///     ).into_statement().into_query().unwrap().into()
    /// }
    /// ```
    fn expand_model_relation_item_parent_method(
        &self,
        rel: &ItemParent,
        field: &Field,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_ident = &field.name.ident;
        let ty = &rel.ty;
        let target_ty = quote!(<#ty as #toasty::RelationOneField>::Model);

        // Partition + sort field idents on the *current* model. By
        // construction, `#[item_parent]` is only allowed on item-collection
        // children, which always have a 2-field PK whose first entry is the
        // partition key and second entry is the sort key. The schema-build
        // validator enforces that contract — the macro relies on the order
        // assembled by `Model::from_ast`.
        let root = self.model.kind.as_root_unwrap();
        let pk = &root.primary_key.fields;
        assert!(
            pk.len() >= 2,
            "item-collection child must have a (partition, sort) primary key; \
             schema-build validation rejects single-field PKs on item-parent models"
        );
        let partition_field_ident = &self.model.fields[pk[0]].name.ident;
        let sort_field_ident = &self.model.fields[pk[1]].name.ident;

        // Parent type's UpperCamelCase ident — the `T` from `Deferred<T>` —
        // surfaces the prefix literal `"<Parent>#"` that the parent's sort
        // key always begins with (sk auto-mint, R7.5).
        let parent_ident = parent_ident_from_deferred(ty);
        let prefix_lit = format!("{parent_ident}#");

        // The `expand_pair_belongs_to_check` mechanism on the parent's
        // `#[has_many]` / `#[has_one]` accessor calls a method named
        // `verify_pair_belongs_to_exists_for_<field>` on the target. The
        // legacy name predates `ItemParent`; emit it here so the parent's
        // pair check resolves the same way as for a `BelongsTo` pair.
        let verify_pair_belongs_to_exists = syn::Ident::new(
            &format!("verify_pair_belongs_to_exists_for_{field_ident}"),
            field_ident.span(),
        );

        quote! {
            #vis fn #field_ident(&self) -> <#ty as #toasty::RelationOneField>::One {
                // Suppress the unused field warning on the marker field
                // itself; lowering reads its identity through its FieldTy,
                // not its in-memory value.
                if false {
                    let _ = &self.#field_ident;
                }

                {
                    use #toasty::IntoStatement;
                    let __filter = #toasty::stmt::Expr::and(
                        #target_ty::fields().#partition_field_ident().eq(&self.#partition_field_ident),
                        #target_ty::fields().#sort_field_ident().starts_with(::std::string::String::from(#prefix_lit)),
                    );
                    <<#ty as #toasty::RelationOneField>::One>::from_stmt(
                        #target_ty::filter(__filter).into_statement().into_query().unwrap()
                    )
                }
            }

            #[doc(hidden)]
            #vis fn #verify_pair_belongs_to_exists(&self) -> &#ty {
                let _ = &self.#field_ident;
                &self.#field_ident
            }
        }
    }

    fn expand_model_relation_has_many_method(&self, rel: &HasMany, field: &Field) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_ident = &field.name.ident;
        let ty = &rel.ty;
        // A `via` relation reaches its terminal through a path of existing
        // relations rather than a single foreign key. It routes through
        // `ViaTarget` (keyed on the terminal element type) so the
        // navigation method works whether the terminal is a model — keeping
        // the rich `QueryMany<M>` builder — or a scalar field, where it yields
        // a plain `Query<List<scalar>>` projecting that field.
        if rel.via.is_some() {
            // The association references the via field; its target and (for a
            // scalar terminal) terminal projection are resolved during lowering
            // from the schema, so nothing here needs the path's shape. The
            // declared element type is validated against the path by the typed
            // accessor and by the `schema()` expansion's terminal pin.
            return quote! {
                #vis fn #field_ident(&self) -> #toasty::ViaMany<#ty> {
                    // Suppress the unused field warning
                    if false {
                        let _ = &self.#field_ident;
                    }

                    {
                        use #toasty::IntoStatement;
                        let __source = self.into_statement().into_query().unwrap().to_list();
                        // The accessor returns the terminal's `ViaTarget::Path`
                        // (a `ManyField` for a model terminal, a `Path` for a
                        // scalar); `.into()` collapses either into the `Path` the
                        // association needs.
                        let __assoc = #toasty::stmt::Association::from_source_and_path(
                            __source,
                            Self::fields().#field_ident().into(),
                        );
                        <<#ty as #toasty::ViaManyField>::Target as #toasty::ViaTarget>::make_via_query(__assoc)
                    }
                }
            };
        }

        let target = quote!(<#ty as #toasty::RelationManyField>::Target);

        let pair_ident = rel.pair.clone().unwrap_or(syn::Ident::new(
            &self.model.name.ident.to_string(),
            rel.span,
        ));
        let pair_check = self.expand_pair_belongs_to_check(PairBelongsToCheck {
            pair_ident: &pair_ident,
            field_ident,
            ty,
            field_trait: quote!(#toasty::RelationManyField),
            rel_span: rel.span,
            relation_kind: "HasMany",
            label: "Has many associations require the target to include a back-reference",
        });

        quote! {
            #vis fn #field_ident(&self) -> #toasty::QueryMany<#target> {
                // Suppress the unused field warning
                if false {
                    let _ = &self.#field_ident;
                }

                #pair_check

                {
                    use #toasty::IntoStatement;
                    <#toasty::QueryMany<#target>>::from_assoc_many(
                        #toasty::stmt::Association::many(
                            self.into_statement().into_query().unwrap().to_list(),
                            Self::fields().#field_ident().into(),
                        ),
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
        let target = quote!(<#ty as #toasty::RelationOneField>::Target);

        // A `via` relation reaches its target through a path of existing
        // relations; it has no paired `BelongsTo`, so skip the back-reference
        // check that direct has-one relations emit.
        let pair_check = if rel.via.is_some() {
            quote! {}
        } else {
            let pair_ident = rel.pair.clone().unwrap_or(syn::Ident::new(
                &self.model.name.ident.to_string(),
                rel.span,
            ));
            self.expand_pair_belongs_to_check(PairBelongsToCheck {
                pair_ident: &pair_ident,
                field_ident,
                ty,
                field_trait: quote!(#toasty::RelationOneField),
                rel_span: rel.span,
                relation_kind: "HasOne",
                label: "Has one associations require the target to include a back-reference",
            })
        };

        quote! {
            #vis fn #field_ident(&self) -> <#ty as #toasty::RelationOneField>::One {
                // Suppress the unused field warning
                if false {
                    let _ = &self.#field_ident;
                }

                #pair_check

                {
                    use #toasty::IntoStatement;
                    let assoc = #toasty::stmt::Association::one(
                        self.into_statement().into_query().unwrap().to_list(),
                        Self::fields().#field_ident().into(),
                    );
                    let query = <#target as #toasty::Model>::wrap_query(
                        assoc.into_statement().into_query().unwrap(),
                    );
                    <#ty as #toasty::RelationOneField>::make_one(query)
                }
            }
        }
    }

    /// Emit a compile-time check that the target model has a
    /// `Deferred<Self>` (or `Deferred<Option<Self>>`) belongs-to field named
    /// `pair_ident`. Shared by the
    /// has-many and has-one accessor expansions; the relation kind ("HasMany"
    /// / "HasOne") and `label` are woven into the `on_unimplemented` diagnostic.
    fn expand_pair_belongs_to_check(&self, check: PairBelongsToCheck<'_>) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        let PairBelongsToCheck {
            pair_ident,
            field_ident,
            ty,
            field_trait,
            rel_span,
            relation_kind,
            label,
        } = check;

        let verify_pair_belongs_to_exists_for_field = syn::Ident::new(
            &format!("verify_pair_belongs_to_exists_for_{pair_ident}"),
            field_ident.span(),
        );

        let verify_a = super::util::ident("A");
        let verify_t = super::util::ident("T");

        let msg = format!(
            "{relation_kind} requires the {{{verify_a}}}::{pair_ident} field to be a relation to `Self`, but it was `{{Self}}` instead"
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
                impl<#verify_a> Verify<#verify_a> for #toasty::Deferred<#model_ident> {
                }

                #[diagnostic::do_not_recommend]
                impl<#verify_a> Verify<#verify_a> for #toasty::Deferred<Option<#model_ident>> {
                }

                #[diagnostic::do_not_recommend]
                impl<#verify_a> Verify<#verify_a> for #model_ident {
                }

                #[diagnostic::do_not_recommend]
                impl<#verify_a> Verify<#verify_a> for Option<#model_ident> {
                }

                fn verify<#verify_t: Verify<#verify_a>, #verify_a>(_: &#verify_t) {
                }

                let instance = load::<<#ty as #field_trait>::Target>();
                verify::<_, <#ty as #field_trait>::Target>(instance.#verify_pair_belongs_to_exists_for_field());
            }
        }
    }
}

/// Pull the parent type ident (e.g. `Tenant`) out of a `Deferred<Tenant>`
/// type. The macro's `extract_deferred_inner` validation runs in
/// `Field::from_ast`, so by the time we land here the type is shaped
/// `Deferred<...>`; we only need the trailing path segment of `T`.
///
/// Falls back to a `proc_macro2::Span::call_site` ident (`Parent`) when
/// the inner is not a path — that branch is unreachable for valid
/// `#[item_parent]` fields but keeps the macro from panicking on
/// malformed input.
fn parent_ident_from_deferred(ty: &syn::Type) -> syn::Ident {
    let syn::Type::Path(type_path) = ty else {
        return syn::Ident::new("Parent", proc_macro2::Span::call_site());
    };

    // Walk Deferred<...> to its inner T.
    let last = type_path
        .path
        .segments
        .last()
        .expect("Deferred path has at least one segment");
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return syn::Ident::new("Parent", proc_macro2::Span::call_site());
    };
    let Some(syn::GenericArgument::Type(syn::Type::Path(inner_path))) = args.args.first() else {
        return syn::Ident::new("Parent", proc_macro2::Span::call_site());
    };

    inner_path
        .path
        .segments
        .last()
        .expect("inner type path has at least one segment")
        .ident
        .clone()
}
