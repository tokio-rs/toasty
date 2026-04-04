use super::{
    ErrorSet, Field, FieldAttr, FieldTy, Index, IndexField, IndexScope, ModelAttr, Name,
    PrimaryKey, Variant, VariantValue, rewrite_self,
};

#[derive(Debug)]
pub(crate) enum ModelKind {
    /// Root model with table, primary key, and query builders
    Root(ModelRoot),
    /// Embedded struct model that is flattened into parent
    EmbeddedStruct(ModelEmbeddedStruct),
    /// Embedded enum stored as a discriminant column (integer or string)
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

    /// True when the embedded struct is a newtype wrapper: `struct Foo(Bar)`
    pub(crate) is_newtype: bool,
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
        // Check for newtype pattern: `struct Foo(Bar)` — only allowed for embedded structs
        if let syn::Fields::Unnamed(unnamed) = &ast.fields {
            if !is_embedded {
                return Err(syn::Error::new_spanned(
                    &ast.fields,
                    "root models must have named fields",
                ));
            }
            if unnamed.unnamed.len() != 1 {
                return Err(syn::Error::new_spanned(
                    &ast.fields,
                    "embedded newtype structs must have exactly one field",
                ));
            }
            return Self::from_newtype_ast(ast, &unnamed.unnamed[0]);
        }

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
                    if model_attr.key.is_some()
                        && let Some(field) = &field.attrs.key
                    {
                        errs.push(syn::Error::new_spanned(
                            field,
                            "field cannot have #[key] attribute when model has #[key] attribute",
                        ));
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
                is_newtype: false,
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

    /// Parse a newtype embedded struct: `struct Foo(Bar)`.
    ///
    /// Creates a single field with no application-level name (`app: None` in
    /// the schema) so that the column name collapses to the parent field's
    /// name — e.g. `email: Email` where `struct Email(String)` produces a
    /// column named `email` rather than `email_0`.
    fn from_newtype_ast(ast: &syn::ItemStruct, inner: &syn::Field) -> syn::Result<Self> {
        if !ast.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &ast.generics,
                "model generics are not supported",
            ));
        }

        let attrs = FieldAttr::from_attrs(&inner.attrs)?;

        // Use a synthetic ident `_0` for code generation purposes (setter
        // methods, etc.), but the schema will record `app: None`.
        let span = ast.ident.span();
        let name = Name {
            parts: vec!["_0".to_string()],
            ident: syn::Ident::new("_0", span),
        };
        let set_ident = syn::Ident::new("set_0", span);
        let with_ident = syn::Ident::new("with_0", span);

        let mut ty = FieldTy::Primitive(inner.ty.clone());
        if let FieldTy::Primitive(ref mut t) = ty {
            rewrite_self(t, &ast.ident);
        }

        let field = Field {
            id: 0,
            attrs,
            name,
            ty,
            set_ident,
            with_ident,
            variant: None,
        };

        Ok(Self {
            vis: ast.vis.clone(),
            name: Name::from_ident(&ast.ident),
            ident: ast.ident.clone(),
            fields: vec![field],
            kind: ModelKind::EmbeddedStruct(ModelEmbeddedStruct {
                field_struct_ident: struct_ident("Fields", ast),
                field_list_struct_ident: struct_list_ident("ListFields", ast),
                update_struct_ident: struct_ident("Update", ast),
                is_newtype: true,
            }),
            indices: vec![],
            table: None,
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

    pub(crate) fn is_newtype(&self) -> bool {
        matches!(&self.kind, ModelKind::EmbeddedStruct(e) if e.is_newtype)
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

        // Parse all variants in a single pass, defaulting omitted discriminants
        // to string labels using the variant identifier.
        for (variant_index, variant) in ast.variants.iter().enumerate() {
            let has_fields = !variant.fields.is_empty();

            // Parse variant attribute
            let explicit_attr = match VariantAttr::from_attrs(&variant.attrs) {
                Ok(a) => a,
                Err(e) => {
                    errs.push(e);
                    continue;
                }
            };

            // Resolve discriminant: explicit value or default to variant name as string
            let attr = match explicit_attr {
                Some(a) => a,
                None => VariantAttr {
                    discriminant: VariantValue::String(variant.ident.to_string()),
                },
            };

            // Collect variant data fields
            let field_pairs: Vec<_> = match &variant.fields {
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

            for (ident, f) in &field_pairs {
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

            match Variant::from_ast(variant, &ast.ident, has_fields, attr) {
                Ok(v) => variants.push(v),
                Err(e) => errs.push(e),
            }
        }

        // Validate: all discriminants must be the same kind (all integer or all string)
        if variants.iter().any(|v| v.attrs.discriminant.is_integer())
            && variants.iter().any(|v| v.attrs.discriminant.is_string())
        {
            errs.push(syn::Error::new_spanned(
                ast,
                "cannot mix integer and string variant discriminants; \
                 all variants must use the same discriminant kind",
            ));
        }

        // Validate: no duplicate discriminant values
        for (i, v) in variants.iter().enumerate() {
            for prev in &variants[..i] {
                if v.attrs.discriminant == prev.attrs.discriminant {
                    errs.push(syn::Error::new_spanned(
                        &v.ident,
                        format!(
                            "duplicate variant discriminant {}; already used by `{}`",
                            v.attrs.discriminant, prev.ident
                        ),
                    ));
                    break;
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
