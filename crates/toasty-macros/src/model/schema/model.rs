use super::{
    ErrorSet, Field, FieldAttr, FieldTy, Index, IndexField, IndexScope, ModelAttr, Name,
    PrimaryKey, Variant, VariantValue,
};

#[derive(Debug)]
pub(crate) enum ModelKind {
    /// Root model with table, primary key, and query builders
    Root(ModelRoot),
    /// Embedded struct model that is flattened into parent
    EmbeddedStruct(ModelEmbeddedStruct),
    /// Embedded enum stored as a single integer discriminant column
    EmbeddedEnum(ModelEmbeddedEnum),
}

impl ModelKind {
    pub(crate) fn as_root_unwrap(&self) -> &ModelRoot {
        match self {
            ModelKind::Root(root) => root,
            ModelKind::EmbeddedStruct(_) => panic!("expected root model, found embedded struct"),
            ModelKind::EmbeddedEnum(_) => panic!("expected root model, found embedded enum"),
        }
    }

    pub(crate) fn as_embedded_unwrap(&self) -> &ModelEmbeddedStruct {
        match self {
            ModelKind::EmbeddedStruct(embedded) => embedded,
            ModelKind::Root(_) => panic!("expected embedded struct, found root model"),
            ModelKind::EmbeddedEnum(_) => panic!("expected embedded struct, found embedded enum"),
        }
    }

    pub(crate) fn as_embedded_enum_unwrap(&self) -> &ModelEmbeddedEnum {
        match self {
            ModelKind::EmbeddedEnum(e) => e,
            ModelKind::Root(_) => panic!("expected embedded enum, found root model"),
            ModelKind::EmbeddedStruct(_) => panic!("expected embedded enum, found embedded struct"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ModelRoot {
    /// Tracks fields in the primary key
    pub(crate) primary_key: PrimaryKey,

    /// The field struct identifier (e.g., `UserFields`)
    pub(crate) field_struct_ident: syn::Ident,

    /// The list field struct identifier (e.g., `UserListFields`)
    pub(crate) field_list_struct_ident: syn::Ident,

    /// The query struct identifier
    pub(crate) query_struct_ident: syn::Ident,

    /// Create builder struct identifier
    pub(crate) create_struct_ident: syn::Ident,

    /// Update builder struct identifier
    pub(crate) update_struct_ident: syn::Ident,
}

#[derive(Debug)]
pub(crate) struct ModelEmbeddedStruct {
    /// The field struct identifier
    pub(crate) field_struct_ident: syn::Ident,

    /// The list field struct identifier (e.g., `AddressListFields`)
    pub(crate) field_list_struct_ident: syn::Ident,

    /// Update builder struct identifier
    pub(crate) update_struct_ident: syn::Ident,
}

#[derive(Debug)]
pub(crate) struct ModelEmbeddedEnum {
    /// The field struct identifier (e.g., `ContactInfoFields`)
    pub(crate) field_struct_ident: syn::Ident,

    /// The list field struct identifier (e.g., `ContactInfoListFields`)
    pub(crate) field_list_struct_ident: syn::Ident,

    /// The enum's variants with their names and discriminant values
    pub(crate) variants: Vec<Variant>,
}

impl ModelEmbeddedEnum {
    /// Returns true if this enum uses string discriminants.
    pub(crate) fn uses_string_discriminants(&self) -> bool {
        self.variants
            .first()
            .map(|v| matches!(v.attrs.discriminant, VariantValue::String(_)))
            .unwrap_or(false)
    }
}

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

    /// Distinguishes root models from embedded models
    pub(crate) kind: ModelKind,

    /// Model indices
    pub(crate) indices: Vec<Index>,

    /// Optional table to map the model to
    pub(crate) table: Option<syn::LitStr>,
}

impl Model {
    pub(crate) fn from_ast(ast: &syn::ItemStruct, is_embedded: bool) -> syn::Result<Self> {
        let syn::Fields::Named(node) = &ast.fields else {
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

        let mut model_attr = ModelAttr::default();
        let mut fields = vec![];
        let mut indices = vec![];
        let mut pk_index_fields = vec![];
        let mut errs = ErrorSet::new();

        if let Err(err) = model_attr.populate_from_ast(&ast.attrs, &names) {
            errs.push(err);
        }

        for (index, node) in node.named.iter().enumerate() {
            match Field::from_ast(node, &ast.ident, index, &names) {
                Ok(field) => {
                    if model_attr.key.is_some() {
                        if let Some(field) = &field.attrs.key {
                            errs.push(syn::Error::new_spanned(
                                field,
                                "field cannot have #[key] attribute when model has #[key] attribute",
                            ));
                        }
                    }

                    fields.push(field);
                }
                Err(err) => errs.push(err),
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        if let Some(attr) = &model_attr.key {
            for field in &attr.partition {
                let index = names.iter().position(|name| name == field).unwrap();
                pk_index_fields.push(IndexField {
                    field: index,
                    scope: IndexScope::Partition,
                });
            }

            for field in &attr.local {
                let index = names.iter().position(|name| name == field).unwrap();
                pk_index_fields.push(IndexField {
                    field: index,
                    scope: IndexScope::Local,
                });
            }
        } else {
            for (offset, field) in fields.iter().enumerate() {
                if field.attrs.key.is_some() {
                    pk_index_fields.push(IndexField {
                        field: offset,
                        scope: IndexScope::Partition,
                    });
                }
            }
        }

        // Return an error if no primary key fields were found (only for root models)
        if !is_embedded && pk_index_fields.is_empty() {
            return Err(syn::Error::new_spanned(
                ast,
                "model must either have a struct-level `#[key]` attribute or at least one field-level `#[key]` attribute",
            ));
        }

        // Build ModelKind based on whether this is embedded or root
        let kind = if is_embedded {
            ModelKind::EmbeddedStruct(ModelEmbeddedStruct {
                field_struct_ident: struct_ident("Fields", ast),
                field_list_struct_ident: struct_list_ident("ListFields", ast),
                update_struct_ident: struct_ident("Update", ast),
            })
        } else {
            let pk_fields = pk_index_fields
                .iter()
                .map(|index_field| index_field.field)
                .collect();

            // Create an index for the primary key
            indices.push(Index {
                fields: pk_index_fields,
                unique: true,
                primary_key: true,
            });

            ModelKind::Root(ModelRoot {
                primary_key: PrimaryKey { fields: pk_fields },
                field_struct_ident: struct_ident("Fields", ast),
                field_list_struct_ident: struct_list_ident("ListFields", ast),
                query_struct_ident: struct_ident("Query", ast),
                create_struct_ident: struct_ident("Create", ast),
                update_struct_ident: struct_ident("Update", ast),
            })
        };

        // Create indices for all fields annotated with unique or index
        collect_field_indices(&fields, &mut indices);

        Ok(Self {
            vis: ast.vis.clone(),
            name: Name::from_ident(&ast.ident),
            ident: ast.ident.clone(),
            fields,
            kind,
            indices,
            table: model_attr.table,
        })
    }

    pub fn primary_key_fields(&self) -> Option<impl ExactSizeIterator<Item = &'_ Field>> {
        match &self.kind {
            ModelKind::Root(root) => Some(
                root.primary_key
                    .fields
                    .iter()
                    .map(|index| &self.fields[*index]),
            ),
            ModelKind::EmbeddedStruct(_) | ModelKind::EmbeddedEnum(_) => None,
        }
    }

    pub(crate) fn has_associations(&self) -> bool {
        self.fields.iter().any(|f| f.ty.is_relation())
    }

    pub(crate) fn from_enum_ast(ast: &syn::ItemEnum) -> syn::Result<Self> {
        use super::variant::VariantAttr;

        if !ast.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &ast.generics,
                "enum generics are not supported",
            ));
        }

        let mut variants = vec![];
        let mut all_fields: Vec<Field> = vec![];
        let mut errs = ErrorSet::new();
        let mut global_field_index = 0usize;

        // First pass: parse variant attributes to determine discriminant kind.
        // Collect (variant_index, parsed VariantAttr or None, has_fields, variant_field_pairs)
        struct VariantInfo<'a> {
            variant: &'a syn::Variant,
            attr: Option<VariantAttr>,
            has_fields: bool,
        }

        let mut variant_infos = vec![];
        let mut has_any_integer = false;
        let mut has_any_string = false;
        let mut has_any_omitted = false;

        for variant in ast.variants.iter() {
            let variant_field_pairs: Vec<_> = match &variant.fields {
                syn::Fields::Unit => vec![],
                syn::Fields::Named(named) => named
                    .named
                    .iter()
                    .map(|f| (f.ident.as_ref().unwrap().clone(), f))
                    .collect(),
                syn::Fields::Unnamed(unnamed) => unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        (
                            syn::Ident::new(&format!("field{i}"), variant.ident.span()),
                            f,
                        )
                    })
                    .collect(),
            };
            let has_fields = !variant_field_pairs.is_empty();

            let attr = match VariantAttr::from_attrs(&variant.attrs) {
                Ok(a) => a,
                Err(e) => {
                    errs.push(e);
                    variant_infos.push(VariantInfo {
                        variant,
                        attr: None,
                        has_fields,
                    });
                    continue;
                }
            };

            match &attr {
                Some(a) => match &a.discriminant {
                    VariantValue::Integer(_) => has_any_integer = true,
                    VariantValue::String(_) => has_any_string = true,
                },
                None => has_any_omitted = true,
            }

            variant_infos.push(VariantInfo {
                variant,
                attr,
                has_fields,
            });
        }

        // Validate: cannot mix integer and string discriminants
        if has_any_integer && (has_any_string || has_any_omitted) {
            errs.push(syn::Error::new_spanned(
                ast,
                "cannot mix integer and string variant discriminants; \
                 all variants must use the same discriminant kind",
            ));
        }

        // If all variants have explicit integer discriminants, use integer mode (existing behavior).
        // Otherwise, use string mode: explicit string labels + default labels for omitted variants.
        let uses_string_discriminants = !has_any_integer;

        for (variant_index, info) in variant_infos.iter().enumerate() {
            // Collect fields
            let variant_field_pairs: Vec<_> = match &info.variant.fields {
                syn::Fields::Unit => vec![],
                syn::Fields::Named(named) => named
                    .named
                    .iter()
                    .map(|f| (f.ident.as_ref().unwrap().clone(), f))
                    .collect(),
                syn::Fields::Unnamed(unnamed) => unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        (
                            syn::Ident::new(&format!("field{i}"), info.variant.ident.span()),
                            f,
                        )
                    })
                    .collect(),
            };

            for (ident, f) in &variant_field_pairs {
                let attrs = match FieldAttr::from_attrs(&f.attrs) {
                    Ok(a) => a,
                    Err(e) => {
                        errs.push(e);
                        continue;
                    }
                };
                let name = Name::from_ident(ident);
                let set_ident = syn::Ident::new(&format!("set_{}", name.ident), ident.span());
                let with_ident = syn::Ident::new(&format!("with_{}", name.ident), ident.span());

                all_fields.push(Field {
                    id: global_field_index,
                    attrs,
                    name,
                    ty: FieldTy::Primitive(f.ty.clone()),
                    set_ident,
                    with_ident,
                    variant: Some(variant_index),
                });
                global_field_index += 1;
            }

            // Resolve discriminant: use explicit attribute or default to variant ident as string
            let resolved_attr = match &info.attr {
                Some(a) => VariantAttr {
                    discriminant: a.discriminant.clone(),
                },
                None => {
                    if uses_string_discriminants {
                        // Default: use variant identifier as string label
                        VariantAttr {
                            discriminant: VariantValue::String(info.variant.ident.to_string()),
                        }
                    } else {
                        // Integer mode but no attribute — error
                        errs.push(syn::Error::new_spanned(
                            info.variant,
                            "embedded enum variant must have a #[column(variant = N)] attribute",
                        ));
                        continue;
                    }
                }
            };

            match Variant::from_ast(info.variant, &ast.ident, info.has_fields, resolved_attr) {
                Ok(v) => variants.push(v),
                Err(e) => {
                    errs.push(e);
                    continue;
                }
            }
        }

        // Check for duplicate discriminant values
        if uses_string_discriminants {
            let mut seen = std::collections::HashMap::<&str, &syn::Ident>::new();
            for v in &variants {
                let label = match &v.attrs.discriminant {
                    VariantValue::String(s) => s.as_str(),
                    _ => unreachable!(),
                };
                if let Some(prev) = seen.get(label) {
                    errs.push(syn::Error::new_spanned(
                        &v.ident,
                        format!(
                            "duplicate variant label \"{}\"; already used by `{}`",
                            label, prev
                        ),
                    ));
                } else {
                    seen.insert(label, &v.ident);
                }
            }
        } else {
            let mut seen = std::collections::HashMap::<i64, &syn::Ident>::new();
            for v in &variants {
                let d = match &v.attrs.discriminant {
                    VariantValue::Integer(n) => *n,
                    _ => unreachable!(),
                };
                if let Some(prev) = seen.get(&d) {
                    errs.push(syn::Error::new_spanned(
                        &v.ident,
                        format!(
                            "duplicate variant value `{}`; already used by `{}`",
                            d, prev
                        ),
                    ));
                } else {
                    seen.insert(d, &v.ident);
                }
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        let mut indices = vec![];
        collect_field_indices(&all_fields, &mut indices);

        Ok(Self {
            vis: ast.vis.clone(),
            name: Name::from_ident(&ast.ident),
            ident: ast.ident.clone(),
            fields: all_fields,
            kind: ModelKind::EmbeddedEnum(ModelEmbeddedEnum {
                field_struct_ident: enum_ident("Fields", ast),
                field_list_struct_ident: enum_list_ident("ListFields", ast),
                variants,
            }),
            indices,
            table: None,
        })
    }
}

fn collect_field_indices(fields: &[Field], indices: &mut Vec<Index>) {
    for (index, field) in fields.iter().enumerate() {
        if field.attrs.is_indexed() {
            indices.push(Index {
                fields: vec![IndexField {
                    field: index,
                    scope: IndexScope::Partition,
                }],
                unique: field.attrs.unique,
                primary_key: false,
            });
        }
    }
}

fn struct_ident(suffix: &str, model: &syn::ItemStruct) -> syn::Ident {
    syn::Ident::new(&format!("{}{}", model.ident, suffix), model.ident.span())
}

/// Generates an ident like `UserListFields` — injects the suffix after the model name.
fn struct_list_ident(suffix: &str, model: &syn::ItemStruct) -> syn::Ident {
    syn::Ident::new(&format!("{}{}", model.ident, suffix), model.ident.span())
}

fn enum_ident(suffix: &str, model: &syn::ItemEnum) -> syn::Ident {
    syn::Ident::new(&format!("{}{}", model.ident, suffix), model.ident.span())
}

fn enum_list_ident(suffix: &str, model: &syn::ItemEnum) -> syn::Ident {
    syn::Ident::new(&format!("{}{}", model.ident, suffix), model.ident.span())
}
