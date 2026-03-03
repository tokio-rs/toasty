use super::{schema, util, Expand};
use crate::schema::VariantField;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    /// True when at least one variant carries data fields, which changes
    /// what `Primitive::ty()` returns.
    pub(super) fn expand_enum_has_data_variants(&self) -> bool {
        self.model
            .kind
            .expect_embedded_enum()
            .variants
            .iter()
            .any(|v| !v.fields.is_empty())
    }

    /// Generates the `EnumVariant` schema structs with globally-assigned field indices.
    /// Field indices are unique across all variants (not per-variant).
    pub(super) fn expand_enum_variants(&self) -> Vec<TokenStream> {
        let toasty = &self.toasty;
        let embedded_enum = self.model.kind.expect_embedded_enum();
        let mut global_field_index = 0usize;

        embedded_enum
            .variants
            .iter()
            .map(|variant| {
                let variant_name = schema::expand_name(toasty, &variant.name);
                let discriminant = variant.discriminant;
                let field_tokens =
                    self.expand_enum_variant_field_tokens(&variant.fields, &mut global_field_index);
                quote! {
                    #toasty::schema::app::EnumVariant {
                        name: #variant_name,
                        discriminant: #discriminant,
                        fields: vec![ #( #field_tokens ),* ],
                    }
                }
            })
            .collect()
    }

    /// Generates `Field` schema tokens for one variant's fields, advancing
    /// `next_index` for each field so indices remain globally unique across
    /// all variants in the enum.
    fn expand_enum_variant_field_tokens(
        &self,
        fields: &[VariantField],
        next_index: &mut usize,
    ) -> Vec<TokenStream> {
        let toasty = &self.toasty;
        fields
            .iter()
            .map(|vf| {
                let index = util::int(*next_index);
                *next_index += 1;
                let app_name = vf.ident.to_string();
                let ty = &vf.ty;
                quote! {
                    #toasty::schema::app::Field {
                        id: #toasty::schema::app::FieldId {
                            model: id,
                            index: #index,
                        },
                        name: #toasty::schema::app::FieldName {
                            app_name: #app_name.to_string(),
                            storage_name: None,
                        },
                        ty: <#ty as #toasty::stmt::Primitive>::field_ty(None),
                        nullable: <#ty as #toasty::stmt::Primitive>::NULLABLE,
                        primary_key: false,
                        auto: None,
                        constraints: vec![],
                    }
                }
            })
            .collect()
    }

    /// Generates match arms for the `Value::I64(d)` branch of `Primitive::load`.
    /// Only unit variants are emitted here; data variants appear in `expand_enum_data_load_arms`.
    pub(super) fn expand_enum_unit_load_arms(&self) -> Vec<TokenStream> {
        let model_ident = &self.model.ident;
        self.model
            .kind
            .expect_embedded_enum()
            .variants
            .iter()
            .filter(|v| v.fields.is_empty())
            .map(|variant| {
                let ident = &variant.ident;
                let discriminant = variant.discriminant;
                quote! { #discriminant => Ok(#model_ident::#ident), }
            })
            .collect()
    }

    /// Generates match arms for the `Value::Record` branch of `Primitive::load`.
    /// Only data variants are emitted; unit variants appear in `expand_enum_unit_load_arms`.
    /// Record layout: `record[0]` is the discriminant, `record[1..]` are the variant's fields
    /// in declaration order (local indices, not global).
    pub(super) fn expand_enum_data_load_arms(&self) -> Vec<TokenStream> {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        self.model
            .kind
            .expect_embedded_enum()
            .variants
            .iter()
            .filter(|v| !v.fields.is_empty())
            .map(|variant| {
                let ident = &variant.ident;
                let discriminant = variant.discriminant;
                if variant.fields_named {
                    let field_loads = variant.fields.iter().enumerate().map(|(i, vf)| {
                        let field_ident = &vf.ident;
                        let ty = &vf.ty;
                        let record_pos = util::int(i + 1);
                        quote! {
                            #field_ident: <#ty as #toasty::stmt::Primitive>::load(record[#record_pos].take())?,
                        }
                    });
                    quote! {
                        #discriminant => Ok(#model_ident::#ident { #( #field_loads )* }),
                    }
                } else {
                    let field_loads = variant.fields.iter().enumerate().map(|(i, vf)| {
                        let ty = &vf.ty;
                        let record_pos = util::int(i + 1);
                        quote! {
                            <#ty as #toasty::stmt::Primitive>::load(record[#record_pos].take())?,
                        }
                    });
                    quote! {
                        #discriminant => Ok(#model_ident::#ident( #( #field_loads )* )),
                    }
                }
            })
            .collect()
    }

    /// Generates match arms for `IntoExpr::into_expr` and `IntoExpr::by_ref`.
    /// Unit variants emit `Value::I64(discriminant)`. Data variants emit
    /// `Value::Record([I64(disc), field_exprs...])`. The same arms work for both
    /// methods: for `by_ref(&self)` match ergonomics bind field names as `&T`,
    /// and `IntoExpr` is implemented for both `T` and `&T`.
    pub(super) fn expand_enum_into_expr_arms(&self) -> Vec<TokenStream> {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        self.model
            .kind
            .expect_embedded_enum()
            .variants
            .iter()
            .map(|variant| {
                let ident = &variant.ident;
                let discriminant = variant.discriminant;
                if variant.fields.is_empty() {
                    // In a mixed enum (has_data_variants), the model value is always a
                    // Record so that `project([0])` uniformly extracts the discriminant
                    // for the `model_to_table` mapping.  Unit-only enums keep the plain
                    // I64 form (cheaper and the load path expects it).
                    if self.expand_enum_has_data_variants() {
                        quote! {
                            #model_ident::#ident => #toasty::stmt::Expr::from_untyped(
                                #toasty::core::stmt::Expr::record([
                                    #toasty::core::stmt::Expr::Value(
                                        #toasty::core::stmt::Value::I64(#discriminant)
                                    )
                                ])
                            ),
                        }
                    } else {
                        quote! {
                            #model_ident::#ident => #toasty::stmt::Expr::from_untyped(
                                #toasty::core::stmt::Expr::Value(
                                    #toasty::core::stmt::Value::I64(#discriminant)
                                )
                            ),
                        }
                    }
                } else {
                    let field_idents: Vec<_> = variant.fields.iter().map(|vf| &vf.ident).collect();
                    let disc_expr = quote! {
                        #toasty::core::stmt::Expr::Value(
                            #toasty::core::stmt::Value::I64(#discriminant)
                        )
                    };
                    let field_exprs = variant.fields.iter().map(|vf| {
                        let field_ident = &vf.ident;
                        let ty = &vf.ty;
                        quote! {
                            {
                                let expr: #toasty::stmt::Expr<#ty> =
                                    #toasty::stmt::IntoExpr::into_expr(#field_ident);
                                let untyped: #toasty::core::stmt::Expr = expr.into();
                                untyped
                            }
                        }
                    });
                    if variant.fields_named {
                        quote! {
                            #model_ident::#ident { #( #field_idents ),* } =>
                                #toasty::stmt::Expr::from_untyped(
                                    #toasty::core::stmt::Expr::record([
                                        #disc_expr,
                                        #( #field_exprs ),*
                                    ])
                                ),
                        }
                    } else {
                        quote! {
                            #model_ident::#ident( #( #field_idents ),* ) =>
                                #toasty::stmt::Expr::from_untyped(
                                    #toasty::core::stmt::Expr::record([
                                        #disc_expr,
                                        #( #field_exprs ),*
                                    ])
                                ),
                        }
                    }
                }
            })
            .collect()
    }

    /// Generates the `Primitive::ty()` return expression. Unit-only enums map to
    /// `Type::I64`; enums with at least one data variant map to `Type::Model`.
    pub(super) fn expand_enum_ty(&self) -> TokenStream {
        let toasty = &self.toasty;
        if self.expand_enum_has_data_variants() {
            quote! { #toasty::Type::Model(Self::id()) }
        } else {
            quote! { #toasty::Type::I64 }
        }
    }
}
