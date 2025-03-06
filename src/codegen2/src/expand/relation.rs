use super::Expand;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_relation_structs(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_ident = &self.model.query_struct_ident;
        let create_builder_ident = &self.model.create_struct_ident;

        quote! {
            #[derive(Debug)]
            #vis struct Many {
                stmt: #toasty::stmt::Association<[#model_ident]>,
            }

            #[derive(Debug)]
            #vis struct One {
                stmt: #toasty::stmt::Select<#model_ident>,
            }

            #[derive(Debug)]
            #vis struct OptionOne {
                stmt: #toasty::stmt::Select<#model_ident>,
            }

            #[derive(Debug)]
            #vis struct ManyField {
                path: #toasty::Path<[#model_ident]>,
            }

            #[derive(Debug)]
            #vis struct OneField {
                path: #toasty::Path<#model_ident>,
            }

            impl Many {
                pub fn from_stmt(stmt: #toasty::stmt::Association<[#model_ident]>) -> Many {
                    Many { stmt }
                }

                // #filter_methods

                /// Iterate all entries in the relation
                #vis async fn all(self, db: &#toasty::Db) -> #toasty::Result<#toasty::Cursor<#model_ident>> {
                    use #toasty::IntoSelect;
                    db.all(self.stmt.into_select()).await
                }

                #vis async fn collect<A>(self, db: &#toasty::Db) -> #toasty::Result<A>
                where
                    A: #toasty::FromCursor<#model_ident>
                {
                    self.all(db).await?.collect().await
                }

                #vis fn query(
                    self,
                    filter: #toasty::stmt::Expr<bool>
                ) -> #query_ident {
                    use #toasty::IntoSelect;
                    let query = self.into_select();
                    #query_ident::from_stmt(query.and(filter))
                }

                #vis fn create(self) -> #create_builder_ident {
                    use #toasty::IntoSelect;
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt.into_select());
                    builder
                }

                /// Add an item to the association
                #vis async fn insert(self, db: &#toasty::Db, item: impl #toasty::IntoExpr<[#model_ident]>) -> #toasty::Result<()> {
                    let stmt = self.stmt.insert(item);
                    db.exec(stmt).await?;
                    Ok(())
                }

                /// Remove items from the association
                #vis async fn remove(self, db: &#toasty::Db, item: impl #toasty::IntoExpr<#model_ident>) -> #toasty::Result<()> {
                    let stmt = self.stmt.remove(item);
                    db.exec(stmt).await?;
                    Ok(())
                }
            }

            impl #toasty::stmt::IntoSelect for Many {
                type Model = #model_ident;

                fn into_select(self) -> #toasty::stmt::Select<Self::Model> {
                    self.stmt.into_select()
                }
            }

            impl One {
                #vis fn from_stmt(stmt: #toasty::stmt::Select<#model_ident>) -> One {
                    One { stmt }
                }

                /// Create a new associated record
                #vis fn create(self) -> #create_builder_ident {
                    use #toasty::IntoSelect;
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt.into_select());
                    builder
                }

                #vis async fn get(self, db: &#toasty::Db) -> #toasty::Result<#model_ident> {
                    use #toasty::IntoSelect;
                    db.get(self.stmt.into_select()).await
                }
            }

            impl #toasty::stmt::IntoSelect for One {
                type Model = #model_ident;

                fn into_select(self) -> #toasty::stmt::Select<Self::Model> {
                    self.stmt.into_select()
                }
            }

            impl OptionOne {
                pub fn from_stmt(stmt: #toasty::stmt::Select<#model_ident>) -> OptionOne {
                    OptionOne { stmt }
                }

                /// Create a new associated record
                #vis fn create(self) -> #create_builder_ident {
                    use #toasty::IntoSelect;
                    let mut builder = #create_builder_ident::default();
                    builder.stmt.set_scope(self.stmt.into_select());
                    builder
                }

                #vis async fn get(self, db: &#toasty::Db) -> #toasty::Result<#toasty::Option<#model_ident>> {
                    use #toasty::IntoSelect;
                    db.first(self.stmt.into_select()).await
                }
            }

            impl ManyField {
                #vis const fn from_path(path: #toasty::Path<[#model_ident]>) -> ManyField {
                    ManyField { path }
                }
            }

            impl Into<#toasty::Path<[#model_ident]>> for ManyField {
                fn into(self) -> #toasty::Path<[#model_ident]> {
                    self.path
                }
            }

            impl OneField {
                #vis const fn from_path(path: #toasty::Path<#model_ident>) -> OneField {
                    OneField { path }
                }

                #vis fn eq<T>(self, rhs: T) -> #toasty::stmt::Expr<bool>
                where
                    T: #toasty::IntoExpr<#model_ident>,
                {
                    use #toasty::IntoExpr;
                    self.path.eq(rhs.into_expr())
                }

                #vis fn in_query<Q>(self, rhs: Q) -> #toasty::stmt::Expr<bool>
                where
                    Q: #toasty::IntoSelect<Model = #model_ident>,
                {
                    self.path.in_query(rhs)
                }
            }

            impl Into<#toasty::Path<#model_ident>> for OneField {
                fn into(self) -> #toasty::Path<#model_ident> {
                    self.path
                }
            }
        }
    }
}
