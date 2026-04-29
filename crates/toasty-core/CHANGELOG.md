# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0](https://github.com/tokio-rs/toasty/compare/toasty-core-v0.4.0...toasty-core-v0.5.0) - 2026-04-27

### Added

- add starts_with and LIKE string prefix filter operators ([#745])
- add #[version] optimistic concurrency control for DynamoDB ([#694])
- support disambiguating has_many/has_one with pair attribute ([#746])
- add Limit::Offset support to DynamoDB driver ([#674])
- add support for floats ([#687])
- add native database enum type support for embedded enums ([#665])
- export `SchemaMutations` for external drivers ([#686])

### Fixed

- fix GetByKey to handle duplicate input keys correctly ([#750])
- fix query results when simplification rules encounter unstable expressions ([#703])

[#665]: https://github.com/tokio-rs/toasty/pull/665
[#674]: https://github.com/tokio-rs/toasty/pull/674
[#686]: https://github.com/tokio-rs/toasty/pull/686
[#687]: https://github.com/tokio-rs/toasty/pull/687
[#694]: https://github.com/tokio-rs/toasty/pull/694
[#703]: https://github.com/tokio-rs/toasty/pull/703
[#745]: https://github.com/tokio-rs/toasty/pull/745
[#746]: https://github.com/tokio-rs/toasty/pull/746
[#750]: https://github.com/tokio-rs/toasty/pull/750

## [0.4.0](https://github.com/tokio-rs/toasty/compare/toasty-core-v0.3.0...toasty-core-v0.4.0) - 2026-04-11

### Added

- add support for newtype embedded structs ([#634](https://github.com/tokio-rs/toasty/pull/634))
- auto-discover related models through fields ([#635](https://github.com/tokio-rs/toasty/pull/635))

### Other

- make Error non_exaustive ([#636](https://github.com/tokio-rs/toasty/pull/636))
- make FieldName::app_name optional to support unnamed fields ([#633](https://github.com/tokio-rs/toasty/pull/633))

## [0.3.0](https://github.com/tokio-rs/toasty/compare/toasty-core-v0.2.0...toasty-core-v0.3.0) - 2026-04-03

### Added

- [**breaking**] add `ModelSet` and `models!` macro to replace `.register::<T>()` ([#615](https://github.com/tokio-rs/toasty/pull/615))

### Fixed

- replace broken BinaryOp::reverse with commute in index matching ([#611](https://github.com/tokio-rs/toasty/pull/611))

### Other

- signature change on Connection trait ([#626](https://github.com/tokio-rs/toasty/pull/626))
- push pagination handling into engine ([#610](https://github.com/tokio-rs/toasty/pull/610))
- add badges to README ([#606](https://github.com/tokio-rs/toasty/pull/606))
- update README examples to use create! macro syntax ([#603](https://github.com/tokio-rs/toasty/pull/603))

## [0.2.0](https://github.com/tokio-rs/toasty/compare/toasty-core-v0.0.0...toasty-core-v0.2.0) - 2026-03-30

### Added

- implement string discriminants for embedded enums ([#580](https://github.com/tokio-rs/toasty/pull/580))
- add tracing-based logging across the ORM ([#586](https://github.com/tokio-rs/toasty/pull/586))
- add count() method to Query ([#534](https://github.com/tokio-rs/toasty/pull/534))
- more compact snapshots ([#523](https://github.com/tokio-rs/toasty/pull/523))
- support has-one conditional updates with existence checks on NoSQL drivers ([#506](https://github.com/tokio-rs/toasty/pull/506))
- add pagination support for composite-key queries on NoSQL drivers ([#484](https://github.com/tokio-rs/toasty/pull/484))
- add Bijection type for field-to-column mappings ([#433](https://github.com/tokio-rs/toasty/pull/433))
- support create statements in toasty::batch ([#417](https://github.com/tokio-rs/toasty/pull/417))
- support indexing embedded enum variant fields ([#401](https://github.com/tokio-rs/toasty/pull/401))
- add #[serialize] attribute bookkeeping and design doc ([#400](https://github.com/tokio-rs/toasty/pull/400))
- support indexing embedded struct fields ([#399](https://github.com/tokio-rs/toasty/pull/399))
- filter on embedded enum variants ([#389](https://github.com/tokio-rs/toasty/pull/389))
- embedded enums with fields ([#381](https://github.com/tokio-rs/toasty/pull/381))
- embedded unit enums ([#355](https://github.com/tokio-rs/toasty/pull/355))
- support embedded structs as field types ([#299](https://github.com/tokio-rs/toasty/pull/299))
- better #[auto] handling for different types ([#262](https://github.com/tokio-rs/toasty/pull/262))
- support auto incrementing IDs ([#192](https://github.com/tokio-rs/toasty/pull/192))
- adds canonicalization simplification ([#245](https://github.com/tokio-rs/toasty/pull/245))
- adds complement law simplifications ([#243](https://github.com/tokio-rs/toasty/pull/243))
- adds `ExprNot` ([#214](https://github.com/tokio-rs/toasty/pull/214))
- introduces constant folding simplifications ([#212](https://github.com/tokio-rs/toasty/pull/212))

### Fixed

- lower ORDER BY expressions ([#439](https://github.com/tokio-rs/toasty/pull/439))
- restore fixed-precision jiff formatting and add driver encoding assertions ([#437](https://github.com/tokio-rs/toasty/pull/437))
- preload HasOne<Option<_>> and refactor ExprLet to Vec bindings ([#402](https://github.com/tokio-rs/toasty/pull/402))
- DDB N+1 Select Fix ([#380](https://github.com/tokio-rs/toasty/pull/380))
- Optimize O(N×M) association algorithm to O(N+M) for all relationship types ([#146](https://github.com/tokio-rs/toasty/pull/146))
- unable to update data for multiple columns ([#101](https://github.com/tokio-rs/toasty/pull/101))

### Other

- upgrade dependencies ([#598](https://github.com/tokio-rs/toasty/pull/598))
- configure readme field in workspace package metadata ([#597](https://github.com/tokio-rs/toasty/pull/597))
- organize imports and format code for consistency ([#595](https://github.com/tokio-rs/toasty/pull/595))
- update README status from Incubating to Preview ([#594](https://github.com/tokio-rs/toasty/pull/594))
- switch Assignments to BTreeMap with multi-value support ([#566](https://github.com/tokio-rs/toasty/pull/566))
- move ty() method from Field trait to Load trait ([#545](https://github.com/tokio-rs/toasty/pull/545))
- remove async_trait reexport from toasty-core ([#539](https://github.com/tokio-rs/toasty/pull/539))
- add rustdoc to remaining undocumented public items in toasty-core ([#537](https://github.com/tokio-rs/toasty/pull/537))
- remove std-util crate ([#538](https://github.com/tokio-rs/toasty/pull/538))
- bump dependencies ([#530](https://github.com/tokio-rs/toasty/pull/530))
- add comprehensive documentation to core types and modules ([#522](https://github.com/tokio-rs/toasty/pull/522))
- rename unwrap methods to expect for consistency and clarity ([#518](https://github.com/tokio-rs/toasty/pull/518))
- remove unused union query functionality ([#515](https://github.com/tokio-rs/toasty/pull/515))
- remove unsafe ([#480](https://github.com/tokio-rs/toasty/pull/480))
- fix API docs link and redesign crate index page ([#472](https://github.com/tokio-rs/toasty/pull/472))
- rename terminal query method .all() to .exec() ([#471](https://github.com/tokio-rs/toasty/pull/471))
- add nightly API docs link to README ([#469](https://github.com/tokio-rs/toasty/pull/469))
- add developer guide link to README ([#466](https://github.com/tokio-rs/toasty/pull/466))
- remove Cursor type and return Vec directly from query execution ([#448](https://github.com/tokio-rs/toasty/pull/448))
- add compile testing for documentation code snippets ([#446](https://github.com/tokio-rs/toasty/pull/446))
- Revert "feat: add Bijection type for field-to-column mappings ([#433](https://github.com/tokio-rs/toasty/pull/433))" ([#436](https://github.com/tokio-rs/toasty/pull/436))
- Add proper errors for missing embed registration ([#435](https://github.com/tokio-rs/toasty/pull/435))
- Fix type system for nested preload: infer_ty union + is_subtype_of ([#426](https://github.com/tokio-rs/toasty/pull/426))
- Add scope-depth-aware expression walker and refactor callers ([#414](https://github.com/tokio-rs/toasty/pull/414))
- add missing single-level preload permutations ([#403](https://github.com/tokio-rs/toasty/pull/403))
- Interactive transactions ([#376](https://github.com/tokio-rs/toasty/pull/376))
- rm Arc from Schema.db field. ([#387](https://github.com/tokio-rs/toasty/pull/387))
- add Expr::Error ([#385](https://github.com/tokio-rs/toasty/pull/385))
- Connection per Db ([#379](https://github.com/tokio-rs/toasty/pull/379))
- remove model_pk_to_table ([#382](https://github.com/tokio-rs/toasty/pull/382))
- add more tests ([#373](https://github.com/tokio-rs/toasty/pull/373))
- Add transaction error variants ([#377](https://github.com/tokio-rs/toasty/pull/377))
- Add transaction isolation levels and read-only mode to Operation::Transaction ([#375](https://github.com/tokio-rs/toasty/pull/375))
- wrap multi-op ExecPlan in BEGIN...COMMIT for atomicity ([#370](https://github.com/tokio-rs/toasty/pull/370))
- cleanup llm context files ([#365](https://github.com/tokio-rs/toasty/pull/365))
- expression eval fixes ([#364](https://github.com/tokio-rs/toasty/pull/364))
- eval Map returns err if applied to non-list ([#361](https://github.com/tokio-rs/toasty/pull/361))
- eval func is err ([#360](https://github.com/tokio-rs/toasty/pull/360))
- rm Expr::Key, ExprReference::Model already covers the case ([#359](https://github.com/tokio-rs/toasty/pull/359))
- expression eval tests & improvements ([#358](https://github.com/tokio-rs/toasty/pull/358))
- support OR queries over primary key ([#350](https://github.com/tokio-rs/toasty/pull/350))
- index values in memory and use it for nested merge ([#352](https://github.com/tokio-rs/toasty/pull/352))
- Improve error messages on missing model registrations ([#346](https://github.com/tokio-rs/toasty/pull/346))
- move Model.fields -> ModelRoot ([#347](https://github.com/tokio-rs/toasty/pull/347))
- Properly implement Bytes primitive ([#345](https://github.com/tokio-rs/toasty/pull/345))
- Remove dead enum code ([#341](https://github.com/tokio-rs/toasty/pull/341))
- use == operator for variable comparisons in tests ([#340](https://github.com/tokio-rs/toasty/pull/340))
- Remove Id type ([#334](https://github.com/tokio-rs/toasty/pull/334))
- rm dead enum code ([#331](https://github.com/tokio-rs/toasty/pull/331))
- support partial updates for embedded structs ([#325](https://github.com/tokio-rs/toasty/pull/325))
- return errors from driver tests ([#319](https://github.com/tokio-rs/toasty/pull/319))
- Add database reset functionality (+ serial tests) ([#322](https://github.com/tokio-rs/toasty/pull/322))
- switch assignments to be a map of Projection and not usize. ([#312](https://github.com/tokio-rs/toasty/pull/312))
- Add database migration CLI tool ([#271](https://github.com/tokio-rs/toasty/pull/271))
- move Capability to Driver ([#300](https://github.com/tokio-rs/toasty/pull/300))
- minor cleanup ([#298](https://github.com/tokio-rs/toasty/pull/298))
- remove anyhow dependency ([#297](https://github.com/tokio-rs/toasty/pull/297))
- remove bail! ([#296](https://github.com/tokio-rs/toasty/pull/296))
- misc tweaks ([#295](https://github.com/tokio-rs/toasty/pull/295))
- UnsupportedFeature ([#294](https://github.com/tokio-rs/toasty/pull/294))
- ExpressionEvaluationFailed ([#293](https://github.com/tokio-rs/toasty/pull/293))
- rename some error types ([#292](https://github.com/tokio-rs/toasty/pull/292))
- InvalidSchemaError ([#291](https://github.com/tokio-rs/toasty/pull/291))
- InvalidResultError ([#290](https://github.com/tokio-rs/toasty/pull/290))
- add "too many records" error ([#289](https://github.com/tokio-rs/toasty/pull/289))
- add is_{error_kind} methods ([#288](https://github.com/tokio-rs/toasty/pull/288))
- reorganize files ([#287](https://github.com/tokio-rs/toasty/pull/287))
- add condition failed error ([#286](https://github.com/tokio-rs/toasty/pull/286))
- validate length ([#285](https://github.com/tokio-rs/toasty/pull/285))
- move inner types to new files ([#284](https://github.com/tokio-rs/toasty/pull/284))
- RecordNotFoundError ([#283](https://github.com/tokio-rs/toasty/pull/283))
- introduce type conversion errors ([#282](https://github.com/tokio-rs/toasty/pull/282))
- add a custom error type ([#279](https://github.com/tokio-rs/toasty/pull/279))
- move more tests to the integration suite ([#277](https://github.com/tokio-rs/toasty/pull/277))
- move more tests to the integration test suite ([#275](https://github.com/tokio-rs/toasty/pull/275))
- move more tests to the test suite ([#273](https://github.com/tokio-rs/toasty/pull/273))
- move more tests to the integration suite ([#270](https://github.com/tokio-rs/toasty/pull/270))
- Move more tests to the integration suite ([#268](https://github.com/tokio-rs/toasty/pull/268))
- move more tests to integration suite. ([#265](https://github.com/tokio-rs/toasty/pull/265))
- extract integration tests to a reusable crate ([#263](https://github.com/tokio-rs/toasty/pull/263))
- Add database connection pooling ([#260](https://github.com/tokio-rs/toasty/pull/260))
- Add native support for Postgres NUMERIC and MySQL DECIMAL with rust_decimal ([#248](https://github.com/tokio-rs/toasty/pull/248))
- Add basic support for bigdecimal::BigDecimal ([#238](https://github.com/tokio-rs/toasty/pull/238))
- Implement backwards navigation for paginated queries ([#234](https://github.com/tokio-rs/toasty/pull/234))
- remove auto-mapping many models to one table ([#225](https://github.com/tokio-rs/toasty/pull/225))
- Update readme ([#230](https://github.com/tokio-rs/toasty/pull/230))
- clarify that `ExprRecord` works like a Rust tuple ([#210](https://github.com/tokio-rs/toasty/pull/210))
- Adds high level docs to core engine components ([#205](https://github.com/tokio-rs/toasty/pull/205))
- Adds docs for expression statements, tests for simplifications ([#204](https://github.com/tokio-rs/toasty/pull/204))
- Add date/time times using `jiff` ([#201](https://github.com/tokio-rs/toasty/pull/201))
- Add `Expr::Default` ([#198](https://github.com/tokio-rs/toasty/pull/198))
- rm impl Default for stmt::Expr ([#196](https://github.com/tokio-rs/toasty/pull/196))
- Allow specifying more storage types, e.g. `TEXT` for UUIDs ([#181](https://github.com/tokio-rs/toasty/pull/181))
- add Type documentation ([#180](https://github.com/tokio-rs/toasty/pull/180))
- Remove app_name from database level Column struct again ([#179](https://github.com/tokio-rs/toasty/pull/179))
- Add support for UUIDs using the uuid crate ([#178](https://github.com/tokio-rs/toasty/pull/178))
- Add support for specifying a different database name for fields ([#174](https://github.com/tokio-rs/toasty/pull/174))
- remove 2 from types and fns ([#172](https://github.com/tokio-rs/toasty/pull/172))
- Add fixed-size Rust primitive type support ([#170](https://github.com/tokio-rs/toasty/pull/170))
- handle empty tables when preloading ([#168](https://github.com/tokio-rs/toasty/pull/168))
- move rest of planner to new engine. ([#167](https://github.com/tokio-rs/toasty/pull/167))
- Combine lowering with new planner's "decompose" step ([#164](https://github.com/tokio-rs/toasty/pull/164))
- Add stmt::Filter + some refactors. ([#163](https://github.com/tokio-rs/toasty/pull/163))
- Integrate KV select path with the new planner ([#162](https://github.com/tokio-rs/toasty/pull/162))
- Include association eager-loading when lowering statement ([#159](https://github.com/tokio-rs/toasty/pull/159))
- refactor lowering to use expr context ([#160](https://github.com/tokio-rs/toasty/pull/160))
- unify ExprReference and ExprColumn ([#158](https://github.com/tokio-rs/toasty/pull/158))
- update readme to align with the working code ([#112](https://github.com/tokio-rs/toasty/pull/112))
- Add `serde::Serialize` support ([#143](https://github.com/tokio-rs/toasty/pull/143))
- refactor ExprColumn to remove direct ColumnId references. ([#156](https://github.com/tokio-rs/toasty/pull/156))
- stop hardcoding FieldId in expressions. ([#155](https://github.com/tokio-rs/toasty/pull/155))
- don't hardcode ModelId in ExprReference ([#154](https://github.com/tokio-rs/toasty/pull/154))
- Include all table refs at the top of a SourceTable ([#152](https://github.com/tokio-rs/toasty/pull/152))
- track association includes on returning clause ([#150](https://github.com/tokio-rs/toasty/pull/150))
- improve infer_ty coverage ([#145](https://github.com/tokio-rs/toasty/pull/145))
- Enhance testing infrastructure and refactor tuple Like implementations ([#139](https://github.com/tokio-rs/toasty/pull/139))
- add context documentation and reorganize project docs ([#137](https://github.com/tokio-rs/toasty/pull/137))
- delete dead code in toasty-core/tests ([#134](https://github.com/tokio-rs/toasty/pull/134))
- reduce glob imports in rest of crates ([#133](https://github.com/tokio-rs/toasty/pull/133))
- Replace glob imports with specific imports in toasty-core ([#132](https://github.com/tokio-rs/toasty/pull/132))
- remove PartialEq from stmt types where not needed ([#127](https://github.com/tokio-rs/toasty/pull/127))
- polish Visit and VisitMut traits ([#126](https://github.com/tokio-rs/toasty/pull/126))
- Add support for unsigned types ([#122](https://github.com/tokio-rs/toasty/pull/122))
- Unify ExprField and ExprReference ([#120](https://github.com/tokio-rs/toasty/pull/120))
- Remove Type::casts_to ([#119](https://github.com/tokio-rs/toasty/pull/119))
- Remove Ty::applies_binary_op - dead code ([#118](https://github.com/tokio-rs/toasty/pull/118))
- Remove `Value::to_$ty() -> Result`. ([#117](https://github.com/tokio-rs/toasty/pull/117))
- Add support for i16 ([#116](https://github.com/tokio-rs/toasty/pull/116))
- Support i8 ([#115](https://github.com/tokio-rs/toasty/pull/115))
- Add some more type-specific tests ([#114](https://github.com/tokio-rs/toasty/pull/114))
- add support for i32 types ([#113](https://github.com/tokio-rs/toasty/pull/113))
- Initial pagination implementation ([#111](https://github.com/tokio-rs/toasty/pull/111))
- add support for "order by" ([#110](https://github.com/tokio-rs/toasty/pull/110))
- rm Query from schema ([#109](https://github.com/tokio-rs/toasty/pull/109))
- rm PartialEq from stmt and schema ([#108](https://github.com/tokio-rs/toasty/pull/108))
- Add annotation to specify DB column type ([#104](https://github.com/tokio-rs/toasty/pull/104))
- ran cargo `clippy --fix -- -Wclippy::use_self` ([#103](https://github.com/tokio-rs/toasty/pull/103))
- Flatten capability struct and DRY db definitions ([#102](https://github.com/tokio-rs/toasty/pull/102))
- switch `driver::Rows::Count` type to u64 ([#98](https://github.com/tokio-rs/toasty/pull/98))
- complete driver, include in CI ([#97](https://github.com/tokio-rs/toasty/pull/97))
- Refactor sql serializer ([#95](https://github.com/tokio-rs/toasty/pull/95))
- move crates to flatter structure ([#91](https://github.com/tokio-rs/toasty/pull/91))
- Switch Toasty to use proc macros for schema declaration ([#76](https://github.com/tokio-rs/toasty/pull/76))
- Fix typo in README.md ([#4](https://github.com/tokio-rs/toasty/pull/4))
- Initial commit
