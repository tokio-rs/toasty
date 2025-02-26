use super::{util, Expand};
use crate::schema::FieldTy::*;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_field_consts(&self) -> TokenStream {
        self.model
            .fields
            .iter()
            .enumerate()
            .map(move |(offset, field)| {
                let toasty = &self.toasty;
                let ident = &field.const_ident;
                let field_offset = util::int(offset);

                match &field.ty {
                    Primitive(primitive) => {
                        // let ty = self.ty(&primitive.ty, 0);
                        let ty = quote!(());

                        quote! {
                            pub const #ident: #toasty::Path<#ty> =
                                #toasty::Path::from_field_index::<Self>(#field_offset);
                        }
                    } /*
                      HasOne(_) | BelongsTo(_) => {
                          let target_struct_path = self.target_struct_path(field, 0);

                          quote! {
                              pub const #const_name: <#target_struct_path as Relation>::OneField =
                                  <#target_struct_path as Relation>::OneField::from_path(
                                      Path::from_field_index::<Self>(#field_offset)
                                  );
                          }
                      }
                      HasMany(_) => {
                          let target_struct_path = self.target_struct_path(field, 0);

                          quote! {
                              pub const #const_name: <#target_struct_path as Relation>::ManyField =
                                  <#target_struct_path as Relation>::ManyField::from_path(
                                      Path::from_field_index::<Self>(#field_offset)
                                  );
                          }
                      }
                      */
                }
            })
            .collect()
    }
}
