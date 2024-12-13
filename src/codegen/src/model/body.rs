use super::*;

impl<'a> Generator<'a> {
    pub(super) fn gen_body(&mut self) -> TokenStream {
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

        let relation_query_structs = self.gen_relation_structs();
        let relation_fields = self.gen_relation_fields();
        let query_structs = self.gen_query_structs();

        let struct_into_expr = self.gen_struct_into_expr();

        let into_select_impl_ref = self.gen_into_select_impl(true);
        let into_select_impl_value = self.gen_into_select_impl(false);

        quote! {
            #container_import

            use toasty::codegen_support::*;

            #[derive(Debug)]
            pub struct #struct_name {
                #struct_fields
            }

            impl #struct_name {
                #field_consts

                pub fn create() -> #create_struct_name {
                    #create_struct_name::default()
                }

                pub fn create_many() -> CreateMany<#struct_name> {
                    CreateMany::default()
                }

                pub fn filter(expr: stmt::Expr<bool>) -> Query {
                    Query::from_stmt(stmt::Select::from_expr(expr))
                }

                #update_method_def

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

            impl stmt::IntoSelect for &#struct_name {
                type Model = #struct_name;

                fn into_select(self) -> stmt::Select<Self::Model> {
                    #into_select_impl_ref
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
                    #into_select_impl_value
                }
            }

            impl stmt::IntoExpr<#struct_name> for &#struct_name {
                fn into_expr(self) -> stmt::Expr<#struct_name> {
                    #struct_into_expr.into()
                }
            }

            impl stmt::IntoExpr<[#struct_name]> for &#struct_name {
                fn into_expr(self) -> stmt::Expr<[#struct_name]> {
                    #struct_into_expr.into()
                }
            }

            #query_struct

            #create_struct_def

            #update_struct_def

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

    fn gen_struct_into_expr(&self) -> TokenStream {
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

    fn gen_into_select_impl(&self, is_ref: bool) -> TokenStream {
        let struct_name = self.self_struct_name();
        let query = self.pk_query();
        let query_name = self.query_method_name(query.id);

        let args = query.args.iter().map(|arg| {
            let name = util::ident(&arg.name);

            if is_ref {
                quote!(&self.#name,)
            } else {
                quote!(self.#name,)
            }
        });

        quote! {
            #struct_name::#query_name(#( #args )*).into_select()
        }
    }
}
