use proc_macro2::TokenStream;

pub(crate) fn int(v: usize) -> TokenStream {
    use std::str::FromStr;
    TokenStream::from_str(&v.to_string()).expect("failed to parse int")
}

/// Creates a new identifier prefixed with `__toasty_` to avoid name collisions
/// with user-defined types in generated code (e.g., generic type parameters).
pub(crate) fn ident(name: &str) -> syn::Ident {
    quote::format_ident!("__toasty_{name}")
}

/// If `ty` is syntactically `Vec<U>` for some `U` that is not literally `u8`,
/// returns `Some(&U)`. Used by the create/update builder macros to swap the
/// setter's `IntoExpr<Vec<U>>` / `Assign<Vec<U>>` bound to the
/// `IntoExpr<List<U>>` / `Assign<List<U>>` form used by the rest of the
/// expression API — see the docs on [`Scalar`](toasty::schema::Scalar) for
/// why `Vec<u8>` (bytes) is excluded.
///
/// Detection is purely syntactic: a user-defined alias like
/// `type Bytes = Vec<u8>` falls through to the `None` branch.
pub(crate) fn vec_scalar_inner(ty: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Path(tp) = ty else {
        return None;
    };
    if tp.qself.is_some() {
        return None;
    }
    if tp.path.segments.len() != 1 {
        return None;
    }
    let seg = &tp.path.segments[0];
    if seg.ident != "Vec" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    let syn::GenericArgument::Type(inner) = &args.args[0] else {
        return None;
    };
    // Exclude Vec<u8> — that's the bytes case, which round-trips through the
    // scalar `IntoExpr<Vec<u8>>` impl (not the per-element `List<u8>` path).
    if let syn::Type::Path(inner_tp) = inner
        && let Some(ident) = inner_tp.path.get_ident()
        && ident == "u8"
    {
        return None;
    }
    Some(inner)
}
