//! Compile-time validation for explicit `#[auto(...)]` strategies.
//!
//! When `#[derive(Model)]` sees an explicit strategy on a field
//! (`#[auto(uuid)]`, `#[auto(uuid(v4))]`, `#[auto(uuid(v7))]`,
//! `#[auto(increment)]`), the generated code emits a one-shot obligation that
//! the field's Rust type implements `AutoCompatible<Tag>` for the matching
//! marker `Tag`. The bare `#[auto]` form already gets a `T: Auto` obligation
//! via the `STRATEGY` const lookup, so it does not need this trait.
//!
//! Mirrors the pattern in [`crate::codegen_support::storage`]: tag types are
//! zero-sized markers, the trait carries a `#[diagnostic::on_unimplemented]`
//! message, and the macro names tags by absolute path so user-side type
//! aliases or shadowing cannot defeat the check.

/// Marker types for each `#[auto(...)]` strategy.
///
/// These types are zero-sized and only ever appear as the `Strategy`
/// parameter to [`AutoCompatible`]. Macro-generated code names them by
/// absolute path.
#[allow(missing_docs)]
pub mod tag {
    pub struct Uuid;
    pub struct Increment;
}

/// Asserts that a Rust field type can carry an auto-generated value of the
/// given strategy.
///
/// Macro-generated code emits a `_check::<FieldType>()` call where `_check`
/// is bounded by `AutoCompatible<Tag>`. The bound is checked against the
/// resolved field type, so type aliases and newtype embeds compose naturally.
#[diagnostic::on_unimplemented(
    message = "field type `{Self}` is not compatible with the requested auto strategy `{Strategy}`",
    label = "incompatible Rust type for `#[auto(...)]`",
    note = "see `toasty::codegen_support::auto` for the compatibility table"
)]
pub trait AutoCompatible<Strategy> {}

// `Option<T>` is compatible with the same strategies as `T`. Auto-populated
// fields are typically non-nullable, but the schema tracks nullability
// separately so the strategy check is unaffected by the option wrapper.
impl<T, S> AutoCompatible<S> for Option<T> where T: AutoCompatible<S> {}

impl AutoCompatible<tag::Increment> for i8 {}
impl AutoCompatible<tag::Increment> for i16 {}
impl AutoCompatible<tag::Increment> for i32 {}
impl AutoCompatible<tag::Increment> for i64 {}
impl AutoCompatible<tag::Increment> for u8 {}
impl AutoCompatible<tag::Increment> for u16 {}
impl AutoCompatible<tag::Increment> for u32 {}
impl AutoCompatible<tag::Increment> for u64 {}
impl AutoCompatible<tag::Increment> for isize {}
impl AutoCompatible<tag::Increment> for usize {}

impl AutoCompatible<tag::Uuid> for uuid::Uuid {}

// A tuple-newtype embed is compatible with whatever its inner type is
// compatible with. Mirrors the `Auto` blanket in
// [`crate::codegen_support::newtype`].
//
// `do_not_recommend` keeps the blanket out of suggestion lists so the
// `AutoCompatible` `#[diagnostic::on_unimplemented]` message wins.
#[diagnostic::do_not_recommend]
impl<T, S> AutoCompatible<S> for T
where
    T: super::newtype::NewtypeOf,
    <T as super::newtype::NewtypeOf>::Inner: AutoCompatible<S>,
{
}
