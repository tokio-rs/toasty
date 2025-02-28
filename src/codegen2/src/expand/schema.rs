use super::Expand;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_schema(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        /*
        let model_ident = &self.model.ident;
        let fields = self.expand_model_field_consts();

        quote! {
            impl #model_ident {
                #fields
            }
        }
        */

        quote! {
            #vis fn schema() -> #toasty::schema::app::Model {
                use #toasty::schema::{app::*, Name};

                /*
                Model {
                    id: ModelId(0),
                    name: Name {
                        parts: vec![],
                    },
                    fields: vec![],
                    primary_key: PrimaryKey {
                        fields: vec![],
                        query: QueryId::placeholder(),
                        index: ModelIndexId::placeholder(),
                    },
                    queries: vec![],
                    indices: vec![],
                    table_name: None,
                }
                */
                todo!()
            }
        }
    }
}
