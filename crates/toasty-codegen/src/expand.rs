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

    /// Tokenized model identifier
    tokenized_id: TokenStream,
}

impl Expand<'_> {
    fn expand(&self) -> TokenStream {
        let model_impls = self.expand_model_impls();
        let model_field_struct = self.expand_model_field_struct();
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

pub(super) fn model(model: &Model) -> TokenStream {
    let toasty = quote!(_toasty::codegen_support);
    let tokenized_id = util::int(model.id);

    Expand {
        model,
        filters: Filter::build_model_filters(model),
        toasty,
        tokenized_id,
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
