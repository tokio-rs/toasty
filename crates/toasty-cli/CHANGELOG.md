# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0](https://github.com/tokio-rs/toasty/compare/toasty-cli-v0.5.0...toasty-cli-v0.6.0) - 2026-05-09

### Fixed

- The CLI now automatically creates a default config file when needed ([#795])

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
