use super::{ErrorSet, Name};

#[derive(Debug)]
pub(crate) struct Field {
    /// Index of field in the containing model
    pub(crate) id: usize,

    /// Field attributes
    pub(crate) attrs: FieldAttrs,

    /// Field name
    pub(crate) name: Name,

    /// Field type
    pub(crate) ty: FieldTy,
}

#[derive(Debug)]
pub(crate) struct FieldAttrs {
    /// True if the field is annotated with `#[key]`
    pub(crate) key: bool,
}

#[derive(Debug)]
pub(crate) enum FieldTy {
    Primitive(syn::Type),
}

impl Field {
    pub(super) fn from_ast(id: usize, field: &syn::Field) -> syn::Result<Field> {
        let Some(ident) = &field.ident else {
            return Err(syn::Error::new_spanned(field, "model fields must be named"));
        };

        let name = Name::from_ident(ident);

        let mut errs = ErrorSet::new();
        let mut attrs = FieldAttrs { key: false };

        for attr in &field.attrs {
            if attr.path().is_ident("key") {
                if attrs.key {
                    errs.push(syn::Error::new_spanned(attr, "duplicate #[key] attribute"));
                } else {
                    attrs.key = true;
                }
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        Ok(Field {
            id,
            attrs,
            name,
            ty: FieldTy::Primitive(field.ty.clone()),
        })
    }
}
