use super::*;

impl<'a> Generator<'a> {
    pub(super) fn gen_model_relation_methods(&self) -> impl Iterator<Item = TokenStream> + '_ {
        self.model
            .fields
            .iter()
            .filter_map(|field| match &field.ty {
                app::FieldTy::BelongsTo(rel) => {
                    Some(self.gen_model_relation_belongs_to_method(rel, field))
                }
                app::FieldTy::HasMany(_) => Some(self.gen_model_relation_has_many_method(field)),
                app::FieldTy::HasOne(_) => Some(self.gen_model_relation_has_one_method(field)),
                _ => None,
            })
    }

    fn gen_model_relation_belongs_to_method(
        &self,
        rel: &app::BelongsTo,
        field: &app::Field,
    ) -> TokenStream {
        let name = self.field_name(field);
        let target_struct = self.target_struct_path(field, 0);
        let mut target_relation = quote!(#target_struct);

        if field.nullable {
            target_relation = quote!(Option<#target_struct>);
        }

        // For proc macros, this will be updated to use field attributes instead of looking at the schema types
        let operands = rel.foreign_key.fields.iter().map(|fk_field| {
            let target_field_const = self.field_const_name(fk_field.target);
            let source_field_name = self.field_name(fk_field.source);

            quote! {
                #target_struct::#target_field_const.eq(&self.#source_field_name)
            }
        });

        let filter = if rel.foreign_key.fields.len() == 1 {
            quote!( #( #operands )* )
        } else {
            quote!( stmt::Expr::and_all([ #(#operands),* ]) )
        };

        quote! {
            pub fn #name(&self) -> <#target_relation as Relation>::One {
                <#target_relation as Relation>::One::from_stmt(
                    #target_struct::filter(#filter).into_select()
                )
            }
        }
    }

    fn gen_model_relation_has_many_method(&self, field: &app::Field) -> TokenStream {
        let name = self.field_name(field);
        let target_struct = self.target_struct_path(field, 0);
        let const_name = self.field_const_name(field);

        quote! {
            pub fn #name(&self) -> <#target_struct as Relation>::Many {
                <#target_struct as Relation>::Many::from_stmt(
                    stmt::Association::many(self.into_select(), Self::#const_name.into())
                )
            }
        }
    }

    fn gen_model_relation_has_one_method(&self, field: &app::Field) -> TokenStream {
        let name = self.field_name(field);
        let target_struct = self.target_struct_path(field, 0);
        let const_name = self.field_const_name(field);
        let mut target_relation = quote!(#target_struct);

        if field.nullable {
            target_relation = quote!(Option<#target_struct>);
        }

        quote! {
            pub fn #name(&self) -> <#target_relation as Relation>::One {
                <#target_relation as Relation>::One::from_stmt(
                    stmt::Association::one(self.into_select(), Self::#const_name.into()).into_select()
                )
            }
        }
    }

    pub(super) fn gen_relations_mod(&self) -> TokenStream {
        let strukt_name = self.self_struct_name();
        let create_struct_name = self.self_create_struct_name();
        let filter_methods = self.gen_relation_filter_methods();

        quote! {
            #[derive(Debug)]
            pub struct Many {
                stmt: stmt::Association<[#strukt_name]>,
            }

            #[derive(Debug)]
            pub struct One {
                stmt: stmt::Select<#strukt_name>,
            }

            #[derive(Debug)]
            pub struct OptionOne {
                stmt: stmt::Select<#strukt_name>,
            }

            pub struct ManyField {
                pub(super) path: Path<[super::#strukt_name]>,
            }

            pub struct OneField {
                pub(super) path: Path<super::#strukt_name>,
            }

            impl Many {
                pub fn from_stmt(stmt: stmt::Association<[#strukt_name]>) -> Many {
                    Many { stmt }
                }

                #filter_methods

                /// Iterate all entries in the relation
                pub async fn all(self, db: &Db) -> Result<Cursor<#strukt_name>> {
                    db.all(self.stmt.into_select()).await
                }

                pub async fn collect<A>(self, db: &Db) -> Result<A>
                where
                    A: FromCursor<#strukt_name>
                {
                    self.all(db).await?.collect().await
                }

                pub fn query(
                    self,
                    filter: stmt::Expr<bool>
                ) -> super::Query {
                    let query = self.into_select();
                    super::Query::from_stmt(query.and(filter))
                }

                pub fn create(self) -> builders::#create_struct_name {
                    let mut builder = builders::#create_struct_name::default();
                    builder.stmt.set_scope(self.stmt.into_select());
                    builder
                }

                /// Add an item to the association
                pub async fn insert(self, db: &Db, item: impl IntoExpr<[#strukt_name]>) -> Result<()> {
                    let stmt = self.stmt.insert(item);
                    db.exec(stmt).await?;
                    Ok(())
                }

                /// Remove items from the association
                pub async fn remove(self, db: &Db, item: impl IntoExpr<#strukt_name>) -> Result<()> {
                    let stmt = self.stmt.remove(item);
                    db.exec(stmt).await?;
                    Ok(())
                }
            }

            impl stmt::IntoSelect for Many {
                type Model = #strukt_name;

                fn into_select(self) -> stmt::Select<Self::Model> {
                    self.stmt.into_select()
                }
            }

            impl One {
                pub fn from_stmt(stmt: stmt::Select<#strukt_name>) -> One {
                    One { stmt }
                }

                /// Create a new associated record
                pub fn create(self) -> builders::#create_struct_name {
                    let mut builder = builders::#create_struct_name::default();
                    builder.stmt.set_scope(self.stmt.into_select());
                    builder
                }

                pub async fn get(self, db: &Db) -> Result<#strukt_name> {
                    db.get(self.stmt.into_select()).await
                }
            }

            impl stmt::IntoSelect for One {
                type Model = #strukt_name;

                fn into_select(self) -> stmt::Select<Self::Model> {
                    self.stmt.into_select()
                }
            }

            impl OptionOne {
                pub fn from_stmt(stmt: stmt::Select<#strukt_name>) -> OptionOne {
                    OptionOne { stmt }
                }

                /// Create a new associated record
                pub fn create(self) -> builders::#create_struct_name {
                    let mut builder = builders::#create_struct_name::default();
                    builder.stmt.set_scope(self.stmt.into_select());
                    builder
                }

                pub async fn get(self, db: &Db) -> Result<Option<#strukt_name>> {
                    db.first(self.stmt.into_select()).await
                }
            }

            impl ManyField {
                pub const fn from_path(path: Path<[super::#strukt_name]>) -> ManyField {
                    ManyField { path }
                }
            }

            impl Into<Path<[#strukt_name]>> for ManyField {
                fn into(self) -> Path<[#strukt_name]> {
                    self.path
                }
            }

            impl OneField {
                pub const fn from_path(path: Path<super::#strukt_name>) -> OneField {
                    OneField { path }
                }

                pub fn eq<T>(self, rhs: T) -> stmt::Expr<bool>
                where
                    T: IntoExpr<super::#strukt_name>,
                {
                    self.path.eq(rhs.into_expr())
                }

                pub fn in_query<Q>(self, rhs: Q) -> toasty::stmt::Expr<bool>
                where
                    Q: stmt::IntoSelect<Model = super::#strukt_name>,
                {
                    self.path.in_query(rhs)
                }
            }

            impl Into<Path<#strukt_name>> for OneField {
                fn into(self) -> Path<#strukt_name> {
                    self.path
                }
            }
        }
    }
}
