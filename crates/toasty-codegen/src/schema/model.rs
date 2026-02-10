use super::{ErrorSet, Field, Index, IndexField, IndexScope, ModelAttr, Name, PrimaryKey};

#[derive(Debug)]
pub(crate) enum ModelKind {
    /// Root model with table, primary key, and query builders
    Root(ModelRoot),
    /// Embedded model that is flattened into parent
    Embedded(ModelEmbedded),
}

impl ModelKind {
    pub(crate) fn expect_root(&self) -> &ModelRoot {
        match self {
            ModelKind::Root(root) => root,
            ModelKind::Embedded(_) => panic!("expected root model, found embedded"),
        }
    }

    pub(crate) fn expect_embedded(&self) -> &ModelEmbedded {
        match self {
            ModelKind::Embedded(embedded) => embedded,
            ModelKind::Root(_) => panic!("expected embedded model, found root"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ModelRoot {
    /// Tracks fields in the primary key
    pub(crate) primary_key: PrimaryKey,

    /// The field struct identifier
    pub(crate) field_struct_ident: syn::Ident,

    /// The query struct identifier
    pub(crate) query_struct_ident: syn::Ident,

    /// Create builder struct identifier
    pub(crate) create_struct_ident: syn::Ident,

    /// Update builder struct identifier
    pub(crate) update_struct_ident: syn::Ident,

    /// Update by query builder struct identifier
    pub(crate) update_query_struct_ident: syn::Ident,
}

#[derive(Debug)]
pub(crate) struct ModelEmbedded {
    /// The field struct identifier
    pub(crate) field_struct_ident: syn::Ident,
}

#[derive(Debug)]
pub(crate) struct Model {
    /// Model name
    pub(crate) name: Name,

    /// Model visibility
    pub(crate) vis: syn::Visibility,

    /// Type identifier
    pub(crate) ident: syn::Ident,

    /// Model fields
    pub(crate) fields: Vec<Field>,

    /// Distinguishes root models from embedded models
    pub(crate) kind: ModelKind,

    /// Model indices
    pub(crate) indices: Vec<Index>,

    /// Optional table to map the model to
    pub(crate) table: Option<syn::LitStr>,
}

impl Model {
    pub(crate) fn from_ast(ast: &syn::ItemStruct, is_embedded: bool) -> syn::Result<Self> {
        let syn::Fields::Named(node) = &ast.fields else {
            return Err(syn::Error::new_spanned(
                &ast.fields,
                "model fields must be named",
            ));
        };

        // Generics are not supported yet
        if !ast.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &ast.generics,
                "model generics are not supported",
            ));
        }

        // First, map field names to identifiers
        let mut names = vec![];

        for field in node.named.iter() {
            if let Some(ident) = &field.ident {
                names.push(ident.clone());
            } else {
                return Err(syn::Error::new_spanned(field, "model fields must be named"));
            }
        }

        let mut model_attr = ModelAttr::default();
        let mut fields = vec![];
        let mut indices = vec![];
        let mut pk_index_fields = vec![];
        let mut errs = ErrorSet::new();

        if let Err(err) = model_attr.populate_from_ast(&ast.attrs, &names) {
            errs.push(err);
        }

        for (index, node) in node.named.iter().enumerate() {
            match Field::from_ast(node, &ast.ident, index, &names) {
                Ok(field) => {
                    if model_attr.key.is_some() {
                        if let Some(field) = &field.attrs.key {
                            errs.push(syn::Error::new_spanned(
                                field,
                                "field cannot have #[key] attribute when model has #[key] attribute",
                            ));
                        }
                    }

                    fields.push(field);
                }
                Err(err) => errs.push(err),
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        if let Some(attr) = &model_attr.key {
            for field in &attr.partition {
                let index = names.iter().position(|name| name == field).unwrap();
                pk_index_fields.push(IndexField {
                    field: index,
                    scope: IndexScope::Partition,
                });
            }

            for field in &attr.local {
                let index = names.iter().position(|name| name == field).unwrap();
                pk_index_fields.push(IndexField {
                    field: index,
                    scope: IndexScope::Local,
                });
            }
        } else {
            for (offset, field) in fields.iter().enumerate() {
                if field.attrs.key.is_some() {
                    pk_index_fields.push(IndexField {
                        field: offset,
                        scope: IndexScope::Partition,
                    });
                }
            }
        }

        // Return an error if no primary key fields were found (only for root models)
        if !is_embedded && pk_index_fields.is_empty() {
            return Err(syn::Error::new_spanned(
                ast,
                "model must either have a struct-level `#[key]` attribute or at least one field-level `#[key]` attribute",
            ));
        }

        // Build ModelKind based on whether this is embedded or root
        let kind = if is_embedded {
            ModelKind::Embedded(ModelEmbedded {
                field_struct_ident: struct_ident("Fields", ast),
            })
        } else {
            let pk_fields = pk_index_fields
                .iter()
                .map(|index_field| index_field.field)
                .collect();

            // Create an index for the primary key
            indices.push(Index {
                fields: pk_index_fields,
                unique: true,
                primary_key: true,
            });

            ModelKind::Root(ModelRoot {
                primary_key: PrimaryKey { fields: pk_fields },
                field_struct_ident: struct_ident("Fields", ast),
                query_struct_ident: struct_ident("Query", ast),
                create_struct_ident: struct_ident("Create", ast),
                update_struct_ident: struct_ident("Update", ast),
                update_query_struct_ident: struct_ident("UpdateQuery", ast),
            })
        };

        // Create indices for all fields annotated with unique or index
        for (index, field) in fields.iter().enumerate() {
            if field.attrs.unique {
                indices.push(Index {
                    fields: vec![IndexField {
                        field: index,
                        scope: IndexScope::Partition,
                    }],
                    unique: true,
                    primary_key: false,
                });
            } else if field.attrs.index {
                indices.push(Index {
                    fields: vec![IndexField {
                        field: index,
                        scope: IndexScope::Partition,
                    }],
                    unique: false,
                    primary_key: false,
                });
            }
        }

        Ok(Self {
            vis: ast.vis.clone(),
            name: Name::from_ident(&ast.ident),
            ident: ast.ident.clone(),
            fields,
            kind,
            indices,
            table: model_attr.table,
        })
    }

    pub fn primary_key_fields(&self) -> Option<impl ExactSizeIterator<Item = &'_ Field>> {
        match &self.kind {
            ModelKind::Root(root) => Some(
                root.primary_key
                    .fields
                    .iter()
                    .map(|index| &self.fields[*index]),
            ),
            ModelKind::Embedded(_) => None,
        }
    }
}

fn struct_ident(suffix: &str, model: &syn::ItemStruct) -> syn::Ident {
    syn::Ident::new(&format!("{}{}", model.ident, suffix), model.ident.span())
}
