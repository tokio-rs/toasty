use crate::schema::Model;

use proc_macro2::TokenStream;
use quote::quote;

pub(super) fn model(model: &Model) -> TokenStream {
    let toasty = quote!(_toasty::codegen_support);
    let ident = &model.ident;
    let id = gen_model_id();

    println!("ID = {id}");

    let code = quote! {
        impl #toasty::Model for #ident {
            const ID: #toasty::ModelId = #toasty::ModelId(#id);
            type Key = ();

            fn load(row: #toasty::ValueRecord) -> Result<Self, #toasty::Error> {
                todo!()
            }
        }
    };

    wrap_in_const(code)
}

fn gen_model_id() -> usize {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    COUNT.fetch_add(1, Ordering::Relaxed)
}

fn wrap_in_const(code: TokenStream) -> TokenStream {
    quote! {
        const _: () = {
            use toasty as _toasty;
            #code
        };
    }
}
