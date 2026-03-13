// README.md code snippets are compile-tested via build.rs rather than rustdoc's
// built-in doctest support. Rustdoc doctests require hidden `# ` boilerplate
// lines inside the markdown to set up context (model definitions, async
// wrappers, etc.). Those lines are visible to anyone reading the raw markdown on
// GitHub or crates.io, so we avoid them for the README. Instead, build.rs
// extracts the code blocks with pulldown-cmark and wraps each one with
// hard-coded boilerplate to produce the generated file included below.
//
// Guide docs (docs/guide/*.md) use standard rustdoc doctests via
// `#[doc = include_str!()]` in lib.rs — hidden boilerplate lines are acceptable
// there because those docs are intended to be read in rendered form on a
// documentation website.
include!(concat!(env!("OUT_DIR"), "/readme_doc_tests.rs"));
