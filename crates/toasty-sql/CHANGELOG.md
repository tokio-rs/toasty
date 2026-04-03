# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0](https://github.com/tokio-rs/toasty/compare/toasty-sql-v0.2.0...toasty-sql-v0.3.0) - 2026-04-03

### Other

- push pagination handling into engine ([#610](https://github.com/tokio-rs/toasty/pull/610))
- add badges to README ([#606](https://github.com/tokio-rs/toasty/pull/606))
- update README examples to use create! macro syntax ([#603](https://github.com/tokio-rs/toasty/pull/603))

## [0.2.0](https://github.com/tokio-rs/toasty/compare/toasty-sql-v0.0.0...toasty-sql-v0.2.0) - 2026-03-30

### Added

- support auto incrementing IDs ([#192](https://github.com/tokio-rs/toasty/pull/192))
- adds `ExprNot` ([#214](https://github.com/tokio-rs/toasty/pull/214))

### Fixed

- unable to update data for multiple columns ([#101](https://github.com/tokio-rs/toasty/pull/101))

### Other

- upgrade dependencies ([#598](https://github.com/tokio-rs/toasty/pull/598))
- configure readme field in workspace package metadata ([#597](https://github.com/tokio-rs/toasty/pull/597))
- organize imports and format code for consistency ([#595](https://github.com/tokio-rs/toasty/pull/595))
- update README status from Incubating to Preview ([#594](https://github.com/tokio-rs/toasty/pull/594))
- switch Assignments to BTreeMap with multi-value support ([#566](https://github.com/tokio-rs/toasty/pull/566))
- add comprehensive documentation to toasty-sql crate ([#543](https://github.com/tokio-rs/toasty/pull/543))
- rename unwrap methods to expect for consistency and clarity ([#518](https://github.com/tokio-rs/toasty/pull/518))
- remove unsafe ([#480](https://github.com/tokio-rs/toasty/pull/480))
- fix API docs link and redesign crate index page ([#472](https://github.com/tokio-rs/toasty/pull/472))
- rename terminal query method .all() to .exec() ([#471](https://github.com/tokio-rs/toasty/pull/471))
- add nightly API docs link to README ([#469](https://github.com/tokio-rs/toasty/pull/469))
- add developer guide link to README ([#466](https://github.com/tokio-rs/toasty/pull/466))
- remove Cursor type and return Vec directly from query execution ([#448](https://github.com/tokio-rs/toasty/pull/448))
- add compile testing for documentation code snippets ([#446](https://github.com/tokio-rs/toasty/pull/446))
- Add offset ([#438](https://github.com/tokio-rs/toasty/pull/438))
- add missing single-level preload permutations ([#403](https://github.com/tokio-rs/toasty/pull/403))
- Interactive transactions ([#376](https://github.com/tokio-rs/toasty/pull/376))
- Add transaction isolation levels and read-only mode to Operation::Transaction ([#375](https://github.com/tokio-rs/toasty/pull/375))
- wrap multi-op ExecPlan in BEGIN...COMMIT for atomicity ([#370](https://github.com/tokio-rs/toasty/pull/370))
- cleanup llm context files ([#365](https://github.com/tokio-rs/toasty/pull/365))
- expression eval tests & improvements ([#358](https://github.com/tokio-rs/toasty/pull/358))
- Remove Id type ([#334](https://github.com/tokio-rs/toasty/pull/334))
- rm dead enum code ([#331](https://github.com/tokio-rs/toasty/pull/331))
- switch assignments to be a map of Projection and not usize. ([#312](https://github.com/tokio-rs/toasty/pull/312))
- Add database migration CLI tool ([#271](https://github.com/tokio-rs/toasty/pull/271))
- add a custom error type ([#279](https://github.com/tokio-rs/toasty/pull/279))
- move more tests to the test suite ([#273](https://github.com/tokio-rs/toasty/pull/273))
- move more tests to integration suite. ([#265](https://github.com/tokio-rs/toasty/pull/265))
- Add basic support for bigdecimal::BigDecimal ([#238](https://github.com/tokio-rs/toasty/pull/238))
- remove auto-mapping many models to one table ([#225](https://github.com/tokio-rs/toasty/pull/225))
- Update readme ([#230](https://github.com/tokio-rs/toasty/pull/230))
- Add date/time times using `jiff` ([#201](https://github.com/tokio-rs/toasty/pull/201))
- Add `Expr::Default` ([#198](https://github.com/tokio-rs/toasty/pull/198))
- Allow specifying more storage types, e.g. `TEXT` for UUIDs ([#181](https://github.com/tokio-rs/toasty/pull/181))
- more post refactor cleanup ([#184](https://github.com/tokio-rs/toasty/pull/184))
- Remove app_name from database level Column struct again ([#179](https://github.com/tokio-rs/toasty/pull/179))
- Add support for UUIDs using the uuid crate ([#178](https://github.com/tokio-rs/toasty/pull/178))
- Add support for specifying a different database name for fields ([#174](https://github.com/tokio-rs/toasty/pull/174))
- handle empty tables when preloading ([#168](https://github.com/tokio-rs/toasty/pull/168))
- move rest of planner to new engine. ([#167](https://github.com/tokio-rs/toasty/pull/167))
- Combine lowering with new planner's "decompose" step ([#164](https://github.com/tokio-rs/toasty/pull/164))
- Add stmt::Filter + some refactors. ([#163](https://github.com/tokio-rs/toasty/pull/163))
- Include association eager-loading when lowering statement ([#159](https://github.com/tokio-rs/toasty/pull/159))
- unify ExprReference and ExprColumn ([#158](https://github.com/tokio-rs/toasty/pull/158))
- update readme to align with the working code ([#112](https://github.com/tokio-rs/toasty/pull/112))
- refactor ExprColumn to remove direct ColumnId references. ([#156](https://github.com/tokio-rs/toasty/pull/156))
- Include all table refs at the top of a SourceTable ([#152](https://github.com/tokio-rs/toasty/pull/152))
- track association includes on returning clause ([#150](https://github.com/tokio-rs/toasty/pull/150))
- add context documentation and reorganize project docs ([#137](https://github.com/tokio-rs/toasty/pull/137))
- reduce glob imports in rest of crates ([#133](https://github.com/tokio-rs/toasty/pull/133))
- Add support for unsigned types ([#122](https://github.com/tokio-rs/toasty/pull/122))
- add support for i32 types ([#113](https://github.com/tokio-rs/toasty/pull/113))
- Initial pagination implementation ([#111](https://github.com/tokio-rs/toasty/pull/111))
- add support for "order by" ([#110](https://github.com/tokio-rs/toasty/pull/110))
- Add annotation to specify DB column type ([#104](https://github.com/tokio-rs/toasty/pull/104))
- ran cargo `clippy --fix -- -Wclippy::use_self` ([#103](https://github.com/tokio-rs/toasty/pull/103))
- complete driver, include in CI ([#97](https://github.com/tokio-rs/toasty/pull/97))
- Refactor sql serializer ([#95](https://github.com/tokio-rs/toasty/pull/95))
- move crates to flatter structure ([#91](https://github.com/tokio-rs/toasty/pull/91))
- Switch Toasty to use proc macros for schema declaration ([#76](https://github.com/tokio-rs/toasty/pull/76))
- Fix typo in README.md ([#4](https://github.com/tokio-rs/toasty/pull/4))
- Initial commit
