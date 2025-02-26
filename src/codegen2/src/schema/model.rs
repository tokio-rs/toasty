use super::{ErrorSet, Field, Index, IndexField, Name, PrimaryKey};

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

    /// Model indices
    pub(crate) indices: Vec<Index>,

    /// Tracks fields in the primary key
    pub(crate) primary_key: PrimaryKey,

    /// The query struct identifier
    pub(crate) query_struct_ident: syn::Ident,

    /// Create builder struct identifier
    pub(crate) create_builder_struct_ident: syn::Ident,

    /// Update builder struct identifier
    pub(crate) update_builder_struct_ident: syn::Ident,
}

impl Model {
    pub(crate) fn from_ast(ast: &syn::ItemStruct) -> syn::Result<Model> {
        let syn::Fields::Named(node) = &ast.fields else {
            return Err(syn::Error::new_spanned(
                &ast.fields,
                "model fields must be named",
            ));
        };

        let mut fields = vec![];
        let mut indices = vec![];
        let mut errs = ErrorSet::new();

        for (index, node) in node.named.iter().enumerate() {
            match Field::from_ast(index, node) {
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

        Ok(Model {
            vis: ast.vis.clone(),
            name: Name::from_ident(&ast.ident),
            ident: ast.ident.clone(),
            fields,
            indices,
            primary_key: PrimaryKey {
                fields: primary_key_fields,
                index: 0,
            },
            query_struct_ident: struct_ident("Query", ast),
            create_builder_struct_ident: struct_ident("Create", ast),
            update_builder_struct_ident: struct_ident("Update", ast),
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
