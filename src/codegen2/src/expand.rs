mod create;
mod fields;
mod filters;
mod model;
mod query;
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
        let query_struct = self.expand_query_struct();
        let create_builder = self.expand_create_builder();

        wrap_in_const(quote! {
            #model_impls
            #query_struct
            #create_builder
        })
    }
}

pub(super) fn model(model: &Model) -> TokenStream {
    Expand {
        model,
        filters: Filter::build_model_filters(model),
        toasty: quote!(_toasty::codegen_support),
    }
    .expand()
}

fn wrap_in_const(code: TokenStream) -> TokenStream {
    quote! {
        const _: () = {
            use toasty as _toasty;
            #code
        };
    }
}
