use crate::schema::db;

use hashbrown::{HashMap, HashSet};

/// A single change to a named enum type between two schema versions.
///
/// Enum types are not top-level schema objects — they are embedded in column
/// definitions. Diffing schemas collects named `TypeEnum` types by scanning
/// columns in both schemas.
pub enum Type<'a> {
    /// A new named enum type must be created.
    Create(&'a db::TypeEnum),

    /// An existing named enum type is renamed without changing its identity.
    Rename {
        /// The enum type before the rename.
        previous: &'a db::TypeEnum,
        /// The enum type after the rename.
        next: &'a db::TypeEnum,
    },

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
    /// schemas. A new name is a rename when the same columns use the type in
    /// both schemas; otherwise, it creates a distinct type.
    ///
    /// # Panics
    ///
    /// Panics if existing variants were removed or reordered. Callers should
    /// validate schema transitions before computing the diff.
    pub(super) fn diff(previous: &'a db::Schema, next: &'a db::Schema) -> Vec<Self> {
        let prev_types = collect_named_enums(previous);
        let next_types = collect_named_enums(next);

        let mut changes = Vec::new();
        let mut renamed = HashSet::new();

        for (name, next_type) in &next_types {
            match prev_types.get(name) {
                None => {
                    let rename = prev_types.iter().find(|(previous_name, previous_type)| {
                        !next_types.contains_key(*previous_name)
                            && !renamed.contains(*previous_name)
                            && previous_type.columns == next_type.columns
                            && variants_are_prefix(previous_type.ty, next_type.ty)
                    });

                    if let Some((previous_name, previous_type)) = rename {
                        changes.push(Self::Rename {
                            previous: previous_type.ty,
                            next: next_type.ty,
                        });
                        renamed.insert(*previous_name);
                        add_variants(&mut changes, name, previous_type.ty, next_type.ty);
                    } else {
                        changes.push(Self::Create(next_type.ty));
                    }
                }
                Some(previous_type) => {
                    add_variants(&mut changes, name, previous_type.ty, next_type.ty)
                }
            }
        }

        changes
    }
}

struct NamedEnum<'a> {
    ty: &'a db::TypeEnum,
    columns: HashSet<(&'a str, &'a str, bool)>,
}

fn collect_named_enums(schema: &db::Schema) -> HashMap<&str, NamedEnum<'_>> {
    let mut result = HashMap::new();
    for table in &schema.tables {
        for column in &table.columns {
            if let Some(type_enum) = column.storage_ty.named_enum()
                && let Some(name) = &type_enum.name
            {
                result
                    .entry(name.as_str())
                    .or_insert_with(|| NamedEnum {
                        ty: type_enum,
                        columns: HashSet::new(),
                    })
                    .columns
                    .insert((
                        table.name.as_str(),
                        column.name.as_str(),
                        matches!(column.storage_ty, db::Type::List(_)),
                    ));
            }
        }
    }
    result
}

fn variants_are_prefix(previous: &db::TypeEnum, next: &db::TypeEnum) -> bool {
    previous.variants.len() <= next.variants.len()
        && previous
            .variants
            .iter()
            .zip(&next.variants)
            .all(|(previous, next)| previous.name == next.name)
}

fn add_variants<'a>(
    changes: &mut Vec<Type<'a>>,
    name: &str,
    previous: &'a db::TypeEnum,
    next: &'a db::TypeEnum,
) {
    assert!(
        next.variants.len() >= previous.variants.len(),
        "enum type `{name}`: removing variants is not supported; previous had {} variants, next has {}",
        previous.variants.len(),
        next.variants.len()
    );

    for (i, (previous, next)) in previous.variants.iter().zip(&next.variants).enumerate() {
        assert!(
            previous.name == next.name,
            "enum type `{name}`: variant at position {i} changed from `{}` to `{}`; \
             reordering or renaming variants is not supported",
            previous.name,
            next.name
        );
    }

    if next.variants.len() > previous.variants.len() {
        changes.push(Type::AddVariants {
            ty: next,
            added: next.variants[previous.variants.len()..].iter().collect(),
        });
    }
}
