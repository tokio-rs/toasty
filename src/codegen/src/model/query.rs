use super::*;

impl<'a> Generator<'a> {
    pub(crate) fn gen_query_struct(&self) -> TokenStream {
        let struct_name = self.self_struct_name();
        let relation_methods = self.gen_relation_methods();

        quote! {
            #[derive(Debug)]
            pub struct Query<'a> {
                stmt: stmt::Select<'a, #struct_name>,
            }

            impl<'a> Query<'a> {
                pub const fn from_stmt(stmt: stmt::Select<'a, #struct_name>) -> Query<'a> {
                    Query { stmt }
                }

                pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, #struct_name>> {
                    db.all(self.stmt).await
                }

                pub async fn first(self, db: &Db) -> Result<Option<#struct_name>> {
                    db.first(self.stmt).await
                }

                pub async fn get(self, db: &Db) -> Result<#struct_name> {
                    db.get(self.stmt).await
                }

                pub fn update(self) -> UpdateQuery<'a> {
                    UpdateQuery::from(self)
                }

                pub async fn delete(self, db: &Db) -> Result<()> {
                    db.exec(self.stmt.delete()).await?;
                    Ok(())
                }

                pub async fn collect<A>(self, db: &'a Db) -> Result<A>
                where
                    A: FromCursor<#struct_name>
                {
                    self.all(db).await?.collect().await
                }

                pub fn filter(self, expr: stmt::Expr<'a, bool>) -> Query<'a> {
                    Query {
                        stmt: self.stmt.and(expr),
                    }
                }

                #relation_methods
            }

            impl<'a> stmt::IntoSelect<'a> for Query<'a> {
                type Model = #struct_name;

                fn into_select(self) -> stmt::Select<'a, #struct_name> {
                    self.stmt
                }
            }

            impl<'a> stmt::IntoSelect<'a> for &Query<'a> {
                type Model = #struct_name;

                fn into_select(self) -> stmt::Select<'a, #struct_name> {
                    self.stmt.clone()
                }
            }

            impl Default for Query<'static> {
                fn default() -> Query<'static> {
                    Query { stmt: stmt::Select::all() }
                }
            }
        }
    }

    fn gen_relation_methods(&self) -> TokenStream {
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

    fn gen_belongs_to_method(&self, field: FieldId, target: ModelId) -> TokenStream {
        let name = self.field_name(field);
        let module_name = self.module_name(target, 0);

        quote! {
            pub fn #name(mut self) -> #module_name::Query<'a> {
                todo!()
            }
        }
    }
}
