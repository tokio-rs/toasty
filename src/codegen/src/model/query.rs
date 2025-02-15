use super::*;

impl Generator<'_> {
    pub(crate) fn gen_query_struct(&self) -> TokenStream {
        let struct_name = self.self_struct_name();
        let relation_methods = self.gen_relation_methods();

        quote! {
            #[derive(Debug)]
            pub struct Query {
                stmt: stmt::Select<#struct_name>,
            }

            impl Query {
                pub const fn from_stmt(stmt: stmt::Select<#struct_name>) -> Query {
                    Query { stmt }
                }

                pub async fn all(self, db: &Db) -> Result<Cursor<#struct_name>> {
                    db.all(self.stmt).await
                }

                pub async fn first(self, db: &Db) -> Result<Option<#struct_name>> {
                    db.first(self.stmt).await
                }

                pub async fn get(self, db: &Db) -> Result<#struct_name> {
                    db.get(self.stmt).await
                }

                pub fn update(self) -> UpdateQuery {
                    UpdateQuery::from(self)
                }

                pub async fn delete(self, db: &Db) -> Result<()> {
                    db.exec(self.stmt.delete()).await?;
                    Ok(())
                }

                pub async fn collect<A>(self, db: &Db) -> Result<A>
                where
                    A: FromCursor<#struct_name>
                {
                    self.all(db).await?.collect().await
                }

                pub fn filter(self, expr: stmt::Expr<bool>) -> Query {
                    Query {
                        stmt: self.stmt.and(expr),
                    }
                }

                #relation_methods
            }

            impl stmt::IntoSelect for Query {
                type Model = #struct_name;

                fn into_select(self) -> stmt::Select<#struct_name> {
                    self.stmt
                }
            }

            impl stmt::IntoSelect for &Query {
                type Model = #struct_name;

                fn into_select(self) -> stmt::Select<#struct_name> {
                    self.stmt.clone()
                }
            }

            impl Default for Query {
                fn default() -> Query {
                    Query { stmt: stmt::Select::all() }
                }
            }
        }
    }

    fn gen_relation_methods(&self) -> TokenStream {
        use app::FieldTy;

        self.model
            .fields
            .iter()
            .filter_map(|field| match &field.ty {
                FieldTy::Primitive(..) => None,
                FieldTy::HasMany(..) => None,
                FieldTy::BelongsTo(belongs_to) => {
                    Some(self.gen_belongs_to_method(field.id, belongs_to.target))
                }
                FieldTy::HasOne(..) => None,
            })
            .collect()
    }

    fn gen_belongs_to_method(&self, field: app::FieldId, target: app::ModelId) -> TokenStream {
        let name = self.field_name(field);
        let module_name = self.module_name(target, 0);

        quote! {
            pub fn #name(mut self) -> #module_name::Query {
                todo!()
            }
        }
    }
}
