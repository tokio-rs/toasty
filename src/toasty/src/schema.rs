use crate::Result;

pub use toasty_core::schema::*;

pub fn from_macro(models: &[app::Model]) -> Result<app::Schema> {
    app::Schema::from_macro(models)
}
