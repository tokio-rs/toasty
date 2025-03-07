use super::{ErrorSet, Field, Index, IndexField, Name, PrimaryKey};

#[derive(Debug)]
pub(crate) struct Model {
    /// Generated model identifier
    pub(crate) id: usize,

    /// Model name
    pub(crate) name: Name,

    /// Model visibility
    pub(crate) vis: syn::Visibility,

    /// Type identifier
    pub(crate) ident: syn::Ident,

    /// Model fields
    pub(crate) fields: Vec<Field>,

    /// Model indices
    pub(crate) indices: Vec<Index>,

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

impl Model {
    pub(crate) fn from_ast(ast: &mut syn::ItemStruct) -> syn::Result<Model> {
        let syn::Fields::Named(node) = &mut ast.fields else {
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

        let mut fields = vec![];
        let mut indices = vec![];
        let mut errs = ErrorSet::new();

        for (index, node) in node.named.iter_mut().enumerate() {
            match Field::from_ast(node, &ast.ident, index, &names) {
                Ok(field) => fields.push(field),
                Err(err) => errs.push(err),
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        let primary_key_fields: Vec<_> = fields
            .iter()
            .enumerate()
            .filter_map(
                |(index, field)| {
                    if field.attrs.key {
                        Some(index)
                    } else {
                        None
                    }
                },
            )
            .collect();

        // Return an error if no primary key fields were found
        if primary_key_fields.is_empty() {
            return Err(syn::Error::new_spanned(
                &ast,
                "model must have at least one #[key] field",
            ));
        }

        // Create an index for the primary key
        indices.push(Index {
            id: 0,
            fields: primary_key_fields
                .iter()
                .copied()
                .map(|field| IndexField { field })
                .collect(),
            unique: true,
            primary_key: true,
        });

        // Create indices for all fields annotated with unique
        for (index, field) in fields.iter().enumerate() {
            if field.attrs.unique {
                let id = indices.len();

                indices.push(Index {
                    id,
                    fields: vec![IndexField { field: index }],
                    unique: true,
                    primary_key: false,
                });
            } else if field.attrs.index {
                let id = indices.len();

                indices.push(Index {
                    id,
                    fields: vec![IndexField { field: index }],
                    unique: false,
                    primary_key: false,
                });
            }
        }

        let id = gen_model_id();

        Ok(Model {
            id,
            vis: ast.vis.clone(),
            name: Name::from_ident(&ast.ident),
            ident: ast.ident.clone(),
            fields,
            indices,
            primary_key: PrimaryKey {
                fields: primary_key_fields,
                index: 0,
            },
            field_struct_ident: struct_ident("Fields", ast),
            query_struct_ident: struct_ident("Query", ast),
            create_struct_ident: struct_ident("Create", ast),
            update_struct_ident: struct_ident("Update", ast),
            update_query_struct_ident: struct_ident("UpdateQuery", ast),
        })
    }

    pub fn primary_key_fields(&self) -> impl ExactSizeIterator<Item = &'_ Field> {
        self.primary_key
            .fields
            .iter()
            .map(|index| &self.fields[*index])
    }
}

fn struct_ident(suffix: &str, model: &syn::ItemStruct) -> syn::Ident {
    syn::Ident::new(&format!("{}{}", model.ident, suffix), model.ident.span())
}

fn gen_model_id() -> usize {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    COUNT.fetch_add(1, Ordering::Relaxed)
}
