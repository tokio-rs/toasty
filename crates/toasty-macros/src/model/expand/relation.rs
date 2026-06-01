use super::Expand;
use crate::model::schema::{BelongsTo, Field, FieldTy, HasMany, HasOne};

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
        let target_ty = quote!(<#ty as #toasty::RelationOneField>::Model);

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
        if let Some(segments) = &rel.via {
            let model_ident = &self.model.ident;
            let terminal_ty = quote!(#toasty::List<<#ty as #toasty::ViaManyField>::Target>);
            let full_path =
                super::schema::expand_via_path(toasty, model_ident, segments, &terminal_ty);
            let terminal_owner =
                super::schema::expand_via_terminal_owner(toasty, model_ident, segments);

            return quote! {
                #vis fn #field_ident(&self) -> <<#ty as #toasty::ViaManyField>::Target as #toasty::ViaTarget>::Query {
                    // Suppress the unused field warning
                    if false {
                        let _ = &self.#field_ident;
                    }

                    {
                        use #toasty::IntoStatement;
                        let __source = self.into_statement().into_query().unwrap().to_list();
                        let __assoc = #toasty::stmt::Association::from_source_and_path(
                            __source,
                            Self::fields().#field_ident(),
                        );
                        // The terminal field (scalar terminals only) is the via
                        // path's last step, on the model the relation chain
                        // reaches.
                        let __via_path: #toasty::core::stmt::Path = #full_path;
                        let __terminal = *__via_path
                            .projection
                            .as_slice()
                            .last()
                            .expect("via path has at least one step");
                        <<#ty as #toasty::ViaManyField>::Target as #toasty::ViaTarget>::make_via_query(
                            __assoc,
                            #terminal_owner,
                            __terminal,
                        )
                    }
                }
            };
        }

        let target = quote!(<#ty as #toasty::RelationManyField>::Model);

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
        let target = quote!(<#ty as #toasty::RelationOneField>::Model);

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

                let instance = load::<<#ty as #field_trait>::Model>();
                verify::<_, <#ty as #field_trait>::Model>(instance.#verify_pair_belongs_to_exists_for_field());
            }
        }
    }
}
