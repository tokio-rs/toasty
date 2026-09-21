use super::{Expand, util};
use crate::model::schema::{BelongsTo, Field, FieldTy, HasMany, HasOne};

use proc_macro2::TokenStream;
use quote::quote;

const MODEL_RESERVED_METHODS: &[&str] = &[
    "fields",
    "create",
    "create_many",
    "update",
    "all",
    "filter",
    "delete",
];

impl Expand<'_> {
    pub(super) fn expand_model_relation_methods(&self) -> TokenStream {
        self.model
            .fields
            .iter()
            .filter(|field| !util::ident_is_reserved(&field.name.ident, MODEL_RESERVED_METHODS))
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

        let doc = self.doc_belongs_to(field_ident);

        quote! {
            #[doc = #doc]
            #vis fn #field_ident(&self) -> <#ty as #toasty::RelationOneField>::One {
                // Suppress the unused field warning
                if false {
                    let _ = &self.#field_ident;
                    #( #suppress_unused_field_warnings )*
                }

                <#ty as #toasty::RelationOneField>::make_one(#target_ty::filter(#filter))
            }

        }
    }

    fn expand_model_relation_has_many_method(&self, rel: &HasMany, field: &Field) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_ident = &field.name.ident;
        let ty = &rel.ty;
        // A via relation uses its terminal's path shape: model terminals
        // support relation navigation, while scalar terminals are list leaves.
        if rel.via.is_some() {
            let doc = self.doc_has_many_via(field_ident);

            // The association references the via field; its target and (for a
            // scalar terminal) terminal projection are resolved during lowering
            // from the schema, so nothing here needs the path's shape. The
            // declared element type is validated against the path by the typed
            // accessor and by the `schema()` expansion's terminal pin.
            return quote! {
                #[doc = #doc]
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

        let doc = self.doc_has_many(field_ident);

        quote! {
            #[doc = #doc]
            #vis fn #field_ident(&self) -> #toasty::QueryMany<#target> {
                // Suppress the unused field warning
                if false {
                    let _ = &self.#field_ident;
                }

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

        let doc = self.doc_has_one(field_ident);

        quote! {
            #[doc = #doc]
            #vis fn #field_ident(&self) -> <#ty as #toasty::RelationOneField>::One {
                // Suppress the unused field warning
                if false {
                    let _ = &self.#field_ident;
                }

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
}
