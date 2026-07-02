use super::{Embed, Field};
use crate::stmt::List;
use toasty_core::schema::app::ModelSet;

/// Schema and runtime information for a `#[document]` field.
///
/// `#[document]` forces a field into document storage — one `jsonb` / `JSON`
/// column rather than column-expansion. The derive macro resolves a
/// `#[document]` field's type shape through this trait, mirroring how the
/// column-expanded case resolves through [`Field`](super::Field).
///
/// For this increment the only implementor is `Vec<T>` where `T:` [`Embed`] —
/// a collection of embedded structs stored as a JSON array of objects.
pub trait Document {
    /// The expression-level type the generated create / update setters bind
    /// through. For `Vec<T>` this is [`List<T>`], so the setters accept any
    /// `impl IntoExpr<List<T>>` (a `Vec<T>`, a slice, an array literal).
    type ExprTarget;

    /// Whether the field accepts `None` / `NULL`.
    const NULLABLE: bool = false;

    /// Whether the field is lazily loaded. A `#[document]` collection is
    /// always loaded eagerly, so this is never `true`; it exists to mirror
    /// [`Field::DEFERRED`](super::Field::DEFERRED) so the derive macro can
    /// resolve both traits through one code path.
    const DEFERRED: bool = false;

    /// Returns the app-level field type for this document field.
    ///
    /// The element type is left as [`stmt::Type::Model`](toasty_core::stmt::Type::Model);
    /// the schema builder resolves it to a
    /// [`stmt::Type::Document`](toasty_core::stmt::Type::Document) once every
    /// embed is registered and its field names are known.
    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy;

    /// Register the embedded element type (and anything reachable from it)
    /// into the given [`ModelSet`].
    fn register(model_set: &mut ModelSet);
}

impl<T: Embed + Field> Document for Vec<T> {
    type ExprTarget = List<T>;

    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        toasty_core::schema::app::FieldTy::Primitive(toasty_core::schema::app::FieldPrimitive {
            ty: toasty_core::stmt::Type::List(Box::new(toasty_core::stmt::Type::Model(
                <T as Embed>::id(),
            ))),
            storage_ty,
            serialize: None,
        })
    }

    fn register(model_set: &mut ModelSet) {
        <T as Field>::register(model_set);
    }
}
