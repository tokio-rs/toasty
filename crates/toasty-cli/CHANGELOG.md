# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.0](https://github.com/tokio-rs/toasty/compare/toasty-cli-v0.8.0...toasty-cli-v0.9.0) - 2026-07-23

### Fixed

- Serialize unconstrained numeric migration snapshots ([#1115])

[#1115]: https://github.com/tokio-rs/toasty/pull/1115

## [0.8.0](https://github.com/tokio-rs/toasty/compare/toasty-cli-v0.7.0...toasty-cli-v0.8.0) - 2026-07-06

### Added

- Emit one toasty::query event per statement and propagate caller spans ([#1071])
- Infer `key` and `references` in `#[belongs_to]` ([#1063])

### Fixed

- Include the path in Toasty config load errors ([#1036])

[#1036]: https://github.com/tokio-rs/toasty/pull/1036
[#1063]: https://github.com/tokio-rs/toasty/pull/1063
[#1071]: https://github.com/tokio-rs/toasty/pull/1071

## [0.7.0](https://github.com/tokio-rs/toasty/compare/toasty-cli-v0.6.1...toasty-cli-v0.7.0) - 2026-05-29

### Added

- Expose migration core as a public API from toasty ([#944])
- Add Turso driver with TransactionMode-aware concurrent writes ([#938])

### Changed

- [**breaking**] Require explicit Deferred for relation fields ([#954])
- [**breaking**] Move schema diff types to schema::diff module ([#929])
- Reorganize db::diff API and consolidate migration types in toasty crate ([#928])

[#928]: https://github.com/tokio-rs/toasty/pull/928
[#929]: https://github.com/tokio-rs/toasty/pull/929
[#938]: https://github.com/tokio-rs/toasty/pull/938
[#944]: https://github.com/tokio-rs/toasty/pull/944
[#954]: https://github.com/tokio-rs/toasty/pull/954

## [0.6.1](https://github.com/tokio-rs/toasty/compare/toasty-cli-v0.6.0...toasty-cli-v0.6.1) - 2026-05-16

- Internal improvements only.

## [0.6.0](https://github.com/tokio-rs/toasty/compare/toasty-cli-v0.5.0...toasty-cli-v0.6.0) - 2026-05-14

### Fixed

- CLI automatically creates a default config if none exists ([#795])

[#795]: https://github.com/tokio-rs/toasty/pull/795

## [0.5.0](https://github.com/tokio-rs/toasty/compare/toasty-cli-v0.4.0...toasty-cli-v0.5.0) - 2026-04-27

### Added

- Generate migrations with timestamp prefixes ([#684])
- Add newlines at the end of generated migration files ([#683])

[#683]: https://github.com/tokio-rs/toasty/pull/683
[#684]: https://github.com/tokio-rs/toasty/pull/684

## [0.3.0](https://github.com/tokio-rs/toasty/compare/toasty-cli-v0.2.0...toasty-cli-v0.3.0) - 2026-04-03

### Other

- signature change on Connection trait ([#626](https://github.com/tokio-rs/toasty/pull/626))
- add badges to README ([#606](https://github.com/tokio-rs/toasty/pull/606))
- update README examples to use create! macro syntax ([#603](https://github.com/tokio-rs/toasty/pull/603))

## [0.2.0](https://github.com/tokio-rs/toasty/compare/toasty-cli-v0.0.0...toasty-cli-v0.2.0) - 2026-03-30

### Other

- upgrade dependencies ([#598](https://github.com/tokio-rs/toasty/pull/598))
- configure readme field in workspace package metadata ([#597](https://github.com/tokio-rs/toasty/pull/597))
- organize imports and format code for consistency ([#595](https://github.com/tokio-rs/toasty/pull/595))
- update README status from Incubating to Preview ([#594](https://github.com/tokio-rs/toasty/pull/594))
- add rustdoc documentation to toasty-cli crate ([#587](https://github.com/tokio-rs/toasty/pull/587))
- bump dependencies ([#530](https://github.com/tokio-rs/toasty/pull/530))
- remove unsafe ([#480](https://github.com/tokio-rs/toasty/pull/480))
- fix API docs link and redesign crate index page ([#472](https://github.com/tokio-rs/toasty/pull/472))
- rename terminal query method .all() to .exec() ([#471](https://github.com/tokio-rs/toasty/pull/471))
- add nightly API docs link to README ([#469](https://github.com/tokio-rs/toasty/pull/469))
- add developer guide link to README ([#466](https://github.com/tokio-rs/toasty/pull/466))
- remove Cursor type and return Vec directly from query execution ([#448](https://github.com/tokio-rs/toasty/pull/448))
- add compile testing for documentation code snippets ([#446](https://github.com/tokio-rs/toasty/pull/446))
- cleanup llm context files ([#365](https://github.com/tokio-rs/toasty/pull/365))
- Redact database password in CLI output ([#362](https://github.com/tokio-rs/toasty/pull/362))
- Add database reset functionality (+ serial tests) ([#322](https://github.com/tokio-rs/toasty/pull/322))
- Add database migration CLI tool ([#271](https://github.com/tokio-rs/toasty/pull/271))
- remove auto-mapping many models to one table ([#225](https://github.com/tokio-rs/toasty/pull/225))
- Update readme ([#230](https://github.com/tokio-rs/toasty/pull/230))
- update readme to align with the working code ([#112](https://github.com/tokio-rs/toasty/pull/112))
- Switch Toasty to use proc macros for schema declaration ([#76](https://github.com/tokio-rs/toasty/pull/76))
- Fix typo in README.md ([#4](https://github.com/tokio-rs/toasty/pull/4))
- Initial commit
