use super::{
    Column, ColumnType, ErrorSet, Field, Index, IndexField, IndexScope, ModelAttr, Name,
    PrimaryKey, Variant, VariantValue,
};
use heck::ToSnakeCase;

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
    pub(crate) fn as_root(&self) -> Option<&ModelRoot> {
        match self {
            ModelKind::Root(root) => Some(root),
            _ => None,
        }
    }

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

    /// Index of the versionable field, if any
    pub(crate) version_field: Option<usize>,

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

    /// True when variant fields are named (struct-like `Foo { a: T }`), false
    /// for tuple-like (`Foo(T)`). Unused when `fields` is empty.
    pub(crate) fields_named: bool,

    /// True when the struct has `#[auto]`: the embedded type is a newtype
    /// whose `Auto` impl is generated to proxy the strategy from its single
    /// inner field's type.
    pub(crate) auto: bool,
}

/// How the enum discriminant column is stored in the database.
#[derive(Debug)]
pub(crate) enum EnumStorageStrategy {
    /// Native database enum type (default for string-label enums).
    /// The optional string is a custom PostgreSQL type name.
    NativeEnum(Option<String>),
    /// Plain text/varchar column, no database-level enum enforcement.
    PlainString(ColumnType),
}

#[derive(Debug)]
pub(crate) struct ModelEmbeddedEnum {
    /// The field struct identifier (e.g., `ContactInfoFields`)
    pub(crate) field_struct_ident: syn::Ident,

    /// The list field struct identifier (e.g., `ContactInfoListFields`)
    pub(crate) field_list_struct_ident: syn::Ident,

    /// The enum's variants with their names and discriminant values
    pub(crate) variants: Vec<Variant>,

    /// Storage strategy for string-discriminant enums. `None` means integer
    /// discriminants (no enum-level storage strategy applies).
    pub(crate) storage_strategy: Option<EnumStorageStrategy>,
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
        let ast_fields = collect_ast_fields(&ast.fields)?;

        // Generics are not supported yet
        if !ast.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &ast.generics,
                "model generics are not supported",
            ));
        }

        // First, map field names to identifiers
        let mut names = vec![];

        for field in &ast_fields {
            if let Some(ident) = &field.ident {
                names.push(ident.clone());
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

        for (index, node) in ast_fields.iter().enumerate() {
            match Field::from_ast(node, &ast.ident, index, index, &names) {
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
            // Validate struct-level `#[auto]`: only meaningful on a single-field
            // newtype, since the generated `Auto` impl proxies to that field's
            // type via trait resolution.
            let auto = match model_attr.auto.take() {
                Some(attr) => {
                    if fields.len() != 1 {
                        return Err(syn::Error::new_spanned(
                            attr,
                            "struct-level #[auto] requires exactly one field",
                        ));
                    }
                    true
                }
                None => false,
            };

            ModelKind::EmbeddedStruct(ModelEmbeddedStruct {
                field_struct_ident: struct_ident("Fields", ast),
                field_list_struct_ident: struct_list_ident("ListFields", ast),
                update_struct_ident: struct_ident("Update", ast),
                fields_named: matches!(ast.fields, syn::Fields::Named(_)),
                auto,
            })
        } else {
            // Struct-level `#[auto]` is only meaningful on embedded newtypes.
            if let Some(attr) = &model_attr.auto {
                return Err(syn::Error::new_spanned(
                    attr,
                    "struct-level #[auto] is only supported on `#[derive(Embed)]` newtypes; \
                     place #[auto] on a model field instead",
                ));
            }
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

            // Iterate all extras rather than bailing on the first so every
            // offending site surfaces in one compile pass.
            let mut version_iter = fields
                .iter()
                .enumerate()
                .filter(|(_, f)| f.attrs.versionable);
            let version_field = version_iter.next().map(|(i, _)| i);
            let mut extra_err: Option<syn::Error> = None;
            for (_, extra) in version_iter {
                let err = syn::Error::new_spanned(
                    &extra.name.ident,
                    "only one field may be annotated with #[version]",
                );
                match &mut extra_err {
                    Some(acc) => acc.combine(err),
                    None => extra_err = Some(err),
                }
            }
            if let Some(err) = extra_err {
                return Err(err);
            }

            ModelKind::Root(ModelRoot {
                primary_key: PrimaryKey { fields: pk_fields },
                version_field,
                field_struct_ident: struct_ident("Fields", ast),
                field_list_struct_ident: struct_list_ident("ListFields", ast),
                query_struct_ident: struct_ident("Query", ast),
                create_struct_ident: struct_ident("Create", ast),
                update_struct_ident: struct_ident("Update", ast),
            })
        };

        // Create indices for model-level #[index(...)] attributes
        for index_attr in &model_attr.indices {
            let mut index_fields = vec![];

            if index_attr.local.is_empty() {
                // Simple mode (e.g. `#[index(a, b, c)]`): all fields land in `partition`.
                // First field is the partition (hash) key, rest are local (sort) keys.
                let mut partition_iter = index_attr.partition.iter();
                if let Some(first) = partition_iter.next() {
                    let idx = names.iter().position(|n| n == first).unwrap();
                    index_fields.push(IndexField {
                        field: idx,
                        scope: IndexScope::Partition,
                    });
                    for field in partition_iter {
                        let idx = names.iter().position(|n| n == field).unwrap();
                        index_fields.push(IndexField {
                            field: idx,
                            scope: IndexScope::Local,
                        });
                    }
                }
            } else {
                // Named mode (e.g. `#[index(partition = a, partition = b, local = c)]`):
                // all `partition` fields are hash keys, all `local` fields are sort keys.
                for field in &index_attr.partition {
                    let idx = names.iter().position(|n| n == field).unwrap();
                    index_fields.push(IndexField {
                        field: idx,
                        scope: IndexScope::Partition,
                    });
                }
                for field in &index_attr.local {
                    let idx = names.iter().position(|n| n == field).unwrap();
                    index_fields.push(IndexField {
                        field: idx,
                        scope: IndexScope::Local,
                    });
                }
            }

            indices.push(Index {
                fields: index_fields,
                unique: false,
                primary_key: false,
            });
        }

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

    pub(crate) fn from_enum_ast(ast: &syn::ItemEnum) -> syn::Result<Self> {
        use super::variant::VariantAttr;

        if !ast.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &ast.generics,
                "enum generics are not supported",
            ));
        }

        // Parse enum-level #[column(type = ...)] attribute to determine storage strategy.
        let mut enum_column_type: Option<ColumnType> = None;
        for attr in &ast.attrs {
            if attr.path().is_ident("column") {
                let col = Column::from_ast(attr)?;
                if let Some(ty) = col.ty {
                    if enum_column_type.is_some() {
                        return Err(syn::Error::new_spanned(
                            attr,
                            "duplicate #[column(type = ...)] attribute on enum",
                        ));
                    }
                    enum_column_type = Some(ty);
                }
            }
        }

        let mut variants = vec![];
        let mut all_fields: Vec<Field> = vec![];
        let mut errs = ErrorSet::new();
        let mut global_field_index = 0usize;
        let model_ident = ast.ident.clone();

        // Parse all variants in a single pass, defaulting omitted discriminants
        // to string labels using the variant identifier converted to snake_case.
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

            // Resolve discriminant: explicit value or default to variant name in snake_case
            let attr = match explicit_attr {
                Some(a) => a,
                None => VariantAttr {
                    discriminant: VariantValue::String(variant.ident.to_string().to_snake_case()),
                },
            };

            let ast_fields = collect_ast_fields(&variant.fields)?;
            let names = ast_fields
                .iter()
                .filter_map(|ast_field| ast_field.ident.clone())
                .collect::<Vec<_>>();

            for (index, ast_field) in ast_fields.iter().enumerate() {
                let mut field =
                    Field::from_ast(ast_field, &model_ident, global_field_index, index, &names)?;
                if field.attrs.deferred {
                    errs.push(syn::Error::new_spanned(
                        ast_field,
                        "#[deferred] is not yet supported on embedded enum variant fields",
                    ));
                }
                field.variant = Some(variant_index);
                all_fields.push(field);
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

        // Validate string label lengths (max 63 bytes).
        for v in &variants {
            if let VariantValue::String(s) = &v.attrs.discriminant
                && s.len() > 63
            {
                errs.push(syn::Error::new_spanned(
                    &v.ident,
                    format!(
                        "variant label \"{}\" is {} bytes; maximum is 63",
                        s,
                        s.len()
                    ),
                ));
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        // Determine storage strategy for string-discriminant enums.
        let uses_strings = variants
            .first()
            .map(|v| v.attrs.discriminant.is_string())
            .unwrap_or(false);

        let storage_strategy = if uses_strings {
            match enum_column_type {
                // Explicit `#[column(type = text)]` or `#[column(type = varchar(N))]`
                // opts out of native enum storage.
                Some(ty) if ty.is_string_like() => Some(EnumStorageStrategy::PlainString(ty)),
                // Explicit `#[column(type = enum)]` or `#[column(type = enum("name"))]`
                Some(ColumnType::Enum(custom_name)) => {
                    Some(EnumStorageStrategy::NativeEnum(custom_name))
                }
                // No explicit type attribute: default to native enum.
                None => Some(EnumStorageStrategy::NativeEnum(None)),
                // Any other type override on an enum is an error.
                Some(_) => {
                    return Err(syn::Error::new_spanned(
                        ast,
                        "unsupported #[column(type = ...)] for enum; \
                         use `text`, `varchar(N)`, or `enum` / `enum(\"name\")`",
                    ));
                }
            }
        } else {
            // Integer discriminants: enum-level type override is not applicable.
            if enum_column_type.is_some() {
                return Err(syn::Error::new_spanned(
                    ast,
                    "#[column(type = ...)] is not supported for integer-discriminant enums",
                ));
            }
            None
        };

        let mut indices = vec![];
        collect_field_indices(&all_fields, &mut indices);

        Ok(Self {
            vis: ast.vis.clone(),
            name: Name::from_ident(&ast.ident),
            ident: model_ident,
            fields: all_fields,
            kind: ModelKind::EmbeddedEnum(ModelEmbeddedEnum {
                field_struct_ident: enum_ident("Fields", ast),
                field_list_struct_ident: enum_list_ident("ListFields", ast),
                variants,
                storage_strategy,
            }),
            indices,
            table: None,
        })
    }
}

fn collect_ast_fields(ast: &syn::Fields) -> syn::Result<Vec<&syn::Field>> {
    Ok(match ast {
        syn::Fields::Named(f) => f.named.iter().collect::<Vec<_>>(),
        syn::Fields::Unnamed(f) => {
            if f.unnamed.len() > 1 {
                return Err(syn::Error::new_spanned(
                    ast,
                    "tuple structs (besides new-type) are not supported",
                ));
            }

            f.unnamed.iter().collect::<Vec<_>>()
        }
        _ => vec![],
    })
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
