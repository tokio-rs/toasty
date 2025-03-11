use super::Expand;
use crate::schema::{BelongsTo, Field, FieldTy, HasMany, HasOne};

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_relation_structs(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_ident = &self.model.query_struct_ident;
        let create_builder_ident = &self.model.create_struct_ident;
        let filter_methods = self.expand_relation_filter_methods();

        quote! {
            #vis struct Many {
                stmt: #toasty::stmt::Association<[#model_ident]>,
            }

            #vis struct One {
                stmt: #toasty::stmt::Select<#model_ident>,
            }

            #vis struct OptionOne {
                stmt: #toasty::stmt::Select<#model_ident>,
            }

            #vis struct ManyField {
                path: #toasty::Path<[#model_ident]>,
            }

            #vis struct OneField {
                path: #toasty::Path<#model_ident>,
            }

            impl Many {
                pub fn from_stmt(stmt: #toasty::stmt::Association<[#model_ident]>) -> Many {
                    Many { stmt }
                }

                #filter_methods

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

    pub(super) fn expand_model_relation_methods(&self) -> TokenStream {
        self.model
            .fields
            .iter()
            .filter_map(|field| match &field.ty {
                FieldTy::BelongsTo(rel) => {
                    Some(self.expand_model_relation_belongs_to_method(rel, field))
                }
                FieldTy::HasMany(rel) => {
                    Some(self.expand_model_relation_has_many_method(rel, field))
                }
                FieldTy::HasOne(rel) => Some(self.expand_model_relation_has_one_method(rel, field)),
                FieldTy::Primitive(_) => None,
            })
            .collect()
    }

    fn expand_model_relation_belongs_to_method(
        &self,
        rel: &BelongsTo,
        field: &Field,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_ident = &field.name.ident;
        let ty = &rel.ty;

        let operands = rel.foreign_key.iter().map(|fk_field| {
            let source = &self.model.fields[fk_field.source];
            let source_field_ident = &source.name.ident;
            let target = &fk_field.target;

            quote! {
                <#ty as #toasty::Relation>::Model::FIELDS.#target.eq(&self.#source_field_ident)
            }
        });

        let filter = if rel.foreign_key.len() == 1 {
            quote!( #( #operands )* )
        } else {
            quote!( stmt::Expr::and_all([ #(#operands),* ]) )
        };

        quote! {
            #vis fn #field_ident(&self) -> <#ty as #toasty::Relation>::One {
                use #toasty::IntoSelect;
                <#ty as #toasty::Relation>::One::from_stmt(
                    <#ty as #toasty::Relation>::Model::filter(#filter).into_select()
                )
            }
        }
    }

    fn expand_model_relation_has_many_method(&self, rel: &HasMany, field: &Field) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_ident = &field.name.ident;
        let ty = &rel.ty;

        quote! {
            #vis fn #field_ident(&self) -> <#ty as #toasty::Relation>::Many {
                use #toasty::IntoSelect;

                <#ty as #toasty::Relation>::Many::from_stmt(
                    #toasty::stmt::Association::many(self.into_select(), Self::FIELDS.#field_ident.into())
                )
            }
        }
    }

    fn expand_model_relation_has_one_method(&self, rel: &HasOne, field: &Field) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_ident = &field.name.ident;
        let ty = &rel.ty;

        quote! {
            #vis fn #field_ident(&self) -> <#ty as #toasty::Relation>::One {
                use #toasty::IntoSelect;

                <#ty as #toasty::Relation>::One::from_stmt(
                    #toasty::stmt::Association::one(self.into_select(), Self::FIELDS.#field_ident.into()).into_select()
                )
            }
        }
    }
}
