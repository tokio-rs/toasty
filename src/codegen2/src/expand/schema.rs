use super::{util, Expand};
use crate::schema::{FieldTy, Name};

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_schema(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let id = &self.tokenized_id;
        let name = self.expand_model_name();
        let fields = self.expand_model_fields();
        let primary_key = self.expand_primary_key();
        let indices = self.expand_model_indices();

        quote! {
            #vis fn schema() -> #toasty::schema::app::Model {
                use #toasty::{
                    schema::{
                        app::*,
                        db::{IndexOp, IndexScope},
                        Name
                    },
                    Type,
                };

                let id = #toasty::ModelId(#id);

                Model {
                    id,
                    name: #name,
                    fields: #fields,
                    primary_key: #primary_key,
                    queries: vec![],
                    indices: #indices,
                    table_name: None,
                }
            }
        }
    }

    fn expand_model_name(&self) -> TokenStream {
        expand_name(&self.model.name)
    }

    fn expand_model_fields(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_id = &self.tokenized_id;

        let fields = self.model.fields.iter().enumerate().map(|(index, field)| {
            let index_tokenized = util::int(index);
            let name = field.name.ident.to_string();
            let ty = match &field.ty {
                FieldTy::Primitive(ty) => {
                    quote!(FieldTy::Primitive(FieldPrimitive {
                        ty: <#ty as #toasty::stmt::Primitive>::TYPE,
                    }))
                }
                FieldTy::BelongsTo(rel) => {
                    let ty = &rel.ty;

                    let fk_fields = rel.foreign_key.iter().map(|fk_field| {
                        let source = util::int(fk_field.source);
                        let target = fk_field.target.to_string();

                        quote! {
                            ForeignKeyField {
                                source: FieldId {
                                    model: #toasty::ModelId(#model_id),
                                    index: #source,
                                },
                                target: <#ty as #toasty::Relation>::field_name_to_id(#target),
                            }
                        }
                    });

                    quote!(FieldTy::BelongsTo(BelongsTo {
                        target:  <#ty as #toasty::Relation>::ID,
                        expr_ty: Type::Model(<#ty as #toasty::Relation>::ID),
                        // The pair is populated at runtime.
                        pair: None,
                        foreign_key: ForeignKey {
                            fields: vec![ #( #fk_fields ),* ],
                        },
                    }))
                }
                FieldTy::HasMany(rel) => {
                    let ty = &rel.ty;
                    let singular_name = expand_name(&rel.singular);

                    quote!(FieldTy::HasMany(HasMany {
                        target: <#ty as #toasty::Relation>::ID,
                        expr_ty: Type::List(Box::new(Type::Model(<#ty as #toasty::Relation>::ID))),
                        singular: #singular_name,
                        // The pair is populated at runtime.
                        pair: FieldId {
                            model: ModelId(usize::MAX),
                            index: usize::MAX,
                        },
                        queries: vec![],
                    }))
                }
            };
            let primary_key = self.model.primary_key.fields.contains(&index);
            let auto = if field.attrs.auto {
                quote!(Some(Auto::Id))
            } else {
                quote!(None)
            };

            quote! {
                Field {
                    id: FieldId {
                        model: #toasty::ModelId(#model_id),
                        index: #index_tokenized,
                    },
                    name: #name.to_string(),
                    ty: #ty,
                    nullable: false,
                    primary_key: #primary_key,
                    auto: #auto,
                }
            }
        });

        quote! {
            vec![ #( #fields ),* ]
        }
    }

    fn expand_primary_key(&self) -> TokenStream {
        let fields = self.model.primary_key.fields.iter().map(|field| {
            let field_tokenized = util::int(*field);

            quote! {
                FieldId {
                    model: id,
                    index: #field_tokenized,
                }
            }
        });

        quote! {
            PrimaryKey {
                fields: vec![ #( #fields ),* ],
                query: QueryId(usize::MAX),
                index: IndexId {
                    model: id,
                    index: 0,
                },
            }
        }
    }

    fn expand_model_indices(&self) -> TokenStream {
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

                    quote! {
                        IndexField {
                            field: FieldId {
                                model: id,
                                index: #field_tokenized,
                            },
                            op: IndexOp::Eq,
                            scope: IndexScope::Partition,
                        }
                    }
                });

                quote! {
                    Index {
                        id: IndexId {
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
