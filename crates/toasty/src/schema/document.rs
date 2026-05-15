use super::{Embed, Register};
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

    /// Returns the app-level field type for this document field.
    ///
    /// The element type is left as [`stmt::Type::Model`](toasty_core::stmt::Type::Model);
    /// the schema builder resolves it to a
    /// [`stmt::Type::Document`](toasty_core::stmt::Type::Document) once every
    /// embed is registered and its field names are known.
    fn document_field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy;

    /// Register the embedded element type (and anything reachable from it)
    /// into the given [`ModelSet`].
    fn register(model_set: &mut ModelSet);
}

impl<T: Embed> Document for Vec<T> {
    type ExprTarget = List<T>;

    fn document_field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        toasty_core::schema::app::FieldTy::Primitive(toasty_core::schema::app::FieldPrimitive {
            ty: toasty_core::stmt::Type::List(Box::new(toasty_core::stmt::Type::Model(
                <T as Register>::id(),
            ))),
            storage_ty,
            serialize: None,
        })
    }

    fn register(model_set: &mut ModelSet) {
        <T as Register>::register(model_set);
    }
}
