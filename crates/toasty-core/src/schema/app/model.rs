use super::{Field, FieldId, FieldPrimitive, Index, Name, PrimaryKey};
use crate::{driver, stmt, Result};
use std::fmt;

/// A model in the application schema.
///
/// Models come in three flavors:
///
/// - [`Model::Root`] -- a top-level model backed by its own database table.
/// - [`Model::EmbeddedStruct`] -- a struct whose fields are flattened into a
///   parent model's table.
/// - [`Model::EmbeddedEnum`] -- an enum stored via a discriminant column plus
///   optional per-variant data columns in the parent table.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::app::{Model, Schema};
///
/// let schema: Schema = /* built from derive macros */;
/// for model in schema.models() {
///     if model.is_root() {
///         println!("Root model: {}", model.name().upper_camel_case());
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub enum Model {
    /// A root model that maps to its own database table and can be queried
    /// directly.
    Root(ModelRoot),
    /// An embedded struct whose fields are flattened into its parent model's
    /// table.
    EmbeddedStruct(EmbeddedStruct),
    /// An embedded enum stored as a discriminant integer column (plus optional
    /// per-variant data columns) in the parent table.
    EmbeddedEnum(EmbeddedEnum),
}

/// A root model backed by its own database table.
///
/// Root models have a primary key, may define indices, and are the only model
/// kind that can be the target of relations. They are the main entities users
/// interact with through Toasty's query API.
///
/// # Examples
///
/// ```ignore
/// let root = model.as_root_unwrap();
/// let pk_fields: Vec<_> = root.primary_key_fields().collect();
/// ```
#[derive(Debug, Clone)]
pub struct ModelRoot {
    /// Uniquely identifies this model within the schema.
    pub id: ModelId,

    /// The model's name.
    pub name: Name,

    /// All fields defined on this model.
    pub fields: Vec<Field>,

    /// The primary key definition. Root models always have a primary key.
    pub primary_key: PrimaryKey,

    /// Optional explicit table name. When `None`, a name is derived from the
    /// model name.
    pub table_name: Option<String>,

    /// Secondary indices defined on this model.
    pub indices: Vec<Index>,
}

impl ModelRoot {
    /// Builds a `SELECT` query that filters by this model's primary key using
    /// the supplied `input` to resolve argument values.
    pub fn find_by_id(&self, mut input: impl stmt::Input) -> stmt::Query {
        let filter = match &self.primary_key.fields[..] {
            [pk_field] => stmt::Expr::eq(
                stmt::Expr::ref_self_field(pk_field),
                input
                    .resolve_arg(&0.into(), &stmt::Projection::identity())
                    .unwrap(),
            ),
            pk_fields => stmt::Expr::and_from_vec(
                pk_fields
                    .iter()
                    .enumerate()
                    .map(|(i, pk_field)| {
                        stmt::Expr::eq(
                            stmt::Expr::ref_self_field(pk_field),
                            input
                                .resolve_arg(&i.into(), &stmt::Projection::identity())
                                .unwrap(),
                        )
                    })
                    .collect(),
            ),
        };

        stmt::Query::new_select(self.id, filter)
    }

    /// Iterate over the fields used for the model's primary key.
    pub fn primary_key_fields(&self) -> impl ExactSizeIterator<Item = &'_ Field> {
        self.primary_key
            .fields
            .iter()
            .map(|pk_field| &self.fields[pk_field.index])
    }

    /// Looks up a field by its application-level name.
    ///
    /// Returns `None` if no field with that name exists on this model.
    pub fn field_by_name(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.name.app_name == name)
    }

    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        for field in &self.fields {
            field.verify(db)?;
        }
        Ok(())
    }
}

/// An embedded struct model whose fields are flattened into its parent model's
/// database table.
///
/// Embedded structs do not have their own table or primary key. Their fields
/// become additional columns in the parent table. Indices declared on an
/// embedded struct's fields are propagated to physical DB indices on the parent
/// table.
///
/// # Examples
///
/// ```ignore
/// let embedded = model.as_embedded_struct_unwrap();
/// for field in &embedded.fields {
///     println!("  embedded field: {}", field.name.app_name);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct EmbeddedStruct {
    /// Uniquely identifies this model within the schema.
    pub id: ModelId,

    /// The model's name.
    pub name: Name,

    /// Fields contained by this embedded struct.
    pub fields: Vec<Field>,

    /// Indices defined on this embedded struct's fields.
    ///
    /// These reference fields within this embedded struct (not the parent
    /// model). The schema builder propagates them to physical DB indices on
    /// the parent table's flattened columns.
    pub indices: Vec<Index>,
}

impl EmbeddedStruct {
    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        for field in &self.fields {
            field.verify(db)?;
        }
        Ok(())
    }
}

/// An embedded enum model stored in the parent table via a discriminant column
/// and optional per-variant data columns.
///
/// The discriminant column holds an integer identifying the active variant.
/// Variants may optionally carry data fields, which are stored as additional
/// nullable columns in the parent table.
///
/// # Examples
///
/// ```ignore
/// let ee = model.as_embedded_enum_unwrap();
/// for variant in &ee.variants {
///     println!("variant {} = {}", variant.name.upper_camel_case(), variant.discriminant);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct EmbeddedEnum {
    /// Uniquely identifies this model within the schema.
    pub id: ModelId,

    /// The model's name.
    pub name: Name,

    /// The primitive type used for the discriminant column.
    pub discriminant: FieldPrimitive,

    /// The enum's variants.
    pub variants: Vec<EnumVariant>,

    /// All fields across all variants, with global indices. Each field's
    /// [`variant`](Field::variant) identifies which variant it belongs to.
    pub fields: Vec<Field>,

    /// Indices defined on this embedded enum's variant fields.
    ///
    /// These reference fields within this embedded enum (not the parent
    /// model). The schema builder propagates them to physical DB indices on
    /// the parent table's flattened columns.
    pub indices: Vec<Index>,
}

/// One variant of an [`EmbeddedEnum`].
///
/// Each variant has a name and a discriminant integer that is stored in the
/// database to identify which variant is active.
///
/// # Examples
///
/// ```ignore
/// let variant = &embedded_enum.variants[0];
/// println!("{}: discriminant = {}", variant.name.upper_camel_case(), variant.discriminant);
/// ```
#[derive(Debug, Clone)]
pub struct EnumVariant {
    /// The Rust variant name.
    pub name: Name,

    /// The integer discriminant value stored in the database column.
    pub discriminant: i64,
}

impl EmbeddedEnum {
    /// Returns true if at least one variant carries data fields.
    pub fn has_data_variants(&self) -> bool {
        !self.fields.is_empty()
    }

    /// Returns fields belonging to a specific variant.
    pub fn variant_fields(&self, variant_index: usize) -> impl Iterator<Item = &Field> {
        let variant_id = VariantId {
            model: self.id,
            index: variant_index,
        };
        self.fields
            .iter()
            .filter(move |f| f.variant == Some(variant_id))
    }

    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        for field in &self.fields {
            field.verify(db)?;
        }
        Ok(())
    }
}

/// Uniquely identifies a [`Model`] within a [`Schema`](super::Schema).
///
/// `ModelId` wraps a `usize` index into the schema's model map. It is `Copy`
/// and can be used as a key for lookups.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::ModelId;
///
/// let id = ModelId(0);
/// let field_id = id.field(2);
/// assert_eq!(field_id.model, id);
/// assert_eq!(field_id.index, 2);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModelId(pub usize);

impl Model {
    /// Returns this model's [`ModelId`].
    pub fn id(&self) -> ModelId {
        match self {
            Model::Root(root) => root.id,
            Model::EmbeddedStruct(embedded) => embedded.id,
            Model::EmbeddedEnum(e) => e.id,
        }
    }

    /// Returns a reference to this model's [`Name`].
    pub fn name(&self) -> &Name {
        match self {
            Model::Root(root) => &root.name,
            Model::EmbeddedStruct(embedded) => &embedded.name,
            Model::EmbeddedEnum(e) => &e.name,
        }
    }

    /// Returns true if this is a root model (has a table and primary key)
    pub fn is_root(&self) -> bool {
        matches!(self, Model::Root(_))
    }

    /// Returns true if this is an embedded model (flattened into parent)
    pub fn is_embedded(&self) -> bool {
        matches!(self, Model::EmbeddedStruct(_) | Model::EmbeddedEnum(_))
    }

    /// Returns true if this model can be the target of a relation
    pub fn can_be_relation_target(&self) -> bool {
        self.is_root()
    }

    /// Returns the inner [`ModelRoot`] if this is a root model.
    pub fn as_root(&self) -> Option<&ModelRoot> {
        match self {
            Model::Root(root) => Some(root),
            _ => None,
        }
    }

    /// Returns a reference to the root model data.
    ///
    /// # Panics
    ///
    /// Panics if this is not a [`Model::Root`].
    pub fn as_root_unwrap(&self) -> &ModelRoot {
        match self {
            Model::Root(root) => root,
            Model::EmbeddedStruct(_) => panic!("expected root model, found embedded struct"),
            Model::EmbeddedEnum(_) => panic!("expected root model, found embedded enum"),
        }
    }

    /// Returns a mutable reference to the root model data.
    ///
    /// # Panics
    ///
    /// Panics if this is not a [`Model::Root`].
    pub fn as_root_mut_unwrap(&mut self) -> &mut ModelRoot {
        match self {
            Model::Root(root) => root,
            Model::EmbeddedStruct(_) => panic!("expected root model, found embedded struct"),
            Model::EmbeddedEnum(_) => panic!("expected root model, found embedded enum"),
        }
    }

    /// Returns a reference to the embedded struct data.
    ///
    /// # Panics
    ///
    /// Panics if this is not a [`Model::EmbeddedStruct`].
    pub fn as_embedded_struct_unwrap(&self) -> &EmbeddedStruct {
        match self {
            Model::EmbeddedStruct(embedded) => embedded,
            Model::Root(_) => panic!("expected embedded struct, found root model"),
            Model::EmbeddedEnum(_) => panic!("expected embedded struct, found embedded enum"),
        }
    }

    /// Returns a reference to the embedded enum data.
    ///
    /// # Panics
    ///
    /// Panics if this is not a [`Model::EmbeddedEnum`].
    pub fn as_embedded_enum_unwrap(&self) -> &EmbeddedEnum {
        match self {
            Model::EmbeddedEnum(e) => e,
            Model::Root(_) => panic!("expected embedded enum, found root model"),
            Model::EmbeddedStruct(_) => panic!("expected embedded enum, found embedded struct"),
        }
    }

    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        match self {
            Model::Root(root) => root.verify(db),
            Model::EmbeddedStruct(embedded) => embedded.verify(db),
            Model::EmbeddedEnum(e) => e.verify(db),
        }
    }
}

/// Identifies a specific variant within an [`EmbeddedEnum`] model.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::ModelId;
///
/// let variant_id = ModelId(1).variant(0);
/// assert_eq!(variant_id.model, ModelId(1));
/// assert_eq!(variant_id.index, 0);
/// ```
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct VariantId {
    /// The enum model this variant belongs to.
    pub model: ModelId,
    /// Index of the variant within `EmbeddedEnum::variants`.
    pub index: usize,
}

impl fmt::Debug for VariantId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "VariantId({}/{})", self.model.0, self.index)
    }
}

impl ModelId {
    /// Create a `FieldId` representing the current model's field at index
    /// `index`.
    pub const fn field(self, index: usize) -> FieldId {
        FieldId { model: self, index }
    }

    /// Create a `VariantId` representing the current model's variant at
    /// `index`.
    pub const fn variant(self, index: usize) -> VariantId {
        VariantId { model: self, index }
    }

    pub(crate) const fn placeholder() -> Self {
        Self(usize::MAX)
    }
}

impl From<&Self> for ModelId {
    fn from(src: &Self) -> Self {
        *src
    }
}

impl From<&mut Self> for ModelId {
    fn from(src: &mut Self) -> Self {
        *src
    }
}

impl From<&Model> for ModelId {
    fn from(value: &Model) -> Self {
        value.id()
    }
}

impl From<&ModelRoot> for ModelId {
    fn from(value: &ModelRoot) -> Self {
        value.id
    }
}

impl fmt::Debug for ModelId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "ModelId({})", self.0)
    }
}
