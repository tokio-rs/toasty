# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0](https://github.com/tokio-rs/toasty/compare/toasty-driver-integration-suite-v0.3.0...toasty-driver-integration-suite-v0.4.0) - 2026-04-11

### Added

- add field shorthand syntax support to create! macro ([#650](https://github.com/tokio-rs/toasty/pull/650))
- support unsigned integer primary keys in DynamoDB ([#617](https://github.com/tokio-rs/toasty/pull/617))
- add support for newtype embedded structs ([#634](https://github.com/tokio-rs/toasty/pull/634))
- auto-discover related models through fields ([#635](https://github.com/tokio-rs/toasty/pull/635))
- support boxed and smart pointer foreign keys in has_many relations ([#630](https://github.com/tokio-rs/toasty/pull/630))

### Other

- make FieldName::app_name optional to support unnamed fields ([#633](https://github.com/tokio-rs/toasty/pull/633))

## [0.3.0](https://github.com/tokio-rs/toasty/compare/toasty-driver-integration-suite-v0.2.0...toasty-driver-integration-suite-v0.3.0) - 2026-04-03

### Added

- [**breaking**] bring back `db.transaction_builder()` API, add non-trait methods for `executor.transaction()` ([#625](https://github.com/tokio-rs/toasty/pull/625))
- add IN list support to query macro filter expressions ([#605](https://github.com/tokio-rs/toasty/pull/605))
- automatic global model discovery with `models!(crate::*)` using the `inventory` crate ([#614](https://github.com/tokio-rs/toasty/pull/614))
- [**breaking**] add `ModelSet` and `models!` macro to replace `.register::<T>()` ([#615](https://github.com/tokio-rs/toasty/pull/615))

### Fixed

- make Assignment<T> Send + Sync by removing boxed closures ([#627](https://github.com/tokio-rs/toasty/pull/627))
- remove bogus `impl<T: IntoExpr<T>> IntoExpr<List<T>> for &T` ([#621](https://github.com/tokio-rs/toasty/pull/621))

### Other

- signature change on Connection trait ([#626](https://github.com/tokio-rs/toasty/pull/626))
- push pagination handling into engine ([#610](https://github.com/tokio-rs/toasty/pull/610))
- add badges to README ([#606](https://github.com/tokio-rs/toasty/pull/606))
- update README examples to use create! macro syntax ([#603](https://github.com/tokio-rs/toasty/pull/603))

## [0.2.0](https://github.com/tokio-rs/toasty/compare/toasty-driver-integration-suite-v0.0.0...toasty-driver-integration-suite-v0.2.0) - 2026-03-30

### Added

- implement string discriminants for embedded enums ([#580](https://github.com/tokio-rs/toasty/pull/580))
- add IntoAssignment trait and has-many update combinators ([#576](https://github.com/tokio-rs/toasty/pull/576))
- add ORDER BY, LIMIT, and OFFSET support to query! macro ([#540](https://github.com/tokio-rs/toasty/pull/540))
- make query structs clone ([#554](https://github.com/tokio-rs/toasty/pull/554))
- implement basic query! macro with filter support ([#533](https://github.com/tokio-rs/toasty/pull/533))
- add count() method to Query ([#534](https://github.com/tokio-rs/toasty/pull/534))
- remove batch filter methods for primary keys ([#524](https://github.com/tokio-rs/toasty/pull/524))
- support has-one conditional updates with existence checks on NoSQL drivers ([#506](https://github.com/tokio-rs/toasty/pull/506))
- add pagination support for composite-key queries on NoSQL drivers ([#484](https://github.com/tokio-rs/toasty/pull/484))
- support model-level key attribute with plain field names ([#457](https://github.com/tokio-rs/toasty/pull/457))
- add scenario! proc-macro to reduce test model duplication ([#451](https://github.com/tokio-rs/toasty/pull/451))
- redesign create\! macro syntax (v2) ([#444](https://github.com/tokio-rs/toasty/pull/444))
- add Bijection type for field-to-column mappings ([#433](https://github.com/tokio-rs/toasty/pull/433))
- support update and delete statements in toasty::batch ([#428](https://github.com/tokio-rs/toasty/pull/428))
- support create statements in toasty::batch ([#417](https://github.com/tokio-rs/toasty/pull/417))
- implement batch queries for sending multiple independent queries in a single round-trip ([#411](https://github.com/tokio-rs/toasty/pull/411))
- implement runtime serialization codegen for #[serialize(json)] fields ([#404](https://github.com/tokio-rs/toasty/pull/404))
- support indexing embedded enum variant fields ([#401](https://github.com/tokio-rs/toasty/pull/401))
- support indexing embedded struct fields ([#399](https://github.com/tokio-rs/toasty/pull/399))
- create macro ([#398](https://github.com/tokio-rs/toasty/pull/398))
- filter on embedded enum variants ([#389](https://github.com/tokio-rs/toasty/pull/389))
- embedded enums with fields ([#381](https://github.com/tokio-rs/toasty/pull/381))
- add support for limit(n) queries. ([#368](https://github.com/tokio-rs/toasty/pull/368))
- embedded unit enums ([#355](https://github.com/tokio-rs/toasty/pull/355))
- support embedded structs as field types ([#299](https://github.com/tokio-rs/toasty/pull/299))

### Fixed

- emit record not found ([#592](https://github.com/tokio-rs/toasty/pull/592))
- strip ExprCast from IsNull/IsNotNull to fix is_none() panic on Option<Uuid> columns  ([#584](https://github.com/tokio-rs/toasty/pull/584))
- make create! macro syntax to be consistent with tuple and array ([#525](https://github.com/tokio-rs/toasty/pull/525))
- reflect correct type in statement generics ([#517](https://github.com/tokio-rs/toasty/pull/517))
- remove ordered comparison methods from embedded enum codegen ([#474](https://github.com/tokio-rs/toasty/pull/474))
- update IntoExpr comment to use List<Model> syntax ([#467](https://github.com/tokio-rs/toasty/pull/467))
- update IntoExpr comment to use List<Model> syntax ([#465](https://github.com/tokio-rs/toasty/pull/465))
- split composite KV filters in engine and enable batch update test ([#442](https://github.com/tokio-rs/toasty/pull/442))
- enable ignored nested preload tests and DRY up set_returning_field ([#441](https://github.com/tokio-rs/toasty/pull/441))
- correct batch_load_index for nested HasMany→HasOne inserts with auto-increment IDs ([#440](https://github.com/tokio-rs/toasty/pull/440))
- lower ORDER BY expressions ([#439](https://github.com/tokio-rs/toasty/pull/439))
- restore fixed-precision jiff formatting and add driver encoding assertions ([#437](https://github.com/tokio-rs/toasty/pull/437))
- preload HasOne<Option<_>> and refactor ExprLet to Vec bindings ([#402](https://github.com/tokio-rs/toasty/pull/402))
- DDB N+1 Select Fix ([#380](https://github.com/tokio-rs/toasty/pull/380))

### Other

- upgrade dependencies ([#598](https://github.com/tokio-rs/toasty/pull/598))
- configure readme field in workspace package metadata ([#597](https://github.com/tokio-rs/toasty/pull/597))
- organize imports and format code for consistency ([#595](https://github.com/tokio-rs/toasty/pull/595))
- add tests for default and mixed string discriminant enums ([#593](https://github.com/tokio-rs/toasty/pull/593))
- update README status from Incubating to Preview ([#594](https://github.com/tokio-rs/toasty/pull/594))
- add regression tests for field ordering in update operations ([#583](https://github.com/tokio-rs/toasty/pull/583))
- update assert_struct! to use new anonymous struct syntax ([#585](https://github.com/tokio-rs/toasty/pull/585))
- switch Assignments to BTreeMap with multi-value support ([#566](https://github.com/tokio-rs/toasty/pull/566))
- make Connection public and Db stateless ([#569](https://github.com/tokio-rs/toasty/pull/569))
- rename PoolConnection to Connection and move to db/connection.rs ([#561](https://github.com/tokio-rs/toasty/pull/561))
- remove async_trait reexport from toasty-core ([#539](https://github.com/tokio-rs/toasty/pull/539))
- remove std-util crate ([#538](https://github.com/tokio-rs/toasty/pull/538))
- reorganize integration test modules with consistent naming ([#529](https://github.com/tokio-rs/toasty/pull/529))
- move batch, page, and create_many into stmt module ([#526](https://github.com/tokio-rs/toasty/pull/526))
- rename unwrap methods to expect for consistency and clarity ([#518](https://github.com/tokio-rs/toasty/pull/518))
- distinguish between single-row and multi-row updates with type system ([#514](https://github.com/tokio-rs/toasty/pull/514))
- refactor tests to use reusable scenario definitions ([#511](https://github.com/tokio-rs/toasty/pull/511))
- reorganize model and relation types under schema module ([#505](https://github.com/tokio-rs/toasty/pull/505))
- rename relation query method from `.get()` to `.exec()` ([#509](https://github.com/tokio-rs/toasty/pull/509))
- move driver types from toasty to toasty_core ([#492](https://github.com/tokio-rs/toasty/pull/492))
- simplify batch filtering with in_list helper and remove tuple IntoExpr impl ([#477](https://github.com/tokio-rs/toasty/pull/477))
- remove unsafe ([#480](https://github.com/tokio-rs/toasty/pull/480))
- add batch rollback tests ([#478](https://github.com/tokio-rs/toasty/pull/478))
- rename test scenarios to describe relationship patterns ([#476](https://github.com/tokio-rs/toasty/pull/476))
- fix API docs link and redesign crate index page ([#472](https://github.com/tokio-rs/toasty/pull/472))
- rename terminal query method .all() to .exec() ([#471](https://github.com/tokio-rs/toasty/pull/471))
- add nightly API docs link to README ([#469](https://github.com/tokio-rs/toasty/pull/469))
- add developer guide link to README ([#466](https://github.com/tokio-rs/toasty/pull/466))
- replace mod.rs files with named module files ([#463](https://github.com/tokio-rs/toasty/pull/463))
- rename toasty::stmt::Select to toasty::stmt::Query ([#460](https://github.com/tokio-rs/toasty/pull/460))
- remove Cursor type and return Vec directly from query execution ([#448](https://github.com/tokio-rs/toasty/pull/448))
- Add support for filtering parents by has_many associations ([#447](https://github.com/tokio-rs/toasty/pull/447))
- add batch create tests for array and vec inputs ([#445](https://github.com/tokio-rs/toasty/pull/445))
- add compile testing for documentation code snippets ([#446](https://github.com/tokio-rs/toasty/pull/446))
- Allow tuples to be used as batch expressions for nested creates ([#443](https://github.com/tokio-rs/toasty/pull/443))
- Add offset ([#438](https://github.com/tokio-rs/toasty/pull/438))
- Revert "feat: add Bijection type for field-to-column mappings ([#433](https://github.com/tokio-rs/toasty/pull/433))" ([#436](https://github.com/tokio-rs/toasty/pull/436))
- Add proper errors for missing embed registration ([#435](https://github.com/tokio-rs/toasty/pull/435))
- Add comprehensive batch tests for association-scoped statements ([#429](https://github.com/tokio-rs/toasty/pull/429))
- Dynamic batch support ([#415](https://github.com/tokio-rs/toasty/pull/415))
- Add scope-depth-aware expression walker and refactor callers ([#414](https://github.com/tokio-rs/toasty/pull/414))
- Use associated type on IntoStatement for output cardinality ([#412](https://github.com/tokio-rs/toasty/pull/412))
- Add comprehensive nested preload integration tests ([#409](https://github.com/tokio-rs/toasty/pull/409))
- Support deeply nested association preloading ([#311](https://github.com/tokio-rs/toasty/pull/311))
- add missing single-level preload permutations ([#403](https://github.com/tokio-rs/toasty/pull/403))
- Interactive transactions ([#376](https://github.com/tokio-rs/toasty/pull/376))
- rm Arc from Schema.db field. ([#387](https://github.com/tokio-rs/toasty/pull/387))
- ExprMatch simplification ([#383](https://github.com/tokio-rs/toasty/pull/383))
- Connection per Db ([#379](https://github.com/tokio-rs/toasty/pull/379))
- Add transaction isolation levels and read-only mode to Operation::Transaction ([#375](https://github.com/tokio-rs/toasty/pull/375))
- wrap multi-op ExecPlan in BEGIN...COMMIT for atomicity ([#370](https://github.com/tokio-rs/toasty/pull/370))
- minor test cleanup ([#367](https://github.com/tokio-rs/toasty/pull/367))
- cleanup llm context files ([#365](https://github.com/tokio-rs/toasty/pull/365))
- Implement `#[default]` and `#[update]` field attributes ([#353](https://github.com/tokio-rs/toasty/pull/353))
- support OR queries over primary key ([#350](https://github.com/tokio-rs/toasty/pull/350))
- move Model.fields -> ModelRoot ([#347](https://github.com/tokio-rs/toasty/pull/347))
- Properly implement Bytes primitive ([#345](https://github.com/tokio-rs/toasty/pull/345))
- Add {update/delete}_by_id snippets ([#308](https://github.com/tokio-rs/toasty/pull/308))
- Fix #317 ([#342](https://github.com/tokio-rs/toasty/pull/342))
- Add is_none() and is_some() filter methods for Option fields ([#337](https://github.com/tokio-rs/toasty/pull/337))
- Migrate last tests to using errors. ([#339](https://github.com/tokio-rs/toasty/pull/339))
- use == operator for variable comparisons in tests ([#340](https://github.com/tokio-rs/toasty/pull/340))
- Remove Id type ([#334](https://github.com/tokio-rs/toasty/pull/334))
- support partial updates for embedded structs ([#325](https://github.com/tokio-rs/toasty/pull/325))
- add .not() method on Expr<bool> for NOT queries ([#315](https://github.com/tokio-rs/toasty/pull/315))
- return errors from driver tests ([#319](https://github.com/tokio-rs/toasty/pull/319))
- Add database reset functionality (+ serial tests) ([#322](https://github.com/tokio-rs/toasty/pull/322))
- Add database migration CLI tool ([#271](https://github.com/tokio-rs/toasty/pull/271))
- support or queries ([#305](https://github.com/tokio-rs/toasty/pull/305))
- allow embedded struct fields in queries ([#303](https://github.com/tokio-rs/toasty/pull/303))
- move Capability to Driver ([#300](https://github.com/tokio-rs/toasty/pull/300))
- UnsupportedFeature ([#294](https://github.com/tokio-rs/toasty/pull/294))
- move more tests to the integration suite ([#277](https://github.com/tokio-rs/toasty/pull/277))
- move more tests to the integration test suite ([#275](https://github.com/tokio-rs/toasty/pull/275))
- move more tests to the integration suite ([#274](https://github.com/tokio-rs/toasty/pull/274))
- move more tests to the test suite ([#273](https://github.com/tokio-rs/toasty/pull/273))
- move more tests to the integration suite ([#270](https://github.com/tokio-rs/toasty/pull/270))
- move field_column_type.rs to integration test suite. ([#269](https://github.com/tokio-rs/toasty/pull/269))
- Move more tests to the integration suite ([#268](https://github.com/tokio-rs/toasty/pull/268))
- move more tests to integration suite. ([#265](https://github.com/tokio-rs/toasty/pull/265))
- extract integration tests to a reusable crate ([#263](https://github.com/tokio-rs/toasty/pull/263))
- remove auto-mapping many models to one table ([#225](https://github.com/tokio-rs/toasty/pull/225))
- Update readme ([#230](https://github.com/tokio-rs/toasty/pull/230))
- update readme to align with the working code ([#112](https://github.com/tokio-rs/toasty/pull/112))
- Switch Toasty to use proc macros for schema declaration ([#76](https://github.com/tokio-rs/toasty/pull/76))
- Fix typo in README.md ([#4](https://github.com/tokio-rs/toasty/pull/4))
- Initial commit
