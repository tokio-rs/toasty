use super::*;

impl<'a> Generator<'a> {
    pub(super) fn gen_model_field_consts(&self) -> TokenStream {
        use app::FieldTy::*;

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
                }
            })
            .collect()
    }
}
