use super::Expand;
use crate::model::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    /// Generate the `CreateMeta` constant for the Model impl.
    pub(super) fn expand_create_meta(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_name = self.model.ident.to_string();

        // Collect FK source field indices — fields referenced by any BelongsTo's key
        let mut fk_source_indices = std::collections::HashSet::new();
        for field in &self.model.fields {
            if let FieldTy::BelongsTo(bt) = &field.ty {
                for fk in &bt.foreign_key {
                    fk_source_indices.insert(fk.source);
                }
            }
        }

        // Build CreateField entries for eligible primitive fields
        let create_fields: Vec<_> = self
            .model
            .fields
            .iter()
            .filter(|f| {
                // TODO: support embedded model fields in CreateMeta
                if !matches!(&f.ty, FieldTy::Primitive(_)) {
                    return false;
                }
                // Not auto
                if f.attrs.auto.is_some() {
                    return false;
                }
                // Not default
                if f.attrs.default_expr.is_some() {
                    return false;
                }
                // Not update
                if f.attrs.update_expr.is_some() {
                    return false;
                }
                // TODO: support #[serialize] fields in CreateMeta (type may not implement Field)
                if f.attrs.serialize.is_some() {
                    return false;
                }
                // Not a FK source field
                if fk_source_indices.contains(&f.id) {
                    return false;
                }
                true
            })
            .map(|f| {
                let name = f.name.ident.to_string();
                let ty = match &f.ty {
                    FieldTy::Primitive(ty) => ty,
                    _ => unreachable!(),
                };
                quote! {
                    #toasty::CreateField {
                        name: #name,
                        required: !<#ty as #toasty::Field>::NULLABLE,
                    }
                }
            })
            .collect();

        // Build CreateBelongsTo entries
        let belongs_to_entries: Vec<_> = self
            .model
            .fields
            .iter()
            .filter_map(|f| {
                if let FieldTy::BelongsTo(bt) = &f.ty {
                    let name = f.name.ident.to_string();
                    let fk_field_names: Vec<_> = bt
                        .foreign_key
                        .iter()
                        .map(|fk| {
                            let source_field = &self.model.fields[fk.source];
                            source_field.name.ident.to_string()
                        })
                        .collect();
                    Some(quote! {
                        #toasty::CreateBelongsTo {
                            name: #name,
                            fk_fields: &[ #( #fk_field_names ),* ],
                        }
                    })
                } else {
                    None
                }
            })
            .collect();

        // Build CreateNested entries for HasMany and HasOne relations.
        // Self-referential relations are skipped to avoid const evaluation
        // cycles: the `CreateNested` references the target model's
        // `CREATE_META`, and when the target is the same model, that creates
        // a cycle in the const dependency graph.
        let nested_entries: Vec<_> =
            self.model
                .fields
                .iter()
                .filter_map(|f| match &f.ty {
                    FieldTy::HasMany(rel) => {
                        if is_self_referential_type(&rel.ty, &self.model.ident) {
                            return None;
                        }
                        let name = f.name.ident.to_string();
                        let ty = &rel.ty;
                        let pair =
                            rel.pair.as_ref().map(|p| p.to_string()).unwrap_or_else(|| {
                                self.model.name.ident.to_string().to_lowercase()
                            });
                        Some(quote! {
                            #toasty::CreateNested {
                                name: #name,
                                meta: &<#ty as #toasty::Relation>::Model::CREATE_META,
                                pair: #pair,
                            }
                        })
                    }
                    FieldTy::HasOne(rel) => {
                        if is_self_referential_type(&rel.ty, &self.model.ident) {
                            return None;
                        }
                        let name = f.name.ident.to_string();
                        let ty = &rel.ty;
                        let pair = self.model.name.ident.to_string().to_lowercase();
                        Some(quote! {
                            #toasty::CreateNested {
                                name: #name,
                                meta: &<#ty as #toasty::Relation>::Model::CREATE_META,
                                pair: #pair,
                            }
                        })
                    }
                    _ => None,
                })
                .collect();

        quote! {
            #toasty::CreateMeta {
                fields: &[ #( #create_fields ),* ],
                nested: &[ #( #nested_entries ),* ],
                belongs_to: &[ #( #belongs_to_entries ),* ],
                model_name: #model_name,
            }
        }
    }
}

/// Check if a relation type (e.g. `HasMany<Person>`) references the same model.
///
/// This is needed because `CreateNested` contains a `&'static` reference to
/// the target model's `CREATE_META`. When the target is the same model (e.g.
/// `HasMany<Person>` on `Person`), the const would reference itself, creating
/// a cycle that the compiler rejects.
fn is_self_referential_type(ty: &syn::Type, model_ident: &syn::Ident) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(last_seg) = type_path.path.segments.last()
        && let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments
    {
        for arg in &args.args {
            if let syn::GenericArgument::Type(syn::Type::Path(inner_path)) = arg
                && let Some(inner_seg) = inner_path.path.segments.last()
                && inner_seg.ident == *model_ident
            {
                return true;
            }
        }
    }
    false
}
