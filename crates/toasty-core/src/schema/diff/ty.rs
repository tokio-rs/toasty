use crate::schema::db;

use hashbrown::HashMap;

/// A single change to a named enum type between two schema versions.
///
/// Enum types are not top-level schema objects — they are embedded in column
/// definitions. [`Type::diff`] collects all named `TypeEnum` types from both
/// schemas (by scanning columns) and computes the changes.
pub enum Type<'a> {
    /// A new named enum type must be created.
    Create(&'a db::TypeEnum),

    /// An existing named enum type has new variants appended.
    AddVariants {
        /// The enum type after the change (contains all variants).
        ty: &'a db::TypeEnum,
        /// Only the newly added variants.
        added: Vec<&'a db::EnumVariant>,
    },
}

impl<'a> Type<'a> {
    /// Computes the enum type diff between two schemas.
    ///
    /// Collects all named `TypeEnum` types from column definitions in both
    /// schemas, matches them by name, and produces the appropriate changes.
    ///
    /// # Panics
    ///
    /// Panics if existing variants were removed or reordered. Callers should
    /// validate schema transitions before computing the diff.
    pub fn diff(previous: &'a db::Schema, next: &'a db::Schema) -> Vec<Self> {
        let prev_types = collect_named_enums(previous);
        let next_types = collect_named_enums(next);

        let mut changes = Vec::new();

        for (name, next_ty) in &next_types {
            match prev_types.get(name) {
                None => {
                    changes.push(Self::Create(next_ty));
                }
                Some(prev_ty) => {
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

                    for (i, prev_name) in prev_names.iter().enumerate() {
                        assert!(
                            next_names[i] == *prev_name,
                            "enum type `{name}`: variant at position {i} changed from \
                             `{prev_name}` to `{}`; reordering or renaming variants \
                             is not supported",
                            next_names[i]
                        );
                    }

                    if next_names.len() > prev_names.len() {
                        let added: Vec<&'a db::EnumVariant> =
                            next_ty.variants[prev_names.len()..].iter().collect();
                        changes.push(Self::AddVariants { ty: next_ty, added });
                    }
                }
            }
        }

        changes
    }
}

fn collect_named_enums(schema: &db::Schema) -> HashMap<&str, &db::TypeEnum> {
    let mut result = HashMap::new();
    for table in &schema.tables {
        for column in &table.columns {
            if let db::Type::Enum(type_enum) = &column.storage_ty
                && let Some(name) = &type_enum.name
            {
                result.entry(name.as_str()).or_insert(type_enum);
            }
        }
    }
    result
}
