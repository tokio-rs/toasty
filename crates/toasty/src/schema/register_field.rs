use super::Load;

/// Schema registration information for a field type.
///
/// This trait captures the information needed to register a field's type in the
/// app schema: its nullability and its [`FieldTy`](toasty_core::schema::app::FieldTy).
/// It is used by the `Register::schema()` implementation that the macro expands.
///
/// Separated from [`Field`](super::Field) so that schema registration does not
/// depend on runtime concerns like update builders or field accessors.
pub trait RegisterField: Load {
    /// Whether or not the type is nullable
    const NULLABLE: bool = false;

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
}
