# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.1](https://github.com/tokio-rs/toasty/compare/toasty-driver-dynamodb-v0.6.0...toasty-driver-dynamodb-v0.6.1) - 2026-05-16

- Internal improvements only.

## [0.6.0](https://github.com/tokio-rs/toasty/compare/toasty-driver-dynamodb-v0.5.0...toasty-driver-dynamodb-v0.6.0) - 2026-05-14

### Added

- Add push, pop, extend, clear, remove_at, remove operations for vector scalar fields ([#887], [#880])
- Add vector scalar field support on MySQL, SQLite, and DynamoDB ([#872])
- Add full-table scan support for DynamoDB ([#821])

[#821]: https://github.com/tokio-rs/toasty/pull/821
[#872]: https://github.com/tokio-rs/toasty/pull/872
[#880]: https://github.com/tokio-rs/toasty/pull/880
[#887]: https://github.com/tokio-rs/toasty/pull/887

## [0.5.0](https://github.com/tokio-rs/toasty/compare/toasty-driver-dynamodb-v0.4.0...toasty-driver-dynamodb-v0.5.0) - 2026-04-27

### Added

- Add starts_with and LIKE string prefix filter operators ([#745])
- Add #[version] optimistic concurrency control for DynamoDB ([#694])
- Add Limit::Offset support to DynamoDB driver ([#674])
- Add support for floats ([#687])
- Add native database enum type support for embedded enums ([#665])
- Add multi-column composite index support ([#664])

[#664]: https://github.com/tokio-rs/toasty/pull/664
[#665]: https://github.com/tokio-rs/toasty/pull/665
[#674]: https://github.com/tokio-rs/toasty/pull/674
[#687]: https://github.com/tokio-rs/toasty/pull/687
[#694]: https://github.com/tokio-rs/toasty/pull/694
[#745]: https://github.com/tokio-rs/toasty/pull/745

## [0.4.0](https://github.com/tokio-rs/toasty/compare/toasty-driver-dynamodb-v0.3.0...toasty-driver-dynamodb-v0.4.0) - 2026-04-11

### Added

- use on-demand billing for DynamoDB table and GSI creation ([#618](https://github.com/tokio-rs/toasty/pull/618))
- support unsigned integer primary keys in DynamoDB ([#617](https://github.com/tokio-rs/toasty/pull/617))

## [0.3.0](https://github.com/tokio-rs/toasty/compare/toasty-driver-dynamodb-v0.2.0...toasty-driver-dynamodb-v0.3.0) - 2026-04-03

### Other

- signature change on Connection trait ([#626](https://github.com/tokio-rs/toasty/pull/626))
- push pagination handling into engine ([#610](https://github.com/tokio-rs/toasty/pull/610))
- add badges to README ([#606](https://github.com/tokio-rs/toasty/pull/606))
- update README examples to use create! macro syntax ([#603](https://github.com/tokio-rs/toasty/pull/603))

## [0.2.0](https://github.com/tokio-rs/toasty/compare/toasty-driver-dynamodb-v0.0.0...toasty-driver-dynamodb-v0.2.0) - 2026-03-30

### Added

- add tracing-based logging across the ORM ([#586](https://github.com/tokio-rs/toasty/pull/586))
- add pagination support for composite-key queries on NoSQL drivers ([#484](https://github.com/tokio-rs/toasty/pull/484))
- support auto incrementing IDs ([#192](https://github.com/tokio-rs/toasty/pull/192))

### Fixed

- DDB N+1 Select Fix ([#380](https://github.com/tokio-rs/toasty/pull/380))

### Other

- upgrade dependencies ([#598](https://github.com/tokio-rs/toasty/pull/598))
- configure readme field in workspace package metadata ([#597](https://github.com/tokio-rs/toasty/pull/597))
- organize imports and format code for consistency ([#595](https://github.com/tokio-rs/toasty/pull/595))
- update README status from Incubating to Preview ([#594](https://github.com/tokio-rs/toasty/pull/594))
- switch Assignments to BTreeMap with multi-value support ([#566](https://github.com/tokio-rs/toasty/pull/566))
- add comprehensive documentation to all database drivers ([#541](https://github.com/tokio-rs/toasty/pull/541))
- remove async_trait reexport from toasty-core ([#539](https://github.com/tokio-rs/toasty/pull/539))
- rename unwrap methods to expect for consistency and clarity ([#518](https://github.com/tokio-rs/toasty/pull/518))
- remove unsafe ([#480](https://github.com/tokio-rs/toasty/pull/480))
- fix API docs link and redesign crate index page ([#472](https://github.com/tokio-rs/toasty/pull/472))
- rename terminal query method .all() to .exec() ([#471](https://github.com/tokio-rs/toasty/pull/471))
- add nightly API docs link to README ([#469](https://github.com/tokio-rs/toasty/pull/469))
- add developer guide link to README ([#466](https://github.com/tokio-rs/toasty/pull/466))
- remove Cursor type and return Vec directly from query execution ([#448](https://github.com/tokio-rs/toasty/pull/448))
- add compile testing for documentation code snippets ([#446](https://github.com/tokio-rs/toasty/pull/446))
- DDB Client Creation Refactor ([#391](https://github.com/tokio-rs/toasty/pull/391))
- add missing single-level preload permutations ([#403](https://github.com/tokio-rs/toasty/pull/403))
- rm Arc from Schema.db field. ([#387](https://github.com/tokio-rs/toasty/pull/387))
- Add transaction error variants ([#377](https://github.com/tokio-rs/toasty/pull/377))
- cleanup llm context files ([#365](https://github.com/tokio-rs/toasty/pull/365))
- expression eval tests & improvements ([#358](https://github.com/tokio-rs/toasty/pull/358))
- Properly implement Bytes primitive ([#345](https://github.com/tokio-rs/toasty/pull/345))
- Add is_none() and is_some() filter methods for Option fields ([#337](https://github.com/tokio-rs/toasty/pull/337))
- Remove dead enum code ([#341](https://github.com/tokio-rs/toasty/pull/341))
- Remove Id type ([#334](https://github.com/tokio-rs/toasty/pull/334))
- rm dead enum code ([#331](https://github.com/tokio-rs/toasty/pull/331))
- Add database reset functionality (+ serial tests) ([#322](https://github.com/tokio-rs/toasty/pull/322))
- switch assignments to be a map of Projection and not usize. ([#312](https://github.com/tokio-rs/toasty/pull/312))
- Add database migration CLI tool ([#271](https://github.com/tokio-rs/toasty/pull/271))
- support or queries ([#305](https://github.com/tokio-rs/toasty/pull/305))
- move Capability to Driver ([#300](https://github.com/tokio-rs/toasty/pull/300))
- remove anyhow dependency ([#297](https://github.com/tokio-rs/toasty/pull/297))
- misc tweaks ([#295](https://github.com/tokio-rs/toasty/pull/295))
- add condition failed error ([#286](https://github.com/tokio-rs/toasty/pull/286))
- RecordNotFoundError ([#283](https://github.com/tokio-rs/toasty/pull/283))
- add a custom error type ([#279](https://github.com/tokio-rs/toasty/pull/279))
- Add database connection pooling ([#260](https://github.com/tokio-rs/toasty/pull/260))
- Remove enum `use` pattern from drivers ([#257](https://github.com/tokio-rs/toasty/pull/257))
- Refactor and unify driver Value handling ([#256](https://github.com/tokio-rs/toasty/pull/256))
- remove auto-mapping many models to one table ([#225](https://github.com/tokio-rs/toasty/pull/225))
- Update readme ([#230](https://github.com/tokio-rs/toasty/pull/230))
- Allow specifying more storage types, e.g. `TEXT` for UUIDs ([#181](https://github.com/tokio-rs/toasty/pull/181))
- Remove app_name from database level Column struct again ([#179](https://github.com/tokio-rs/toasty/pull/179))
- Add support for UUIDs using the uuid crate ([#178](https://github.com/tokio-rs/toasty/pull/178))
- Add support for specifying a different database name for fields ([#174](https://github.com/tokio-rs/toasty/pull/174))
- move rest of planner to new engine. ([#167](https://github.com/tokio-rs/toasty/pull/167))
- Combine lowering with new planner's "decompose" step ([#164](https://github.com/tokio-rs/toasty/pull/164))
- unify ExprReference and ExprColumn ([#158](https://github.com/tokio-rs/toasty/pull/158))
- update readme to align with the working code ([#112](https://github.com/tokio-rs/toasty/pull/112))
- refactor ExprColumn to remove direct ColumnId references. ([#156](https://github.com/tokio-rs/toasty/pull/156))
- reduce glob imports in rest of crates ([#133](https://github.com/tokio-rs/toasty/pull/133))
- Add support for unsigned types ([#122](https://github.com/tokio-rs/toasty/pull/122))
- Support concurrent test execution ([#121](https://github.com/tokio-rs/toasty/pull/121))
- Add support for i16 ([#116](https://github.com/tokio-rs/toasty/pull/116))
- Support i8 ([#115](https://github.com/tokio-rs/toasty/pull/115))
- add support for i32 types ([#113](https://github.com/tokio-rs/toasty/pull/113))
- ran cargo `clippy --fix -- -Wclippy::use_self` ([#103](https://github.com/tokio-rs/toasty/pull/103))
- Flatten capability struct and DRY db definitions ([#102](https://github.com/tokio-rs/toasty/pull/102))
- switch `driver::Rows::Count` type to u64 ([#98](https://github.com/tokio-rs/toasty/pull/98))
- Refactor sql serializer ([#95](https://github.com/tokio-rs/toasty/pull/95))
- move crates to flatter structure ([#91](https://github.com/tokio-rs/toasty/pull/91))
- Switch Toasty to use proc macros for schema declaration ([#76](https://github.com/tokio-rs/toasty/pull/76))
- Fix typo in README.md ([#4](https://github.com/tokio-rs/toasty/pull/4))
- Initial commit
