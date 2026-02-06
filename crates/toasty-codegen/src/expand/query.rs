use super::Expand;
use crate::schema::{BelongsTo, Field, FieldTy, HasMany, HasOne};

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_query_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_struct_ident = &self.model.kind.expect_root().query_struct_ident;
        let update_query_struct_ident = &self.model.kind.expect_root().update_query_struct_ident;
        let filter_methods = self.expand_query_filter_methods();
        let relation_methods = self.expand_relation_methods();

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

                #vis fn update(self) -> #update_query_struct_ident {
                    #update_query_struct_ident::from(self)
                }

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

                #vis fn paginate(self, per_page: usize) -> #toasty::stmt::Paginate<#model_ident> {
                    #toasty::stmt::Paginate::new(self.stmt, per_page)
                }

                #vis fn filter(self, expr: #toasty::stmt::Expr<bool>) -> #query_struct_ident {
                    #query_struct_ident {
                        stmt: self.stmt.and(expr),
                    }
                }

                #vis fn order_by(mut self, order_by: impl Into<#toasty::stmt::OrderBy>) -> #query_struct_ident {
                    self.stmt.order_by(order_by);
                    self
                }

                #vis fn include<T: ?Sized>(mut self, path: impl #toasty::Into<#toasty::Path<T>>) -> #query_struct_ident {
                    self.stmt.include(path.into());
                    self
                }

                #relation_methods
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

    fn expand_relation_methods(&self) -> TokenStream {
        self.model
            .fields
            .iter()
            .filter_map(|field| match &field.ty {
                FieldTy::BelongsTo(rel) => Some(self.expand_belongs_to_method(field, rel)),
                FieldTy::HasMany(rel) => Some(self.expand_has_many_method(field, rel)),
                FieldTy::HasOne(rel) => Some(self.expand_has_one_method(field, rel)),
                FieldTy::Primitive(..) => None,
            })
            .collect()
    }

    fn expand_belongs_to_method(&self, field: &Field, rel: &BelongsTo) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let target = &rel.ty;
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;

        quote! {
            #vis fn #field_ident(mut self) -> <#target as #toasty::Relation>::Query {
                use #toasty::IntoSelect;
                <#target as #toasty::Relation>::Query::from_stmt(
                    #toasty::stmt::Association::many_via_one(
                        self.stmt, #model_ident::fields().#field_ident().into()
                    ).into_select()
                )
            }
        }
    }

    fn expand_has_many_method(&self, field: &Field, rel: &HasMany) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let target = &rel.ty;
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;

        quote! {
            #vis fn #field_ident(mut self) -> <#target as #toasty::Relation>::Query {
                use #toasty::IntoSelect;
                <#target as #toasty::Relation>::Query::from_stmt(
                    #toasty::stmt::Association::many(
                        self.stmt, #model_ident::fields().#field_ident().into()
                    ).into_select()
                )
            }
        }
    }

    fn expand_has_one_method(&self, field: &Field, rel: &HasOne) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let target = &rel.ty;
        let model_ident = &self.model.ident;
        let field_ident = &field.name.ident;

        quote! {
            #vis fn #field_ident(mut self) -> <#target as #toasty::Relation>::Query {
                use #toasty::IntoSelect;
                <#target as #toasty::Relation>::Query::from_stmt(
                    #toasty::stmt::Association::many_via_one(
                        self.stmt, #model_ident::fields().#field_ident().into()
                    ).into_select()
                )
            }
        }
    }
}
