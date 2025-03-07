use super::Expand;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_query_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_struct_ident = &self.model.query_struct_ident;
        let filter_methods = self.expand_query_filter_methods();

        quote! {
            #vis struct #query_struct_ident {
                stmt: #toasty::stmt::Select<#model_ident>,
            }

            impl #query_struct_ident {
                #vis const fn from_stmt(stmt: #toasty::stmt::Select<#model_ident>) -> #query_struct_ident {
                    #query_struct_ident { stmt }
                }

                #filter_methods

                #vis async fn all(self, db: &#toasty::Db) -> #toasty::Result<#toasty::Cursor<#model_ident>> {
                    db.all(self.stmt).await
                }

                #vis async fn first(self, db: &#toasty::Db) -> #toasty::Result<#toasty::Option<#model_ident>> {
                    db.first(self.stmt).await
                }

                #vis async fn get(self, db: &#toasty::Db) -> #toasty::Result<#model_ident> {
                    db.get(self.stmt).await
                }

                /*
                #vis fn update(self) -> builders::UpdateQuery {
                    builders::UpdateQuery::from(self)
                }
                */

                #vis async fn delete(self, db: &#toasty::Db) -> #toasty::Result<()> {
                    db.exec(self.stmt.delete()).await?;
                    Ok(())
                }

                #vis async fn collect<A>(self, db: &#toasty::Db) -> #toasty::Result<A>
                where
                    A: #toasty::FromCursor<#model_ident>
                {
                    self.all(db).await?.collect().await
                }

                #vis fn filter(self, expr: #toasty::stmt::Expr<bool>) -> #query_struct_ident {
                    #query_struct_ident {
                        stmt: self.stmt.and(expr),
                    }
                }

                #vis fn include<T: ?Sized>(mut self, path: impl #toasty::Into<#toasty::Path<T>>) -> #query_struct_ident {
                    self.stmt.include(path.into());
                    self
                }

                // #relation_methods
            }

            impl #toasty::stmt::IntoSelect for #query_struct_ident {
                type Model = #model_ident;

                fn into_select(self) -> #toasty::stmt::Select<#model_ident> {
                    self.stmt
                }
            }

            impl #toasty::stmt::IntoSelect for &#query_struct_ident {
                type Model = #model_ident;

                fn into_select(self) -> #toasty::stmt::Select<#model_ident> {
                    self.stmt.clone()
                }
            }

            impl #toasty::Default for #query_struct_ident {
                fn default() -> #query_struct_ident {
                    #query_struct_ident { stmt: #toasty::stmt::Select::all() }
                }
            }
        }
    }
}
