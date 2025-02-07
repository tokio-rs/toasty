use super::*;

impl<'a> Generator<'a> {
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

        /*
        let relation_query_structs = self.gen_relation_structs();
        let relation_fields = self.gen_relation_fields();
        let query_structs = self.gen_query_structs();

        let struct_into_expr = self.gen_struct_into_expr();
        */

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

                #model_filters

                pub fn create() -> builders::#create_struct_name {
                    builders::#create_struct_name::default()
                }

                pub fn create_many() -> CreateMany<#struct_name> {
                    CreateMany::default()
                }

                #update_method_def

                /*
                pub fn filter(expr: stmt::Expr<bool>) -> Query {
                    Query::from_stmt(stmt::Select::filter(expr))
                }

                pub async fn delete(self, db: &Db) -> Result<()> {
                    let stmt = self.into_select().delete();
                    db.exec(stmt).await?;
                    Ok(())
                }
                */
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

            impl<'a> Relation<'a> for #struct_name {
                type ManyField = relations::Many<'a>;
                type OneField = relations::One<'a>;
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

            /*
            impl stmt::IntoExpr<#struct_name> for #struct_name {
                fn into_expr(self) -> stmt::Expr<#struct_name> {
                    todo!()
                }
            }

            impl stmt::IntoExpr<#struct_name> for &#struct_name {
                fn into_expr(self) -> stmt::Expr<#struct_name> {
                    #struct_into_expr.into()
                }
            }

            impl stmt::IntoExpr<[#struct_name]> for &#struct_name {
                fn into_expr(self) -> stmt::Expr<[#struct_name]> {
                    stmt::Expr::list([self])
                }
            }
            */

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

            /*
            pub mod fields {
                use super::*;

                #relation_fields
            }

            pub mod relation {
                use super::*;
                use toasty::Cursor;

                #relation_query_structs
            }

            pub mod queries {
                use super::*;

                #query_structs
            }
            */
        }
    }

    fn self_key_ty(&self) -> TokenStream {
        let mut tys = self
            .model
            .primary_key_fields()
            .map(|field| self.field_ty(field, 0));

        if tys.len() == 1 {
            let ty = tys.next().unwrap();
            ty
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
                        #name: HasMany<#ty>,
                    }
                }
                FieldTy::HasOne(_) => quote!(),
                FieldTy::BelongsTo(rel) => {
                    let name = self.field_name(field);
                    let ty = self.model_struct_path(rel.target, 0);
                    quote! {
                        #name: BelongsTo<#ty>,
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

    /*
    fn gen_struct_into_expr(&self) -> TokenStream {
        use app::FieldTy;

        let mut pk_exprs = vec![];

        for field in self.model.primary_key_fields() {
            let field_name = self.field_name(field.id);

            match &field.ty {
                FieldTy::Primitive(_) => {
                    pk_exprs.push(quote! {
                        &self.#field_name
                    });
                }
                FieldTy::BelongsTo(belongs_to) => {
                    for fk in &belongs_to.foreign_key.fields {
                        let fk_name = self.field_name(fk.target);
                        pk_exprs.push(quote! {
                            &self.#field_name.#fk_name
                        });
                    }
                }
                _ => todo!(),
            }
        }

        let pk_expr = match &pk_exprs[..] {
            [pk_expr] => quote!( #pk_expr ),
            pk_exprs => quote!( ( #( #pk_exprs, )* ) ),
        };

        quote! {
            stmt::Key::from_expr(#pk_expr)
        }
    }
    */
}
