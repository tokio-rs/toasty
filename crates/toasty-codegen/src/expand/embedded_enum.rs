use super::{schema, util, Expand};
use crate::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    /// Returns fields belonging to a specific variant index.
    fn variant_fields(&self, variant_index: usize) -> Vec<&crate::schema::Field> {
        self.model
            .fields
            .iter()
            .filter(|f| f.variant == Some(variant_index))
            .collect()
    }

    /// True when at least one variant carries data fields, which changes
    /// what `Primitive::ty()` returns.
    pub(super) fn expand_enum_has_data_variants(&self) -> bool {
        !self.model.fields.is_empty()
    }

    /// Generates tokens for an `is_variant(path, variant_id)` expression.
    /// Reused by `is_{variant}()` methods and the `matches()` method.
    fn expand_is_variant_expr(&self, variant_idx: &TokenStream) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        quote! {
            {
                let path_stmt: #toasty::core::stmt::Expr = {
                    let p: #toasty::core::stmt::Path = self.path().into();
                    p.into_stmt()
                };
                let variant_id = #toasty::core::schema::app::VariantId {
                    model: <#model_ident as #toasty::Register>::id(),
                    index: #variant_idx,
                };
                #toasty::stmt::Expr::from_untyped(
                    #toasty::core::stmt::Expr::is_variant(path_stmt, variant_id)
                )
            }
        }
    }

    /// Generates delegated comparison methods (`eq`, `ne`, `gt`, `ge`, `lt`,
    /// `le`, `in_set`) that forward to `self.path()`.
    fn expand_comparison_methods(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;

        let methods = ["eq", "ne", "gt", "ge", "lt", "le"].iter().map(|name| {
            let method_ident = syn::Ident::new(name, proc_macro2::Span::call_site());
            quote! {
                #vis fn #method_ident(&self, rhs: impl #toasty::stmt::IntoExpr<#model_ident>) -> #toasty::stmt::Expr<bool> {
                    self.path().#method_ident(rhs)
                }
            }
        });

        quote! {
            #( #methods )*

            #vis fn in_set(&self, rhs: impl #toasty::stmt::IntoExpr<[#model_ident]>) -> #toasty::stmt::Expr<bool> {
                self.path().in_set(rhs)
            }
        }
    }

    /// Generates the `{Enum}Fields` struct for embedded enums with
    /// `is_{variant}()` methods, variant accessor methods, and delegated
    /// comparison methods. Also generates per-variant field structs for
    /// data-carrying variants (e.g., `ContactInfoEmailFields`).
    pub(super) fn expand_enum_field_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let embedded_enum = self.model.kind.expect_embedded_enum();
        let field_struct_ident = &embedded_enum.field_struct_ident;

        let is_variant_methods: Vec<_> = embedded_enum
            .variants
            .iter()
            .enumerate()
            .map(|(variant_index, variant)| {
                let method_name = &variant.is_method_ident;
                let variant_idx = util::int(variant_index);
                let is_variant_check = self.expand_is_variant_expr(&variant_idx);

                quote! {
                    #vis fn #method_name(&self) -> #toasty::stmt::Expr<bool> {
                        #is_variant_check
                    }
                }
            })
            .collect();

        // Generate variant accessor methods for data-carrying variants.
        let variant_accessor_methods: Vec<_> = embedded_enum
            .variants
            .iter()
            .filter(|v| v.variant_handle_ident.is_some())
            .map(|variant| {
                let method_name = &variant.name.ident;
                let variant_handle_ident = variant.variant_handle_ident.as_ref().unwrap();

                quote! {
                    #vis fn #method_name(&self) -> #variant_handle_ident {
                        #variant_handle_ident {
                            path: self.path()
                        }
                    }
                }
            })
            .collect();

        // Generate per-variant handle + field structs for data-carrying variants.
        let variant_field_structs: Vec<_> = embedded_enum
            .variants
            .iter()
            .enumerate()
            .filter(|(_, v)| v.variant_handle_ident.is_some())
            .map(|(variant_index, variant)| {
                let variant_handle_ident = variant.variant_handle_ident.as_ref().unwrap();
                let variant_field_struct_ident = variant.field_struct_ident.as_ref().unwrap();
                let variant_idx = util::int(variant_index);
                let is_variant_check = self.expand_is_variant_expr(&variant_idx);

                let field_methods: Vec<_> = self
                    .variant_fields(variant_index)
                    .iter()
                    .enumerate()
                    .map(|(field_index, field)| {
                        let field_ident = &field.name.ident;
                        let field_ty = expect_primitive_ty(field);
                        let field_offset = util::int(field_index);
                        self.expand_primitive_field_method(field_ident, field_ty, &field_offset)
                    })
                    .collect();

                quote! {
                    #vis struct #variant_handle_ident {
                        path: #toasty::Path<#model_ident>,
                    }

                    impl #variant_handle_ident {
                        fn path(&self) -> #toasty::Path<#model_ident> {
                            self.path.clone()
                        }

                        #vis fn matches(
                            &self,
                            f: impl FnOnce(#variant_field_struct_ident) -> #toasty::stmt::Expr<bool>,
                        ) -> #toasty::stmt::Expr<bool> {
                            let is_var: #toasty::stmt::Expr<bool> = #is_variant_check;
                            let variant_id = #toasty::core::schema::app::VariantId {
                                model: <#model_ident as #toasty::Register>::id(),
                                index: #variant_idx,
                            };
                            let fields = #variant_field_struct_ident {
                                path: self.path().into_variant(variant_id),
                            };
                            let body = f(fields);
                            is_var.and(body)
                        }
                    }

                    #vis struct #variant_field_struct_ident {
                        path: #toasty::Path<#model_ident>,
                    }

                    impl #variant_field_struct_ident {
                        fn path(&self) -> #toasty::Path<#model_ident> {
                            self.path.clone()
                        }

                        #( #field_methods )*
                    }
                }
            })
            .collect();

        let comparison_methods = self.expand_comparison_methods();

        quote! {
            #vis struct #field_struct_ident {
                path: #toasty::Path<#model_ident>,
            }

            impl #field_struct_ident {
                fn path(&self) -> #toasty::Path<#model_ident> {
                    self.path.clone()
                }

                #( #is_variant_methods )*

                #( #variant_accessor_methods )*

                #comparison_methods
            }

            #( #variant_field_structs )*
        }
    }

    /// Generates the `EnumVariant` schema structs (without fields — fields are
    /// stored at the `EmbeddedEnum` level).
    pub(super) fn expand_enum_variants(&self) -> Vec<TokenStream> {
        let toasty = &self.toasty;
        let embedded_enum = self.model.kind.expect_embedded_enum();

        embedded_enum
            .variants
            .iter()
            .map(|variant| {
                let variant_name = schema::expand_name(toasty, &variant.name);
                let discriminant = variant.discriminant;
                quote! {
                    #toasty::schema::app::EnumVariant {
                        name: #variant_name,
                        discriminant: #discriminant,
                    }
                }
            })
            .collect()
    }

    /// Generates the flat list of `Field` schema tokens for all variant fields,
    /// with each field tagged with its `VariantId`.
    pub(super) fn expand_enum_schema_fields(&self) -> Vec<TokenStream> {
        let toasty = &self.toasty;

        self.model
            .fields
            .iter()
            .map(|field| {
                let index = util::int(field.id);
                let app_name = field.name.ident.to_string();
                let ty = expect_primitive_ty(field);
                let variant_index = field.variant.expect("enum field must have variant");
                let variant_idx = util::int(variant_index);
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
                        variant: Some(#toasty::core::schema::app::VariantId {
                            model: id,
                            index: #variant_idx,
                        }),
                    }
                }
            })
            .collect()
    }

    /// Generates match arms for the `Value::I64(d)` branch of `Primitive::load`.
    /// Only unit variants are emitted here; data variants appear in `expand_enum_data_load_arms`.
    pub(super) fn expand_enum_unit_load_arms(&self) -> Vec<TokenStream> {
        let model_ident = &self.model.ident;
        let embedded_enum = self.model.kind.expect_embedded_enum();

        embedded_enum
            .variants
            .iter()
            .enumerate()
            .filter(|(variant_index, _)| self.variant_fields(*variant_index).is_empty())
            .map(|(_, variant)| {
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
        let embedded_enum = self.model.kind.expect_embedded_enum();

        embedded_enum
            .variants
            .iter()
            .enumerate()
            .filter(|(variant_index, _)| !self.variant_fields(*variant_index).is_empty())
            .map(|(variant_index, variant)| {
                let ident = &variant.ident;
                let discriminant = variant.discriminant;
                let fields = self.variant_fields(variant_index);

                let field_loads: Vec<_> = fields.iter().enumerate().map(|(i, field)| {
                    let field_ident = &field.name.ident;
                    let ty = expect_primitive_ty(field);
                    let record_pos = util::int(i + 1);
                    let load = quote! { <#ty as #toasty::stmt::Primitive>::load(record[#record_pos].take())? };
                    if variant.fields_named {
                        quote! { #field_ident: #load, }
                    } else {
                        quote! { #load, }
                    }
                }).collect();

                let construction = if variant.fields_named {
                    quote! { #model_ident::#ident { #( #field_loads )* } }
                } else {
                    quote! { #model_ident::#ident( #( #field_loads )* ) }
                };

                quote! {
                    #discriminant => Ok(#construction),
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
        let embedded_enum = self.model.kind.expect_embedded_enum();

        embedded_enum
            .variants
            .iter()
            .enumerate()
            .map(|(variant_index, variant)| {
                let ident = &variant.ident;
                let discriminant = variant.discriminant;
                let fields = self.variant_fields(variant_index);

                let discriminant_expr = quote!(#toasty::core::stmt::Expr::Value(
                    #toasty::core::stmt::Value::I64(#discriminant)
                ));

                if fields.is_empty() {
                    // In a mixed enum (has_data_variants), the model value is always a
                    // Record so that `project([0])` uniformly extracts the discriminant
                    // for the `model_to_table` mapping.  Unit-only enums keep the plain
                    // I64 form (cheaper and the load path expects it).
                    if self.expand_enum_has_data_variants() {
                        quote! {
                            #model_ident::#ident => #toasty::stmt::Expr::from_untyped(
                                #toasty::core::stmt::Expr::record([#discriminant_expr])
                            ),
                        }
                    } else {
                        quote! {
                            #model_ident::#ident => #toasty::stmt::Expr::from_untyped(
                                #discriminant_expr
                            ),
                        }
                    }
                } else {
                    let field_idents: Vec<_> = fields.iter().map(|f| &f.name.ident).collect();

                    let pattern = if variant.fields_named {
                        quote! { #model_ident::#ident { #( #field_idents ),* } }
                    } else {
                        quote! { #model_ident::#ident( #( #field_idents ),* ) }
                    };

                    let field_exprs = fields.iter().map(|field| {
                        let field_ident = &field.name.ident;
                        let ty = expect_primitive_ty(field);
                        self.expand_into_untyped_expr(ty, field_ident)
                    });

                    quote! {
                        #pattern =>
                            #toasty::stmt::Expr::from_untyped(
                                #toasty::core::stmt::Expr::record([
                                    #discriminant_expr,
                                    #( #field_exprs ),*
                                ])
                            ),
                    }
                }
            })
            .collect()
    }

    /// Generates the `Primitive::ty()` return expression. Unit-only enums map to
    /// `Type::I64`; enums with at least one data variant map to `Type::Model`.
    pub(super) fn expand_enum_primitive_ty(&self) -> TokenStream {
        let toasty = &self.toasty;
        if self.expand_enum_has_data_variants() {
            quote! { #toasty::Type::Model(<Self as #toasty::Register>::id()) }
        } else {
            quote! { #toasty::Type::I64 }
        }
    }
}

fn expect_primitive_ty(field: &crate::schema::Field) -> &syn::Type {
    match &field.ty {
        FieldTy::Primitive(ty) => ty,
        _ => panic!("expected primitive field type for enum variant field"),
    }
}
