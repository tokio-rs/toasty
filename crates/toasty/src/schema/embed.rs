use super::Register;

/// Trait for embedded types that are flattened into their parent model's table.
///
/// Embedded types don't have their own tables or primary keys. They can't be
/// queried independently or used as relation targets. Their fields are flattened
/// into the parent model's table columns.
pub trait Embed: Register {
    // Inherits id() and schema() from Register
    // No additional methods needed
}
