use crate::{
    schema::db::ColumnId,
    stmt::{PathFieldSet, Projection},
};
use indexmap::IndexMap;

/// Maps a model field to its database storage representation.
///
/// Different field types have different storage strategies:
/// - Primitive fields map to a single column
/// - Embedded fields flatten to multiple columns (one per primitive field in the embedded struct)
/// - Relation fields (`BelongsTo`, `HasMany`, `HasOne`) don't have direct column storage
#[derive(Debug, Clone)]
pub enum Field {
    /// A primitive field stored in a single column.
    Primitive(FieldPrimitive),

    /// An embedded struct field flattened into multiple columns.
    Embedded(FieldEmbedded),

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
    /// - `Embedded`  → union of all nested primitive bits (recursively)
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
            Field::Embedded(e) => e.field_mask.clone(),
            Field::Relation(r) => r.field_mask.clone(),
        }
    }

    /// Returns the sub-projection from the root model field to this field
    /// within the embedded type hierarchy. Identity for root-level fields.
    pub fn sub_projection(&self) -> &Projection {
        static IDENTITY: Projection = Projection::identity();
        match self {
            Field::Primitive(p) => &p.sub_projection,
            Field::Embedded(e) => &e.sub_projection,
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

    pub fn as_embedded(&self) -> Option<&FieldEmbedded> {
        match self {
            Field::Embedded(e) => Some(e),
            _ => None,
        }
    }

    /// Returns an iterator over all (column, lowering) pairs impacted by this field.
    ///
    /// For primitive fields, yields a single pair.
    /// For embedded fields, yields all columns in the flattened structure.
    /// For relation fields, yields nothing.
    pub fn columns(&self) -> impl Iterator<Item = (ColumnId, usize)> + '_ {
        let single = match self {
            Field::Primitive(fp) => Some((fp.column, fp.lowering)),
            _ => None,
        };
        let multi = match self {
            Field::Embedded(fe) => Some(fe.columns.iter().map(|(k, v)| (*k, *v))),
            _ => None,
        };
        single.into_iter().chain(multi.into_iter().flatten())
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
pub struct FieldEmbedded {
    /// Per-field mappings for the embedded struct's fields.
    ///
    /// Indexed by field index within the embedded model. Relation fields use
    /// `Field::Relation` (though they aren't allowed in embedded types).
    pub fields: Vec<Field>,

    /// Flattened mapping from columns to lowering expression indices.
    ///
    /// This map contains all columns impacted by this embedded field, paired
    /// with their corresponding lowering expression index in `Model::model_to_table`.
    /// This allows treating full embedded struct updates uniformly with partial
    /// updates and primitive field updates.
    pub columns: IndexMap<ColumnId, usize>,

    /// Update coverage mask for this embedded field.
    ///
    /// The union of the `field_mask` bits of every primitive nested within this
    /// embedded struct (recursively). During update lowering, intersecting
    /// `changed_mask` with this mask reveals whether the update has full coverage
    /// of the embedded field (intersection equals `field_mask`) or only partial
    /// coverage (recurse into sub-fields to build a `SparseRecord`).
    pub field_mask: PathFieldSet,

    /// The projection from the root model field down to this embedded field
    /// within the type hierarchy. Identity for root-level embedded fields.
    ///
    /// When the embedded field is fully covered by an update, we emit
    /// `project(ref_self_field(root_field_id), sub_projection)` to reference
    /// the entire embedded value for constantization.
    pub sub_projection: Projection,
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
    ///
    /// A singleton bitset giving the relation a unique bit in the model's field
    /// mask space. This lets relation assignments be detected uniformly through
    /// the same mask intersection logic used for primitive and embedded fields,
    /// even though relations have no column storage in this table.
    pub field_mask: PathFieldSet,
}
