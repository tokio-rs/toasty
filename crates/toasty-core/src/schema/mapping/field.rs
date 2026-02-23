use crate::{
    schema::db::ColumnId,
    stmt::{PathFieldSet, Projection},
};
use indexmap::IndexMap;

/// Maps a model field to its database storage representation.
///
/// Different field types have different storage strategies:
/// - Primitive fields map to a single column
/// - Struct fields flatten an embedded struct to multiple columns
/// - Enum fields map to a discriminant column plus per-variant data columns
/// - Relation fields (`BelongsTo`, `HasMany`, `HasOne`) don't have direct column storage
#[derive(Debug, Clone)]
pub enum Field {
    /// A primitive field stored in a single column.
    Primitive(FieldPrimitive),

    /// An embedded struct field flattened into multiple columns.
    Struct(FieldStruct),

    /// An embedded enum field stored as a discriminant column plus per-variant data columns.
    Enum(FieldEnum),

    /// A relation field that doesn't map to columns in this table.
    Relation(FieldRelation),
}

impl Field {
    /// Returns the update coverage mask for this field.
    ///
    /// Each primitive (leaf) field in the model is assigned a unique bit.
    /// The mask for a given mapping field is the set of those bits that
    /// correspond to the primitives it covers:
    ///
    /// - `Primitive` → singleton set containing only its own bit
    /// - `Struct`    → union of all nested primitive bits (recursively)
    /// - `Enum`      → singleton set (the whole enum value changes atomically)
    /// - `Relation`  → singleton set (assigned a bit for uniform tracking)
    ///
    /// Masks are used during update lowering to determine whether a partial
    /// update fully covers an embedded field or only touches some of its
    /// sub-fields. Intersecting `changed_mask` with a field's `field_mask`
    /// yields the subset of that field's primitives being updated; equality
    /// with the full `field_mask` means full coverage.
    pub fn field_mask(&self) -> PathFieldSet {
        match self {
            Field::Primitive(p) => p.field_mask.clone(),
            Field::Struct(s) => s.field_mask.clone(),
            Field::Enum(e) => e.field_mask.clone(),
            Field::Relation(r) => r.field_mask.clone(),
        }
    }

    /// Returns the sub-projection from the root model field to this field
    /// within the embedded type hierarchy. Identity for root-level fields.
    pub fn sub_projection(&self) -> &Projection {
        static IDENTITY: Projection = Projection::identity();
        match self {
            Field::Primitive(p) => &p.sub_projection,
            Field::Struct(s) => &s.sub_projection,
            Field::Enum(e) => &e.sub_projection,
            Field::Relation(_) => &IDENTITY,
        }
    }

    pub fn is_relation(&self) -> bool {
        matches!(self, Field::Relation(_))
    }

    pub fn as_primitive(&self) -> Option<&FieldPrimitive> {
        match self {
            Field::Primitive(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_primitive_mut(&mut self) -> Option<&mut FieldPrimitive> {
        match self {
            Field::Primitive(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_struct(&self) -> Option<&FieldStruct> {
        match self {
            Field::Struct(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_enum(&self) -> Option<&FieldEnum> {
        match self {
            Field::Enum(e) => Some(e),
            _ => None,
        }
    }

    /// Returns an iterator over all (column, lowering) pairs impacted by this field.
    ///
    /// For primitive fields, yields a single pair.
    /// For struct fields, yields all flattened columns.
    /// For enum fields, yields the discriminant column plus all variant data columns.
    /// For relation fields, yields nothing.
    pub fn columns(&self) -> impl Iterator<Item = (ColumnId, usize)> + '_ {
        match self {
            Field::Primitive(fp) => Box::new(std::iter::once((fp.column, fp.lowering)))
                as Box<dyn Iterator<Item = (ColumnId, usize)> + '_>,
            Field::Struct(fs) => Box::new(fs.columns.iter().map(|(k, v)| (*k, *v))),
            Field::Enum(fe) => Box::new(
                std::iter::once((fe.disc_column, fe.disc_lowering)).chain(
                    fe.variants
                        .iter()
                        .flat_map(|v| v.fields.iter().flat_map(|f| f.columns())),
                ),
            ),
            Field::Relation(_) => Box::new(std::iter::empty()),
        }
    }
}

/// Maps a primitive field to its table column.
#[derive(Debug, Clone)]
pub struct FieldPrimitive {
    /// The table column that stores this field's value.
    pub column: ColumnId,

    /// Index into `Model::model_to_table` for this field's lowering expression.
    ///
    /// The expression at this index converts the model field value to the
    /// column value during `INSERT` and `UPDATE` operations.
    pub lowering: usize,

    /// Update coverage mask for this primitive field.
    ///
    /// A singleton bitset containing the unique bit assigned to this primitive
    /// within the model's field mask space. During update lowering, accumulated
    /// `changed_mask` bits are intersected with each field's `field_mask` to
    /// determine which fields are affected by a partial update.
    pub field_mask: PathFieldSet,

    /// The projection from the root model field (the top-level embedded field
    /// containing this primitive) down to this primitive within the embedded
    /// type hierarchy. Identity for root-level primitives.
    ///
    /// Used when building `Returning::Changed` expressions: we emit
    /// `project(ref_self_field(root_field_id), sub_projection)` so the
    /// existing lowering and constantization pipeline resolves it to the
    /// correct column value without needing to carry assignment expressions.
    pub sub_projection: Projection,
}

/// Maps an embedded struct field to its flattened column representation.
///
/// Embedded fields are stored by flattening their primitive fields into columns
/// with names like `{field}_{embedded_field}`. This structure tracks the mapping
/// for each field in the embedded struct.
#[derive(Debug, Clone)]
pub struct FieldStruct {
    /// Per-field mappings for the embedded struct's fields.
    ///
    /// Indexed by field index within the embedded model.
    pub fields: Vec<Field>,

    /// Flattened mapping from columns to lowering expression indices.
    ///
    /// This map contains all columns impacted by this embedded field, paired
    /// with their corresponding lowering expression index in `Model::model_to_table`.
    pub columns: IndexMap<ColumnId, usize>,

    /// Update coverage mask for this embedded field.
    ///
    /// The union of the `field_mask` bits of every primitive nested within this
    /// embedded struct (recursively).
    pub field_mask: PathFieldSet,

    /// The projection from the root model field down to this embedded field
    /// within the type hierarchy. Identity for root-level embedded fields.
    pub sub_projection: Projection,
}

/// Maps an embedded enum field to its discriminant column and per-variant data columns.
///
/// The discriminant column always stores the active variant's integer discriminant.
/// Each data variant additionally has nullable columns for its fields; unit variants
/// have no extra columns (all variant-field columns are NULL for them).
#[derive(Debug, Clone)]
pub struct FieldEnum {
    /// The discriminant column ID.
    pub disc_column: ColumnId,

    /// Index into `Model::model_to_table` for the discriminant lowering expression.
    pub disc_lowering: usize,

    /// Per-variant mappings, in the same order as `app::EmbeddedEnum::variants`.
    pub variants: Vec<EnumVariant>,

    /// Update coverage mask for the enum field (singleton: the whole enum changes atomically).
    pub field_mask: PathFieldSet,

    /// Sub-projection from the root model field to this enum field.
    pub sub_projection: Projection,
}

/// Mapping for a single variant of an embedded enum.
#[derive(Debug, Clone)]
pub struct EnumVariant {
    /// The discriminant value for this variant.
    pub discriminant: i64,

    /// Field mappings for this variant's data fields, in declaration order.
    /// Empty for unit variants. Supports nesting (each entry is a full `Field`).
    pub fields: Vec<Field>,
}

/// Maps a relation field (`BelongsTo`, `HasMany`, `HasOne`).
///
/// Relations don't map to columns in this table — they are resolved through
/// joins or foreign keys in other tables. A unique bit is assigned in the
/// model's field mask space so that relation assignments are detected uniformly
/// through the same mask intersection logic used for primitive and embedded fields.
#[derive(Debug, Clone)]
pub struct FieldRelation {
    /// Update coverage mask for this relation field.
    pub field_mask: PathFieldSet,
}
