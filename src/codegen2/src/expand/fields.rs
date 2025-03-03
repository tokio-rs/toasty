use super::{util, Expand};
use crate::schema::FieldTy::*;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_field_consts(&self) -> TokenStream {
        let toasty = &self.toasty;

        self.model
            .fields
            .iter()
            .enumerate()
            .map(move |(offset, field)| {
                let const_ident = &field.name.const_ident;
                let field_offset = util::int(offset);

                match &field.ty {
                    Primitive(ty) => {
                        quote! {
                            pub const #const_ident: #toasty::Path<#ty> =
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
