# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0](https://github.com/tokio-rs/toasty/compare/toasty-driver-turso-v0.6.1...toasty-driver-turso-v0.7.0) - 2026-05-26

### Added

- Raw SQL execution API ([#965])
- Turso driver with TransactionMode-aware concurrent writes ([#938])

### Changed

- [**breaking**] Deferred relation fields are now required ([#954])
- [**breaking**] Terminal query method renamed from `.all()` to `.exec()` ([#471])
- [**breaking**] Removed `Cursor` type; queries now return `Vec` directly ([#448])
- [**breaking**] Removed automatic mapping of multiple models to a single table ([#225])
- [**breaking**] Switched to proc macros for schema declaration ([#76])

[#76]: https://github.com/tokio-rs/toasty/pull/76
[#225]: https://github.com/tokio-rs/toasty/pull/225
[#448]: https://github.com/tokio-rs/toasty/pull/448
[#471]: https://github.com/tokio-rs/toasty/pull/471
[#938]: https://github.com/tokio-rs/toasty/pull/938
[#954]: https://github.com/tokio-rs/toasty/pull/954
[#965]: https://github.com/tokio-rs/toasty/pull/965
