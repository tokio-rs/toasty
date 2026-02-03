extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn;

#[proc_macro_derive(
    Model,
    attributes(key, auto, column, index, unique, table, has_many, has_one, belongs_to)
)]
pub fn derive_model(input: TokenStream) -> TokenStream {
    match toasty_codegen::generate(input.into()) {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(Embed, attributes(column))]
pub fn derive_embed(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        const _: () = {
            use toasty as _toasty;

            impl _toasty::Model for #name {
                type Query = ();
                type Create = ();
                type Update<'a> = ();
                type UpdateQuery = ();

                fn id() -> _toasty::codegen_support::ModelId {
                    _toasty::codegen_support::generate_unique_id()
                }

                fn load(_row: _toasty::codegen_support::ValueRecord) -> Result<Self, _toasty::Error> {
                    panic!("embedded types cannot be loaded directly")
                }

                fn schema() -> _toasty::codegen_support::schema::app::Model {
                    use _toasty::codegen_support::schema;

                    // For now, return a minimal embedded model with no fields
                    // This is a no-op registration
                    schema::app::Model {
                        id: Self::id(),
                        name: schema::Name::new(stringify!(#name)),
                        fields: vec![],
                        kind: schema::app::ModelKind::Embedded,
                        indices: vec![],
                        table_name: None,
                    }
                }
            }
        };
    };

    expanded.into()
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
