mod create;
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
            fn id() -> #toasty::ModelId {
                static ID: std::sync::OnceLock<#toasty::ModelId> = std::sync::OnceLock::new();
                *ID.get_or_init(|| #toasty::generate_unique_id())
            }

            #model_schema
        }

        impl #toasty::Embed for #model_ident {}

        impl #toasty::stmt::Primitive for #model_ident {
            type FieldAccessor = #field_struct_ident;
            type UpdateBuilder<'a> = #update_struct_ident<'a>;

            const NULLABLE: bool = false;

            fn ty() -> #toasty::Type {
                #toasty::Type::Model(Self::id())
            }

            fn load(value: #toasty::Value) -> #toasty::Result<Self> {
                #load_body
            }

            fn reload(&mut self, value: #toasty::Value) -> #toasty::Result<()> {
                #reload_body
            }

            fn make_field_accessor(path: #toasty::Path<Self>) -> Self::FieldAccessor {
                #field_struct_ident { path }
            }

            fn make_update_builder<'a>(
                stmt: &'a mut #toasty::core::stmt::Update,
                projection: #toasty::core::stmt::Projection,
            ) -> Self::UpdateBuilder<'a> {
                #update_struct_ident { stmt, projection }
            }

            fn field_ty(_storage_ty: Option<#toasty::schema::db::Type>) -> #toasty::schema::app::FieldTy {
                #toasty::schema::app::FieldTy::Embedded(
                    #toasty::schema::app::Embedded {
                        target: Self::id(),
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

fn wrap_in_const(code: TokenStream) -> TokenStream {
    quote! {
        const _: () = {
            use toasty as _toasty;
            #code
        };
    }
}
