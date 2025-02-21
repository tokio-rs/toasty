use super::*;

impl Generator<'_> {
    pub(super) fn gen_model_body(&mut self) -> TokenStream {
        // Build field-level codegen state
        let model_id = util::int(self.model.id.0);

        let container_import = self.container_import();

        let key_ty = self.self_key_ty();
        let struct_name = self.self_struct_name();
        let struct_fields = self.gen_struct_fields();
        let struct_load_fields = self.gen_struct_load_fields();

        let create_struct_def = self.gen_create_struct();
        let create_struct_name = self.self_create_struct_name();

        let update_struct_def = self.gen_update_struct_def();
        let update_method_def = self.gen_model_update_method_def();

        let field_consts = self.gen_model_field_consts();

        let query_struct = self.gen_query_struct();
        let model_filters = self.gen_model_filter_methods(0);

        let relation_methods = self.gen_model_relation_methods();

        let into_expr_body_ref = self.gen_model_into_expr_body(true);
        let into_expr_body_val = self.gen_model_into_expr_body(false);
        let into_select_body_ref = self.gen_model_into_select_body(true);
        let into_select_body_value = self.gen_model_into_select_body(false);

        let relations_mod = self.gen_relations_mod();

        quote! {
            #container_import

            use toasty::codegen_support::*;

            #[derive(Debug)]
            pub struct #struct_name {
                #struct_fields
            }

            impl #struct_name {
                #field_consts

                #( #relation_methods )*

                #model_filters

                pub fn create() -> builders::#create_struct_name {
                    builders::#create_struct_name::default()
                }

                pub fn create_many() -> CreateMany<#struct_name> {
                    CreateMany::default()
                }

                #update_method_def

                pub fn filter(expr: stmt::Expr<bool>) -> Query {
                    Query::from_stmt(stmt::Select::filter(expr))
                }

                pub async fn delete(self, db: &Db) -> Result<()> {
                    let stmt = self.into_select().delete();
                    db.exec(stmt).await?;
                    Ok(())
                }
            }

            impl Model for #struct_name {
                const ID: ModelId = ModelId(#model_id);
                type Key = #key_ty;

                fn load(mut record: ValueRecord) -> Result<Self, Error> {
                    Ok(#struct_name {
                        #struct_load_fields
                    })
                }
            }

            impl Relation for #struct_name {
                type Query = Query;
                type Many = relations::Many;
                type ManyField = relations::ManyField;
                type One = relations::One;
                type OneField = relations::OneField;
                type OptionOne = relations::OptionOne;
            }

            impl stmt::IntoSelect for &#struct_name {
                type Model = #struct_name;

                fn into_select(self) -> stmt::Select<Self::Model> {
                    #into_select_body_ref
                }
            }

            impl stmt::IntoSelect for &mut #struct_name {
                type Model = #struct_name;

                fn into_select(self) -> stmt::Select<Self::Model> {
                    (&*self).into_select()
                }
            }

            impl stmt::IntoSelect for #struct_name {
                type Model = #struct_name;

                fn into_select(self) -> stmt::Select<Self::Model> {
                    #into_select_body_value
                }
            }

            impl stmt::IntoExpr<#struct_name> for #struct_name {
                fn into_expr(self) -> stmt::Expr<#struct_name> {
                    #into_expr_body_val
                }

                fn by_ref(&self) -> stmt::Expr<#struct_name> {
                    #into_expr_body_ref
                }
            }

            impl stmt::IntoExpr<[#struct_name]> for #struct_name {
                fn into_expr(self) -> stmt::Expr<[#struct_name]> {
                    stmt::Expr::list([self])
                }

                fn by_ref(&self) -> stmt::Expr<[#struct_name]> {
                    stmt::Expr::list([self])
                }
            }

            #query_struct

            pub mod builders {
                use super::*;

                #create_struct_def

                #update_struct_def
            }

            pub mod relations {
                use super::*;

                #relations_mod
            }
        }
    }

    fn self_key_ty(&self) -> TokenStream {
        let mut tys = self
            .model
            .primary_key_fields()
            .map(|field| self.field_ty(field, 0));

        if tys.len() == 1 {
            tys.next().unwrap()
        } else {
            quote! {
                ( #( #tys, )* )
            }
        }
    }

    /// Each model field has an associated struct field
    fn gen_struct_fields(&self) -> TokenStream {
        use app::FieldTy;

        self.model
            .fields
            .iter()
            .map(|field| match &field.ty {
                FieldTy::HasMany(rel) => {
                    let name = self.field_name(field);
                    let ty = self.model_struct_path(rel.target, 0);
                    quote! {
                        pub #name: HasMany<#ty>,
                    }
                }
                FieldTy::HasOne(_) => quote!(),
                FieldTy::BelongsTo(rel) => {
                    let name = self.field_name(field);
                    let ty = self.model_struct_path(rel.target, 0);
                    quote! {
                        pub #name: BelongsTo<#ty>,
                    }
                }
                FieldTy::Primitive(..) => {
                    let name = self.field_name(field);
                    let mut ty = self.field_ty(field, 0);

                    if field.nullable {
                        ty = quote!(Option<#ty>);
                    }

                    quote! {
                        pub #name: #ty,
                    }
                }
            })
            .collect()
    }

    fn gen_struct_load_fields(&self) -> TokenStream {
        use app::FieldTy;

        self.model
            .fields
            .iter()
            .map(|field| {
                let index = util::int(field.id.index);
                let name = self.field_name(field.id);

                match &field.ty {
                    FieldTy::HasMany(_) => {
                        quote!(#name: HasMany::load(record[#index].take())?,)
                    }
                    FieldTy::HasOne(_) => quote!(),
                    FieldTy::BelongsTo(_) => {
                        quote!(#name: BelongsTo::load(record[#index].take())?,)
                    }
                    FieldTy::Primitive(primitive) => {
                        let load = self.primitive_from_value(
                            &primitive.ty,
                            field.nullable,
                            quote!(record[#index].take()),
                        );
                        quote!(#name: #load,)
                    }
                }
            })
            .collect()
    }
}
