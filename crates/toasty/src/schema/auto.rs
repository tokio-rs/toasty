use super::ModelField;
use toasty_core::schema::app::AutoStrategy;

#[diagnostic::on_unimplemented(
    message = "Toasty cannot automatically set values for type `{Self}`",
    label = "Toasty cannot automatically set values for this field",
    note = "Is the field annotated with #[auto]?"
)]
pub trait Auto: ModelField {
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
