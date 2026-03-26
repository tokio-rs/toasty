use std::{rc::Rc, sync::Arc};

use crate::stmt::Path;

use std::borrow::Cow;
use toasty_core::stmt;

pub trait Field: Sized {
    /// The type returned when accessing this field from a Fields struct.
    /// For primitives, this is Path<Origin, Self>.
    /// For embedded types, this is {Type}Fields<Origin>.
    type FieldAccessor<Origin>;

    /// The type of the update builder for this field.
    /// For embedded types, this is {Type}Update<'a>.
    /// For primitives, this will be {Type}Update<'a> once implemented.
    type UpdateBuilder<'a>;

    /// Build a field accessor from a path.
    /// For primitives, returns the path as-is.
    /// For embedded types, wraps the path in a Fields struct.
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

impl Field for String {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Field for Vec<u8> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = ();

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Field> Field for Option<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T> Field for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: Field,
{
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Field for uuid::Uuid {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Field for bool {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Field> Field for Arc<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Field> Field for Rc<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Field> Field for Box<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Field for isize {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Field for usize {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

#[cfg(feature = "rust_decimal")]
impl Field for rust_decimal::Decimal {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

#[cfg(feature = "bigdecimal")]
impl Field for bigdecimal::BigDecimal {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}
