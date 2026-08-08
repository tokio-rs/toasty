# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.0](https://github.com/tokio-rs/toasty/compare/toasty-driver-turso-v0.8.0...toasty-driver-turso-v0.9.0) - 2026-07-23

### Added

- Support for Turso Sync ([#1072])
- #[document] Storage for embedded types with nested-path filtering ([#1028])

[#1028]: https://github.com/tokio-rs/toasty/pull/1028
[#1072]: https://github.com/tokio-rs/toasty/pull/1072

## [0.8.0](https://github.com/tokio-rs/toasty/compare/toasty-driver-turso-v0.7.0...toasty-driver-turso-v0.8.0) - 2026-07-06

### Added

- Emit one toasty::query event per statement and propagate caller spans ([#1071])
- Infer `key` and `references` in `#[belongs_to]` ([#1063])
- Implement serde serialization and deserialization for toasty::Json<T> ([#1035])

[#1035]: https://github.com/tokio-rs/toasty/pull/1035
[#1063]: https://github.com/tokio-rs/toasty/pull/1063
[#1071]: https://github.com/tokio-rs/toasty/pull/1071

## [0.7.0](https://github.com/tokio-rs/toasty/compare/toasty-driver-turso-v0.6.1...toasty-driver-turso-v0.7.0) - 2026-05-29

### Added

- Raw SQL execution API ([#965])
- Turso driver with TransactionMode-aware concurrent writes ([#938])

### Changed

- [**breaking**] Require Deferred relation fields ([#954])
- [**breaking**] Rename terminal query method `.all()` to `.exec()` ([#471])
- [**breaking**] Remove Cursor type and return Vec directly from query execution ([#448])
- [**breaking**] Switch to proc macros for schema declaration ([#76])
- [**breaking**] Remove auto-mapping of many models to one table ([#225])

[#76]: https://github.com/tokio-rs/toasty/pull/76
[#225]: https://github.com/tokio-rs/toasty/pull/225
[#448]: https://github.com/tokio-rs/toasty/pull/448
[#471]: https://github.com/tokio-rs/toasty/pull/471
[#938]: https://github.com/tokio-rs/toasty/pull/938
[#954]: https://github.com/tokio-rs/toasty/pull/954
[#965]: https://github.com/tokio-rs/toasty/pull/965
