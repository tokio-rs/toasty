use super::{EnumVariant, Schema, TypeEnum};

/// The diff between named enum types across two schema versions.
///
/// Enum types are not top-level schema objects — they are embedded in column
/// definitions. This diff collects all named `TypeEnum` types from both schemas
/// (by scanning columns) and computes the changes.
pub struct TypesDiff<'a> {
    items: Vec<TypesDiffItem<'a>>,
}

/// A single change to a named enum type.
pub enum TypesDiffItem<'a> {
    /// A new named enum type must be created.
    CreateType(&'a TypeEnum),

    /// An existing named enum type has new variants appended.
    AddVariants {
        /// The enum type after the change (contains all variants).
        ty: &'a TypeEnum,
        /// Only the newly added variants.
        added: Vec<&'a EnumVariant>,
    },
}

impl<'a> TypesDiff<'a> {
    /// Computes the enum type diff between two schemas.
    ///
    /// Collects all named `TypeEnum` types from column definitions in both
    /// schemas, matches them by name, and produces the appropriate diff items.
    ///
    /// # Panics
    ///
    /// Panics if existing variants were removed or reordered. Callers should
    /// validate schema transitions before computing the diff.
    pub fn from(previous: &'a Schema, next: &'a Schema) -> Self {
        let prev_types = collect_named_enums(previous);
        let next_types = collect_named_enums(next);

        let mut items = Vec::new();

        for (name, next_ty) in &next_types {
            match prev_types.get(name) {
                None => {
                    // New enum type
                    items.push(TypesDiffItem::CreateType(next_ty));
                }
                Some(prev_ty) => {
                    // Existing enum type — check for changes
                    let prev_names: Vec<&str> =
                        prev_ty.variants.iter().map(|v| v.name.as_str()).collect();
                    let next_names: Vec<&str> =
                        next_ty.variants.iter().map(|v| v.name.as_str()).collect();

                    assert!(
                        next_names.len() >= prev_names.len(),
                        "enum type `{name}`: removing variants is not supported; \
                         previous had {} variants, next has {}",
                        prev_names.len(),
                        next_names.len()
                    );

                    // The first N variants must match exactly (same names, same order)
                    for (i, prev_name) in prev_names.iter().enumerate() {
                        assert!(
                            next_names[i] == *prev_name,
                            "enum type `{name}`: variant at position {i} changed from \
                             `{prev_name}` to `{}`; reordering or renaming variants \
                             is not supported",
                            next_names[i]
                        );
                    }

                    // Collect newly added variants (appended after existing ones)
                    if next_names.len() > prev_names.len() {
                        let added: Vec<&'a EnumVariant> =
                            next_ty.variants[prev_names.len()..].iter().collect();
                        items.push(TypesDiffItem::AddVariants { ty: next_ty, added });
                    }
                }
            }
        }

        Self { items }
    }

    /// Returns an iterator over the diff items.
    pub fn iter(&self) -> impl Iterator<Item = &TypesDiffItem<'a>> {
        self.items.iter()
    }

    /// Returns `true` if there are no enum type changes.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Collects all named `TypeEnum` types from column definitions across all tables.
///
/// If the same type name appears on multiple columns, the first occurrence wins
/// (they must be identical per the schema verifier).
fn collect_named_enums(schema: &Schema) -> hashbrown::HashMap<&str, &TypeEnum> {
    let mut result = hashbrown::HashMap::new();
    for table in &schema.tables {
        for column in &table.columns {
            if let super::Type::Enum(type_enum) = &column.storage_ty
                && let Some(name) = &type_enum.name
            {
                result.entry(name.as_str()).or_insert(type_enum);
            }
        }
    }
    result
}
