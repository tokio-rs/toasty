use super::{BelongsTo, Column, ErrorSet, HasMany, HasOne, Name};
use super::column::ColumnType;

use syn::spanned::Spanned;

#[derive(Debug)]
pub(crate) struct Field {
    /// Index of field in the containing model
    pub(crate) id: usize,

    /// Field attributes
    pub(crate) attrs: FieldAttr,

    /// Field name
    pub(crate) name: Name,

    /// Field type
    pub(crate) ty: FieldTy,

    /// Identifier for setter method on update builder
    pub(crate) set_ident: syn::Ident,
}

#[derive(Debug)]
pub(crate) struct FieldAttr {
    /// True if the field is annotated with `#[key]`
    pub(crate) key: Option<syn::Attribute>,

    /// True if the field is annotated with `#[unique]`
    pub(crate) unique: bool,

    /// True if toasty should automatically set the value
    pub(crate) auto: bool,

    /// True if the field is indexed
    pub(crate) index: bool,

    /// Optional database column name and / or type
    pub(crate) column: Option<Column>,
}

#[derive(Debug)]
pub(crate) enum FieldTy {
    Primitive(syn::Type),
    BelongsTo(BelongsTo),
    HasMany(HasMany),
    HasOne(HasOne),
}

impl Field {
    pub(super) fn from_ast(
        field: &syn::Field,
        model_ident: &syn::Ident,
        id: usize,
        names: &[syn::Ident],
    ) -> syn::Result<Self> {
        let Some(ident) = &field.ident else {
            return Err(syn::Error::new_spanned(field, "model fields must be named"));
        };

        let name = Name::from_ident(ident);
        let set_ident = syn::Ident::new(&format!("set_{}", name.ident), ident.span());

        let mut errs = ErrorSet::new();
        let mut attrs = FieldAttr {
            key: None,
            unique: false,
            auto: false,
            index: false,
            column: None,
        };

        let mut ty = None;

        for attr in &field.attrs {
            if attr.path().is_ident("key") {
                if attrs.key.is_some() {
                    errs.push(syn::Error::new_spanned(attr, "duplicate #[key] attribute"));
                } else {
                    attrs.key = Some(attr.clone());
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
            } else if attr.path().is_ident("belongs_to") {
                if ty.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "field has more than one relation attribute",
                    ));
                } else {
                    ty = Some(FieldTy::BelongsTo(BelongsTo::from_ast(
                        attr, &field.ty, names,
                    )?));
                }
            } else if attr.path().is_ident("has_many") {
                if ty.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "field has more than one relation attribute",
                    ));
                } else {
                    ty = Some(FieldTy::HasMany(HasMany::from_ast(
                        attr,
                        ident,
                        &field.ty,
                        field.span(),
                    )?));
                }
            } else if attr.path().is_ident("has_one") {
                if ty.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "field has more than one relation attribute",
                    ));
                } else {
                    ty = Some(FieldTy::HasOne(HasOne::from_ast(&field.ty, field.span())?));
                }
            } else if attr.path().is_ident("column") {
                if attrs.column.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[column] attribute",
                    ));
                } else {
                    attrs.column = Some(Column::from_ast(attr)?);
                }
            } else if attr.path().is_ident("toasty") {
                // todo
            }
        }

        if ty.is_some() && attrs.column.is_some() {
            errs.push(syn::Error::new_spanned(
                field,
                "relation fields cannot have a database type",
            ));
        }

        // Validate column type compatibility for primitive fields
        if ty.is_none() && attrs.column.is_some() {
            if let Err(validation_error) = validate_column_type_compatibility(
                &field.ty,
                attrs.column.as_ref().unwrap(),
                &field.ident.as_ref().unwrap().to_string(),
                field.span(),
            ) {
                errs.push(validation_error);
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        let mut ty = ty.unwrap_or_else(|| FieldTy::Primitive(field.ty.clone()));

        match &mut ty {
            FieldTy::BelongsTo(rel) => {
                rewrite_self(&mut rel.ty, model_ident);
            }
            FieldTy::HasMany(rel) => {
                rewrite_self(&mut rel.ty, model_ident);
            }
            FieldTy::HasOne(rel) => {
                rewrite_self(&mut rel.ty, model_ident);
            }
            FieldTy::Primitive(ty) => {
                rewrite_self(ty, model_ident);
            }
        }

        Ok(Self {
            id,
            attrs,
            name,
            ty,
            set_ident,
        })
    }
}

fn rewrite_self(ty: &mut syn::Type, model: &syn::Ident) {
    use syn::visit_mut::VisitMut;

    struct RewriteSelf<'a>(&'a syn::Ident);

    impl VisitMut for RewriteSelf<'_> {
        fn visit_path_mut(&mut self, path: &mut syn::Path) {
            syn::visit_mut::visit_path_mut(self, path);

            if path.is_ident("Self") {
                // print!("SELF; ident={:#?}", self.0);
                path.segments[0].ident = self.0.clone();
            }
        }
    }

    RewriteSelf(model).visit_type_mut(ty);
}

/// Validates that the specified column type is compatible with the Rust field type
fn validate_column_type_compatibility(
    field_type: &syn::Type,
    column_type: &Column,
    field_name: &str,
    field_span: proc_macro2::Span,
) -> syn::Result<()> {
    // Only validate if a column type is explicitly specified
    let Some(ref col_ty) = column_type.ty else {
        return Ok(());
    };

    // Extract the base type from the Rust field type (handles Option<T>, etc.)
    let base_type = extract_base_type(field_type);

    // Check compatibility based on the base type using direct AST pattern matching
    if is_string_type(&base_type) {
        match col_ty {
            ColumnType::Text | ColumnType::VarChar(_) => Ok(()),
            _ => Err(syn::Error::new(
                field_span,
                format!(
                    "Invalid column type '{}' for field of type 'String'\n\n\
                     = note: String fields are compatible with: text, varchar(n)\n\
                     = help: Did you mean: #[column(type = text)]?",
                    format_column_type(col_ty)
                ),
            )),
        }
    } else if let Some((size, is_signed)) = get_integer_info(&base_type) {
        validate_integer_type(col_ty, size, is_signed, &extract_type_name(&base_type), field_name, field_span)
    } else if is_uuid_type(&base_type) {
        match col_ty {
            ColumnType::Text | ColumnType::Blob | ColumnType::Binary(16) => Ok(()),
            _ => Err(syn::Error::new(
                field_span,
                format!(
                    "Invalid column type '{}' for field of type 'uuid::Uuid'\n\n\
                     = note: UUID fields are compatible with: text, blob, binary(16)\n\
                     = help: Did you mean: #[column(type = text)]?",
                    format_column_type(col_ty)
                ),
            )),
        }
    } else if is_bool_type(&base_type) {
        match col_ty {
            ColumnType::Boolean => Ok(()),
            _ => Err(syn::Error::new(
                field_span,
                format!(
                    "Invalid column type '{}' for field of type 'bool'\n\n\
                     = note: Boolean fields are compatible with: boolean\n\
                     = help: Did you mean: #[column(type = boolean)]?",
                    format_column_type(col_ty)
                ),
            )),
        }
    } else {
        // For unknown or unsupported types, we don't validate
        // This allows custom types and future type additions
        Ok(())
    }
}

/// Returns integer type information (size in bytes, is_signed) for known integer types
fn get_integer_info(ty: &syn::Type) -> Option<(u8, bool)> {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if type_path.path.segments.len() == 1 {
                match segment.ident.to_string().as_str() {
                    "i8" => Some((1, true)),
                    "i16" => Some((2, true)),
                    "i32" => Some((4, true)),
                    "i64" => Some((8, true)),
                    "isize" => Some((8, true)), // Maps to i64
                    "u8" => Some((1, false)),
                    "u16" => Some((2, false)),
                    "u32" => Some((4, false)),
                    "u64" => Some((8, false)),
                    "usize" => Some((8, false)), // Maps to u64
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// Extracts the type name from a syn::Type for error messages
fn extract_type_name(ty: &syn::Type) -> String {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if type_path.path.segments.len() == 1 {
                return segment.ident.to_string();
            }
        }
    }
    "unknown".to_string()
}

/// Validates integer type compatibility with size requirements
fn validate_integer_type(
    col_ty: &ColumnType,
    min_bytes: u8,
    is_signed: bool,
    type_name: &str,
    field_name: &str,
    field_span: proc_macro2::Span,
) -> syn::Result<()> {
    match col_ty {
        ColumnType::Integer(size) if is_signed => {
            if *size < min_bytes {
                Err(syn::Error::new(
                    field_span,
                    format!(
                        "Invalid column type 'integer({})' for field '{}: {}'\n\n\
                         = note: {} requires at least {} bytes of storage\n\
                         = help: Valid storage types for {}: {}",
                        size,
                        field_name,
                        type_name,
                        type_name,
                        min_bytes,
                        type_name,
                        generate_valid_integer_suggestions(min_bytes, true)
                    ),
                ))
            } else {
                Ok(())
            }
        }
        ColumnType::UnsignedInteger(size) if !is_signed => {
            if *size < min_bytes {
                Err(syn::Error::new(
                    field_span,
                    format!(
                        "Invalid column type 'unsignedinteger({})' for field '{}: {}'\n\n\
                         = note: {} requires at least {} bytes of storage\n\
                         = help: Valid storage types for {}: {}",
                        size,
                        field_name,
                        type_name,
                        type_name,
                        min_bytes,
                        type_name,
                        generate_valid_integer_suggestions(min_bytes, false)
                    ),
                ))
            } else {
                Ok(())
            }
        }
        ColumnType::Integer(_) if !is_signed => Err(syn::Error::new(
            field_span,
            format!(
                "Invalid column type '{}' for unsigned integer field\n\n\
                 = note: Unsigned integer types are not compatible with signed storage\n\
                 = help: Valid storage types: {}",
                format_column_type(col_ty),
                generate_valid_integer_suggestions(min_bytes, false)
            ),
        )),
        ColumnType::UnsignedInteger(_) if is_signed => Err(syn::Error::new(
            field_span,
            format!(
                "Invalid column type '{}' for field '{}: {}'\n\n\
                 = note: Signed integer types are not compatible with unsigned storage\n\
                 = help: Valid storage types for {}: {}",
                format_column_type(col_ty),
                field_name,
                type_name,
                type_name,
                generate_valid_integer_suggestions(min_bytes, true)
            ),
        )),
        _ => Err(syn::Error::new(
            field_span,
            format!(
                "Invalid column type '{}' for integer field\n\n\
                 = help: Valid storage types: {}",
                format_column_type(col_ty),
                generate_valid_integer_suggestions(min_bytes, is_signed)
            ),
        )),
    }
}

/// Extracts the base type from a potentially wrapped type like Option<T>
fn extract_base_type(ty: &syn::Type) -> syn::Type {
    match ty {
        syn::Type::Path(type_path) => {
            let path = &type_path.path;
            if let Some(segment) = path.segments.last() {
                // Handle Option<T>
                if segment.ident == "Option" {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                            return extract_base_type(inner_type);
                        }
                    }
                }
                // Handle uuid::Uuid
                if path.segments.len() == 2
                    && path.segments[0].ident == "uuid"
                    && path.segments[1].ident == "Uuid"
                {
                    return syn::parse_quote!(Uuid);
                }
            }
            ty.clone()
        }
        _ => ty.clone(),
    }
}

/// Formats a ColumnType for display in error messages
fn format_column_type(col_ty: &ColumnType) -> String {
    match col_ty {
        ColumnType::Boolean => "boolean".to_string(),
        ColumnType::Integer(size) => format!("integer({})", size),
        ColumnType::UnsignedInteger(size) => format!("unsignedinteger({})", size),
        ColumnType::Text => "text".to_string(),
        ColumnType::VarChar(size) => format!("varchar({})", size),
        ColumnType::Binary(size) => format!("binary({})", size),
        ColumnType::Blob => "blob".to_string(),
        ColumnType::Timestamp(precision) => format!("timestamp({})", precision),
        ColumnType::Date => "date".to_string(),
        ColumnType::Time(precision) => format!("time({})", precision),
        ColumnType::DateTime(precision) => format!("datetime({})", precision),
        ColumnType::Custom(custom) => custom.value(),
    }
}

/// Generates helpful suggestions for valid integer storage types
fn generate_valid_integer_suggestions(min_bytes: u8, is_signed: bool) -> String {
    let prefix = if is_signed { "integer" } else { "unsignedinteger" };
    let valid_sizes: Vec<u8> = [1, 2, 4, 8]
        .into_iter()
        .filter(|&size| size >= min_bytes)
        .collect();
    
    valid_sizes
        .into_iter()
        .map(|size| format!("{}({})", prefix, size))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Checks if the type is String using direct AST pattern matching
fn is_string_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "String" && type_path.path.segments.len() == 1;
        }
    }
    false
}


/// Checks if the type is uuid::Uuid using direct AST pattern matching
fn is_uuid_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        let path = &type_path.path;
        if path.segments.len() == 2 {
            return path.segments[0].ident == "uuid" && path.segments[1].ident == "Uuid";
        }
        // Also check for the bare "Uuid" type that extract_base_type produces
        if path.segments.len() == 1 {
            return path.segments[0].ident == "Uuid";
        }
    }
    false
}

/// Checks if the type is bool using direct AST pattern matching
fn is_bool_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "bool" && type_path.path.segments.len() == 1;
        }
    }
    false
}
