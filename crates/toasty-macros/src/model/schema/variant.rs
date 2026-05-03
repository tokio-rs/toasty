use super::{Column, ErrorSet, Name, VariantValue};

#[derive(Debug)]
pub(crate) struct Variant {
    /// Rust identifier for this variant (e.g., `Pending`)
    pub(crate) ident: syn::Ident,

    /// Name parts for schema generation
    pub(crate) name: Name,

    /// Variant attributes
    pub(crate) attrs: VariantAttr,

    /// True when variant fields are named (struct-like `Foo { a: T }`), false
    /// for tuple-like (`Foo(T)`). Unused when `fields` is empty.
    pub(crate) fields_named: bool,

    /// Ident for the `is_{variant}()` method (e.g., `is_email`)
    pub(crate) is_method_ident: syn::Ident,

    /// Variant handle struct identifier (e.g., `ContactInfoEmailVariant`).
    /// Only set for data-carrying variants. Returned by the
    /// `{variant_method}()` accessor on the parent `{Enum}Fields` struct;
    /// carries both the `matches(closure)` filter API and the variant-field
    /// path accessors.
    pub(crate) variant_handle_ident: Option<syn::Ident>,
}

#[derive(Debug)]
pub(crate) struct VariantAttr {
    /// Discriminant value stored in the database column.
    pub(crate) discriminant: VariantValue,
}

impl VariantAttr {
    pub(crate) fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Option<Self>> {
        let mut errs = ErrorSet::new();
        let mut discriminant = None;

        for attr in attrs {
            if attr.path().is_ident("column") {
                match Column::from_ast(attr) {
                    Ok(col) => {
                        if let Some(d) = col.variant {
                            discriminant = Some(d);
                        }
                    }
                    Err(e) => errs.push(e),
                }
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        Ok(discriminant.map(|d| VariantAttr { discriminant: d }))
    }
}

impl Variant {
    pub(crate) fn from_ast(
        variant: &syn::Variant,
        enum_ident: &syn::Ident,
        has_fields: bool,
        attrs: VariantAttr,
    ) -> syn::Result<Self> {
        let fields_named = matches!(&variant.fields, syn::Fields::Named(_));
        let name = Name::from_ident(&variant.ident);
        let is_method_ident =
            syn::Ident::new(&format!("is_{}", name.as_str()), variant.ident.span());

        let variant_handle_ident = if has_fields {
            Some(syn::Ident::new(
                &format!("{}{}Variant", enum_ident, variant.ident),
                variant.ident.span(),
            ))
        } else {
            None
        };

        Ok(Variant {
            ident: variant.ident.clone(),
            name,
            attrs,
            fields_named,
            is_method_ident,
            variant_handle_ident,
        })
    }
}
