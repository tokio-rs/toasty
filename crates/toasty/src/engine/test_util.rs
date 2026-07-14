//! Shared test utilities for engine unit tests.

use toasty_core::{
    driver::Capability,
    schema::{
        Builder,
        app::{self, Model},
    },
};

/// Build a minimal schema with no models (useful for expression-level tests).
pub fn test_schema() -> toasty_core::Schema {
    Builder::new()
        .build(app::Schema::default(), &Capability::SQLITE)
        .expect("empty schema should build")
}

/// Build a schema from the given macro-generated models.
pub fn test_schema_with(models: &[Model]) -> toasty_core::Schema {
    let app_schema =
        app::Schema::from_macro(models.iter().cloned()).expect("schema should build from macro");

    Builder::new()
        .build(app_schema, &Capability::SQLITE)
        .expect("schema should build")
}

/// Build a schema using PostgreSQL capability (for enum type tests).
pub fn test_schema_postgresql(models: &[Model]) -> toasty_core::Schema {
    let app_schema =
        app::Schema::from_macro(models.iter().cloned()).expect("schema should build from macro");

    Builder::new()
        .build(app_schema, &Capability::POSTGRESQL)
        .expect("schema should build")
}
