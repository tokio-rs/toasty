use super::{util, Expand};
use crate::schema::{AutoStrategy, Column, FieldTy, ModelKind, Name, UuidVersion};

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_schema(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        let name = expand_name(toasty, &self.model.name);
        let fields = self.expand_model_fields();
        let indices = self.expand_model_indices();
        let table_name = self.expand_table_name();

        let kind = match &self.model.kind {
            ModelKind::Root(_) => {
                let primary_key = self.expand_primary_key();
                quote! {
                    #toasty::schema::app::ModelKind::Root(
                        #toasty::schema::app::ModelRoot {
                            primary_key: #primary_key,
                            table_name: #table_name,
                        }
                    )
                }
            }
            ModelKind::Embedded(_) => {
                quote! {
                    #toasty::schema::app::ModelKind::Embedded
                }
            }
        };

        quote! {
            fn schema() -> #toasty::schema::app::Model {
                let id = #model_ident::id();

                #toasty::schema::app::Model {
                    id,
                    name: #name,
                    fields: #fields,
                    kind: #kind,
                    indices: #indices,
                }
            }
        }
    }

    fn expand_model_fields(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;

        let fields = self.model.fields.iter().enumerate().map(|(index, field)| {
            let index_tokenized = util::int(index);
            let field_ty;
            let nullable;

            let name = {
                let app_name = field.name.ident.to_string();
                let storage_name = match field.attrs.column.as_ref().and_then(|column| column.name.as_ref()) {
                    Some(name) => quote! { Some(#name.to_string()) },
                    None => quote! { None },
                };
                quote! {
                    #toasty::schema::app::FieldName {
                        app_name: #app_name.to_string(),
                        storage_name: #storage_name,
                    }
                }
            };

            match &field.ty {
                FieldTy::Primitive(ty) => {
                    let storage_ty = match &field.attrs.column {
                        Some(Column { ty: Some(col_ty), ..}) => {
                            let expanded = col_ty.expand_with(toasty);
                            quote!(Some(#expanded))
                        }
                        _ => quote!(None),
                    };

                    nullable = quote!(<#ty as #toasty::stmt::Primitive>::NULLABLE);
                    field_ty = quote!(<#ty as #toasty::stmt::Primitive>::field_ty(#storage_ty));
                }
                FieldTy::BelongsTo(rel) => {
                    let ty = &rel.ty;

                    let fk_fields = rel.foreign_key.iter().map(|fk_field| {
                        let source = util::int(fk_field.source);
                        let target = fk_field.target.to_string();

                        quote! {
                            #toasty::schema::app::ForeignKeyField {
                                source: #toasty::schema::app::FieldId {
                                    model: #model_ident::id(),
                                    index: #source,
                                },
                                target: <#ty as #toasty::Relation>::field_name_to_id(#target),
                            }
                        }
                    });

                    nullable = quote!(<#ty as #toasty::Relation>::nullable());
                    field_ty = quote!(#toasty::schema::app::FieldTy::BelongsTo(#toasty::schema::app::BelongsTo {
                        target:  <#ty as #toasty::Relation>::Model::id(),
                        expr_ty: #toasty::Type::Model(<#ty as #toasty::Relation>::Model::id()),
                        // The pair is populated at runtime.
                        pair: None,
                        foreign_key: #toasty::schema::app::ForeignKey {
                            fields: vec![ #( #fk_fields ),* ],
                        },
                    }));
                }
                FieldTy::HasMany(rel) => {
                    let ty = &rel.ty;
                    let singular_name = expand_name(toasty, &rel.singular);

                    nullable = quote!(<#ty as #toasty::Relation>::nullable());
                    field_ty = quote!(#toasty::schema::app::FieldTy::HasMany(#toasty::schema::app::HasMany {
                        target: <#ty as #toasty::Relation>::Model::id(),
                        expr_ty: #toasty::Type::List(Box::new(#toasty::Type::Model(<#ty as #toasty::Relation>::Model::id()))),
                        singular: #singular_name,
                        // The pair is populated at runtime.
                        pair: #toasty::schema::app::FieldId {
                            model: #toasty::schema::app::ModelId(usize::MAX),
                            index: usize::MAX,
                        },
                    }));
                }
                FieldTy::HasOne(rel) => {
                    let ty = &rel.ty;

                    nullable = quote!(<#ty as #toasty::Relation>::nullable());
                    field_ty = quote!(#toasty::schema::app::FieldTy::HasOne(#toasty::schema::app::HasOne {
                        target: <#ty as #toasty::Relation>::Model::id(),
                        expr_ty: #toasty::Type::Model(<#ty as #toasty::Relation>::Model::id()),
                        // The pair is populated at runtime.
                        pair: #toasty::schema::app::FieldId {
                            model: #toasty::schema::app::ModelId(usize::MAX),
                            index: usize::MAX,
                        },
                    }));
                }
            }

            let primary_key = self.model.primary_key_fields()
                .map(|mut fields| fields.any(|f| self.model.fields.iter().position(|field| std::ptr::eq(field, f)) == Some(index)))
                .unwrap_or(false);
            let auto = match &field.attrs.auto {
                None => quote! { None },
                Some(auto) => {
                    let FieldTy::Primitive(ty) = &field.ty else { todo!("better error handling") };

                    match auto {
                        AutoStrategy::Unspecified => {
                            assert!(primary_key, "TODO: better error handling");

                            quote! { Some(<#ty as #toasty::stmt::Auto>::STRATEGY) }
                         }
                        AutoStrategy::Uuid(UuidVersion::V4) => quote! { Some(#toasty::schema::app::AutoStrategy::Uuid(#toasty::schema::app::UuidVersion::V4)) },
                        AutoStrategy::Uuid(UuidVersion::V7) => quote! { Some(#toasty::schema::app::AutoStrategy::Uuid(#toasty::schema::app::UuidVersion::V4)) },
                        AutoStrategy::Increment => quote! { Some(#toasty::schema::app::AutoStrategy::Increment) },
                    }
                }
            };

            quote! {
                #toasty::schema::app::Field {
                    id: #toasty::schema::app::FieldId {
                        model: #model_ident::id(),
                        index: #index_tokenized,
                    },
                    name: #name,
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
        let primary_key = match &self.model.kind {
            ModelKind::Root(root) => &root.primary_key,
            ModelKind::Embedded(_) => panic!("expand_primary_key called on embedded model"),
        };

        let fields = primary_key
            .fields
            .iter()
            .map(|field| {
                let field_tokenized = util::int(*field);

                quote! {
                    #toasty::schema::app::FieldId {
                        model: id,
                        index: #field_tokenized,
                    }
                }
            })
            .collect::<Vec<_>>();

        quote! {
            #toasty::schema::app::PrimaryKey {
                fields: vec![ #( #fields ),* ],
                index: #toasty::schema::app::IndexId {
                    model: id,
                    index: 0,
                },
            }
        }
    }

    fn expand_model_indices(&self) -> TokenStream {
        use crate::schema::IndexScope;

        let toasty = &self.toasty;

        let indices = self
            .model
            .indices
            .iter()
            .enumerate()
            .map(|(index, model_index)| {
                let index_tokenized = util::int(index);
                let unique = &model_index.unique;
                let primary_key = &model_index.primary_key;

                let fields = model_index.fields.iter().map(|index_field| {
                    let field_tokenized = util::int(index_field.field);
                    let scope = match &index_field.scope {
                        IndexScope::Partition => quote!(#toasty::schema::db::IndexScope::Partition),
                        IndexScope::Local => quote!(#toasty::schema::db::IndexScope::Local),
                    };

                    quote! {
                        #toasty::schema::app::IndexField {
                            field: #toasty::schema::app::FieldId {
                                model: id,
                                index: #field_tokenized,
                            },
                            op: #toasty::schema::db::IndexOp::Eq,
                            scope: #scope,
                        }
                    }
                });

                quote! {
                    #toasty::schema::app::Index {
                        id: #toasty::schema::app::IndexId {
                            model: id,
                            index: #index_tokenized,
                        },
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

fn expand_name(toasty: &TokenStream, name: &Name) -> TokenStream {
    let parts = name.parts.iter().map(|part| {
        let part = part.to_string();
        quote! { #part.to_string() }
    });

    quote! {
        #toasty::schema::Name {
            parts: vec![#( #parts ),*],
        }
    }
}
