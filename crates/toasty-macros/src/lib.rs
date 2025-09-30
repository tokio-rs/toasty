extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{Fields, Variant};

#[proc_macro_derive(
    Model,
    attributes(key, auto, db, index, unique, table, has_many, has_one, belongs_to)
)]
pub fn derive_model(input: TokenStream) -> TokenStream {
    match toasty_codegen::generate(input.into()) {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn compute_type_variant(enum_ident: &Ident, variant: &Variant) -> proc_macro2::TokenStream {
    let ident = &variant.ident;
    let fields = match variant.fields {
        Fields::Unit => quote!(Vec::new()),
        _ => todo!("fields unsupported vor enum #{ident}"),
    };
    quote!(EnumVariant { discriminant: #enum_ident::#ident as usize, fields: #fields })
}

fn discriminant_match(enum_ident: &Ident, variant: &Variant) -> proc_macro2::TokenStream {
    let ident = &variant.ident;
    quote!(
        if v == #enum_ident::#ident as usize {
            return Ok(#enum_ident::#ident);
        }
    )
}

#[proc_macro_derive(ToastyEnum)]
pub fn derive_enum(input: TokenStream) -> TokenStream {
    let item: syn::ItemEnum = syn::parse(input).unwrap();
    let ident = &item.ident;
    let compute_type_variants = item.variants.iter().map(|v| compute_type_variant(ident, v));
    let discriminant_matches = item.variants.iter().map(|v| discriminant_match(ident, v));
    quote! {
    const _: () = {
    use anyhow::Result;
    use toasty::{self, stmt::{Primitive, IntoExpr, Expr}};
    use toasty_core::{self, stmt::{self, EnumVariant, TypeEnum, ValueRecord}};
    impl Primitive for #ident {
        fn ty() -> stmt::Type {
            stmt::Type::Enum(TypeEnum {
                variants: vec![#(#compute_type_variants,)*]
            })
        }

        fn load(value: stmt::Value) -> Result<Self> {
            let stmt::Value::Enum(value_enum) = value else {
                anyhow::bail!("not an enum: #{value:#?}");
            };

            let v = value_enum.variant;
            #(#discriminant_matches)*
            anyhow::bail!("not matching any discriminant: #{v}");
        }
    }

    impl IntoExpr<#ident> for #ident {
        fn into_expr(self) -> Expr<#ident> {
            let variant = self as usize;
            Expr::from_untyped(stmt::Expr::Value(stmt::Value::Enum(stmt::ValueEnum {
                variant,
                fields: ValueRecord { fields: Vec::new() }
            })))
        }

        fn by_ref(&self) -> Expr<#ident> {
            todo!()
        }
    }
    };
    }
    .into()
}

#[proc_macro]
pub fn include_schema(_input: TokenStream) -> TokenStream {
    todo!()
}

#[proc_macro]
pub fn query(_input: TokenStream) -> TokenStream {
    quote!(println!("TODO")).into()
}

#[proc_macro]
pub fn create(_input: TokenStream) -> TokenStream {
    quote!(println!("TODO")).into()
}
