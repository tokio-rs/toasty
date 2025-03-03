use super::{util, Expand};
use crate::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_schema(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let id = &self.tokenized_id;
        let name = self.expand_model_name();
        let fields = self.expand_model_fields();

        quote! {
            #vis fn schema() -> #toasty::schema::app::Model {
                use #toasty::{schema::{app::*, Name}, Type};

                let id = #toasty::ModelId(#id);

                Model {
                    id,
                    name: #name,
                    fields: #fields,
                    primary_key: PrimaryKey {
                        fields: vec![],
                        query: QueryId(usize::MAX),
                        index: IndexId {
                            model: id,
                            index: 0,
                        },
                    },
                    queries: vec![],
                    indices: vec![],
                    table_name: None,
                }
            }
        }
    }

    fn expand_model_name(&self) -> TokenStream {
        let parts = self.model.name.parts.iter().map(|part| {
            let part = part.to_string();
            quote! { #part.to_string() }
        });

        quote! {
            Name {
                parts: vec![#( #parts ),*],
            }
        }
    }

    fn expand_model_fields(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_id = &self.tokenized_id;

        let fields = self.model.fields.iter().enumerate().map(|(index, field)| {
            let index = util::int(index);
            let name = field.name.ident.to_string();
            let ty = match &field.ty {
                FieldTy::Primitive(ty) => {
                    quote!(FieldTy::Primitive(FieldPrimitive {
                        ty: Type::primitive::<#ty>(),
                    }))
                }
            };

            quote! {
                Field {
                    id: FieldId {
                        model: #toasty::ModelId(#model_id),
                        index: #index,
                    },
                    name: #name.to_string(),
                    ty: #ty,
                    nullable: false,
                    primary_key: false,
                    auto: None,
                }
            }
        });

        quote! {
            vec![ #( #fields ),* ]
        }
    }
}
