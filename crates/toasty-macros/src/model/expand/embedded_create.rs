use super::{Expand, util};
use crate::model::schema::{Field, FieldTy, ModelKind};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

impl Expand<'_> {
    pub(super) fn expand_foreign_key_references(
        &self,
        rel: &crate::model::schema::BelongsTo,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let ty = &rel.ty;
        let references = rel.foreign_key.iter().map(|fk| {
            let name = &fk.target;
            quote!(<<#ty as #toasty::RelationOneField>::Target as #toasty::Model>::field_name_to_id(stringify!(#name)))
        });
        quote!([#(#references),*])
    }
    pub(super) fn expand_embedded_create(&self) -> TokenStream {
        match &self.model.kind {
            ModelKind::EmbeddedStruct(_) => {
                self.embedded_create(None, self.model.fields.iter().collect())
            }
            ModelKind::EmbeddedEnum(model) => model
                .variants
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    self.embedded_create(
                        Some(index),
                        self.model
                            .fields
                            .iter()
                            .filter(|f| f.variant == Some(index))
                            .collect(),
                    )
                })
                .collect(),
            _ => unreachable!(),
        }
    }

    fn embedded_create(&self, variant: Option<usize>, fields: Vec<&Field>) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model = &self.model.ident;
        let (builder, constructor, discriminant) = if let Some(index) = variant {
            let variant = &self.model.kind.as_embedded_enum_unwrap().variants[index];
            (
                format_ident!("{}{}Create", model, variant.ident),
                format_ident!("__toasty_create_{}", variant.ident),
                Some(self.expand_discriminant_value_expr(&variant.attrs.discriminant)),
            )
        } else {
            (
                format_ident!("{}Create", model),
                format_ident!("__toasty_create"),
                None,
            )
        };
        let offset = usize::from(variant.is_some());
        let mut defaults = Vec::new();
        if let Some(discriminant) = discriminant {
            defaults.push(discriminant);
        }
        defaults.extend(
            fields
                .iter()
                .map(|_| quote!(#toasty::core::stmt::Expr::null())),
        );
        let methods = fields.iter().enumerate().map(|(index, field)| {
            let name = &field.name.ident;
            let index = util::int(index + offset);
            let (ty, expr) = match &field.ty {
                FieldTy::Primitive(ty) => (
                    quote!(FieldExprTarget<#ty>),
                    quote!(value.into_expr().into()),
                ),
                FieldTy::BelongsTo(rel) => {
                    let ty = &rel.ty;
                    let references = self.expand_foreign_key_references(rel);
                    (
                        quote!(<#ty as #toasty::RelationOneField>::Expr),
                        quote!(#toasty::core::stmt::Expr::record(#references.into_iter().map(|field| value.by_ref_field(field)))),
                    )
                }
                _ => unreachable!(),
            };
            quote! {
                #vis fn #name(mut self, value: impl #toasty::IntoExpr<#ty>) -> Self {
                    self.fields[#index] = #expr;
                    self
                }
            }
        });
        quote! {
            #vis struct #builder { fields: Vec<#toasty::core::stmt::Expr> }
            impl #model {
                #[doc(hidden)]
                #[allow(non_snake_case)]
                #vis fn #constructor() -> #builder { #builder { fields: vec![#(#defaults),*] } }
            }
            impl #builder { #(#methods)* }
            impl #toasty::IntoExpr<#model> for #builder {
                fn into_expr(self) -> #toasty::Expr<#model> { #toasty::Expr::from_untyped(#toasty::core::stmt::Expr::record(self.fields)) }
                fn by_ref(&self) -> #toasty::Expr<#model> { #toasty::Expr::from_untyped(#toasty::core::stmt::Expr::record(self.fields.clone())) }
            }
            impl #toasty::IntoExpr<Option<#model>> for #builder {
                fn into_expr(self) -> #toasty::Expr<Option<#model>> { <Self as #toasty::IntoExpr<#model>>::into_expr(self).cast() }
                fn by_ref(&self) -> #toasty::Expr<Option<#model>> { <Self as #toasty::IntoExpr<#model>>::by_ref(self).cast() }
            }
            impl #toasty::Assign<#model> for #builder {
                fn into_assignment(self) -> #toasty::stmt::Assignment<#model> {
                    #toasty::stmt::set(<Self as #toasty::IntoExpr<#model>>::into_expr(self))
                }
            }
            impl #toasty::Assign<Option<#model>> for #builder {
                fn into_assignment(self) -> #toasty::stmt::Assignment<Option<#model>> {
                    #toasty::stmt::set(<Self as #toasty::IntoExpr<Option<#model>>>::into_expr(self))
                }
            }
        }
    }
}
