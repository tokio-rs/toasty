use super::Expand;
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

// `expand_relation_structs` and `expand_many_chain_methods` were removed
// when this branch rebased onto the new item-collection-4 base — that base
// generates the `Many<Kind>` / `One<Kind>` / `OptionOne<Kind>` relation scope
// types and their method bodies through a different code path. The surviving
// model-side accessors live in `expand_model_relation_methods` below.
impl Expand<'_> {
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
        let target_ty = quote!(<#ty as #toasty::RelationOneField>::Target);

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

                let __filter = #toasty::stmt::Expr::and(
                    #target_ty::fields().#partition_field_ident().eq(&self.#partition_field_ident),
                    #target_ty::fields().#sort_field_ident().starts_with(::std::string::String::from(#prefix_lit)),
                );
                <#ty as #toasty::RelationOneField>::make_one(#target_ty::filter(__filter))
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
