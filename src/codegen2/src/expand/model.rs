use super::Expand;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_impls(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let id = gen_model_id();
        let field_consts = self.expand_model_field_consts();
        let query_struct_ident = &self.model.query_struct_ident;
        let create_builder_ident = &self.model.create_builder_struct_ident;

        /*
        let into_select_body_ref = self.gen_model_into_select_body(true);
        let into_select_body_value = self.gen_model_into_select_body(false);
        */

        quote! {
            impl #model_ident {
                #field_consts

                #vis fn create() -> #create_builder_ident {
                    #create_builder_ident::default()
                }

                #vis fn filter(expr: #toasty::stmt::Expr<bool>) -> #query_struct_ident {
                    #query_struct_ident::from_stmt(#toasty::stmt::Select::filter(expr))
                }

                #vis async fn delete(self, db: &#toasty::Db) -> #toasty::Result<()> {
                    use #toasty::IntoSelect;
                    let stmt = self.into_select().delete();
                    db.exec(stmt).await?;
                    Ok(())
                }
            }

            impl #toasty::Model for #model_ident {
                const ID: #toasty::ModelId = #toasty::ModelId(#id);

                fn load(row: #toasty::ValueRecord) -> #toasty::Result<Self> {
                    todo!()
                }
            }

            impl #toasty::stmt::IntoSelect for &#model_ident {
                type Model = #model_ident;

                fn into_select(self) -> stmt::Select<Self::Model> {
                    // #into_select_body_ref
                    todo!()
                }
            }

            impl stmt::IntoSelect for &mut #model_ident {
                type Model = #model_ident;

                fn into_select(self) -> stmt::Select<Self::Model> {
                    (&*self).into_select()
                }
            }

            impl stmt::IntoSelect for #model_ident {
                type Model = #model_ident;

                fn into_select(self) -> stmt::Select<Self::Model> {
                    // #into_select_body_value
                    todo!()
                }
            }
        }
    }

    pub(super) fn gen_model_into_select_body(&self, by_ref: bool) -> TokenStream {
        /*
        let fields = self
            .model
            .primary_key_fields()
            .map(|field| field.index)
            .collect::<Vec<_>>();

        let ident = self.filter_method_ident(&fields);
        let arg_idents = self.gen_filter_arg_idents(&fields);

        let amp = if by_ref { quote!(&) } else { quote!() };

        quote! {
            Query::default()
                .#ident( #( #amp self.#arg_idents ),* )
                .stmt
        }
        */
        quote! {
            todo!("implement `gen_model_into_select_body`");
        }
    }
}

fn gen_model_id() -> usize {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    COUNT.fetch_add(1, Ordering::Relaxed)
}
