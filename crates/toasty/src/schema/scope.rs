use std::{rc::Rc, sync::Arc};

use crate::stmt::Path;

use std::borrow::Cow;
use toasty_core::stmt;

pub trait Scope: Sized {
    /// The type returned when accessing this field from a Scopes struct.
    /// For primitives, this is Path<Origin, Self>.
    /// For embedded types, this is {Type}Scopes<Origin>.
    type FieldAccessor<Origin>;

    /// The type of the update builder for this field.
    /// For embedded types, this is {Type}Update<'a>.
    /// For primitives, this will be {Type}Update<'a> once implemented.
    type UpdateBuilder<'a>;

    /// Build a field accessor from a path.
    /// For primitives, returns the path as-is.
    /// For embedded types, wraps the path in a Scopes struct.
    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin>;

    /// Build an update builder from assignments and a projection.
    /// For primitives, this returns `()` (no builder).
    /// For embedded types, this is overridden to construct the {Type}Update builder.
    fn make_update_builder<'a>(
        _assignments: &'a mut stmt::Assignments,
        _projection: stmt::Projection,
    ) -> Self::UpdateBuilder<'a> {
        // Embedded types must override this method.
        // For primitive types (where UpdateBuilder = ()), this is never called.
        panic!("make_update_builder must be overridden")
    }
}

impl Scope for String {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Scope for Vec<u8> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = ();

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Scope> Scope for Option<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T> Scope for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: Scope,
{
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Scope for uuid::Uuid {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Scope for bool {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Scope> Scope for Arc<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Scope> Scope for Rc<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Scope> Scope for Box<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Scope for isize {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Scope for usize {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

#[cfg(feature = "rust_decimal")]
impl Scope for rust_decimal::Decimal {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

#[cfg(feature = "bigdecimal")]
impl Scope for bigdecimal::BigDecimal {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}
