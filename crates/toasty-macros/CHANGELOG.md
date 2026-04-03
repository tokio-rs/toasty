# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0](https://github.com/tokio-rs/toasty/compare/toasty-macros-v0.2.0...toasty-macros-v0.3.0) - 2026-04-03

### Added

- add IN list support to query macro filter expressions ([#605](https://github.com/tokio-rs/toasty/pull/605))
- automatic global model discovery with `models!(crate::*)` using the `inventory` crate ([#614](https://github.com/tokio-rs/toasty/pull/614))
- add Assign<T> trait and stmt combinators for unified update mutations ([#607](https://github.com/tokio-rs/toasty/pull/607))

### Fixed

- make Assignment<T> Send + Sync by removing boxed closures ([#627](https://github.com/tokio-rs/toasty/pull/627))
- remove bogus `impl<T: IntoExpr<T>> IntoExpr<List<T>> for &T` ([#621](https://github.com/tokio-rs/toasty/pull/621))

### Other

- replace IntoExpr<T> for &Option<T> with Field::key_constraint ([#619](https://github.com/tokio-rs/toasty/pull/619))
- push pagination handling into engine ([#610](https://github.com/tokio-rs/toasty/pull/610))
- add badges to README ([#606](https://github.com/tokio-rs/toasty/pull/606))
- update README examples to use create! macro syntax ([#603](https://github.com/tokio-rs/toasty/pull/603))

## [0.2.0](https://github.com/tokio-rs/toasty/compare/toasty-macros-v0.0.0...toasty-macros-v0.2.0) - 2026-03-30

### Added

- implement string discriminants for embedded enums ([#580](https://github.com/tokio-rs/toasty/pull/580))
- add IntoAssignment trait and has-many update combinators ([#576](https://github.com/tokio-rs/toasty/pull/576))
- add Path<Origin> associated type and new_path/new_root_path methods to Model trait ([#574](https://github.com/tokio-rs/toasty/pull/574))
- add create() method to Fields and ListFields structs ([#572](https://github.com/tokio-rs/toasty/pull/572))
- add Scope trait and implement it for HasMany ([#570](https://github.com/tokio-rs/toasty/pull/570))
- add ORDER BY, LIMIT, and OFFSET support to query! macro ([#540](https://github.com/tokio-rs/toasty/pull/540))
- make query structs clone ([#554](https://github.com/tokio-rs/toasty/pull/554))
- implement Create trait for ManyField and OneField relation structs ([#550](https://github.com/tokio-rs/toasty/pull/550))
- implement basic query! macro with filter support ([#533](https://github.com/tokio-rs/toasty/pull/533))
- redesign create\! macro syntax (v2) ([#444](https://github.com/tokio-rs/toasty/pull/444))
- implement runtime serialization codegen for #[serialize(json)] fields ([#404](https://github.com/tokio-rs/toasty/pull/404))
- support indexing embedded struct fields ([#399](https://github.com/tokio-rs/toasty/pull/399))
- create macro ([#398](https://github.com/tokio-rs/toasty/pull/398))
- support embedded structs as field types ([#299](https://github.com/tokio-rs/toasty/pull/299))

### Fixed

- emit record not found ([#592](https://github.com/tokio-rs/toasty/pull/592))
- update error messages to reference ModelField instead of Field ([#565](https://github.com/tokio-rs/toasty/pull/565))
- bring back associated type for `Model::Create` ([#555](https://github.com/tokio-rs/toasty/pull/555))
- make create! macro syntax to be consistent with tuple and array ([#525](https://github.com/tokio-rs/toasty/pull/525))

### Other

- upgrade dependencies ([#598](https://github.com/tokio-rs/toasty/pull/598))
- configure readme field in workspace package metadata ([#597](https://github.com/tokio-rs/toasty/pull/597))
- organize imports and format code for consistency ([#595](https://github.com/tokio-rs/toasty/pull/595))
- add tests for default and mixed string discriminant enums ([#593](https://github.com/tokio-rs/toasty/pull/593))
- update README status from Incubating to Preview ([#594](https://github.com/tokio-rs/toasty/pull/594))
- remove with_<field> closure methods from create builders ([#582](https://github.com/tokio-rs/toasty/pull/582))
- replace CreateMany in create! macro with path-based nested builders ([#575](https://github.com/tokio-rs/toasty/pull/575))
- replace ManyField/OneField with per-model fields structs ([#571](https://github.com/tokio-rs/toasty/pull/571))
- delete unused Field trait and rename ModelField to Field ([#568](https://github.com/tokio-rs/toasty/pull/568))
- inline Create trait into Model and Relation traits ([#567](https://github.com/tokio-rs/toasty/pull/567))
- relax Auto trait bound from Field to ModelField ([#563](https://github.com/tokio-rs/toasty/pull/563))
- add warn(missing_docs) to toasty-macros ([#560](https://github.com/tokio-rs/toasty/pull/560))
- move reload from Field trait to Load trait ([#556](https://github.com/tokio-rs/toasty/pull/556))
- extract RegisterField trait for schema registration concerns ([#553](https://github.com/tokio-rs/toasty/pull/553))
- extract Create<T> trait from Model and Relation ([#549](https://github.com/tokio-rs/toasty/pull/549))
- move ty() method from Field trait to Load trait ([#545](https://github.com/tokio-rs/toasty/pull/545))
- remove `fn load` from Field trait, use Load supertrait ([#544](https://github.com/tokio-rs/toasty/pull/544))
- consolidate toasty-codegen into toasty-macros ([#536](https://github.com/tokio-rs/toasty/pull/536))
- enable doc tests in toasty-macros by fixing code examples ([#532](https://github.com/tokio-rs/toasty/pull/532))
- reorganize model and relation types under schema module ([#505](https://github.com/tokio-rs/toasty/pull/505))
- add comprehensive documentation for Embed derive macro ([#503](https://github.com/tokio-rs/toasty/pull/503))
- add documentation for `create!` macro ([#502](https://github.com/tokio-rs/toasty/pull/502))
- add comprehensive documentation to Model derive macro ([#490](https://github.com/tokio-rs/toasty/pull/490))
- remove unsafe ([#480](https://github.com/tokio-rs/toasty/pull/480))
- fix API docs link and redesign crate index page ([#472](https://github.com/tokio-rs/toasty/pull/472))
- rename terminal query method .all() to .exec() ([#471](https://github.com/tokio-rs/toasty/pull/471))
- add nightly API docs link to README ([#469](https://github.com/tokio-rs/toasty/pull/469))
- add developer guide link to README ([#466](https://github.com/tokio-rs/toasty/pull/466))
- remove Cursor type and return Vec directly from query execution ([#448](https://github.com/tokio-rs/toasty/pull/448))
- add compile testing for documentation code snippets ([#446](https://github.com/tokio-rs/toasty/pull/446))
- cleanup llm context files ([#365](https://github.com/tokio-rs/toasty/pull/365))
- Implement `#[default]` and `#[update]` field attributes ([#353](https://github.com/tokio-rs/toasty/pull/353))
- remove auto-mapping many models to one table ([#225](https://github.com/tokio-rs/toasty/pull/225))
- Update readme ([#230](https://github.com/tokio-rs/toasty/pull/230))
- Add support for specifying a different database name for fields ([#174](https://github.com/tokio-rs/toasty/pull/174))
- update readme to align with the working code ([#112](https://github.com/tokio-rs/toasty/pull/112))
- Switch proc macro to `#[derive(Model)]` ([#105](https://github.com/tokio-rs/toasty/pull/105))
- move crates to flatter structure ([#91](https://github.com/tokio-rs/toasty/pull/91))
- Switch Toasty to use proc macros for schema declaration ([#76](https://github.com/tokio-rs/toasty/pull/76))
- Fix typo in README.md ([#4](https://github.com/tokio-rs/toasty/pull/4))
- Initial commit
