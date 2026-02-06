use super::{util, Expand};
use crate::schema::FieldTy::{BelongsTo, HasMany, HasOne, Primitive};
use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_field_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_struct_ident = self.field_struct_ident();
        let model_ident = &self.model.ident;

        // Generate methods that return field paths for the model
        let methods = self
            .model
            .fields
            .iter()
            .enumerate()
            .map(move |(offset, field)| {
                let field_ident = &field.name.ident;
                let field_offset = util::int(offset);

                match &field.ty {
                    Primitive(ty) => {
                        // Use the Primitive trait's FieldAccessor to determine the return type
                        // For primitives, this will be Path<T>
                        // For embedded types, this will be {Type}Fields
                        quote! {
                            #vis fn #field_ident(&self) -> <#ty as #toasty::stmt::Primitive>::FieldAccessor {
                                <#ty as #toasty::stmt::Primitive>::make_field_accessor(
                                    self.path().chain(#toasty::Path::from_field_index::<#model_ident>(#field_offset))
                                )
                            }
                        }
                    }
                    BelongsTo(rel) => {
                        let ty = &rel.ty;

                        quote! {
                            #vis fn #field_ident(&self) -> <#ty as #toasty::Relation>::OneField {
                                <#ty as #toasty::Relation>::OneField::from_path(
                                    self.path().chain(#toasty::Path::from_field_index::<#model_ident>(#field_offset))
                                )
                            }
                        }
                    }
                    HasMany(rel) => {
                        let ty = &rel.ty;

                        quote! {
                            #vis fn #field_ident(&self) -> <#ty as #toasty::Relation>::ManyField {
                                <#ty as #toasty::Relation>::ManyField::from_path(
                                    self.path().chain(#toasty::Path::from_field_index::<#model_ident>(#field_offset))
                                )
                            }
                        }
                    }
                    HasOne(rel) => {
                        let ty = &rel.ty;

                        quote! {
                            #vis fn #field_ident(&self) -> <#ty as #toasty::Relation>::OneField {
                                <#ty as #toasty::Relation>::OneField::from_path(
                                    self.path().chain(#toasty::Path::from_field_index::<#model_ident>(#field_offset))
                                )
                            }
                        }
                    }
                }
            });

        // Generate struct with path field
        quote!(
            #vis struct #field_struct_ident {
                path: #toasty::Path<#model_ident>,
            }

            impl #field_struct_ident {
                fn path(&self) -> #toasty::Path<#model_ident> {
                    self.path.clone()
                }

                #( #methods )*
            }
        )
    }

    pub(super) fn expand_model_field_struct_init(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_struct_ident = self.field_struct_ident();

        // Generate fields() as a method instead of const to avoid const initialization issues
        // This will be placed inside the existing impl block for the model
        quote!(
            #vis fn fields() -> #field_struct_ident {
                #field_struct_ident {
                    path: #toasty::Path::root(),
                }
            }
        )
    }

    fn field_struct_ident(&self) -> &syn::Ident {
        use crate::schema::ModelKind;

        match &self.model.kind {
            ModelKind::Root(root) => &root.field_struct_ident,
            ModelKind::Embedded(embedded) => &embedded.field_struct_ident,
        }
    }

    pub(super) fn expand_field_name_to_id(&self) -> TokenStream {
        let toasty = &self.toasty;

        let fields = self
            .model
            .fields
            .iter()
            .enumerate()
            .map(move |(offset, field)| {
                let field_name = field.name.ident.to_string();
                let field_offset = util::int(offset);

                quote!( #field_name => FieldId { model: Self::id(), index: #field_offset }, )
            });

        quote! {
            fn field_name_to_id(name: &str) -> #toasty::FieldId {
                use #toasty::{FieldId, Model, Register};

                match name {
                    #( #fields )*
                    _ => todo!("field_name_to_id: {}", name),
                }
            }
        }
    }
}
