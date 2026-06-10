//! Parser for `#[item_parent]` field attribute.
//!
//! Records that a field is the parent navigation for an item-collection
//! child. The parent type is read from the field's `Deferred<T>` type at
//! a later validation step (after all model attributes are gathered).

use syn::spanned::Spanned;

#[derive(Debug, Clone)]
pub(crate) struct ItemParentAttr {
    /// Span of the attribute, used for diagnostics.
    pub(crate) span: proc_macro2::Span,
}

impl ItemParentAttr {
    pub(super) fn from_ast(attr: &syn::Attribute) -> syn::Result<Self> {
        if !matches!(attr.meta, syn::Meta::Path(_)) {
            return Err(syn::Error::new_spanned(
                attr,
                "`#[item_parent]` takes no arguments; the parent type is read from the field's `Deferred<T>` type",
            ));
        }
        Ok(Self { span: attr.span() })
    }
}

/// Synthesised relation kind for `#[item_parent]` fields.
///
/// Created by the model post-pass in [`super::Model::from_ast`] when a field
/// carries `#[item_parent]`. Mirrors [`super::BelongsTo`] but carries no
/// foreign-key columns: an item-collection child encodes its parent in its
/// own partition + sort keys (R2.9), so navigation does not lower to a
/// value-equality join. The macro layer only retains the field's declared
/// `Deferred<T>` type; the target [`ModelId`](toasty_core::schema::app::ModelId)
/// is resolved at runtime through `<T as Register>::id()`.
#[derive(Debug)]
pub(crate) struct ItemParent {
    /// The field's declared type — `Deferred<Parent>` from the user.
    pub(crate) ty: syn::Type,
}

/// Extract `T` from a `Deferred<T>` type. Returns `Err` if the type is
/// not `Deferred<T>` shaped.
pub(crate) fn extract_deferred_inner(
    field_name: &syn::Ident,
    field_ty: &syn::Type,
) -> syn::Result<syn::Type> {
    let syn::Type::Path(type_path) = field_ty else {
        return Err(syn::Error::new_spanned(
            field_ty,
            format!(
                "`#[item_parent]` field `{field_name}` must be `Deferred<T>`; found a non-path type. Toasty does not support eager parent loading."
            ),
        ));
    };

    let last = type_path.path.segments.last().ok_or_else(|| {
        syn::Error::new_spanned(field_ty, "empty type path on `#[item_parent]` field")
    })?;

    if last.ident != "Deferred" {
        return Err(syn::Error::new_spanned(
            field_ty,
            format!(
                "`#[item_parent]` field `{field_name}` must be `Deferred<T>`; found `{}`. Toasty does not support eager parent loading.",
                quote::quote! { #field_ty }
            ),
        ));
    }

    let arg_count = match &last.arguments {
        syn::PathArguments::AngleBracketed(args) => args.args.len(),
        _ => 0,
    };
    if arg_count != 1 {
        return Err(syn::Error::new_spanned(
            field_ty,
            format!(
                "`#[item_parent]` field `{field_name}`: `Deferred` accepts exactly one type argument"
            ),
        ));
    }

    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        unreachable!("arg_count == 1 implies AngleBracketed");
    };
    let syn::GenericArgument::Type(inner) = args.args.first().expect("arg_count == 1") else {
        return Err(syn::Error::new_spanned(
            field_ty,
            "`Deferred`'s type argument must be a type",
        ));
    };

    Ok(inner.clone())
}
