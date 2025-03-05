use syn::parse_quote;

use super::{BelongsTo, ErrorSet, Name};

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

    /// True if the field is annotated with `#[unique]`
    pub(crate) unique: bool,

    /// True if toasty should automatically set the value
    pub(crate) auto: bool,

    /// True if the field is indexed
    pub(crate) index: bool,
}

#[derive(Debug)]
pub(crate) enum FieldTy {
    Primitive(syn::Type),
    BelongsTo(BelongsTo),
}

impl Field {
    pub(super) fn from_ast(
        field: &mut syn::Field,
        id: usize,
        names: &[syn::Ident],
    ) -> syn::Result<Field> {
        let Some(ident) = &field.ident else {
            return Err(syn::Error::new_spanned(field, "model fields must be named"));
        };

        let name = Name::from_ident(ident);

        let mut errs = ErrorSet::new();
        let mut attrs = FieldAttrs {
            key: false,
            unique: false,
            auto: false,
            index: false,
        };
        let mut belongs_to = None;

        let mut i = 0;
        while i < field.attrs.len() {
            let attr = &field.attrs[i];

            if attr.path().is_ident("key") {
                if attrs.key {
                    errs.push(syn::Error::new_spanned(attr, "duplicate #[key] attribute"));
                } else {
                    attrs.key = true;
                }
            } else if attr.path().is_ident("auto") {
                if attrs.auto {
                    errs.push(syn::Error::new_spanned(attr, "duplicate #[auto] attribute"));
                } else {
                    attrs.auto = true;
                }
            } else if attr.path().is_ident("unique") {
                if attrs.unique {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[unique] attribute",
                    ));
                } else {
                    attrs.unique = true;
                }
            } else if attr.path().is_ident("index") {
                if attrs.index {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[index] attribute",
                    ));
                } else {
                    attrs.index = true;
                }
            } else if attr.path().is_ident("relation") {
                belongs_to = Some(BelongsTo::from_ast(attr, &field.ty, names)?);
            } else {
                i += 1;
                continue;
            }

            field.attrs.remove(i);
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        if belongs_to.is_some() {
            let ty = &field.ty;
            field.ty = parse_quote!(toasty::codegen_support::BelongsTo<#ty>);
        }

        Ok(Field {
            id,
            attrs,
            name,
            ty: if let Some(belongs_to) = belongs_to {
                FieldTy::BelongsTo(belongs_to)
            } else {
                FieldTy::Primitive(field.ty.clone())
            },
        })
    }
}
