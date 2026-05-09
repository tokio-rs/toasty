use super::Field;
use toasty_core::schema::app::AutoStrategy;

#[diagnostic::on_unimplemented(
    message = "Toasty cannot automatically set values for type `{Self}`",
    label = "Toasty cannot automatically set values for this field",
    note = "Add #[auto] to the field. For an embedded newtype, also add \
            #[auto] to the struct so the strategy is proxied from its inner \
            type."
)]
/// A field type that supports automatic value generation.
///
/// Types implementing this trait can be used with the `#[auto]` attribute on
/// model fields. The database or Toasty runtime assigns values for these
/// fields automatically when a new record is created.
///
/// Built-in implementations:
///
/// | Type | Strategy |
/// |---|---|
/// | Integer types (`i8`..`i64`, `u8`..`u64`, `isize`, `usize`) | Auto-increment |
/// | `uuid::Uuid` | UUID v7 |
pub trait Auto: Field {
    /// The strategy the runtime uses to generate values for this type.
    const STRATEGY: AutoStrategy;
}

impl Auto for i8 {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Auto for i16 {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Auto for i32 {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Auto for i64 {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Auto for u8 {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Auto for u16 {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Auto for u32 {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Auto for u64 {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Auto for isize {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Auto for usize {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Auto for uuid::Uuid {
    const STRATEGY: AutoStrategy = AutoStrategy::Uuid(toasty_core::schema::app::UuidVersion::V7);
}
