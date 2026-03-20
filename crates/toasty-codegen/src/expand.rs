mod create;
mod embedded_enum;
mod fields;
mod filters;
mod model;
mod query;
mod relation;
mod schema;
mod update;
mod util;

use filters::Filter;

use crate::schema::Model;

use proc_macro2::TokenStream;
use quote::quote;

struct Expand<'a> {
    /// The model being expanded
    model: &'a Model,

    /// Model filter methods
    filters: Vec<Filter>,

    /// Path prefix for toasty types
    toasty: TokenStream,
}

impl Expand<'_> {
    fn expand(&self) -> TokenStream {
        let model_impls = self.expand_model_impls();
        let model_field_struct = self.expand_field_struct();
        let query_struct = self.expand_query_struct();
        let create_builder = self.expand_create_builder();
        let update_builder = self.expand_update_builder();
        let relation_structs = self.expand_relation_structs();

        wrap_in_const(quote! {
            #model_impls
            #model_field_struct
            #query_struct
            #create_builder
            #update_builder
            #relation_structs
        })
    }
}

pub(super) fn root_model(model: &Model) -> TokenStream {
    let toasty = quote!(_toasty::codegen_support);

    Expand {
        model,
        filters: Filter::build_model_filters(model),
        toasty,
    }
    .expand()
}

pub(super) fn embedded_model(model: &Model) -> TokenStream {
    let toasty = quote!(_toasty::codegen_support);
    let model_ident = &model.ident;
    let embedded = model.kind.expect_embedded();
    let field_struct_ident = &embedded.field_struct_ident;
    let update_struct_ident = &embedded.update_struct_ident;

    let expand = Expand {
        model,
        filters: vec![],
        toasty: toasty.clone(),
    };

    let model_schema = expand.expand_model_schema();
    let into_expr_body_val = expand.expand_embedded_into_expr_body(false);
    let into_expr_body_ref = expand.expand_embedded_into_expr_body(true);
    let load_body = expand.expand_load_body();
    let reload_body = expand.expand_embedded_reload_body();
    let embedded_field_struct = expand.expand_field_struct();
    let embedded_model_impls = expand.expand_embedded_model_impls();
    let embedded_update_builder = expand.expand_embedded_update_builder();

    wrap_in_const(quote! {
        #embedded_field_struct

        #embedded_update_builder

        #embedded_model_impls

        impl #toasty::Register for #model_ident {
            fn id() -> #toasty::core::schema::app::ModelId {
                static ID: std::sync::OnceLock<#toasty::core::schema::app::ModelId> = std::sync::OnceLock::new();
                *ID.get_or_init(|| #toasty::generate_unique_id())
            }

            #model_schema
        }

        impl #toasty::Embed for #model_ident {}

        impl #toasty::Field for #model_ident {
            type FieldAccessor<__Origin> = #field_struct_ident<__Origin>;
            type UpdateBuilder<'a> = #update_struct_ident<'a>;

            const NULLABLE: bool = false;

            fn ty() -> #toasty::core::stmt::Type {
                #toasty::core::stmt::Type::Model(<Self as #toasty::Register>::id())
            }

            fn load(value: #toasty::core::stmt::Value) -> #toasty::Result<Self> {
                #load_body
            }

            fn reload(&mut self, value: #toasty::core::stmt::Value) -> #toasty::Result<()> {
                #reload_body
            }

            fn make_field_accessor<__Origin>(path: #toasty::Path<__Origin, Self>) -> Self::FieldAccessor<__Origin> {
                #field_struct_ident { path }
            }

            fn make_update_builder<'a>(
                assignments: &'a mut #toasty::core::stmt::Assignments,
                projection: #toasty::core::stmt::Projection,
            ) -> Self::UpdateBuilder<'a> {
                #update_struct_ident { assignments, projection }
            }

            fn field_ty(
                _storage_ty: Option<#toasty::core::schema::db::Type>,
            ) -> #toasty::core::schema::app::FieldTy {
                #toasty::core::schema::app::FieldTy::Embedded(
                    #toasty::core::schema::app::Embedded {
                        target: <Self as #toasty::Register>::id(),
                        expr_ty: Self::ty(),
                    }
                )
            }
        }

        impl #toasty::stmt::IntoExpr<#model_ident> for #model_ident {
            fn into_expr(self) -> #toasty::stmt::Expr<#model_ident> {
                #into_expr_body_val
            }

            fn by_ref(&self) -> #toasty::stmt::Expr<#model_ident> {
                #into_expr_body_ref
            }
        }
    })
}

pub(super) fn embedded_enum(model: &Model) -> TokenStream {
    let toasty = quote!(_toasty::codegen_support);
    let model_ident = &model.ident;

    let e = Expand {
        model,
        filters: vec![],
        toasty: toasty.clone(),
    };

    let name = schema::expand_name(&toasty, &model.name);
    let variant_tokens = e.expand_enum_variants();
    let field_tokens = e.expand_enum_schema_fields();
    let indices = e.expand_model_indices();
    let unit_load_arms = e.expand_enum_unit_load_arms();
    let data_load_arms = e.expand_enum_data_load_arms();
    let into_expr_arms = e.expand_enum_into_expr_arms();
    let ty_expr = e.expand_enum_primitive_ty();

    let embedded_enum = model.kind.expect_embedded_enum();
    let field_struct_ident = &embedded_enum.field_struct_ident;
    let enum_field_struct = e.expand_enum_field_struct();

    wrap_in_const(quote! {
        #enum_field_struct

        impl #toasty::Register for #model_ident {
            fn id() -> #toasty::core::schema::app::ModelId {
                static ID: std::sync::OnceLock<#toasty::core::schema::app::ModelId> = std::sync::OnceLock::new();
                *ID.get_or_init(|| #toasty::generate_unique_id())
            }

            fn schema() -> #toasty::core::schema::app::Model {
                let id = Self::id();
                #toasty::core::schema::app::Model::EmbeddedEnum(
                    #toasty::core::schema::app::EmbeddedEnum {
                        id,
                        name: #name,
                        discriminant: #toasty::core::schema::app::FieldPrimitive {
                            ty: #toasty::core::stmt::Type::I64,
                            storage_ty: ::std::option::Option::None,
                            serialize: ::std::option::Option::None,
                        },
                        variants: vec![ #( #variant_tokens ),* ],
                        fields: vec![ #( #field_tokens ),* ],
                        indices: #indices,
                    }
                )
            }
        }

        impl #toasty::Embed for #model_ident {}

        impl #toasty::Field for #model_ident {
            type FieldAccessor<__Origin> = #field_struct_ident<__Origin>;
            type UpdateBuilder<'a> = ();

            fn ty() -> #toasty::core::stmt::Type {
                #ty_expr
            }

            fn load(value: #toasty::core::stmt::Value) -> #toasty::Result<Self> {
                match value {
                    #toasty::core::stmt::Value::I64(d) => match d {
                        #( #unit_load_arms )*
                        _ => Err(#toasty::Error::type_conversion(
                            #toasty::core::stmt::Value::I64(d),
                            stringify!(#model_ident),
                        )),
                    },
                    #toasty::core::stmt::Value::Record(mut record) => match record[0].take() {
                        #toasty::core::stmt::Value::I64(d) => match d {
                            #( #data_load_arms )*
                            _ => Err(#toasty::Error::type_conversion(
                                #toasty::core::stmt::Value::I64(d),
                                stringify!(#model_ident),
                            )),
                        },
                        other => Err(#toasty::Error::type_conversion(
                            other,
                            stringify!(#model_ident),
                        )),
                    },
                    value => Err(#toasty::Error::type_conversion(value, stringify!(#model_ident))),
                }
            }

            fn make_field_accessor<__Origin>(path: #toasty::Path<__Origin, Self>) -> Self::FieldAccessor<__Origin> {
                #field_struct_ident { path }
            }

            fn field_ty(
                _storage_ty: Option<#toasty::core::schema::db::Type>,
            ) -> #toasty::core::schema::app::FieldTy {
                #toasty::core::schema::app::FieldTy::Embedded(
                    #toasty::core::schema::app::Embedded {
                        target: <Self as #toasty::Register>::id(),
                        expr_ty: Self::ty(),
                    }
                )
            }
        }

        impl #toasty::stmt::IntoExpr<#model_ident> for #model_ident {
            fn into_expr(self) -> #toasty::stmt::Expr<#model_ident> {
                match self { #( #into_expr_arms )* }
            }

            fn by_ref(&self) -> #toasty::stmt::Expr<#model_ident> {
                match self { #( #into_expr_arms )* }
            }
        }
    })
}

// === Shared token-generation helpers ===

impl Expand<'_> {
    /// Generates a block that converts a Rust value into an untyped `core::stmt::Expr`
    /// via the typed `IntoExpr` trait.
    ///
    /// Produced token pattern:
    /// ```ignore
    /// {
    ///     let expr: Expr<T> = IntoExpr::into_expr(value);
    ///     let untyped: core::stmt::Expr = expr.into();
    ///     untyped
    /// }
    /// ```
    fn expand_into_untyped_expr(
        &self,
        ty: impl quote::ToTokens,
        value: impl quote::ToTokens,
    ) -> TokenStream {
        let toasty = &self.toasty;
        quote! {
            {
                let expr: #toasty::stmt::Expr<#ty> =
                    #toasty::stmt::IntoExpr::into_expr(#value);
                let untyped: #toasty::core::stmt::Expr = expr.into();
                untyped
            }
        }
    }

    /// Generates a field accessor method for a `BelongsTo` or `HasOne`
    /// relation using `Relation::OneField`.
    fn expand_one_relation_field_method(
        &self,
        field_ident: &syn::Ident,
        ty: &syn::Type,
        field_offset: &TokenStream,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;

        quote! {
            #vis fn #field_ident(&self) -> <#ty as #toasty::Relation>::OneField<__Origin> {
                <#ty as #toasty::Relation>::OneField::from_path(
                    self.path().chain(
                        #toasty::Path::<#model_ident, _>::from_field_index(#field_offset)
                    )
                )
            }
        }
    }

    /// Generates a field accessor method for a primitive field using the
    /// `Field::make_field_accessor` trait.
    fn expand_primitive_field_method(
        &self,
        field_ident: &syn::Ident,
        ty: &syn::Type,
        field_offset: &TokenStream,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;

        quote! {
            #vis fn #field_ident(&self) -> <#ty as #toasty::Field>::FieldAccessor<__Origin> {
                <#ty as #toasty::Field>::make_field_accessor(
                    self.path().chain(
                        #toasty::Path::<#model_ident, _>::from_field_index(#field_offset)
                    )
                )
            }
        }
    }
}

fn wrap_in_const(code: TokenStream) -> TokenStream {
    quote! {
        const _: () = {
            use toasty as _toasty;
            #code
        };
    }
}
