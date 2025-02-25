use super::{ErrorSet, Field, Name};

#[derive(Debug)]
pub(crate) struct Model {
    /// Model name
    pub(crate) name: Name,

    /// Type identifier
    pub(crate) ident: syn::Ident,

    /// Model fields
    pub(crate) fields: Vec<Field>,
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
        let mut errs = ErrorSet::new();

        for node in &node.named {
            match Field::from_ast(node) {
                Ok(field) => fields.push(field),
                Err(err) => errs.push(err),
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        Ok(Model {
            name: Name::from_ident(&ast.ident),
            ident: ast.ident.clone(),
            fields,
        })
    }
}
