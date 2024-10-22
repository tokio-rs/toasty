use toasty_core::schema::Model;

use proc_macro2::TokenStream;

pub struct Output<'a> {
    /// Per-model output
    pub models: Vec<ModelOutput<'a>>,
}

/// Generated code for a single model
pub struct ModelOutput<'a> {
    /// Model the output is associated with
    pub model: &'a Model,

    /// Module name
    pub module_name: syn::Ident,

    /// Body of the client module.
    pub body: TokenStream,
}
