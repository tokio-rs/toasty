mod primitive;
pub use primitive::{FieldPrimitive, SerializeFormat};

use super::{
    AutoStrategy, BelongsTo, Constraint, Embedded, HasMany, HasOne, Model, ModelId, Schema,
    VariantId,
};
use crate::{Result, driver, stmt};
use std::fmt;

/// A single field within a model.
///
/// Fields are the building blocks of a model's data structure. Each field has a
/// unique [`FieldId`], a name, a type (primitive, embedded, or relation), and
/// metadata such as nullability, primary-key membership, auto-population
/// strategy, and validation constraints.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::app::{Field, Schema};
///
/// let schema: Schema = /* ... */;
/// let model = schema.model(model_id).as_root_unwrap();
/// for field in &model.fields {
///     println!("{}: primary_key={}", field.name.app_name_or_unnamed(), field.primary_key);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Field {
    /// Uniquely identifies this field within its containing model.
    pub id: FieldId,

    /// The field's application and storage names.
    pub name: FieldName,

    /// The field's type: primitive, embedded, or a relation variant.
    pub ty: FieldTy,

    /// `true` if this field accepts `None` / `NULL` values.
    pub nullable: bool,

    /// `true` if this field is part of the model's primary key.
    pub primary_key: bool,

    /// If set, Toasty automatically populates this field on insert.
    pub auto: Option<AutoStrategy>,

    /// Validation constraints applied to this field's values.
    pub constraints: Vec<Constraint>,

    /// If this field belongs to an enum variant, identifies that variant.
    /// `None` for fields on root models and embedded structs.
    pub variant: Option<VariantId>,
}

/// Uniquely identifies a [`Field`] within a schema.
///
/// Composed of the owning model's [`ModelId`] and a positional index into that
/// model's field list.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::{FieldId, ModelId};
///
/// let id = FieldId { model: ModelId(0), index: 2 };
/// assert_eq!(id.index, 2);
/// ```
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FieldId {
    /// The model this field belongs to.
    pub model: ModelId,
    /// Positional index within the model's field list.
    pub index: usize,
}

/// The name of a field, with separate application and storage representations.
///
/// The `app_name` is the Rust-facing name (e.g., `user_name`). It is
/// `Option<String>` to support unnamed (tuple) fields in the future; for now it
/// is always `Some`. The optional `storage_name` overrides the column name used
/// in the database; when `None`, the `app_name` is used as the storage name.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::FieldName;
///
/// let name = FieldName {
///     app_name: Some("user_name".to_string()),
///     storage_name: Some("username".to_string()),
/// };
/// assert_eq!(name.storage_name(), "username");
///
/// let default_name = FieldName {
///     app_name: Some("email".to_string()),
///     storage_name: None,
/// };
/// assert_eq!(default_name.storage_name(), "email");
/// ```
#[derive(Debug, Clone)]
pub struct FieldName {
    /// The application-level (Rust) name of the field. `None` for unnamed
    /// (tuple) fields.
    pub app_name: Option<String>,
    /// Optional override for the database column name. When `None`, `app_name`
    /// is used.
    pub storage_name: Option<String>,
}

impl FieldName {
    /// Returns the application-level name, or `"<unnamed>"` when the field has
    /// no app name (unnamed / tuple field).
    pub fn app_name_or_unnamed(&self) -> &str {
        self.app_name.as_deref().unwrap_or("<unnamed>")
    }

    /// Returns the storage (database column) name for this field.
    ///
    /// Falls back to [`app_name`](FieldName::app_name) when no explicit
    /// storage name is set.
    ///
    /// # Panics
    ///
    /// Panics if both `storage_name` and `app_name` are `None`.
    pub fn storage_name(&self) -> &str {
        self.storage_name
            .as_deref()
            .or(self.app_name.as_deref())
            .expect("FieldName must have at least one of app_name or storage_name")
    }
}

/// The type of a [`Field`], distinguishing primitives, embedded types, and
/// relation variants.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::{FieldPrimitive, FieldTy};
/// use toasty_core::stmt::Type;
///
/// let ty = FieldTy::Primitive(FieldPrimitive {
///     ty: Type::String,
///     storage_ty: None,
///     serialize: None,
/// });
/// assert!(ty.is_primitive());
/// assert!(!ty.is_relation());
/// ```
#[derive(Clone)]
pub enum FieldTy {
    /// A primitive (scalar) field backed by a single column.
    Primitive(FieldPrimitive),
    /// An embedded struct or enum, flattened into the parent table.
    Embedded(Embedded),
    /// The owning side of a relationship (stores the foreign key).
    BelongsTo(BelongsTo),
    /// The inverse side of a one-to-many relationship.
    HasMany(HasMany),
    /// The inverse side of a one-to-one relationship.
    HasOne(HasOne),
}

impl Field {
    /// Returns this field's [`FieldId`].
    pub fn id(&self) -> FieldId {
        self.id
    }

    /// Returns a reference to this field's [`FieldName`].
    pub fn name(&self) -> &FieldName {
        &self.name
    }

    /// Returns a reference to this field's [`FieldTy`].
    pub fn ty(&self) -> &FieldTy {
        &self.ty
    }

    /// Returns `true` if this field is nullable.
    pub fn nullable(&self) -> bool {
        self.nullable
    }

    /// Returns `true` if this field is part of the primary key.
    pub fn primary_key(&self) -> bool {
        self.primary_key
    }

    /// Returns the auto-population strategy, if one is configured.
    pub fn auto(&self) -> Option<&AutoStrategy> {
        self.auto.as_ref()
    }

    /// Returns `true` if this field uses auto-increment for value generation.
    pub fn is_auto_increment(&self) -> bool {
        self.auto().map(|auto| auto.is_increment()).unwrap_or(false)
    }

    /// Returns `true` if this field is a relation (`BelongsTo`, `HasMany`, or
    /// `HasOne`).
    pub fn is_relation(&self) -> bool {
        self.ty.is_relation()
    }

    /// Returns a fully qualified name for the field.
    pub fn full_name(&self, schema: &Schema) -> String {
        let model = schema.model(self.id.model);
        format!(
            "{}::{}",
            model.name().upper_camel_case(),
            self.name.app_name_or_unnamed()
        )
    }

    /// If the field is a relation, return the relation's target ModelId.
    pub fn relation_target_id(&self) -> Option<ModelId> {
        match &self.ty {
            FieldTy::BelongsTo(belongs_to) => Some(belongs_to.target),
            FieldTy::HasMany(has_many) => Some(has_many.target),
            _ => None,
        }
    }

    /// If the field is a relation, return the target of the relation.
    pub fn relation_target<'a>(&self, schema: &'a Schema) -> Option<&'a Model> {
        self.relation_target_id().map(|id| schema.model(id))
    }

    /// Returns the expression type this field evaluates to.
    ///
    /// For primitives this is the scalar type; for relations and embedded types
    /// it is the type visible to the application layer.
    pub fn expr_ty(&self) -> &stmt::Type {
        match &self.ty {
            FieldTy::Primitive(primitive) => &primitive.ty,
            FieldTy::Embedded(embedded) => &embedded.expr_ty,
            FieldTy::BelongsTo(belongs_to) => &belongs_to.expr_ty,
            FieldTy::HasMany(has_many) => &has_many.expr_ty,
            FieldTy::HasOne(has_one) => &has_one.expr_ty,
        }
    }

    /// Returns the paired relation field, if this field is a relation.
    ///
    /// For `BelongsTo` this returns the inverse `HasMany`/`HasOne` (if linked).
    /// For `HasMany` and `HasOne` this returns the paired `BelongsTo`.
    /// Returns `None` for primitive and embedded fields.
    pub fn pair(&self) -> Option<FieldId> {
        match &self.ty {
            FieldTy::Primitive(_) => None,
            FieldTy::Embedded(_) => None,
            FieldTy::BelongsTo(belongs_to) => belongs_to.pair,
            FieldTy::HasMany(has_many) => Some(has_many.pair),
            FieldTy::HasOne(has_one) => Some(has_one.pair),
        }
    }

    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        if let FieldTy::Primitive(primitive) = &self.ty
            && let Some(storage_ty) = &primitive.storage_ty
        {
            storage_ty.verify(db)?;
        }

        Ok(())
    }
}

impl FieldTy {
    /// Returns `true` if this is a [`FieldTy::Primitive`].
    pub fn is_primitive(&self) -> bool {
        matches!(self, Self::Primitive(..))
    }

    /// Returns the inner [`FieldPrimitive`] if this is a primitive field.
    pub fn as_primitive(&self) -> Option<&FieldPrimitive> {
        match self {
            Self::Primitive(primitive) => Some(primitive),
            _ => None,
        }
    }

    /// Returns the inner [`FieldPrimitive`], panicking if this is not a
    /// primitive field.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not [`FieldTy::Primitive`].
    #[track_caller]
    pub fn as_primitive_unwrap(&self) -> &FieldPrimitive {
        match self {
            Self::Primitive(simple) => simple,
            _ => panic!("expected simple field, but was {self:?}"),
        }
    }

    /// Returns a mutable reference to the inner [`FieldPrimitive`], panicking
    /// if this is not a primitive field.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not [`FieldTy::Primitive`].
    #[track_caller]
    pub fn as_primitive_mut_unwrap(&mut self) -> &mut FieldPrimitive {
        match self {
            Self::Primitive(simple) => simple,
            _ => panic!("expected simple field, but was {self:?}"),
        }
    }

    /// Returns `true` if this is a [`FieldTy::Embedded`].
    pub fn is_embedded(&self) -> bool {
        matches!(self, Self::Embedded(..))
    }

    /// Returns the inner [`Embedded`] if this is an embedded field.
    pub fn as_embedded(&self) -> Option<&Embedded> {
        match self {
            Self::Embedded(embedded) => Some(embedded),
            _ => None,
        }
    }

    /// Returns the inner [`Embedded`], panicking if this is not an embedded
    /// field.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not [`FieldTy::Embedded`].
    #[track_caller]
    pub fn as_embedded_unwrap(&self) -> &Embedded {
        match self {
            Self::Embedded(embedded) => embedded,
            _ => panic!("expected embedded field, but was {self:?}"),
        }
    }

    /// Returns a mutable reference to the inner [`Embedded`], panicking if
    /// this is not an embedded field.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not [`FieldTy::Embedded`].
    #[track_caller]
    pub fn as_embedded_mut_unwrap(&mut self) -> &mut Embedded {
        match self {
            Self::Embedded(embedded) => embedded,
            _ => panic!("expected embedded field, but was {self:?}"),
        }
    }

    /// Returns `true` if this is a relation type (`BelongsTo`, `HasMany`, or
    /// `HasOne`).
    pub fn is_relation(&self) -> bool {
        matches!(
            self,
            Self::BelongsTo(..) | Self::HasMany(..) | Self::HasOne(..)
        )
    }

    /// Returns `true` if this is a `HasMany` or `HasOne` relation.
    pub fn is_has_n(&self) -> bool {
        matches!(self, Self::HasMany(..) | Self::HasOne(..))
    }

    /// Returns `true` if this is a [`FieldTy::HasMany`].
    pub fn is_has_many(&self) -> bool {
        matches!(self, Self::HasMany(..))
    }

    /// Returns the inner [`HasMany`] if this is a has-many field.
    pub fn as_has_many(&self) -> Option<&HasMany> {
        match self {
            Self::HasMany(has_many) => Some(has_many),
            _ => None,
        }
    }

    /// Returns the inner [`HasMany`], panicking if this is not a has-many
    /// field.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not [`FieldTy::HasMany`].
    #[track_caller]
    pub fn as_has_many_unwrap(&self) -> &HasMany {
        match self {
            Self::HasMany(has_many) => has_many,
            _ => panic!("expected field to be `HasMany`, but was {self:?}"),
        }
    }

    /// Returns a mutable reference to the inner [`HasMany`], panicking if
    /// this is not a has-many field.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not [`FieldTy::HasMany`].
    #[track_caller]
    pub fn as_has_many_mut_unwrap(&mut self) -> &mut HasMany {
        match self {
            Self::HasMany(has_many) => has_many,
            _ => panic!("expected field to be `HasMany`, but was {self:?}"),
        }
    }

    /// Returns the inner [`HasOne`] if this is a has-one field.
    pub fn as_has_one(&self) -> Option<&HasOne> {
        match self {
            Self::HasOne(has_one) => Some(has_one),
            _ => None,
        }
    }

    /// Returns `true` if this is a [`FieldTy::HasOne`].
    pub fn is_has_one(&self) -> bool {
        matches!(self, Self::HasOne(..))
    }

    /// Returns the inner [`HasOne`], panicking if this is not a has-one field.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not [`FieldTy::HasOne`].
    #[track_caller]
    pub fn as_has_one_unwrap(&self) -> &HasOne {
        match self {
            Self::HasOne(has_one) => has_one,
            _ => panic!("expected field to be `HasOne`, but it was {self:?}"),
        }
    }

    /// Returns a mutable reference to the inner [`HasOne`], panicking if
    /// this is not a has-one field.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not [`FieldTy::HasOne`].
    #[track_caller]
    pub fn as_has_one_mut_unwrap(&mut self) -> &mut HasOne {
        match self {
            Self::HasOne(has_one) => has_one,
            _ => panic!("expected field to be `HasOne`, but it was {self:?}"),
        }
    }

    /// Returns `true` if this is a [`FieldTy::BelongsTo`].
    pub fn is_belongs_to(&self) -> bool {
        matches!(self, Self::BelongsTo(..))
    }

    /// Returns the inner [`BelongsTo`] if this is a belongs-to field.
    pub fn as_belongs_to(&self) -> Option<&BelongsTo> {
        match self {
            Self::BelongsTo(belongs_to) => Some(belongs_to),
            _ => None,
        }
    }

    /// Returns the inner [`BelongsTo`], panicking if this is not a belongs-to
    /// field.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not [`FieldTy::BelongsTo`].
    #[track_caller]
    pub fn as_belongs_to_unwrap(&self) -> &BelongsTo {
        match self {
            Self::BelongsTo(belongs_to) => belongs_to,
            _ => panic!("expected field to be `BelongsTo`, but was {self:?}"),
        }
    }

    /// Returns a mutable reference to the inner [`BelongsTo`], panicking if
    /// this is not a belongs-to field.
    ///
    /// # Panics
    ///
    /// Panics if `self` is not [`FieldTy::BelongsTo`].
    #[track_caller]
    pub fn as_belongs_to_mut_unwrap(&mut self) -> &mut BelongsTo {
        match self {
            Self::BelongsTo(belongs_to) => belongs_to,
            _ => panic!("expected field to be `BelongsTo`, but was {self:?}"),
        }
    }
}

impl fmt::Debug for FieldTy {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive(ty) => ty.fmt(fmt),
            Self::Embedded(ty) => ty.fmt(fmt),
            Self::BelongsTo(ty) => ty.fmt(fmt),
            Self::HasMany(ty) => ty.fmt(fmt),
            Self::HasOne(ty) => ty.fmt(fmt),
        }
    }
}

impl FieldId {
    pub(crate) fn placeholder() -> Self {
        Self {
            model: ModelId::placeholder(),
            index: usize::MAX,
        }
    }
}

impl From<&Self> for FieldId {
    fn from(val: &Self) -> Self {
        *val
    }
}

impl From<&Field> for FieldId {
    fn from(val: &Field) -> Self {
        val.id
    }
}

impl From<FieldId> for usize {
    fn from(val: FieldId) -> Self {
        val.index
    }
}

impl fmt::Debug for FieldId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "FieldId({}/{})", self.model.0, self.index)
    }
}
