// Compile-check code examples in the user guide.
//
// Each guide chapter is attached as module-level documentation via
// `include_str!`. Running `cargo test -p guide-doctests --doc` executes every
// fenced Rust code block that isn't marked `ignore`, `no_run`, or
// `compile_fail`, giving us compile-time verification of the guide examples.
//
// Code blocks in the markdown use hidden `# ` lines for boilerplate (imports,
// fn main wrappers) so the rendered guide stays clean while the examples remain
// compilable.

#[doc = include_str!("../../../docs/guide/src/introduction.md")]
mod introduction {}

#[doc = include_str!("../../../docs/guide/src/getting-started.md")]
mod getting_started {}

#[doc = include_str!("../../../docs/guide/src/defining-models.md")]
mod defining_models {}

#[doc = include_str!("../../../docs/guide/src/keys-and-auto-generation.md")]
mod keys_and_auto_generation {}
