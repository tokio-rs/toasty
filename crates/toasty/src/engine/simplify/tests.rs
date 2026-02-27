use toasty_core::{
    driver::Capability,
    schema::{
        app::{self, Model},
        Builder,
    },
};

mod association;
mod expr_and;
mod expr_any;
mod expr_binary_op;
mod expr_cast;
mod expr_exists;
mod expr_in_list;
mod expr_is_null;
mod expr_list;
mod expr_map;
mod expr_match;
mod expr_not;
mod expr_or;
mod expr_project;
mod expr_record;
mod lift_in_subquery;
mod lift_pk_select;
mod prop_const;
mod stmt_query;

pub fn test_schema() -> toasty_core::Schema {
    Builder::new()
        .build(app::Schema::default(), &Capability::SQLITE)
        .expect("empty schema should build")
}

pub fn test_schema_with(models: &[Model]) -> toasty_core::Schema {
    let app_schema = app::Schema::from_macro(models).expect("schema should build from macro");

    Builder::new()
        .build(app_schema, &Capability::SQLITE)
        .expect("schema should build")
}
