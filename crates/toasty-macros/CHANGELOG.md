# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.0](https://github.com/tokio-rs/toasty/compare/toasty-macros-v0.8.0...toasty-macros-v0.9.0) - 2026-07-23

### Added

- Order-by support in includes ([#1109])
- Relation link/unlink return a builder instead of executing eagerly ([#1118])
- Support for serde_json::Value fields ([#1116])
- Native JSON and JSONB column storage ([#1114])
- [**breaking**] Explicit column types required for JSON fields ([#1106])
- Filter associations in include ([#1089])
- Integer storage for enum discriminants ([#1101])
- Upsert support ([#1091])
- Shared variant fields and enum-level index/unique attributes ([#1078])
- Enum-level rename_all for embedded enum labels ([#1083])
- Scalar implementation for unit enum embeds ([#1082])
- Document storage for embedded types with nested-path filtering ([#1028])

### Fixed

- Support any() on many-to-many relations ([#1097])
- Store Vec<native-enum> as native enum array on Postgres ([#1092])
- Generate doc comments on model methods ([#1087])
- Normalize raw identifiers in collision checks ([#1085])
- Allow update! expressions to read target model fields ([#1074])

[#1028]: https://github.com/tokio-rs/toasty/pull/1028
[#1074]: https://github.com/tokio-rs/toasty/pull/1074
[#1078]: https://github.com/tokio-rs/toasty/pull/1078
[#1082]: https://github.com/tokio-rs/toasty/pull/1082
[#1083]: https://github.com/tokio-rs/toasty/pull/1083
[#1085]: https://github.com/tokio-rs/toasty/pull/1085
[#1087]: https://github.com/tokio-rs/toasty/pull/1087
[#1089]: https://github.com/tokio-rs/toasty/pull/1089
[#1091]: https://github.com/tokio-rs/toasty/pull/1091
[#1092]: https://github.com/tokio-rs/toasty/pull/1092
[#1097]: https://github.com/tokio-rs/toasty/pull/1097
[#1101]: https://github.com/tokio-rs/toasty/pull/1101
[#1106]: https://github.com/tokio-rs/toasty/pull/1106
[#1109]: https://github.com/tokio-rs/toasty/pull/1109
[#1114]: https://github.com/tokio-rs/toasty/pull/1114
[#1116]: https://github.com/tokio-rs/toasty/pull/1116
[#1118]: https://github.com/tokio-rs/toasty/pull/1118

## [0.8.0](https://github.com/tokio-rs/toasty/compare/toasty-macros-v0.7.0...toasty-macros-v0.8.0) - 2026-07-06

### Added

- Infer `key` and `references` in `#[belongs_to]` ([#1063])
- Share columns across enum variants via `#[column("name")]` ([#1064])
- Index unit enum types ([#1027])
- Between operator for queries ([#1029])
- Composite unique indices ([#1018])
- Support scalar terminal fields in `has_many` ([#1012])

### Changed

- [**breaking**] Derive default table names at compile time ([#1070])
- [**breaking**] Rename `RelationManyField`/`RelationOneField` associated type to `Target` ([#1015])
- [**breaking**] Align `stmt::Query` with per-model `Query` ([#1011])
- [**breaking**] Unify per-model query structs into `Query<T>` ([#995])
- [**breaking**] Remove the `Register` trait ([#1006])
- Remove compile-time field validation from `create!` macro ([#997])

[#995]: https://github.com/tokio-rs/toasty/pull/995
[#997]: https://github.com/tokio-rs/toasty/pull/997
[#1006]: https://github.com/tokio-rs/toasty/pull/1006
[#1011]: https://github.com/tokio-rs/toasty/pull/1011
[#1012]: https://github.com/tokio-rs/toasty/pull/1012
[#1015]: https://github.com/tokio-rs/toasty/pull/1015
[#1018]: https://github.com/tokio-rs/toasty/pull/1018
[#1027]: https://github.com/tokio-rs/toasty/pull/1027
[#1029]: https://github.com/tokio-rs/toasty/pull/1029
[#1063]: https://github.com/tokio-rs/toasty/pull/1063
[#1064]: https://github.com/tokio-rs/toasty/pull/1064
[#1070]: https://github.com/tokio-rs/toasty/pull/1070

## [0.7.0](https://github.com/tokio-rs/toasty/compare/toasty-macros-v0.6.1...toasty-macros-v0.7.0) - 2026-05-29

### Added

- Generate field projection methods on Query/Many/One ([#987])
- Add update! macro for concise field updates ([#980])
- Reject create() on multi-step relation scopes at compile time ([#978])
- [**breaking**] Remove singular has-many create-builder methods ([#977])
- Remove `#[deferred]` field attribute in favor of `Deferred<T>` ([#961])
- Support eager relation fields ([#958])
- Support `.include()` of multi-step `via` relations ([#946])
- Add Turso driver with TransactionMode-aware concurrent writes ([#938])
- Allow #[version] on tuple-newtype embeds of u64 ([#930])
- [**breaking**] Replace `#[serialize(json)]` with `toasty::Json<T>` wrapper ([#926])
- Expose primary-key type via Model::PrimaryKey ([#921])
- Add multi-step (via) has_many and has_one relations ([#890])
- [**breaking**] Delete Relation trait and tighten relation field shapes ([#967])
- [**breaking**] Merge relation field traits ([#971])
- [**breaking**] Require Deferred for relation fields ([#954])

### Fixed

- Respect `pair` attribute in `#[has_one]` macro ([#927])

[#890]: https://github.com/tokio-rs/toasty/pull/890
[#921]: https://github.com/tokio-rs/toasty/pull/921
[#926]: https://github.com/tokio-rs/toasty/pull/926
[#927]: https://github.com/tokio-rs/toasty/pull/927
[#930]: https://github.com/tokio-rs/toasty/pull/930
[#938]: https://github.com/tokio-rs/toasty/pull/938
[#946]: https://github.com/tokio-rs/toasty/pull/946
[#954]: https://github.com/tokio-rs/toasty/pull/954
[#958]: https://github.com/tokio-rs/toasty/pull/958
[#961]: https://github.com/tokio-rs/toasty/pull/961
[#967]: https://github.com/tokio-rs/toasty/pull/967
[#971]: https://github.com/tokio-rs/toasty/pull/971
[#977]: https://github.com/tokio-rs/toasty/pull/977
[#978]: https://github.com/tokio-rs/toasty/pull/978
[#980]: https://github.com/tokio-rs/toasty/pull/980
[#987]: https://github.com/tokio-rs/toasty/pull/987

## [0.6.1](https://github.com/tokio-rs/toasty/compare/toasty-macros-v0.6.0...toasty-macros-v0.6.1) - 2026-05-16

### Added

- Add `.select()` projection through HasMany relations ([#894])
- Chain relation methods on `Many` for multi-step queries via paths ([#903])
- Enforce stricter validation on field-level `#[index]` attributes ([#909])
- Give `One` and `OptionOne` precise query types ([#889])

### Fixed

- Enable `belongs_to` with embed-typed primary keys ([#912])
- Improve `belongs_to` syntax and diagnostics for composite keys ([#905])

[#889]: https://github.com/tokio-rs/toasty/pull/889
[#894]: https://github.com/tokio-rs/toasty/pull/894
[#903]: https://github.com/tokio-rs/toasty/pull/903
[#905]: https://github.com/tokio-rs/toasty/pull/905
[#909]: https://github.com/tokio-rs/toasty/pull/909
[#912]: https://github.com/tokio-rs/toasty/pull/912

## [0.6.0](https://github.com/tokio-rs/toasty/compare/toasty-macros-v0.5.0...toasty-macros-v0.6.0) - 2026-05-14

### Added

- Add Vec<scalar> model fields with PostgreSQL native array storage ([#866])
- Support custom index names via `name = "..."` ([#842])
- Proxy Auto through tuple-newtype Embed types ([#836])
- Add .select() column projection, including through BelongsTo relations ([#820], [#827])
- Validate column storage types via type checker ([#832])
- Add compile-time validation for create! macro field sets ([#648])
- Add #[deferred] field attribute and Deferred<T> wrapper, with support for embedded types ([#793], [#799])
- Add latest_by query ([#707])
- Add all filter for associations ([#784])

### Fixed

- Validate explicit `#[auto(...)]` strategies via type checker ([#851])

[#648]: https://github.com/tokio-rs/toasty/pull/648
[#707]: https://github.com/tokio-rs/toasty/pull/707
[#784]: https://github.com/tokio-rs/toasty/pull/784
[#793]: https://github.com/tokio-rs/toasty/pull/793
[#799]: https://github.com/tokio-rs/toasty/pull/799
[#820]: https://github.com/tokio-rs/toasty/pull/820
[#827]: https://github.com/tokio-rs/toasty/pull/827
[#832]: https://github.com/tokio-rs/toasty/pull/832
[#836]: https://github.com/tokio-rs/toasty/pull/836
[#842]: https://github.com/tokio-rs/toasty/pull/842
[#851]: https://github.com/tokio-rs/toasty/pull/851
[#866]: https://github.com/tokio-rs/toasty/pull/866

## [0.5.0](https://github.com/tokio-rs/toasty/compare/toasty-macros-v0.4.0...toasty-macros-v0.5.0) - 2026-04-27

### Added

- Add optimistic concurrency control with #[version] attribute for DynamoDB ([#694])
- Support array syntax for partition/local key declarations ([#738])
- Add pair attribute to disambiguate has_many/has_one relations ([#746])
- Add support for float types ([#687])
- Add native database enum support ([#665])
- Add multi-column composite index support ([#664])

### Fixed

- Support Rust raw identifiers in model schemas ([#761])

[#664]: https://github.com/tokio-rs/toasty/pull/664
[#665]: https://github.com/tokio-rs/toasty/pull/665
[#687]: https://github.com/tokio-rs/toasty/pull/687
[#694]: https://github.com/tokio-rs/toasty/pull/694
[#738]: https://github.com/tokio-rs/toasty/pull/738
[#746]: https://github.com/tokio-rs/toasty/pull/746
[#761]: https://github.com/tokio-rs/toasty/pull/761

## [0.4.0](https://github.com/tokio-rs/toasty/compare/toasty-macros-v0.3.0...toasty-macros-v0.4.0) - 2026-04-11

### Added

- add field shorthand syntax support to create! macro ([#650](https://github.com/tokio-rs/toasty/pull/650))
- add support for newtype embedded structs ([#634](https://github.com/tokio-rs/toasty/pull/634))
- auto-discover related models through fields ([#635](https://github.com/tokio-rs/toasty/pull/635))

### Other

- fix UUID v7 auto-generation strategy code generation ([#646](https://github.com/tokio-rs/toasty/pull/646))
- make FieldName::app_name optional to support unnamed fields ([#633](https://github.com/tokio-rs/toasty/pull/633))

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
