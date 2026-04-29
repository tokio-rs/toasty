use super::{Expand, util};
use crate::model::schema::SerializeAttr;
use crate::model::schema::{
    AutoStrategy, Column, FieldTy, ModelKind, Name, SerializeFormat, UuidVersion,
};

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

        let model = match &self.model.kind {
            ModelKind::Root(root) => {
                let primary_key = self.expand_primary_key();
                let version_field = match root.version_field {
                    Some(idx) => {
                        let idx_tok = util::int(idx);
                        quote! {
                            Some(#toasty::core::schema::app::FieldId {
                                model: id,
                                index: #idx_tok,
                            })
                        }
                    }
                    None => quote! { None },
                };
                quote! {
                    #toasty::core::schema::app::Model::Root(
                        #toasty::core::schema::app::ModelRoot {
                            id,
                            name: #name,
                            fields: #fields,
                            primary_key: #primary_key,
                            table_name: #table_name,
                            indices: #indices,
                            version_field: #version_field,
                        }
                    )
                }
            }
            ModelKind::EmbeddedStruct(_) => {
                quote! {
                    #toasty::core::schema::app::Model::EmbeddedStruct(
                        #toasty::core::schema::app::EmbeddedStruct {
                            id,
                            name: #name,
                            fields: #fields,
                            indices: #indices,
                        }
                    )
                }
            }
            ModelKind::EmbeddedEnum(_) => {
                panic!("expand_model_schema called on EmbeddedEnum; use embedded_enum() instead")
            }
        };

        quote! {
            fn schema() -> #toasty::core::schema::app::Model {
                let id = #model_ident::id();

                #model
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

            let field_named = match &self.model.kind {
                ModelKind::Root(_) => true,
                ModelKind::EmbeddedStruct(model_embedded_struct) => model_embedded_struct.fields_named,
                ModelKind::EmbeddedEnum(model_embedded_enum) => {
                    let variant = field.variant.unwrap();
                    model_embedded_enum.variants[variant].fields_named
                }
            };

            let name = {
                let app_name = if field_named {
                    let n = field.name.as_str();
                    quote! { Some(#n.to_string()) }
                } else {
                    quote! { None }
                };

                let storage_name = match field.attrs.column.as_ref().and_then(|column| column.name.as_ref()) {
                    Some(name) => quote! { Some(#name.to_string()) },
                    None => quote! { None },
                };

                quote! {
                    #toasty::core::schema::app::FieldName {
                        app: #app_name,
                        storage: #storage_name,
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

                    match &field.attrs.serialize {
                        Some(SerializeAttr { format, nullable: serialize_nullable }) => {
                            let serialize_format = match format {
                                SerializeFormat::Json => {
                                    quote!(Some(#toasty::core::schema::app::SerializeFormat::Json))
                                }
                            };
                            let nullable_lit = *serialize_nullable;

                            nullable = quote!(#nullable_lit);
                            field_ty = quote!(#toasty::core::schema::app::FieldTy::Primitive(
                                #toasty::core::schema::app::FieldPrimitive {
                                    ty: #toasty::core::stmt::Type::String,
                                    storage_ty: #storage_ty,
                                    serialize: #serialize_format,
                                }
                            ));
                        }
                        None => {
                            nullable = quote!(<#ty as #toasty::Field>::NULLABLE);
                            field_ty = quote!(<#ty as #toasty::Field>::field_ty(#storage_ty));
                        }
                    }
                }
                FieldTy::BelongsTo(rel) => {
                    let ty = &rel.ty;

                    let fk_fields = rel.foreign_key.iter().map(|fk_field| {
                        let source = util::int(fk_field.source);
                        let target = fk_field.target.to_string();

                        quote! {
                            #toasty::core::schema::app::ForeignKeyField {
                                source: #toasty::core::schema::app::FieldId {
                                    model: #model_ident::id(),
                                    index: #source,
                                },
                                target: <#ty as #toasty::Relation>::field_name_to_id(#target),
                            }
                        }
                    });

                    nullable = quote!(<#ty as #toasty::Relation>::nullable());
                    field_ty = quote!(<#ty as #toasty::Relation>::belongs_to_field_ty(
                        #toasty::core::schema::app::ForeignKey {
                            fields: vec![ #( #fk_fields ),* ],
                        },
                    ));
                }
                FieldTy::HasMany(rel) => {
                    let ty = &rel.ty;
                    let singular_name = expand_name(toasty, &rel.singular);
                    let pair = expand_pair(toasty, ty, rel.pair.as_ref());

                    nullable = quote!(<#ty as #toasty::Relation>::nullable());
                    field_ty = quote!(<#ty as #toasty::Relation>::has_many_field_ty(#singular_name, #pair));
                }
                FieldTy::HasOne(rel) => {
                    let ty = &rel.ty;
                    let pair = expand_pair(toasty, ty, rel.pair.as_ref());

                    nullable = quote!(<#ty as #toasty::Relation>::nullable());
                    field_ty = quote!(<#ty as #toasty::Relation>::has_one_field_ty(#pair));
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

                            quote! { Some(<#ty as #toasty::Auto>::STRATEGY) }
                         }
                        AutoStrategy::Uuid(UuidVersion::V4) => quote! { Some(#toasty::core::schema::app::AutoStrategy::Uuid(#toasty::core::schema::app::UuidVersion::V4)) },
                        AutoStrategy::Uuid(UuidVersion::V7) => quote! { Some(#toasty::core::schema::app::AutoStrategy::Uuid(#toasty::core::schema::app::UuidVersion::V7)) },
                        AutoStrategy::Increment => quote! { Some(#toasty::core::schema::app::AutoStrategy::Increment) },
                    }
                }
            };

            let versionable = field.attrs.versionable;

            quote! {
                #toasty::core::schema::app::Field {
                    id: #toasty::core::schema::app::FieldId {
                        model: #model_ident::id(),
                        index: #index_tokenized,
                    },
                    name: #name,
                    ty: #field_ty,
                    nullable: #nullable,
                    primary_key: #primary_key,
                    auto: #auto,
                    versionable: #versionable,
                    constraints: vec![],
                    variant: None,
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
            ModelKind::EmbeddedStruct(_) => panic!("expand_primary_key called on embedded struct"),
            ModelKind::EmbeddedEnum(_) => panic!("expand_primary_key called on embedded enum"),
        };

        let fields = primary_key
            .fields
            .iter()
            .map(|field| {
                let field_tokenized = util::int(*field);

                quote! {
                    #toasty::core::schema::app::FieldId {
                        model: id,
                        index: #field_tokenized,
                    }
                }
            })
            .collect::<Vec<_>>();

        quote! {
            #toasty::core::schema::app::PrimaryKey {
                fields: vec![ #( #fields ),* ],
                index: #toasty::core::schema::app::IndexId {
                    model: id,
                    index: 0,
                },
            }
        }
    }

    pub(super) fn expand_model_indices(&self) -> TokenStream {
        use crate::model::schema::IndexScope;

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
                        IndexScope::Partition => {
                            quote!(#toasty::core::schema::db::IndexScope::Partition)
                        }
                        IndexScope::Local => quote!(#toasty::core::schema::db::IndexScope::Local),
                    };

                    quote! {
                        #toasty::core::schema::app::IndexField {
                            field: #toasty::core::schema::app::FieldId {
                                model: id,
                                index: #field_tokenized,
                            },
                            op: #toasty::core::schema::db::IndexOp::Eq,
                            scope: #scope,
                        }
                    }
                });

                quote! {
                    #toasty::core::schema::app::Index {
                        id: #toasty::core::schema::app::IndexId {
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

impl Expand<'_> {
    /// Generate calls to register all models reachable from this model's fields.
    ///
    /// For primitive fields, no call is emitted (the default `Field::register`
    /// is a no-op). For embedded fields, `<Type as Field>::register` is called.
    /// For relation fields (BelongsTo, HasMany, HasOne), `<TargetType as
    /// Register>::register` is called directly.
    pub(super) fn expand_field_register_calls(&self) -> Vec<TokenStream> {
        let toasty = &self.toasty;

        self.model
            .fields
            .iter()
            .filter_map(|field| match &field.ty {
                FieldTy::Primitive(ty) => {
                    // Fields with #[serialize] store arbitrary types as JSON
                    // strings — they don't implement Field.
                    if field.attrs.serialize.is_some() {
                        return None;
                    }
                    // Primitives use Field::register which delegates to inner
                    // type if it's an embedded type (via the Field impl).
                    Some(quote! {
                        <#ty as #toasty::Field>::register(model_set);
                    })
                }
                FieldTy::BelongsTo(rel) => {
                    let ty = &rel.ty;
                    Some(quote! {
                        <<#ty as #toasty::Relation>::Model as #toasty::Register>::register(model_set);
                    })
                }
                FieldTy::HasMany(rel) => {
                    let ty = &rel.ty;
                    Some(quote! {
                        <<#ty as #toasty::Relation>::Model as #toasty::Register>::register(model_set);
                    })
                }
                FieldTy::HasOne(rel) => {
                    let ty = &rel.ty;
                    Some(quote! {
                        <<#ty as #toasty::Relation>::Model as #toasty::Register>::register(model_set);
                    })
                }
            })
            .collect()
    }
}

pub(super) fn expand_name(toasty: &TokenStream, name: &Name) -> TokenStream {
    let parts = name.parts.iter().map(|part| {
        let part = part.to_string();
        quote! { #part.to_string() }
    });

    quote! {
        #toasty::core::schema::Name {
            parts: vec![#( #parts ),*],
        }
    }
}

fn expand_pair(
    toasty: &TokenStream,
    target_ty: &syn::Type,
    pair: Option<&syn::Ident>,
) -> TokenStream {
    match pair {
        Some(ident) => {
            let name = ident.to_string();
            quote! { Some(<#target_ty as #toasty::Relation>::field_name_to_id(#name)) }
        }
        None => quote! { None },
    }
}
