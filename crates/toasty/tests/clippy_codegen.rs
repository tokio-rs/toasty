//! Compile generated field checks with
//! `cargo +nightly clippy -p toasty --test clippy_codegen`.

#![deny(clippy::used_underscore_items)]

#[derive(toasty::Model)]
pub struct FieldIndexes {
    #[key]
    pub id: i64,

    #[index]
    pub parent_id: i64,

    #[unique]
    pub name: String,
}

#[derive(toasty::Model)]
#[index(parent_id)]
#[unique(name)]
pub struct ModelIndexes {
    #[key]
    pub id: i64,

    pub parent_id: i64,
    pub name: String,
}

#[derive(toasty::Model)]
pub struct CompatibilityChecks {
    #[key]
    #[auto(increment)]
    pub id: i64,

    #[column(type = text)]
    pub name: String,

    #[version]
    pub version: u64,
}

#[derive(toasty::Model)]
pub struct UuidAuto {
    #[key]
    #[auto(uuid(v4))]
    pub id: toasty::stmt::Uuid,
}
