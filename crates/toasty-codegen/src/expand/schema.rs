use super::{util, Expand};
use crate::schema::{ColumnType, FieldTy, Name};

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_schema(&self) -> TokenStream {
        let toasty = &self.toasty;
        let name = self.expand_model_name();
        let fields = self.expand_model_fields();
        let primary_key = self.expand_primary_key();
        let indices = self.expand_model_indices();
        let table_name = self.expand_table_name();

        quote! {
            fn schema() -> #toasty::schema::Model {
                use #toasty::{
                    schema::{
                        app,
                        db::{self, IndexOp, IndexScope},
                        Name,
                    },
                    Model,
                    Type,
                };

                #toasty::schema::Model {
                    type_id: std::any::TypeId::of::<Self>(),
                    name: #name,
                    fields: #fields,
                    primary_key: #primary_key,
                    indices: #indices,
                    table_name: #table_name,
                }
            }
        }
    }

    fn expand_model_name(&self) -> TokenStream {
        expand_name(&self.model.name)
    }

    fn expand_model_fields(&self) -> TokenStream {
        let toasty = &self.toasty;

        let fields = self.model.fields.iter().enumerate().map(|(index, field)| {
            let name = field.name.ident.to_string();
            let field_ty;
            let nullable;

            match &field.ty {
                FieldTy::Primitive(ty) => {
                    let storage_ty = match &field.attrs.db {
                        Some(ColumnType::VarChar(size)) => {
                            let size = util::int(*size);
                            quote!(Some(db::Type::VarChar(#size)))
                        }
                        None => quote!(None),
                    };

                    nullable = quote!(<#ty as #toasty::stmt::Primitive>::NULLABLE);
                    field_ty = quote!(#toasty::schema::FieldTy::Primitive(app::FieldPrimitive {
                        ty: <#ty as #toasty::stmt::Primitive>::TYPE,
                        storage_ty: #storage_ty,
                    }));
                }
                FieldTy::BelongsTo(rel) => {
                    let ty = &rel.ty;

                    let fk_fields = rel.foreign_key.iter().map(|fk_field| {
                        let source_name = self.model.fields[fk_field.source].name.ident.to_string();
                        let target_name = fk_field.target.to_string();
                        quote! {
                            #toasty::schema::ForeignKeyField {
                                source: #source_name.to_string(),
                                target: #target_name.to_string(),
                            }
                        }
                    });

                    nullable = quote!(<#ty as #toasty::Relation>::nullable());
                    field_ty = quote!(#toasty::schema::FieldTy::BelongsTo(#toasty::schema::BelongsTo {
                        target: std::any::TypeId::of::<<#ty as #toasty::Relation>::Model>(),
                        expr_ty: Type::Model(#toasty::schema::app::ModelId(0)), // Placeholder - will be resolved
                        foreign_key: vec![ #( #fk_fields ),* ],
                    }));
                }
                FieldTy::HasMany(rel) => {
                    let ty = &rel.ty;
                    let singular_name = expand_name(&rel.singular);

                    nullable = quote!(<#ty as #toasty::Relation>::nullable());
                    field_ty = quote!(#toasty::schema::FieldTy::HasMany(#toasty::schema::HasMany {
                        target: std::any::TypeId::of::<<#ty as #toasty::Relation>::Model>(),
                        expr_ty: Type::List(Box::new(Type::Model(#toasty::schema::app::ModelId(0)))), // Placeholder
                        singular: #singular_name,
                    }));
                }
                FieldTy::HasOne(rel) => {
                    let ty = &rel.ty;

                    nullable = quote!(<#ty as #toasty::Relation>::nullable());
                    field_ty = quote!(#toasty::schema::FieldTy::HasOne(#toasty::schema::HasOne {
                        target: std::any::TypeId::of::<<#ty as #toasty::Relation>::Model>(),
                        expr_ty: Type::Model(#toasty::schema::app::ModelId(0)), // Placeholder
                    }));
                }
            }

            let primary_key = self.model.primary_key.fields.contains(&index);
            let auto = if field.attrs.auto {
                quote!(Some(app::Auto::Id))
            } else {
                quote!(None)
            };

            quote! {
                #toasty::schema::Field {
                    name: #name.to_string(),
                    ty: #field_ty,
                    nullable: #nullable,
                    primary_key: #primary_key,
                    auto: #auto,
                    constraints: vec![],
                }
            }
        });

        quote! {
            vec![ #( #fields ),* ]
        }
    }

    fn expand_primary_key(&self) -> TokenStream {
        let toasty = &self.toasty;
        let fields = self.model.primary_key.fields.iter().map(|field| {
            let field_index = util::int(*field);
            quote! { #field_index }
        });

        quote! {
            #toasty::schema::PrimaryKey {
                fields: vec![ #( #fields ),* ],
            }
        }
    }

    fn expand_model_indices(&self) -> TokenStream {
        use crate::schema::IndexScope;
        let toasty = &self.toasty;

        let indices = self.model.indices.iter().map(|model_index| {
            let unique = &model_index.unique;
            let primary_key = &model_index.primary_key;

            let fields = model_index.fields.iter().map(|index_field| {
                let field_index = util::int(index_field.field);
                let scope = match &index_field.scope {
                    IndexScope::Partition => quote!(IndexScope::Partition),
                    IndexScope::Local => quote!(IndexScope::Local),
                };

                quote! {
                    #toasty::schema::IndexField {
                        field: #field_index,
                        scope: #scope,
                    }
                }
            });

            quote! {
                #toasty::schema::Index {
                    fields: vec![ #( #fields ),* ],
                    unique: #unique,
                    primary_key: #primary_key,
                }
            }
        });

        quote! {
            vec![ #( #indices ),* ]
        }
    }

    fn expand_table_name(&self) -> TokenStream {
        if let Some(table_name) = &self.model.table {
            let table_name = table_name.value();
            quote! { Some(#table_name.to_string()) }
        } else {
            quote! { None }
        }
    }
}

fn expand_name(name: &Name) -> TokenStream {
    let parts = name.parts.iter().map(|part| {
        let part = part.to_string();
        quote! { #part.to_string() }
    });

    quote! {
        Name {
            parts: vec![#( #parts ),*],
        }
    }
}
