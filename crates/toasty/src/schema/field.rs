use super::{Embed, Load};
use crate::stmt::{self, Expr, List};
use toasty_core::schema::{
    app::{Model, ModelSet},
    db,
};

/// Schema and runtime information for a field type.
///
/// This trait captures the information needed to register a field's type in the
/// app schema (nullability, [`FieldTy`](toasty_core::schema::app::FieldTy)) as
/// well as runtime helpers for building field paths and update builders.
/// It is used by the `schema()` implementation that the macro expands for
/// [`Model`](super::Model) and [`Embed`](super::Embed) types.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as a field type",
    label = "this field's type is not storable",
    note = "If `{Self}` is a Toasty model, this is a relation field and must be annotated \
            with `#[belongs_to]`, `#[has_one]`, or `#[has_many]`.",
    note = "Otherwise `{Self}` must be a supported primitive (string, integer, bool, uuid, \
            …) or a wrapper around one (`Option<_>`, `Vec<_>`, `Box<_>`, `Arc<_>`, `Rc<_>`)."
)]
pub trait Field: Load<Output = Self> {
    /// The expression-level type of this field.
    ///
    /// This drives both the field's path target (the second parameter of
    /// the underlying `Path`) **and** the `IntoExpr`/`Assign` bound on
    /// generated setters. For most types this is `Self`. For
    /// `Vec<T: Scalar>` it is [`List<T>`] so the field accessor returns a
    /// `Path<_, List<T>>` (giving access to `contains`, `is_superset`,
    /// `len`, …) and create/update setters accept any
    /// `impl IntoExpr<List<T>>` (Vec, slice, array literal, …).
    ///
    /// Decoupling this from `Self` lets the accessor macro construct the
    /// path with the right target type from the start, so `new_path` is
    /// identity for every primitive impl.
    type ExprTarget;

    /// The type returned when accessing this field from a Fields struct.
    /// For primitives, this is Path<Origin, Self>.
    /// For embedded types, this is {Type}Fields<Origin>.
    type Path<Origin>;

    /// The type returned when accessing this field from a list Fields struct.
    /// For primitives, this is Path<Origin, List<Self::ExprTarget>>.
    /// For embedded types, this is {Type}ListFields<Origin>.
    type ListPath<Origin>;

    /// The type of the update builder for this field.
    /// For embedded types, this is {Type}Update<'a>.
    /// For primitives, this will be {Type}Update<'a> once implemented.
    type Update<'a>;

    /// The unwrapped type used for foreign key filter expressions.
    /// Primitives map to themselves. Wrappers like `Option<T>`, `Box<T>`,
    /// `Arc<T>`, and `Rc<T>` map to `T::Inner`.
    type Inner;

    /// Whether or not the type is nullable
    const NULLABLE: bool = false;

    /// Whether this field is omitted from default loads.
    const DEFERRED: bool = false;

    /// Build a field path from a raw path of the field's
    /// [`Self::ExprTarget`].
    ///
    /// For primitives, returns the path as-is.
    /// For embedded types, wraps the path in a Fields struct.
    fn new_path<Origin>(path: stmt::Path<Origin, Self::ExprTarget>) -> Self::Path<Origin>
    where
        Self: Sized;

    /// Build a list field path from a raw path.
    /// For primitives, returns the path as-is.
    /// For embedded types, wraps the path in a ListFields struct.
    fn new_list_path<Origin>(
        path: stmt::Path<Origin, List<Self::ExprTarget>>,
    ) -> Self::ListPath<Origin>
    where
        Self: Sized;

    /// Build an update builder from assignments and a projection.
    /// For primitives, this returns `()` (no builder).
    /// For embedded types, this is overridden to construct the {Type}Update builder.
    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a>;

    /// Returns the app-level field type for this primitive.
    /// Default implementation returns a Primitive field type.
    /// Embedded types override this to return Embedded field type.
    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        toasty_core::schema::app::FieldTy::Primitive(toasty_core::schema::app::FieldPrimitive {
            ty: Self::ty(),
            storage_ty,
            serialize: None,
        })
    }

    /// Build a boolean filter expression comparing this field value against
    /// a target path. Used by generated belongs-to accessors to build FK
    /// lookups.
    ///
    /// For `T`, returns `target.eq(self)`. For `Option<T>`, returns
    /// `target.eq(inner)` when `Some`, or `false` when `None`.
    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool>;

    /// Register any models referenced by this field type into the given
    /// [`ModelSet`].
    ///
    /// The default implementation is a no-op, suitable for primitive types.
    /// Embedded types override this to register themselves and recurse into
    /// their own fields, which is how embeds are discovered transitively from
    /// the models that contain them.
    fn register(_model_set: &mut ModelSet) {}
}

macro_rules! impl_field_primitive {
    ($ty:ty) => {
        impl Field for $ty {
            type ExprTarget = Self;
            type Path<Origin> = stmt::Path<Origin, Self>;
            type ListPath<Origin> = stmt::Path<Origin, List<Self::ExprTarget>>;
            type Update<'a> = ();
            type Inner = Self;

            fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
                path
            }

            fn new_list_path<Origin>(
                path: stmt::Path<Origin, List<Self::ExprTarget>>,
            ) -> Self::ListPath<Origin> {
                path
            }

            fn new_update<'a>(
                _assignments: &'a mut toasty_core::stmt::Assignments,
                _projection: toasty_core::stmt::Projection,
            ) -> Self::Update<'a> {
            }

            fn key_constraint<Origin>(
                &self,
                target: stmt::Path<Origin, Self::Inner>,
            ) -> Expr<bool> {
                target.eq(self)
            }
        }
    };
}

impl_field_primitive!(String);
impl_field_primitive!(uuid::Uuid);
impl_field_primitive!(bool);
impl_field_primitive!(isize);
impl_field_primitive!(usize);

impl Field for Vec<u8> {
    type ExprTarget = Self;
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self::ExprTarget>>;
    type Update<'a> = ();
    type Inner = Self;

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(
        path: stmt::Path<Origin, List<Self::ExprTarget>>,
    ) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        toasty_core::schema::app::FieldTy::Primitive(toasty_core::schema::app::FieldPrimitive {
            ty: toasty_core::stmt::Type::Bytes,
            storage_ty,
            serialize: None,
        })
    }

    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        target.eq(self)
    }
}

/// A `Vec<T>` of embedded structs (`T: Embed`) is a `#[document]` collection —
/// `Primitive(List(Model(T::id())))`, stored by the schema builder as a single
/// JSON array of objects. The `field_ty` override below derives the storage
/// type configured for unit-enum discriminants; for struct embeds it matches
/// the trait default.
///
/// This is the only *blanket* `Field for Vec<_>` impl. A blanket
/// `impl<T: Scalar> Field for Vec<T>` cannot coexist with it: the compiler
/// can't prove `Scalar` and `Embed` are disjoint (`Scalar` is public, so a
/// downstream embed type could implement it), so the two would overlap. The
/// scalar element types therefore get *concrete* `Vec<$t>` impls (via
/// [`impl_scalar!`] below), which the orphan rule proves disjoint from this
/// blanket — exactly as the concrete [`Vec<u8>`] impl above already is.
impl<T> Field for Vec<T>
where
    T: Embed + Field,
{
    type ExprTarget = List<T>;
    type Path<Origin> = stmt::Path<Origin, List<T>>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self::ExprTarget>>;
    type Update<'a> = ();
    type Inner = Self;

    fn new_path<Origin>(path: stmt::Path<Origin, List<T>>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(
        path: stmt::Path<Origin, List<Self::ExprTarget>>,
    ) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    /// Lifts a field-level element override into a list storage type, then
    /// falls back to the unit enum's discriminant storage.
    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        let storage_ty = storage_ty.map(db::Type::list).or_else(|| {
            let Model::EmbeddedEnum(embed) = <T as Embed>::schema() else {
                return None;
            };

            if embed.has_data_variants() {
                return None;
            }

            embed.discriminant.storage_ty.map(db::Type::list)
        });
        toasty_core::schema::app::FieldTy::Primitive(toasty_core::schema::app::FieldPrimitive {
            ty: <Self as super::Load>::ty(),
            storage_ty,
            serialize: None,
        })
    }

    fn key_constraint<Origin>(&self, _target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        unreachable!("Vec<T> fields cannot be used as foreign-key targets");
    }

    fn register(model_set: &mut ModelSet) {
        <T as Field>::register(model_set);
    }
}

/// Marks scalar (non-composite) types that are valid as the element type of a
/// model-level `Vec<T>` collection field. The trait is implemented for every
/// primitive `Field` (string, integers, uuid, …); `Vec<u8>` is intentionally
/// not a [`Scalar`] so that bytes keep their existing scalar storage and don't
/// get re-routed through the collection-field path.
pub trait Scalar {}

/// Marks types that `#[document]` storage accepts: a `#[derive(Embed)]`
/// struct, or a `Vec` of them.
///
/// The `#[document]` attribute resolves the field's app-level type through
/// this trait, so applying it to anything else — a scalar, an enum embed, a
/// `Vec<scalar>` — is a compile error rather than a silently ignored
/// attribute. The schema builder re-validates the resolved shape at build
/// time, but only a trait bound can catch the mistake at compile time.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot use `#[document]` storage",
    label = "`#[document]` requires a `#[derive(Embed)]` struct or a `Vec` of them",
    note = "enum embeds and scalar collections do not yet support document storage; \
            remove the `#[document]` attribute"
)]
pub trait Document: Field {
    /// The app-level type of the document column: `Model(id)` for a bare
    /// embed, `List(Model(id))` for a collection.
    fn document_ty() -> toasty_core::stmt::Type {
        <Self as Load>::ty()
    }
}

/// A `Vec` of document-capable embeds is itself document-capable — the
/// collection is stored as one JSON array of objects. Mirrors the blanket
/// `Field for Vec<T>` impl above.
impl<T> Document for Vec<T> where T: Document + Embed + Field {}

/// Registers a scalar collection element type. For each `$t` this generates
/// both `impl Scalar for $t` (opting it into the collection-path operators)
/// and a concrete `impl Field for Vec<$t>` (the collection field impl). The
/// `Vec` impl is per-type rather than a blanket `impl<T: Scalar>` so it stays
/// coherent with the blanket `impl<T: Embed> Field for Vec<T>` above — see that
/// impl's docs. A single list keeps the two in sync.
macro_rules! impl_scalar {
    ( $( $t:ty ),* $(,)? ) => {
        $(
            impl Scalar for $t {}

            impl Field for Vec<$t> {
                // The `List<T>` marker is the expression target so the field's
                // path is `Path<_, List<T>>` (giving `contains`, `is_superset`,
                // `len`, …) and create/update setters bind through
                // `IntoExpr<List<T>>` (Vec, slice, array literal). `new_path`
                // stays identity; `field_ty` uses the default
                // `Primitive(<Self as Load>::ty())` = `Primitive(List($t))`.
                type ExprTarget = List<$t>;
                type Path<Origin> = stmt::Path<Origin, List<$t>>;
                type ListPath<Origin> = stmt::Path<Origin, List<Self::ExprTarget>>;
                type Update<'a> = ();
                type Inner = Self;

                fn new_path<Origin>(path: stmt::Path<Origin, List<$t>>) -> Self::Path<Origin> {
                    path
                }

                fn new_list_path<Origin>(
                    path: stmt::Path<Origin, List<Self::ExprTarget>>,
                ) -> Self::ListPath<Origin> {
                    path
                }

                fn new_update<'a>(
                    _assignments: &'a mut toasty_core::stmt::Assignments,
                    _projection: toasty_core::stmt::Projection,
                ) -> Self::Update<'a> {
                }

                fn key_constraint<Origin>(
                    &self,
                    _target: stmt::Path<Origin, Self::Inner>,
                ) -> Expr<bool> {
                    // A foreign key cannot reference a `Vec<scalar>` field —
                    // collections aren't valid key types.
                    unreachable!("Vec<T> fields cannot be used as foreign-key targets");
                }
            }
        )*
    };
}

impl_scalar!(
    String,
    bool,
    i8,
    i16,
    i32,
    i64,
    u16,
    u32,
    u64,
    isize,
    usize,
    f32,
    f64,
    uuid::Uuid
);

#[cfg(feature = "rust_decimal")]
impl_scalar!(rust_decimal::Decimal);

#[cfg(feature = "bigdecimal")]
impl_scalar!(bigdecimal::BigDecimal);

#[cfg(feature = "jiff")]
impl_scalar!(
    jiff::Timestamp,
    jiff::Zoned,
    jiff::civil::Date,
    jiff::civil::Time,
    jiff::civil::DateTime
);

impl<T: Field> Field for Option<T> {
    type ExprTarget = Self;
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self::ExprTarget>>;
    type Update<'a> = ();
    type Inner = T::Inner;
    const NULLABLE: bool = true;

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(
        path: stmt::Path<Origin, List<Self::ExprTarget>>,
    ) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    /// Delegate the field-type description to `T` so wrappers carrying
    /// schema-relevant metadata (e.g. `Json<U>::serialize = Some(Json)`)
    /// propagate through `Option<_>` instead of getting flattened to the
    /// default `serialize: None`.
    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        T::field_ty(storage_ty)
    }

    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        match self {
            Some(value) => T::key_constraint(value, target),
            None => Expr::from_value(toasty_core::stmt::Value::Bool(false)),
        }
    }

    fn register(model_set: &mut ModelSet) {
        T::register(model_set);
    }
}

impl<T: Field> Field for std::sync::Arc<T> {
    type ExprTarget = Self;
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self::ExprTarget>>;
    type Update<'a> = ();
    type Inner = T::Inner;

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(
        path: stmt::Path<Origin, List<Self::ExprTarget>>,
    ) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        T::field_ty(storage_ty)
    }

    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        T::key_constraint(self, target)
    }

    fn register(model_set: &mut ModelSet) {
        T::register(model_set);
    }
}

impl<T: Field> Field for std::rc::Rc<T> {
    type ExprTarget = Self;
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self::ExprTarget>>;
    type Update<'a> = ();
    type Inner = T::Inner;

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(
        path: stmt::Path<Origin, List<Self::ExprTarget>>,
    ) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        T::field_ty(storage_ty)
    }

    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        T::key_constraint(self, target)
    }

    fn register(model_set: &mut ModelSet) {
        T::register(model_set);
    }
}

impl<T: Field> Field for Box<T> {
    type ExprTarget = Self;
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self::ExprTarget>>;
    type Update<'a> = ();
    type Inner = T::Inner;

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(
        path: stmt::Path<Origin, List<Self::ExprTarget>>,
    ) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        T::field_ty(storage_ty)
    }

    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        T::key_constraint(self, target)
    }

    fn register(model_set: &mut ModelSet) {
        T::register(model_set);
    }
}

#[cfg(feature = "rust_decimal")]
impl_field_primitive!(rust_decimal::Decimal);

#[cfg(feature = "bigdecimal")]
impl_field_primitive!(bigdecimal::BigDecimal);
