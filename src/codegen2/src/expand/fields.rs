use super::{util, Expand};
use crate::schema::FieldTy::*;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_field_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_struct_ident = &self.model.field_struct_ident;

        let fields = self.model.fields.iter().map(move |field| {
            let field_ident = &field.name.ident;

            match &field.ty {
                    Primitive(ty) => {
                        quote! {
                            #vis #field_ident: #toasty::Path<#ty>,
                        }
                    }
                    BelongsTo(rel) /* | HasOne(_) */ => {
                        let ty = &rel.ty;

                        quote! {
                            #vis #field_ident: <#ty as #toasty::Relation>::OneField,
                        }
                    }
                    /*
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
        });

        quote!(
            #vis struct #field_struct_ident {
                #( #fields )*
            }
        )
    }

    pub(super) fn expand_model_field_struct_init(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_struct_ident = &self.model.field_struct_ident;

        let fields = self
            .model
            .fields
            .iter()
            .enumerate()
            .map(move |(offset, field)| {
                let field_ident = &field.name.ident;
                let field_offset = util::int(offset);

                match &field.ty {
                    Primitive(ty) => {
                        quote! {
                            #field_ident: #toasty::Path::from_field_index::<Self>(#field_offset),
                        }
                    }
                    BelongsTo(rel) /* | HasOne(_) */ => {
                        let ty = &rel.ty;

                        quote! {
                            #field_ident: <#ty as #toasty::Relation>::OneField::from_path(
                                #toasty::Path::from_field_index::<Self>(#field_offset)
                            ),
                        }
                    }
                    /*
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
            });

        quote!(
            #vis const FIELDS: #field_struct_ident = #field_struct_ident {
                #( #fields )*
            };
        )
    }
}
