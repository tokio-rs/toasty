//! Shared codegen for fields whose value passes through `#[serialize]` or
//! `#[deferred]` (or both). Each lifecycle stage (setter input, decode,
//! deferred wrap) lives in one place so adding a new combination changes one
//! site, not several.

use super::Expand;
use crate::model::schema::{Field, FieldTy};
use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    /// The user-facing type a setter accepts for a `#[serialize(...)]` field.
    ///
    /// `#[deferred]` wraps the storage type in `Deferred<T>`, but callers
    /// still pass `T`. Without this unwrap the setter would demand
    /// `Deferred<T>` and `serde_json::to_string` would encode the wrapper's
    /// `Option<T>` shape instead of the bare value.
    pub(super) fn serialize_setter_input_ty(&self, field: &Field) -> TokenStream {
        let toasty = &self.toasty;
        let FieldTy::Primitive(ty) = &field.ty else {
            unreachable!("serialize_setter_input_ty called on non-primitive field");
        };
        if field.attrs.deferred {
            quote!(<#ty as #toasty::Defer>::Inner)
        } else {
            quote!(#ty)
        }
    }

    /// Decode a `#[serialize(json)]` column value into the in-memory value.
    ///
    /// Returns a block expression. The caller must bind `value: stmt::Value`
    /// in the surrounding scope. The block evaluates to `T` for a
    /// non-nullable field or `Option<T>` for a nullable one — i.e. exactly
    /// what would be assigned to a non-`#[deferred]` field of the user's
    /// declared type.
    pub(super) fn expand_serialize_decode(&self, field: &Field) -> TokenStream {
        let toasty = &self.toasty;
        let field_name_str = field.name.as_str();

        let nullable = field.attrs.serialize.as_ref().is_some_and(|s| s.nullable);

        let json_decode = quote! {
            let json_str = <String as #toasty::Load>::load(value)?;
            #toasty::serde_json::from_str(&json_str)
                .map_err(|e| #toasty::Error::from_args(
                    format_args!("failed to deserialize field '{}': {}", #field_name_str, e)
                ))?
        };

        if nullable {
            quote! {
                if value.is_null() { None } else { Some({ #json_decode }) }
            }
        } else {
            quote! { { #json_decode } }
        }
    }

    /// Wrap an in-memory decoded value in `Deferred::from(...)`.
    ///
    /// Pins the intermediate type to `<Field as Defer>::Inner` so that `?`
    /// inside `decoded` (e.g. the JSON deserializer) infers concretely
    /// instead of collapsing to `!` under the generic `Deferred::from<T>`.
    pub(super) fn wrap_in_deferred(&self, field: &Field, decoded: TokenStream) -> TokenStream {
        let toasty = &self.toasty;
        let FieldTy::Primitive(ty) = &field.ty else {
            unreachable!("wrap_in_deferred called on non-primitive field");
        };
        let inner_ty = quote!(<#ty as #toasty::Defer>::Inner);
        quote! {
            {
                let parsed: #inner_ty = #decoded;
                #toasty::Deferred::from(parsed)
            }
        }
    }
}
