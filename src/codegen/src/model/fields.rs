use super::*;

impl<'a> Generator<'a> {
    pub(super) fn gen_model_field_consts(&self) -> TokenStream {
        use FieldTy::*;

        self.model
            .fields
            .iter()
            .map(move |field| {
                let const_name = self.field_const_name(field);
                let field_offset = util::int(field.id.index);
                match &field.ty {
                    Primitive(primitive) => {
                        let ty = self.ty(&primitive.ty, 0);

                        quote! {
                            pub const #const_name: Path<#ty> = Path::from_field_index::<Self>(#field_offset);
                        }
                    }
                    HasMany(..) | HasOne(..) | BelongsTo(..) => {
                        let module_name = self.module_name(field.id.model, 0);
                        let relation_struct_name = self.relation_struct_name(field);

                        quote! {
                            pub const #const_name: #module_name::fields::#relation_struct_name =
                                #module_name::fields::#relation_struct_name::from_path(Path::from_field_index::<Self>(#field_offset));
                        }
                    }
                }
            })
            .collect()
    }

    pub(super) fn gen_path_methods(&self, model: &'a Model, depth: usize) -> TokenStream {
        use FieldTy::*;

        model
            .fields
            .iter()
            .map(move |field| {
                let name = self.field_name(field.id);
                let struct_path = self.model_struct_path(model, 1);
                let const_name = self.field_const_name(field);

                match &field.ty {
                    Primitive(primitive) => {
                        let ty = self.ty(&primitive.ty, depth);

                        quote! {
                            pub fn #name(mut self) -> Path<#ty> {
                                self.path.chain(#struct_path::#const_name)
                            }
                        }
                    }
                    HasMany(_) | HasOne(_) | BelongsTo(_) => {
                        if model.id == field.id.model {
                            let relation_struct_name = self.relation_struct_name(field);
    
                            quote! {
                                pub fn #name(mut self) -> #relation_struct_name {
                                    let path = self.path.chain(#struct_path::#const_name);
                                    #relation_struct_name::from_path(path)
                                }
                            }
                        } else {
                            let module_name = self.module_name(field.id.model, depth);
                            let relation_struct_name = self.relation_struct_name(field);

                            quote! {
                                pub fn #name(mut self) -> #module_name::fields::#relation_struct_name {
                                    let path = self.path.chain(#struct_path::#const_name);
                                    #module_name::fields::#relation_struct_name::from_path(path)
                                }
                            }
                        }
                    }
                }
            })
            .collect()
    }
}
