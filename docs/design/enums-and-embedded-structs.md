# Enum and Embedded Struct Support in Toasty

## Overview

This design addresses [Issue #280](https://github.com/tokio-rs/toasty/issues/280) by adding support for:
1. **Enum types** as model field types (unit variants, tuple variants, struct variants)
2. **Embedded structs** as composite types within models (no separate table)

Both features share similar challenges around schema evolution, database representation, and the mapping layer.

## Design TODO List

### Open Questions to Resolve

- [ ] **Attribute syntax**: Finalize attribute naming and structure
  - Current proposal: `#[toasty(storage = "json")]`, `#[toasty(enum)]`, `#[toasty(embedded)]`
  - Alternative: `#[toasty::json]`, `#[toasty::enum]` (path-based)?
  - Alternative: `#[storage(json)]`, `#[enum]` (separate attributes)?
  - Need to decide on consistency with existing Toasty attributes
  - Consider: `#[toasty(column = "...")]` vs `#[column("...")]`

- [x] **Column naming customization syntax**: Template-based column naming with variables
  - **Decision**: Use template strings with `{variable}` placeholders
  - **Available variables**:
    - `{field}` - the enum field name on the parent model (e.g., `critter`)
    - `{variant}` - the variant name in snake_case (e.g., `human`, `lizard`)
    - `{name}` - the field name within the variant (e.g., `race`)
  - **Default template**: `{field}_{variant}_{name}` → `critter_human_race`
  - **Customization examples**:
    - On variant: `#[toasty(column = "{field}_h_{name}")]` → `critter_h_race`
    - On variant field: `#[toasty(column = "{field}_{variant}_species")]` → `critter_human_species`
    - On model field: `#[toasty(column = "creature_{variant}_{name}")]` → `creature_human_race`
  - **Discriminator column**: Separate attribute `#[toasty(discriminator = "creature_type")]` on model field
  - See "Column Naming for Flattened Enums" section for full specification

- [ ] **Default storage strategy**: Confirm flattened as global default
  - Flattened for enums (queryable, indexable)
  - Flattened for embedded structs (queryable)
  - JSON only when explicitly requested
  - Any exceptions? (e.g., deeply nested structures auto-detect?)

- [ ] **Type-level vs field-level storage configuration**: Which takes precedence?
  - Recommendation: Field overrides type, type overrides global default
  - But need to validate this doesn't cause confusion
  - How to handle conflicts/warnings?

- [ ] **JSON format for enums**: Externally tagged vs other formats
  - Current proposal: Externally tagged `{"Variant": {...}}`
  - Matches serde default, familiar to users
  - But more verbose - is this okay?
  - Should we support multiple formats later?

- [ ] **Embedded type registration**: Auto-register or explicit?
  - Recommendation: Auto-register when used in models
  - But what about circular dependencies?
  - What about enums/structs defined in other crates?

- [ ] **Relations in embedded types**: Allow or forbid?
  - Initial recommendation: Forbid (compile error)
  - But some users might want this for complex models
  - Defer to later phase or never?

- [ ] **Serde integration**: Custom JSON or leverage serde?
  - Recommendation: Custom initially, serde as opt-in later
  - But custom means we have to handle all edge cases
  - Worth the complexity?

- [ ] **Unit-only enum optimization**: Special case or same as data-carrying?
  - Can we detect unit-only and skip field columns entirely?
  - Just discriminator column, simpler schema
  - Worth the special case in codegen?

- [ ] **Tuple variant support**: Phase 1 or defer?
  - Tuple variants: `Phone(String, String)` vs `Phone { country: String, number: String }`
  - How to name columns for tuple fields? `critter_phone_0`, `critter_phone_1`?
  - More common in Rust, but less clear in DB schema
  - Defer to later phase?

- [ ] **DynamoDB special handling**: Does flattened storage make sense?
  - DynamoDB has native map/list types
  - Flattened storage might not map naturally
  - Should DynamoDB force JSON storage?
  - Or use native map with special mapping?

- [ ] **Schema migration tooling**: Generate migrations or manual?
  - Adding variant fields → add nullable columns (safe)
  - Removing variant fields → orphan columns (manual cleanup?)
  - Changing storage strategy → requires data migration (fail? warn?)
  - Auto-generate SQL or just detect and error?

- [ ] **NULL handling in variant fields**: Store NULL or omit?
  - When `critter = 'Human'`, should `critter_lizard_race` be NULL or never written?
  - NULL is simpler, but wastes space
  - Default values vs NULL for `Option<T>` fields?

- [ ] **Discriminator type**: String, integer ordinal, or database enum?
  - String: `'Human'`, `'Lizard'` (verbose but clear)
  - Integer: `0`, `1` (compact but opaque)
  - DB enum: Native type (best of both, but not portable)
  - Recommendation: DB enum when available, string fallback
  - But what if variant order changes? Ordinals break.

- [ ] **Numeric field IDs for evolution**: Should we support `#[toasty(id = 1)]`?
  - **Background**: Protobuf, Thrift, FlatBuffers all use numeric field IDs
  - **Benefit**: Enables safe renaming of variants and fields
  - **Current approach**: Use Rust field/variant names → column names
  - **Problem**: Renaming in Rust requires column rename (schema migration)
  - **Option A**: Support optional `#[toasty(id = 1)]` attribute
    - Column name includes ID: `critter_1_race` instead of `critter_human_race`
    - Can rename variant freely without migration
    - IDs must be unique and never reused
  - **Option B**: Always require IDs (like protobuf)
    - More boilerplate
    - Better for evolution
    - Breaking change if added later
  - **Option C**: Don't support IDs, use `#[toasty(rename = "old_name")]` for evolution
    - Simpler initially
    - Renaming requires attribute (not automatic)
    - Column names stay human-readable
  - **Question**: Which option? A (optional) seems best balance

- [ ] **Deprecation support**: How to handle deprecated variants/fields?
  - **Background**: FlatBuffers/Protobuf never delete, only deprecate
  - **Benefit**: Prevents accidental field ID/name reuse, preserves old data
  - **Option A**: Support `#[deprecated]` standard Rust attribute
    - Variant/field marked deprecated in Rust code
    - Column kept in database (not dropped)
    - Warning when querying deprecated variant
    - Natural Rust convention
  - **Option B**: Custom `#[toasty(deprecated)]` attribute
    - More control (can add reason, version)
    - Example: `#[toasty(deprecated = "use NewVariant instead")]`
  - **Option C**: `#[toasty(deprecated(since = "1.2", note = "..."))]`
    - Matches Rust deprecation attributes
    - Tracks when deprecated
  - **Question**: Option A (standard `#[deprecated]`) or Option C (extended)?
  - **Implementation**: 
    - Keep column in schema
    - Generate code with deprecation warnings
    - Document in schema metadata

- [ ] **Default values for new fields**: How to handle missing data?
  - **Background**: FlatBuffers warns "never change defaults", critical for evolution
  - **Problem**: Old data won't have new variant fields, need safe defaults
  - **Option A**: Require `#[toasty(default)]` or `#[toasty(default = value)]`
    - Explicit default for new fields
    - `#[toasty(default)]` uses `Default::default()`
    - `#[toasty(default = "unknown")]` for custom value
  - **Option B**: Require all new fields be `Option<T>`
    - Simpler - NULL is the default
    - More ergonomic
    - Forces nullable types
  - **Option C**: Both - `Option<T>` or explicit default required
    - Maximum safety
    - Choose based on whether None is a valid value
  - **Question**: Option C (require either Option or default)?
  - **Warning**: Changing defaults after initial definition is breaking change
  - **Validation**: Detect default changes in `toasty schema check`

### Design Gaps to Fill

- [ ] **Error handling strategy**: What errors can occur and how to report them?
  - Invalid discriminator value (unknown variant)
  - Missing required field data
  - Type mismatch in JSON deserialization
  - Schema mismatch (expected flattened, got JSON)
  - Column name collisions

- [ ] **Nested embedded types**: How deep can we go?
  - `enum Creature { Human { address: Address } }` where `Address` is embedded
  - Column naming: `critter_human_address_street`?
  - Any depth limits?
  - Performance implications?

- [ ] **Multiple enums with same variant names**: Column collisions?
  - `enum A { Foo { x: i32 } }` and `enum B { Foo { x: String } }`
  - Both in same model → `field_a_foo_x` vs `field_b_foo_x` (okay)
  - But what if field names collide? Need detection/error

- [ ] **Index generation**: Automatic or manual?
  - Should we auto-generate index on discriminator column?
  - What about variant field indexes?
  - User control via attributes?

- [ ] **Constraint validation**: Check discriminator matches populated fields?
  - At application level (Rust) or database level (CHECK constraints)?
  - CHECK constraints are complex and database-specific
  - Application-level validation easier but less safe

- [ ] **Query builder API details**: Exact method signatures
  - `.is_variant("Human")` or `.is_human()`?
  - `.as_human()` returns what type? Proxy to fields?
  - How to make type-safe? (See Challenge #4)

- [ ] **Generated code structure**: What exactly gets generated?
  - Schema registration code
  - Conversion traits
  - Query helper methods
  - Field accessor types
  - Need mockups/examples

- [ ] **Backwards compatibility**: How to avoid breaking changes?
  - If we add enum support, does it affect existing code?
  - New reserved keywords? (e.g., `enum`, `embedded`)
  - Attribute conflicts?

- [ ] **Documentation requirements**: What do users need to know?
  - When to use flattened vs JSON
  - How to migrate between storage strategies
  - Column naming rules
  - Limitations (no relations in embedded types)
  - Performance implications

### Technical Details to Specify

- [ ] **Expression type semantics**: What do new expr types mean?
  - `Expr::EnumDiscriminator` - returns what type? String? Enum?
  - `Expr::EnumField` - NULL behavior when wrong variant?
  - `Expr::ReconstructEnum` - validation? Errors?

- [ ] **Value representation**: How are enums represented in `stmt::Value`?
  - New `Value::Enum` variant?
  - Or reuse `Value::Record` with special structure?
  - Or `Value::String` for discriminator + `Value::Record` for fields?

- [ ] **Mapping expressions details**: Exact structure
  - `model_to_table` for flattened enum - which expressions?
  - `table_to_model` for flattened enum - reconstruction logic?
  - Nullable field handling

- [ ] **Driver capability negotiation**: How drivers report enum support?
  - `Capability.native_enum: bool`
  - `Capability.json_type: bool`
  - What if driver doesn't support either?

- [ ] **Column length limits**: Handle long generated names?
  - PostgreSQL limit: 63 characters
  - MySQL limit: 64 characters
  - What if `{field}_{variant}_{field_name}` exceeds?
  - Truncation? Hashing? Error?

- [ ] **Case sensitivity**: Variant names in database?
  - Rust: `CamelCase` variants
  - Database: `snake_case`, `CamelCase`, `UPPER_CASE`?
  - Recommendation: preserve Rust case or convert?
  - Affects queries and deserialization

### Examples to Write

- [ ] **Basic enum example**: Simple enum with flattened storage
- [ ] **Complex enum example**: Enum with multiple variants, nested fields
- [ ] **JSON storage example**: When and how to use JSON
- [ ] **Embedded struct example**: Address in User
- [ ] **Migration example**: Adding variant, adding field
- [ ] **Query example**: Filtering by variant, filtering by variant field
- [ ] **Index example**: Creating index on discriminator
- [ ] **Error example**: Invalid variant in database
- [ ] **Custom column names example**: Using attributes to override

### Next Steps

1. **Resolve attribute syntax** - Needs decision before any codegen work
2. **Write detailed examples** - Validate design with realistic code
3. ~~**Finalize column naming rules**~~ - ✅ Done: Template-based naming with `{field}`, `{variant}`, `{name}` variables
4. **Specify expression semantics** - Needed for Phase 1 implementation
5. **Create proof-of-concept** - Spike to validate approach

## Survey of Existing Solutions

This section examines how other libraries (Rust ORMs and schema serialization libraries) handle similar problems around enum types, embedded structures, and schema evolution.

### Rust ORMs

#### Diesel ORM
- **Enum Support**: Via [`diesel-derive-enum`](https://github.com/adwhit/diesel-derive-enum) crate
- **Approach**: `#[derive(DbEnum)]` macro for PostgreSQL native ENUM types
- **Limitations**: 
  - **Unit variants only** - cannot carry data
  - PostgreSQL-specific (SQLite uses CHECK constraints as workaround)
  - Variants map to database strings (snake_case by default)
- **Customization**: `#[db_enum(rename = "...")]` for variant names
- **Schema Evolution**: Diesel CLI generates enum types in schema, v3.0.0 uses `#[db_enum(...)]` namespace
- **Lesson for Toasty**: Unit-only enums are well-solved; we need to handle data-carrying variants

**Source**: [diesel-derive-enum](https://github.com/adwhit/diesel-derive-enum), [Diesel custom types guide](https://kitsu.me/posts/2020_05_24_custom_types_in_diesel)

#### SeaORM
- **Enum Support**: Two approaches - `DeriveActiveEnum` and `DeriveValueType`
  - `DeriveActiveEnum`: For string, integer, or native DB enums (unit variants)
  - `DeriveValueType`: Since 1.1.8 supports enums, since 2.0.0 supports structs that convert to/from strings
- **Embedded Types**: Via `DeriveValueType` for custom types that serialize to single columns
- **Evolution**: [SeaORM 2.0 update](https://dev.to/seaql/seaorm-20-strongly-typed-column-i08) improved type safety with strongly-typed columns
- **Composite Keys**: Supports composite primary keys (max arity 12)
- **Lesson for Toasty**: Two-tier approach (simple enums vs complex types) is common; SeaORM uses serialization for complex types

**Source**: [SeaORM ActiveEnum docs](https://www.sea-ql.org/SeaORM/docs/generate-entity/enumeration/), [SeaORM NewType docs](https://www.sea-ql.org/SeaORM/docs/generate-entity/newtype/)

#### SQLx
- **Custom Types**: `#[derive(sqlx::Type)]` trait for custom types
- **Enum Support**: 
  - PostgreSQL user-defined ENUMs: `#[sqlx(type_name = "color", rename_all = "lowercase")]`
  - Integer representation: `#[repr(i32)]` on enum
  - String representation: `#[sqlx(type_name = "text")]`
- **Struct Types**: PostgreSQL composite types via `#[derive(sqlx::Type)]` on structs (PostgreSQL only)
- **Limitations**: Struct support is PostgreSQL-specific
- **Lesson for Toasty**: Multiple representation strategies (native enum, integer, string) mirrors our approach

**Source**: [SQLx Type trait docs](https://docs.rs/sqlx/latest/sqlx/trait.Type.html), [SQLx postgres types](https://docs.rs/sqlx/latest/sqlx/postgres/types/index.html)

### Cross-Language Schema Libraries

These libraries solve similar problems (nested structures, schema evolution, union types) for wire protocols and can inform our database storage design.

#### Protocol Buffers (protobuf)

**Enum Representation**:
- Simple enums (unit variants only)
- First enumerant must be zero (default value)

**Union Types** (`oneof`):
- Like Rust enums with data, but **schema evolution is tricky**
- [Adding field to oneof](https://yokota.blog/2021/08/26/understanding-protobuf-compatibility/) = forward incompatible
- [Removing field from oneof](https://softwaremill.com/schema-evolution-protobuf-scalapb-fs2grpc/) = backward incompatible
- Moving field into oneof = **not safe** (data loss risk)
- Can safely move single field into **new** oneof

**Embedded Messages**:
- Nested messages work like top-level messages
- Standard backward compatibility rules apply
- Adding fields is safe (old code ignores new fields)

**Evolution Strategy**:
- Field numbers are permanent (never reuse)
- Mark new fields as `optional` with defaults
- Use `reserved` for removed fields

**Lesson for Toasty**: 
- Oneof evolution is hard - our flattened approach may be easier to evolve
- Field numbering strategy prevents conflicts (we use names)
- Reserved field names important for preventing accidents

**Source**: [Protobuf schema evolution guide](https://jsontotable.org/blog/protobuf/protobuf-schema-evolution), [Understanding Protobuf compatibility](https://yokota.blog/2021/08/26/understanding-protobuf-compatibility/), [Schema evolution with ScalaPB](https://softwaremill.com/schema-evolution-protobuf-scalapb-fs2grpc/)

#### Prost (Rust protobuf implementation)

**Oneof Code Generation**:
- Proto `oneof` becomes Rust `Option<enum>`
- Enum defined in nested module: `message Foo { oneof widget { ... } }` → `Option<foo::Widget>`
- Idiomatic Rust enums with data: `enum Widget { Quux(i32), Bar(String) }`
- Customization via attributes in prost-build config

**Lesson for Toasty**:
- Nested module pattern keeps generated code organized
- `Option<enum>` for nullable oneofs is clean
- Codegen customization via attributes is powerful

**Source**: [prost GitHub](https://github.com/tokio-rs/prost), [Rust protobuf generated code guide](https://protobuf.dev/reference/rust/rust-generated/)

#### Apache Thrift

**Enum Support**:
- Simple enums with pre-defined constants
- Used for restricting field values

**Union Types**:
- Like C++ unions - exactly one field active at a time
- All fields implicitly optional
- No required/optional distinction (simpler than structs)

**Schema Evolution Rules**:
- Field IDs (not names) identify fields - **can rename freely**
- Cannot remove fields (can deprecate, never reuse ID)
- Can only add **optional** fields to structs
- For unions, can add new variants (since all fields are optional)

**Lesson for Toasty**:
- ID-based identification better for evolution than names
- Thrift's union model simpler than protobuf oneof
- Clear rules for safe evolution

**Source**: [Apache Thrift IDL](https://thrift.apache.org/docs/idl), [Thrift Missing Guide](http://diwakergupta.github.io/thrift-missing-guide/)

#### Cap'n Proto

**Union Types**:
- **Must be embedded in structs** (not free-standing)
- Allows adding fields to parent struct later (evolution-friendly)
- Union members numbered in same space as struct fields
- Can move existing field into **new** union safely

**Nested Structures**:
- Fields can be added anywhere, but numbering reflects addition order
- Groups logically encapsulate fields
- Fields need not be contiguous in memory (evolution-friendly)

**Evolution Capabilities**:
- New fields, enums, methods can be added (number must be larger)
- Forward compatibility: new readers use defaults for missing fields
- Backward compatibility: old readers ignore unknown fields
- Moving field to new union is safe

**Lesson for Toasty**:
- Union-in-struct requirement enables better evolution (similar to our embedded enums in models)
- Field numbering order matters for evolution
- Their evolution model is very flexible

**Source**: [Cap'n Proto schema language](https://capnproto.org/language.html), [Cap'n Proto encoding](https://capnproto.org/encoding.html)

#### FlatBuffers

**Schema Evolution**:
- New tables/vectors/structs always allowed
- New fields **must be at end** of table (unless using `id` attribute)
- Never remove fields - mark as `deprecated` instead
- Unions: can add variants to end

**Union Encoding**:
- Combination of two fields: enum (choice) + offset (data)
- Adding variant to end is safe
- Provides flexibility for future changes

**Versioning**:
- File identifiers ensure type safety
- Explicit version fields recommended
- `flatc --conform` checks schema evolution correctness

**Important Constraint**:
- **Don't change default values** after initial definition
- Defaults not stored in serialized data
- Changing defaults breaks compatibility

**Lesson for Toasty**:
- Explicit deprecation better than deletion
- Two-field encoding for unions (discriminator + data) matches our flattened approach
- Default value stability is critical
- Tooling can validate schema evolution

**Source**: [FlatBuffers evolution docs](https://flatbuffers.dev/evolution/), [FlatBuffers schema versioning](https://app.studyraid.com/en/read/10837/332121/schema-versioning-and-evolution)

#### Apache Avro

**Union Types**:
- Express optionality (no explicit optional/required)
- Common pattern: `union { null, Type }` for optional fields
- **Default value must match first type in union**
- If null is default, must be first: `["null", "Type"]`

**Records in Unions**:
- Each record must have unique full name
- Incompatible record evolution in union may be missed by registries
- Union branches must resolve to distinct types

**Schema Evolution**:
- Default values only used when reading old data (missing field)
- First matching schema in reader's union is used
- Well-defined compatibility rules (BACKWARD, FORWARD, FULL)

**Lesson for Toasty**:
- Union-based optionality is clever but complex
- Default value ordering matters (first type gets default)
- Multiple compatibility modes (we should document ours)
- Records-in-unions have naming constraints (our variant fields may too)

**Source**: [Avro specification](https://avro.apache.org/docs/1.11.1/specification/), [Practical Avro schema evolution](https://medium.com/expedia-group-tech/practical-schema-evolution-with-avro-c07af8ba1725), [Avro union types in Hackolade](https://hackolade.com/help/UniontypesinAvro.html)

### Key Patterns Observed

#### 1. **Unit-Only Enums vs Data-Carrying Enums**
- **Diesel, SeaORM, SQLx**: Support unit-only enums well (map to DB enums or strings)
- **Protobuf, Thrift, Avro**: Simple enums separate from union types
- **Toasty approach**: Unified type system, different storage for unit vs data-carrying

#### 2. **Field Identification**
- **Protobuf, Thrift, FlatBuffers**: Use numeric field IDs (stable across renames)
- **Avro**: Uses field names (renaming requires aliases)
- **Toasty**: Uses field names - consider numeric IDs for flattened storage?

#### 3. **Union/Oneof Evolution Challenges**
- **Protobuf**: Oneof evolution is error-prone (data loss risks)
- **Thrift**: Simpler (all fields optional, can add variants)
- **Cap'n Proto**: Must embed in struct (enables later additions)
- **FlatBuffers**: Add variants to end only
- **Toasty approach**: Flattened storage more flexible than JSON oneofs

#### 4. **Nested/Embedded Structures**
- **All wire protocols**: Support nested messages naturally
- **Rust ORMs**: Limited support (SeaORM's DeriveValueType, SQLx PostgreSQL-only)
- **Toasty opportunity**: First-class embedded struct support with flattening

#### 5. **Default Values**
- **Protobuf, Thrift, Cap'n Proto**: Defaults critical for missing fields
- **Avro**: Default must match first union type
- **FlatBuffers**: Never change defaults (not stored, only used for missing data)
- **Toasty**: Need default strategy for new variant fields

#### 6. **Deprecation vs Deletion**
- **FlatBuffers, Protobuf**: Mark deprecated, never delete/reuse
- **Thrift**: Can remove but never reuse ID
- **Toasty**: Should support `#[deprecated]` attribute, keep column

#### 7. **Two-Field Union Encoding**
- **FlatBuffers**: Discriminator enum + offset
- **Cap'n Proto**: Tag + union storage
- **Toasty flattened**: Discriminator column + variant field columns (extended version)

#### 8. **Tooling for Validation**
- **FlatBuffers**: `flatc --conform` checks evolution
- **Avro**: Schema Registry validates compatibility
- **Protobuf**: Schema Registry with compatibility modes
- **Toasty opportunity**: CLI tool to validate schema evolution

### Applicable Solutions for Toasty

Based on the survey, here are patterns we should adopt:

1. **Numeric Field IDs (Optional)**: Consider `#[toasty(id = 1)]` for stable field identification in flattened storage (enables renames)

2. **Deprecation Support**: Add `#[deprecated]` attribute, keep columns, warn in queries

3. **Reserved Names**: Add `#[toasty(reserved = "old_field_name")]` to prevent accidents

4. **Default Values**: Require `#[toasty(default)]` or `Option<T>` for new variant fields

5. **Explicit Versioning**: Consider `#[toasty(version = 2)]` on models for explicit version tracking

6. **Evolution Validation**: CLI command `toasty schema check` to validate evolution rules

7. **Two-Tier Storage**: Unit-only enums → native DB enum (like Diesel/SeaORM), data-carrying → flattened (our innovation)

8. **Codegen Customization**: Prost-style build configuration for generated code attributes

9. **Compatibility Modes**: Document BACKWARD, FORWARD, FULL compatibility (like Avro)

10. **Nested Modules**: Organize generated code in modules (like prost) for clarity

## Design Challenges

This section captures the key problems and trade-offs we need to address:

### 1. Rust Enums vs Database Enums (Impedance Mismatch)

**Challenge:** Rust enums can carry data, database enums cannot.

```rust
// Rust enum - can carry data
enum Status {
    Pending,
    Active { since: DateTime },
    Completed { at: DateTime, reason: String },
}

// PostgreSQL ENUM - just labels
CREATE TYPE status AS ENUM ('Pending', 'Active', 'Completed');
```

**Implications:**
- Native DB enums are efficient but limited to unit variants
- JSON storage supports data-carrying variants but loses type safety and queryability
- Need to choose storage based on enum complexity

**Resolution Strategy:**
- Automatic selection: unit-only → native enum, data-carrying → JSON
- User can override with `#[toasty(storage = "...")]`
- Migration path when evolving from unit-only to data-carrying

### 2. Storage Strategy Selection

**Challenge:** Multiple valid ways to store the same data, each with trade-offs.

**Options:**
- **Native DB ENUM**: Efficient, type-safe, queryable, but only for unit variants
- **JSON**: Flexible, schema-evolution friendly, but not queryable/indexable
- **Flattened columns**: Queryable, indexable, but requires migrations and many nullable columns
- **Separate table**: Clean schema, but requires JOINs on every access

**Implications:**
- No single "best" strategy - depends on use case
- Need to support multiple strategies
- Need clear defaults to avoid decision paralysis
- Storage strategy affects query capabilities
- **Indexing by variant is critical** - can't efficiently index JSON-stored enums
- Most users will want queryable enums by default

**Resolution Strategy:**
- **Default: Flattened storage** - discriminator column + nullable field columns
  - Discriminator can use native DB enum when available (PostgreSQL, MySQL)
  - All variant fields get their own nullable columns
  - Indexable and queryable out of the box
- **Opt-in: JSON storage** via `#[toasty(storage = "json")]`
  - For deeply nested structures
  - When queryability isn't needed
  - For schema flexibility
- Document trade-offs clearly
- Make migration between strategies possible (but not automatic)

### 3. Schema Evolution and Backward Compatibility

**Challenge:** Database contains data serialized with old schema, code uses new schema.

**Evolution scenarios:**
- Adding new enum variants (should work seamlessly)
- Adding fields to existing variants (need defaults or nullable)
- Removing variants (how to handle old data?)
- Renaming variants (need alias/mapping)
- Changing variant field types (complex migration)

**Implications:**
- JSON deserialization must handle missing fields
- Need attributes for defaults: `#[toasty(default)]`
- Need attributes for renames: `#[toasty(rename = "OldName")]`
- Need strategy for unknown variants (error vs ignore)

**Resolution Strategy:**
- Strict by default (error on unknown variant/missing required field)
- Opt-in leniency with attributes
- Clear migration guides for breaking changes
- Versioning strategy for future (not Phase 1)

### 4. Query Interface Design

**Challenge:** How should users filter/query on enum variants and embedded fields?

**For enums:**
```rust
// Variant check - should this work with all storage strategies?
User::all().filter(User::FIELDS.status().is_pending())

// Field access - only works with flattened storage
User::all().filter(User::FIELDS.status().as_active().since().gt(yesterday))

// JSON path - database-specific, complex
User::all().filter(User::FIELDS.status().json_path("$.since").gt(yesterday))
```

**For embedded structs:**
```rust
// Field access - only works with flattened storage
User::all().filter(User::FIELDS.address().city().eq("Seattle"))

// JSON path - database-specific
User::all().filter(User::FIELDS.address().json_path("$.city").eq("Seattle"))
```

**Implications:**
- Storage strategy determines query capabilities
- Need clear compile-time errors when operations aren't supported
- Different syntax for JSON vs flattened vs native enum
- Type safety matters - prevent invalid queries

**Resolution Strategy:**
- Phase 1: Basic variant checks only (`.is_variant()`)
- Phase 3: Flattened storage enables full field access
- Future: JSON path support (database-specific)
- Generate compile-time errors for unsupported operations

### 5. Mapping Layer Complexity

**Challenge:** Converting between Rust types and database columns is complex for enums/embedded types.

**For JSON storage:**
- Need bidirectional JSON serialization/deserialization
- Handle NULL values
- Handle serialization errors gracefully

**For flattened storage:**
- One Rust enum → multiple database columns
- Need expressions to pack/unpack: `Expr::EnumDiscriminator`, `Expr::ReconstructEnum`
- Affects entire query pipeline (simplify, lower, plan, execute)

**For native enums:**
- String <-> Enum conversions
- Variant name mapping
- Case sensitivity considerations

**Implications:**
- Extends `schema::mapping::Model` with new expression types
- Affects `Value::cast()` with new conversion logic
- Impacts query engine (new expression types in AST)
- Each storage strategy has different mapping logic

**Resolution Strategy:**
- Start with JSON (simpler mapping, just serialize/deserialize)
- Add native enum support (string conversions)
- Add flattened storage later (complex expressions, full pipeline impact)
- Reuse existing mapping infrastructure

### 6. Multi-Database Support

**Challenge:** Different databases have different capabilities and native types.

**Database capabilities:**
- **PostgreSQL**: User-defined ENUM types, JSONB with query support
- **MySQL**: Column-level ENUM, JSON type with limited query support
- **SQLite**: No native ENUM, TEXT storage, JSON1 extension (optional)
- **DynamoDB**: Native map/list types, no enums

**Implications:**
- Storage strategy may vary by database
- Native enum implementation differs (PostgreSQL type vs MySQL column constraint)
- JSON type differs (JSONB vs JSON vs TEXT)
- Feature availability varies (JSON path queries)

**Resolution Strategy:**
- Use driver capabilities to select storage
- Graceful degradation (SQLite: TEXT instead of ENUM)
- Common denominator for Phase 1 (JSON as TEXT works everywhere)
- Database-specific optimizations later

### 7. Codegen Complexity

**Challenge:** Generating correct code for all combinations of features.

**Need to generate:**
- Schema registration for embedded/enum types
- Conversion traits (`From<T>` for `Value`, `TryFrom<Value>` for `T`)
- Field accessors for nested filtering
- Query helper methods (`.is_variant()`, `.as_variant()`)
- Different code for each storage strategy

**Implications:**
- Large expansion of codegen logic
- Need to test all combinations
- Error messages must be clear
- Generated code must be debuggable

**Resolution Strategy:**
- Reuse existing codegen patterns
- Generate minimal code (rely on trait impls)
- Clear error messages for unsupported combinations
- Incremental implementation (JSON first, flattened later)

### 8. Type System Extensions

**Challenge:** Representing embedded types in Toasty's type system.

**Need to track:**
- Embedded type definitions (fields, variants)
- Storage strategy per field
- Relationship to parent model
- Expression type vs storage type

**Current type system:**
- `stmt::Type` - expression types (what queries work with)
- `schema::db::Type` - storage types (how database stores it)
- `schema::app::Field` - application-level field definitions

**Implications:**
- Add `Type::Embedded(EmbeddedId)` and enhance `Type::Enum`
- Add `FieldTy::Embedded` to represent embedded fields
- Add `ModelKind::Embedded` and `ModelKind::Enum`
- Track storage strategy in schema

**Resolution Strategy:**
- Extend existing type system (not a new parallel system)
- Track embedded types as special kinds of models
- Mapping layer handles conversion between app and storage types

### 9. Testing Complexity

**Challenge:** Need to test across multiple databases, storage strategies, and feature combinations.

**Test matrix:**
- 4 databases (SQLite, PostgreSQL, MySQL, DynamoDB)
- 3 storage strategies (native enum, JSON, flattened)
- Unit variants, struct variants, tuple variants
- Embedded structs, nested embedded types
- Schema evolution scenarios
- Error cases (invalid data, type mismatches)

**Implications:**
- Large test suite
- Slow CI times
- Need database infrastructure for integration tests
- Need to test migration paths

**Resolution Strategy:**
- Incremental testing (phase by phase)
- Focus on integration tests (not unit tests)
- Use existing test infrastructure (`tests/` crate)
- Test common path thoroughly, edge cases selectively

### 10. Migration Story

**Challenge:** How do users migrate existing data when changing storage strategies?

**Migration scenarios:**
- Unit-only enum → data-carrying enum (native → JSON)
- JSON storage → flattened storage (for queryability)
- Adding/removing variants
- Renaming fields

**Implications:**
- Need SQL migration generation
- Data migration vs schema migration
- Potential data loss on incompatible changes
- Rollback strategy

**Resolution Strategy:**
- Phase 1: No automatic migrations, manual SQL
- Phase 4: Generate migration SQL for safe changes
- Document unsafe migrations (user responsibility)
- Warn on breaking changes at compile time

### 11. Column Naming for Flattened Enums

**Challenge:** How to name database columns for variant fields in flattened storage?

**Example:**
```rust
struct User {
    critter: Creature,
}

enum Creature {
    Human { race: String },
    Lizard { race: String },
}
```

**Possible naming schemes:**
1. **Variant-prefixed** (recommended): `critter`, `critter_human_race`, `critter_lizard_race`
2. **Flat with separator**: `critter`, `critter_race` (ambiguous - which variant?)
3. **Nested**: `critter`, `critter.human.race` (not valid SQL identifier)
4. **Shortened**: `critter`, `critter_h_race`, `critter_l_race` (unclear)

**Implications:**
- Need deterministic column naming algorithm
- Must avoid name collisions (multiple variants with same field name)
- Must handle long names (database column length limits)
- User customization needed for legacy schemas
- Need to handle nested embedded structs in variants

**Resolution Strategy: Template-Based Column Naming**

Use template strings with `{variable}` placeholders that get substituted at schema build time.

**Template Variables:**

| Variable | Description | Example Value |
|----------|-------------|---------------|
| `{field}` | The enum field name on the parent model | `critter` |
| `{variant}` | The variant name (snake_case) | `human`, `lizard` |
| `{name}` | The field name within the variant | `race`, `habitat` |

**Default Templates:**
- **Discriminator column**: `{field}` → `critter`
- **Variant field column**: `{field}_{variant}_{name}` → `critter_human_race`

**Customization Levels:**

Templates can be specified at three levels (later levels override earlier):

1. **Model field level** - applies to discriminator and all variant fields:
   ```rust
   struct User {
       #[toasty(discriminator = "creature_type")]  // discriminator only
       #[toasty(column = "creature_{variant}_{name}")]  // variant fields
       critter: Creature,
   }
   ```
   Result: `creature_type`, `creature_human_race`, `creature_lizard_race`

2. **Variant level** - applies to all fields in that variant:
   ```rust
   #[derive(Model)]
   #[toasty(enum)]
   enum Creature {
       #[toasty(column = "{field}_h_{name}")]  // shorthand for Human
       Human { race: String },

       Lizard { race: String, habitat: String },  // uses default
   }
   ```
   Result: `critter_h_race`, `critter_lizard_race`, `critter_lizard_habitat`

3. **Variant field level** - applies to specific field only:
   ```rust
   #[derive(Model)]
   #[toasty(enum)]
   enum Creature {
       Human {
           #[toasty(column = "{field}_human_species")]  // override just this field
           race: String,
       },
       Lizard { race: String },
   }
   ```
   Result: `critter_human_species`, `critter_lizard_race`

**Nested Embedded Structs:**

When a variant contains an embedded struct, additional variables are available:

| Variable | Description | Example |
|----------|-------------|---------|
| `{path}` | Full path from variant to leaf field (dot-separated) | `address.city` |
| `{parent}` | Immediate parent field name | `address` |

```rust
#[derive(Model)]
#[toasty(embedded)]
struct Address {
    city: String,
    street: String,
}

#[derive(Model)]
#[toasty(enum)]
enum ContactInfo {
    Email { address: String },
    Mail { address: Address },  // embedded struct
}

#[derive(Model)]
struct User {
    #[key]
    id: Id<Self>,
    contact: ContactInfo,
}
```

Default column names (using `{field}_{variant}_{path}` with dots → underscores):
- `contact` (discriminator)
- `contact_email_address`
- `contact_mail_address_city`
- `contact_mail_address_street`

Custom template example:
```rust
#[derive(Model)]
#[toasty(enum)]
enum ContactInfo {
    Email { address: String },

    #[toasty(column = "{field}_m_{path}")]  // shorter prefix for Mail
    Mail { address: Address },
}
```

Result: `contact_m_address_city`, `contact_m_address_street`

**Literal Braces:**

To include a literal `{` or `}` in the column name, escape with double braces:
- `{{` → `{`
- `}}` → `}`

**Validation Rules:**

1. **Unknown variables**: Compile error if template contains unknown `{variable}`
2. **Empty result**: Compile error if substitution results in empty string
3. **Invalid SQL identifier**: Compile error if result contains invalid characters
4. **Collision detection**: Compile error if two fields resolve to same column name
5. **Length limits**: Warning if column name exceeds 63 characters (PostgreSQL limit)

**Complete Example:**

```rust
#[derive(Model)]
struct User {
    #[key]
    id: Id<Self>,

    #[toasty(discriminator = "creature_kind")]
    #[toasty(column = "c_{variant}_{name}")]  // prefix with 'c_' for all
    critter: Creature,
}

#[derive(Model)]
#[toasty(enum)]
enum Creature {
    Human {
        race: String,        // c_human_race
        age: i32,            // c_human_age
    },

    #[toasty(column = "c_lz_{name}")]  // override: shorter prefix for Lizard
    Lizard {
        race: String,        // c_lz_race

        #[toasty(column = "c_lizard_env")]  // override: specific column name
        habitat: String,     // c_lizard_env
    },
}
```

Generated schema:
```sql
CREATE TABLE user (
    id INTEGER PRIMARY KEY,
    creature_kind TEXT NOT NULL,   -- discriminator
    c_human_race TEXT,             -- Human.race
    c_human_age INTEGER,           -- Human.age
    c_lz_race TEXT,                -- Lizard.race (variant-level override)
    c_lizard_env TEXT              -- Lizard.habitat (field-level override)
);
```

### 12. Field-Level vs Type-Level Storage Configuration

**Challenge:** Where should users specify storage strategy - on the type or on the field?

**Option A: Type-level** (applies to all uses)
```rust
#[derive(Model)]
#[toasty(enum, storage = "json")]  // All fields of this type use JSON
enum Status { ... }

struct User {
    status: Status,  // Uses JSON (from type definition)
}
```

**Option B: Field-level** (per-field override)
```rust
#[derive(Model)]
#[toasty(enum)]
enum Status { ... }

struct User {
    #[toasty(storage = "json")]  // This field uses JSON
    status: Status,
    
    #[toasty(storage = "flattened")]  // This field uses flattened
    backup_status: Status,
}
```

**Option C: Both** (field overrides type)
```rust
#[derive(Model)]
#[toasty(enum, storage = "json")]  // Default
enum Status { ... }

struct User {
    status: Status,  // Uses JSON (from type)
    
    #[toasty(storage = "flattened")]  // Override to flattened
    primary_status: Status,
}
```

**Implications:**
- Type-level is simpler but less flexible
- Field-level allows different storage for same type in different contexts
- Both adds complexity but maximum flexibility
- Affects codegen (where to read storage strategy from)

**Resolution Strategy:**
- **Type-level default**: `#[toasty(enum, storage = "flattened")]` on enum definition
- **Field-level override**: `#[toasty(storage = "json")]` on specific fields
- If neither specified: use global default (flattened for enums, JSON for deeply nested)
- Codegen reads type-level first, then checks for field-level override

## Design Principles

### 1. Leverage Existing Mapping Layer
Toasty already has a sophisticated mapping between application-level types (`schema::app`) and database-level storage (`schema::db`). We'll extend this rather than create a parallel system.

### 2. Support Multiple Storage Strategies
Different applications have different needs. We'll support multiple serialization strategies with sane defaults, similar to serde's approach but adapted for database storage.

### 3. Plan for Schema Evolution
Database data persists across deployments. The design must handle:
- Adding new enum variants
- Adding/removing fields from struct variants
- Renaming variants (with migration support)
- Default values for missing data

### 4. Start Simple, Expand Later
Initial implementation focuses on the most common use cases:
- Simple enums (unit variants only)
- Enums with data (struct variants)
- Embedded structs (no relations)

Later iterations can add:
- Tuple variants
- Nested enums/structs
- Relations within embedded types (complex)

## Core Concepts

### Unified `#[derive(Model)]` Approach

Rather than introducing a separate `#[derive(Embedded)]`, we propose using `#[derive(Model)]` with attributes to indicate the model is embedded:

```rust
// Regular model - has its own table
#[derive(Model)]
struct User {
    #[key]
    id: Id<Self>,
    name: String,
    address: Address,  // Embedded field
    status: Status,    // Enum field
}

// Embedded model - no table, stored inline
#[derive(Model)]
#[toasty(embedded)]
struct Address {
    // No id field required for embedded
    street: String,
    city: String,
    zip: String,
}

// Enum type
#[derive(Model)]
#[toasty(enum)]
enum Status {
    Pending,
    Active { since: DateTime },
    Suspended { reason: String },
}
```

**Rationale:**
- Reuses existing codegen infrastructure
- Clear semantic distinction via `#[toasty(embedded)]` and `#[toasty(enum)]`
- Embedded types still generate query helpers (for filtering on nested fields)
- Simpler mental model (everything is a Model, some are embedded)
- Easier to promote embedded → table later if needed

### Enum Support

Enums are a special case of embedded types - they're composite types that don't get their own tables:

```rust
#[derive(Model)]
struct User {
    #[key]
    id: Id<Self>,
    name: String,
    contact: ContactMethod,  // Enum field
}

#[derive(Model)]
#[toasty(enum)]
enum ContactMethod {
    Email(String),
    Phone { country_code: String, number: String },
    Mail { address: Address },  // Can nest embedded structs
}
```

## Database Storage Strategies

### Strategy 1: Flattened Columns with Discriminator (DEFAULT)

**Storage:**
- One discriminator column stores the variant name/ordinal
- Separate nullable columns for each field in each variant
- Column names: `{field}_{variant}_{field_name}`

**Pros:**
- **Queryable and indexable** - can filter by variant, index variant fields
- Leverages native DB enum types when available (PostgreSQL, MySQL)
- Type-safe at database level
- Efficient storage (no serialization overhead)
- Natural for typical enum use cases

**Cons:**
- Many nullable columns if variants have many fields
- Schema migrations required when adding/removing variant fields
- Column name length can be long for nested structures
- Not ideal for deeply nested or highly dynamic structures

**Example:**
```rust
#[derive(Model)]
struct User {
    #[key]
    id: Id<Self>,
    critter: Creature,  // Flattened by default
}

#[derive(Model)]
#[toasty(enum)]
enum Creature {
    Human { race: String },
    Lizard { race: String, habitat: String },
}
```

Database schema:
```sql
-- PostgreSQL with native enum
CREATE TYPE creature AS ENUM ('Human', 'Lizard');

CREATE TABLE user (
    id INTEGER PRIMARY KEY,
    critter creature NOT NULL,           -- Discriminator (native enum)
    critter_human_race TEXT,             -- NULL if not Human
    critter_lizard_race TEXT,            -- NULL if not Lizard
    critter_lizard_habitat TEXT          -- NULL if not Lizard
);

-- Can create indexes
CREATE INDEX idx_user_creature ON user(critter);
CREATE INDEX idx_human_race ON user(critter_human_race) WHERE critter = 'Human';
```

**Variant-specific columns are nullable:**
- When `critter = 'Human'`, only `critter_human_race` has a value
- When `critter = 'Lizard'`, only `critter_lizard_*` columns have values
- Database enforces that discriminator and variant columns stay in sync

**Column naming details:**
- Discriminator: `{field}` → `critter`
- Variant fields: `{field}_{variant_snake_case}_{field_name}` → `critter_human_race`
- Customizable via attributes (see Challenge #11)

### Strategy 2: JSON/JSONB Column (OPT-IN)

**Storage:**
- Single column stores entire value as JSON
- Works for all databases (SQLite JSON, PostgreSQL JSONB, MySQL JSON)

**Pros:**
- Simple schema (one column)
- Easy schema evolution (add variants, fields without migration)
- Preserves full structure
- Good for deeply nested or dynamic data
- Efficient for NoSQL databases (DynamoDB maps directly)

**Cons:**
- **Can't efficiently query/index nested fields**
- Full value must be deserialized
- No type safety at database level
- Harder to debug (opaque storage)

**Example:**
```rust
#[derive(Model)]
struct User {
    #[key]
    id: Id<Self>,
    
    #[toasty(storage = "json")]  // Explicit opt-in
    metadata: Metadata,
}

#[derive(Model)]
#[toasty(enum)]
enum Metadata {
    Basic { name: String },
    Extended { name: String, tags: Vec<String>, extra: HashMap<String, String> },
}
```

Database schema:
```sql
CREATE TABLE user (
    id INTEGER PRIMARY KEY,
    metadata TEXT  -- or JSONB in PostgreSQL
);

-- Example data
-- '{"Basic": {"name": "Alice"}}'
-- '{"Extended": {"name": "Bob", "tags": ["admin"], "extra": {"role": "owner"}}}'
```

**JSON Format:**
Uses serde-compatible externally tagged format for enums:
```json
{"Email": "user@example.com"}
{"Phone": {"country_code": "1", "number": "555-1234"}}
{"Pending": null}
```

For embedded structs, standard JSON object:
```json
{"street": "123 Main", "city": "Seattle", "zip": "98101"}
```

**When to use JSON storage:**
- Deeply nested structures (more than 2-3 levels)
- Highly dynamic data (many variants, many fields)
- Schema evolution is critical (frequent changes)
- Queryability is not needed
- DynamoDB as primary database (native map support)

### Strategy 3: Separate Table (Advanced)

**Storage:**
- One-to-one relation to a separate table
- Each variant becomes a row with discriminator

**Pros:**
- Clean schema
- Supports complex nested structures
- Can add indexes per variant

**Cons:**
- Requires JOIN for every access
- More complex implementation
- Overkill for simple enums

**Future consideration** - not in initial implementation.

## Type System Extensions

### `stmt::Type` Extensions

```rust
pub enum Type {
    // ... existing types ...
    
    /// An embedded struct stored inline
    Embedded(EmbeddedId),
    
    /// An enum with multiple variants (already exists but needs enhancement)
    Enum(TypeEnum),
}

/// Metadata about an embedded type
pub struct EmbeddedType {
    pub id: EmbeddedId,
    pub name: String,
    pub fields: Vec<EmbeddedField>,
    pub storage: EmbeddedStorage,
}

pub struct EmbeddedField {
    pub name: String,
    pub ty: Type,
    pub nullable: bool,
}

pub enum EmbeddedStorage {
    Json,
    Flattened,
}
```

### `schema::app` Extensions

```rust
pub enum ModelKind {
    Table,      // Regular model with table
    Embedded,   // Embedded in parent (struct)
    Enum,       // Enum type
}

pub struct Model {
    // ... existing fields ...
    pub kind: ModelKind,
    
    // For enums only
    pub variants: Option<Vec<EnumVariant>>,
}

pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<Field>,  // Empty for unit variants
}

pub enum FieldTy {
    Primitive(FieldPrimitive),
    BelongsTo(BelongsTo),
    HasMany(HasMany),
    HasOne(HasOne),
    Embedded(FieldEmbedded),  // NEW
}

pub struct FieldEmbedded {
    /// The embedded type (struct or enum)
    pub embedded_id: ModelId,  // References the embedded Model
    
    /// How it's stored in the database
    pub storage: EmbeddedStorage,
    
    /// The type this field evaluates to
    pub expr_ty: stmt::Type,
}

pub enum EmbeddedStorage {
    Json,
    Flattened,
}
```

### `schema::db` Extensions

For JSON storage:
```rust
pub enum Type {
    // ... existing types ...
    
    /// JSON/JSONB column (TEXT for SQLite/MySQL, JSONB for PostgreSQL)
    Json,
}
```

For flattened storage, no new types needed - just multiple columns.

## Mapping Layer Extensions

The mapping layer needs to handle:
1. Serializing Rust enum/struct → database column(s)
2. Deserializing database column(s) → Rust enum/struct

### JSON Storage Mapping

**Model to Table:**
```rust
// User { contact: ContactMethod::Phone { country_code: "1", number: "555-1234" } }
// becomes
// user.contact = '{"Phone":{"country_code":"1","number":"555-1234"}}'

model_to_table: ExprRecord([
    Expr::Reference(id_field),
    Expr::Cast(
        Expr::Reference(contact_field),
        Type::String  // JSON serialization cast
    )
])
```

**Table to Model:**
```rust
table_to_model: ExprRecord([
    Expr::Column(id_column),
    Expr::Cast(
        Expr::Column(contact_column),
        Type::Enum(ContactMethodId)  // JSON deserialization cast
    )
])
```

The `Value::cast()` method handles JSON serialization/deserialization:
```rust
impl Type {
    pub fn cast(&self, value: Value) -> Result<Value> {
        // ... existing casts ...
        
        // Enum <-> String (JSON)
        (Value::Enum(variant, fields), Type::String) => {
            Value::String(serde_json::to_string(&value)?)
        }
        (Value::String(json), Type::Enum(enum_id)) => {
            Value::Enum(serde_json::from_str(&json)?)
        }
        
        // Embedded <-> String (JSON)
        (Value::Record(fields), Type::String) if is_embedded => {
            Value::String(serde_json::to_string(&fields)?)
        }
        (Value::String(json), Type::Embedded(embedded_id)) => {
            Value::Record(serde_json::from_str(&json)?)
        }
    }
}
```

### Flattened Storage Mapping

More complex - need to pack/unpack multiple columns. Requires new expression types:

**New Expression Types:**
```rust
pub enum Expr {
    // ... existing variants ...
    
    /// Extract the discriminator (variant name) from an enum
    EnumDiscriminator(Box<Expr>),
    
    /// Extract a specific field from an enum variant
    /// Returns NULL if the variant doesn't have this field
    EnumField {
        expr: Box<Expr>,
        field: String,
    },
    
    /// Reconstruct an enum from discriminator + field columns
    ReconstructEnum {
        enum_id: ModelId,
        discriminator: Box<Expr>,
        fields: Vec<(String, Expr)>,
    },
}
```

**Model to Table:**
```rust
// Status::Active { since: DateTime }
// becomes
// status_variant = 'Active', status_since = 123456789, status_reason = NULL, status_until = NULL

model_to_table: ExprRecord([
    Expr::Reference(id_field),
    Expr::EnumDiscriminator(Box::new(Expr::Reference(status_field))),
    Expr::EnumField { 
        expr: Box::new(Expr::Reference(status_field)), 
        field: "since".to_string() 
    },
    Expr::EnumField { 
        expr: Box::new(Expr::Reference(status_field)), 
        field: "reason".to_string() 
    },
    Expr::EnumField { 
        expr: Box::new(Expr::Reference(status_field)), 
        field: "until".to_string() 
    },
])
```

**Table to Model:**
```rust
table_to_model: ExprRecord([
    Expr::Column(id_column),
    Expr::ReconstructEnum {
        enum_id: StatusId,
        discriminator: Box::new(Expr::Column(status_variant_column)),
        fields: vec![
            ("since".to_string(), Expr::Column(status_since_column)),
            ("reason".to_string(), Expr::Column(status_reason_column)),
            ("until".to_string(), Expr::Column(status_until_column)),
        ]
    }
])
```

## Codegen Strategy

### Embedded Struct Codegen

```rust
#[derive(Model)]
#[toasty(embedded)]
struct Address {
    street: String,
    city: String,
    zip: String,
}
```

Generates:
1. **No `Model` trait impl** (not a top-level model)
2. **Schema registration** as an embedded type:
   ```rust
   impl Address {
       pub(crate) fn register_schema(builder: &mut SchemaBuilder) {
           builder.add_embedded("Address")
               .field("street", Type::String)
               .field("city", Type::String)
               .field("zip", Type::String);
       }
   }
   ```
3. **Conversion traits**: 
   ```rust
   impl From<Address> for toasty::stmt::Value { ... }
   impl TryFrom<toasty::stmt::Value> for Address { ... }
   ```
4. **Field accessors** for query building (when used with flattened storage):
   ```rust
   impl AddressField {
       pub fn street(&self) -> StringField { ... }
       pub fn city(&self) -> StringField { ... }
       pub fn zip(&self) -> StringField { ... }
   }
   ```

### Enum Codegen

```rust
#[derive(Model)]
#[toasty(enum)]
enum ContactMethod {
    Email(String),
    Phone { country_code: String, number: String },
}
```

Generates:
1. **Schema registration** as an enum type:
   ```rust
   impl ContactMethod {
       pub(crate) fn register_schema(builder: &mut SchemaBuilder) {
           builder.add_enum("ContactMethod")
               .variant("Email")
                   .field(Type::String)
               .variant("Phone")
                   .field("country_code", Type::String)
                   .field("number", Type::String);
       }
   }
   ```
2. **Conversion traits**: JSON serialization/deserialization
   ```rust
   impl From<ContactMethod> for toasty::stmt::Value { ... }
   impl TryFrom<toasty::stmt::Value> for ContactMethod { ... }
   ```
3. **Variant constructors** (these are just the standard Rust enum constructors)
4. **Query helpers** for filtering:
   ```rust
   impl ContactMethodField {
       pub fn is_email(&self) -> BoolFilter { ... }
       pub fn is_phone(&self) -> BoolFilter { ... }
       
       // For flattened storage
       pub fn as_phone(&self) -> PhoneVariantField { ... }
   }
   ```

## Query Interface

### Filtering on Embedded Fields

```rust
// Flattened storage allows direct filtering
User::all()
    .filter(User::FIELDS.address().city().eq("Seattle"))
    .all(&db)
    .await?;

// JSON storage requires JSON path syntax (database-specific, future)
User::all()
    .filter(User::FIELDS.metadata().json_path("$.plan").eq("Pro"))
    .all(&db)
    .await?;
```

### Filtering on Enum Variants

```rust
// Check variant (works with both JSON and flattened storage)
User::all()
    .filter(User::FIELDS.contact().is_email())
    .all(&db)
    .await?;

// Filter on variant field (flattened storage only)
User::all()
    .filter(User::FIELDS.contact().as_phone().country_code().eq("1"))
    .all(&db)
    .await?;

// For JSON storage, variant field filtering would require database-specific JSON functions
// This would be a future enhancement
```

## Schema Evolution

### JSON Storage Evolution

**Backward compatible:**
- Adding new variants ✓
- Adding new fields to variants (with `Option<T>` or defaults) ✓
- Renaming variants (requires data migration or rename attribute)

**Handling:**
```rust
#[derive(Model)]
#[toasty(enum)]
enum Status {
    Pending,
    Active { since: DateTime },
    
    // Old name in DB was "Cancelled"
    #[toasty(rename = "Cancelled")]
    Canceled { reason: String },
    
    Suspended { 
        reason: String,
        // Not present in old data, uses Default::default()
        #[toasty(default)]
        auto_resume: bool,
    },
}
```

**Deserialization strategy:**
- Unknown variants → error (strict by default)
- Missing fields with `#[toasty(default)]` → use Default value
- Missing fields without default → error
- Extra fields → ignored (forward compatibility)

### Flattened Storage Evolution

**Requires migrations:**
- Adding variants → add discriminator value (no migration), add columns for new fields (migration)
- Adding fields → add nullable columns (migration)
- Removing variants/fields → column remains (can be dropped manually)

**Migration strategy:**
- Toasty generates migration SQL for column additions
- Discriminator can accept new values without schema migration
- User responsible for data migration when renaming

## Implementation Phases

### Phase 1: Flattened Storage for Enums (MVP)
**Goal:** Support enums with flattened storage (discriminator + field columns)

**Tasks:**
1. **toasty-core**: Add `ModelKind::Enum` and `Model::variants` field
2. **toasty-core**: Add `FieldTy::Embedded` variant for enum fields
3. **toasty-core**: Add `db::Type::Enum { name, variants }` for native DB enums
4. **toasty-codegen**: Parse `#[toasty(enum)]` attribute, detect enum types
5. **toasty-codegen**: Build `Model` with `kind: ModelKind::Enum` and populate `variants`
6. **toasty-core**: Extend `Type::Enum` with proper metadata
7. **toasty-core/builder**: Generate columns for flattened storage:
   - Discriminator column: `{field}` with `db::Type::Enum` (or TEXT for SQLite)
   - Variant field columns: `{field}_{variant}_{field_name}` (all nullable)
8. **toasty-core**: Add new expression types:
   - `Expr::EnumDiscriminator` - extract variant name
   - `Expr::EnumField` - extract field from variant (returns NULL if wrong variant)
   - `Expr::ReconstructEnum` - rebuild enum from discriminator + field columns
9. **toasty-core/mapping**: Build mapping expressions for flattened enums
10. **toasty-sql**: 
    - Serialize `db::Type::Enum` → `CREATE TYPE` (PostgreSQL) or column `ENUM(...)` (MySQL) or `TEXT` (SQLite)
    - Serialize new expression types to SQL
11. **toasty/engine**: Handle new expression types in simplify/lower/plan/execute
12. **toasty-driver-***: Handle enum value conversions (string <-> enum variant)
13. **toasty-codegen**: Generate conversion traits and query helpers
14. **toasty-codegen**: Generate field accessor methods for filtering on variant fields
15. **tests**: Add tests across all drivers

**Deliverable:** Users can use enums in models with flattened storage, can query/index by variant

**Files to modify:**
- `crates/toasty-core/src/schema/db/ty.rs` - Add `Type::Enum`
- `crates/toasty-core/src/schema/app/model.rs` - Add `ModelKind::Enum`, `variants`
- `crates/toasty-core/src/schema/app/field.rs` - Add `FieldTy::Embedded`
- `crates/toasty-core/src/stmt/ty.rs` - Enhance `Type::Enum`
- `crates/toasty-core/src/stmt/expr.rs` - Add new expression types
- `crates/toasty-core/src/stmt/visit*.rs` - Update visitors
- `crates/toasty-core/src/stmt/value.rs` - Add enum conversions
- `crates/toasty-core/src/schema/builder/table.rs` - Generate flattened columns
- `crates/toasty-core/src/schema/mapping/*.rs` - Build flattened mapping expressions
- `crates/toasty-codegen/src/schema/mod.rs` - Parse enum attributes
- `crates/toasty-codegen/src/expand/mod.rs` - Generate enum code
- `crates/toasty-sql/src/serializer/ty.rs` - Serialize enum type
- `crates/toasty-sql/src/serializer/expr.rs` - Serialize new expressions
- `crates/toasty/src/engine/simplify/*.rs` - Handle new expressions
- `crates/toasty/src/engine/lower/*.rs` - Lower new expressions
- `crates/toasty/src/engine/plan/*.rs` - Plan new expressions
- `crates/toasty-driver-*/src/value.rs` - Driver-specific conversions
- `tests/tests/enums.rs` - New test file

**Estimated complexity:** High (affects entire query pipeline)

### Phase 2: JSON Storage (Opt-in)
**Goal:** Support JSON storage as an alternative for deeply nested or dynamic data

**Tasks:**
1. **toasty-core**: Add `db::Type::Json` variant
2. **toasty-core**: Add `EmbeddedStorage::Json` to track storage strategy
3. **toasty-codegen**: Parse `#[toasty(storage = "json")]` attribute on fields/types
4. **toasty-core**: Add JSON serialization/deserialization to `Value::cast()`
   - Use `serde_json` for serialization
   - Externally tagged format for enums: `{"VariantName": {...}}`
   - Standard JSON objects for embedded structs
5. **toasty-core/builder**: For JSON storage, generate single column with `db::Type::Json`
6. **toasty-core/mapping**: Build simple cast expressions for JSON fields
7. **toasty-sql**: Map `db::Type::Json` → `TEXT` (SQLite/MySQL) or `JSONB` (PostgreSQL)
8. **toasty-driver-***: Handle JSON value conversions in each driver
9. **tests**: Add tests for JSON storage across all drivers
10. **tests**: Test migration path from flattened to JSON (manual)

**Deliverable:** Users can opt into JSON storage for specific fields

**Files to modify:**
- `crates/toasty-core/src/schema/db/ty.rs` - Add `Type::Json`
- `crates/toasty-core/src/schema/app/field.rs` - Add `EmbeddedStorage` enum
- `crates/toasty-core/src/stmt/value.rs` - Add JSON conversions
- `crates/toasty-core/src/schema/builder/table.rs` - Generate JSON columns
- `crates/toasty-core/src/schema/mapping/*.rs` - JSON mapping expressions
- `crates/toasty-codegen/src/schema/mod.rs` - Parse storage attributes
- `crates/toasty-sql/src/serializer/ty.rs` - Serialize JSON type
- `crates/toasty-driver-*/src/value.rs` - JSON conversions
- `tests/tests/json_storage.rs` - New test file

**Estimated complexity:** Medium (simpler than flattened, but adds new code path)

### Phase 3: Embedded Structs (Flattened by default)
**Goal:** Support embedded structs as composite types within models

**Tasks:**
1. **toasty-core**: Add `ModelKind::Embedded`
2. **toasty-codegen**: Parse `#[toasty(embedded)]` attribute
3. **toasty-codegen**: Build `Model` with `kind: ModelKind::Embedded`
4. **toasty-core**: Add `Type::Embedded` with metadata
5. **toasty-core/builder**: Generate flattened columns for embedded struct fields
   - Column names: `{field}_{struct_field}`
6. **toasty-core/mapping**: Build mapping expressions for embedded structs
   - Similar to enum flattening but no discriminator
7. **toasty-codegen**: Generate conversion traits for embedded structs
8. **toasty-codegen**: Generate field accessors for filtering on embedded fields
9. **tests**: Tests for nested structures (Address in User, etc.)
10. **tests**: Test enum variants containing embedded structs
11. **tests**: Test embedded structs with JSON storage opt-in

**Deliverable:** Users can embed structs in models, flattened by default, JSON opt-in

**Files to modify:**
- `crates/toasty-core/src/schema/app/model.rs` - Add `ModelKind::Embedded`
- `crates/toasty-core/src/stmt/ty.rs` - Add `Type::Embedded`
- `crates/toasty-core/src/stmt/value.rs` - Add embedded struct conversions
- `crates/toasty-core/src/schema/builder/table.rs` - Generate embedded columns
- `crates/toasty-core/src/schema/mapping/*.rs` - Handle embedded fields
- `crates/toasty-codegen/src/schema/mod.rs` - Parse embedded attributes
- `crates/toasty-codegen/src/expand/mod.rs` - Generate embedded struct code
- `tests/tests/embedded.rs` - New test file

**Estimated complexity:** Medium

### Phase 4: Schema Evolution Support (Future)
**Goal:** Handle backward compatibility

**Tasks:**
1. **toasty-codegen**: Support `#[toasty(rename = "...")]` for variants/fields
2. **toasty-codegen**: Support `#[toasty(default)]` for new fields
3. **toasty-core**: Implement versioned deserialization
   - Track old variant/field names
   - Apply defaults for missing fields
4. **toasty**: Migration tooling for flattened storage
   - Generate SQL for adding columns
   - Warn about removed columns
5. **tests**: Tests for schema evolution scenarios
   - Add variant, query old data
   - Add field with default, query old data
   - Rename variant with attribute

**Deliverable:** Safe schema evolution

**Files to modify:**
- `crates/toasty-codegen/src/schema/mod.rs` - Parse evolution attributes
- `crates/toasty-core/src/stmt/value.rs` - Versioned deserialization
- `crates/toasty/migrations/*.rs` - Migration generation (if exists)
- `tests/tests/schema_evolution.rs` - New test file

**Estimated complexity:** Medium-High

### Phase 5: Advanced Features (Future)
- Tuple variants support
- Separate table storage strategy
- Relations within embedded types (complex)
- JSON path filtering across all databases (database-specific)
- Custom serialization formats (protobuf, msgpack, etc.)
- Nested enums (enum containing enum)

## Open Questions

### 1. Default JSON Format

**Question:** Should we use serde's externally-tagged format or a simpler format?

**Option A: Externally tagged (serde default)**
```json
{"Email": "user@example.com"}
{"Phone": {"country_code": "1", "number": "555-1234"}}
{"Pending": null}
```

**Option B: Internally tagged**
```json
{"type": "Email", "value": "user@example.com"}
{"type": "Phone", "country_code": "1", "number": "555-1234"}
{"type": "Pending"}
```

**Option C: Custom compact format**
```json
["Email", "user@example.com"]
["Phone", {"country_code": "1", "number": "555-1234"}]
["Pending"]
```

**Recommendation:** Option A (externally tagged)
- Standard serde format, users are familiar with it
- Easy to deserialize with existing tools
- Clear structure
- Forward compatible if we add serde integration later

### 2. Embedded Type Registration

**Question:** Should embedded types be explicitly registered or auto-registered?

**Option A: Auto-registration**
```rust
db.register::<User>()  // Auto-registers Address and Status
  .connect(...)
  .await?;
```

**Option B: Explicit registration**
```rust
db.register::<User>()
  .register_embedded::<Address>()
  .register_enum::<Status>()
  .connect(...)
  .await?;
```

**Recommendation:** Option A (auto-registration)
- Less boilerplate
- Can't forget to register embedded types
- Embedded types are dependencies of the model
- Can still allow explicit registration for edge cases

### 3. Relation Support in Embedded Types

**Question:** Should we allow relations in embedded types?

**Initial recommendation:** No relations in embedded types
- `BelongsTo`, `HasMany`, `HasOne` not allowed in embedded models
- Compile-time error if detected
- Keeps implementation simple
- Can be added later if there's demand

**Rationale:**
- Embedded types should be value objects, not entities
- Relations imply separate identity, which conflicts with embedding
- Complex implementation (how to handle JOINs for nested relations?)

### 4. Serde Integration

**Question:** Should we use serde for serialization or implement custom?

**Option A: Custom JSON serialization**
- Full control over format
- No serde dependency for basic usage
- Can optimize for database use cases

**Option B: Use serde**
- Mature, well-tested
- Users can customize with serde attributes
- Handles edge cases automatically

**Option C: Both (serde optional)**
- Default: custom lightweight JSON
- Opt-in: use serde with feature flag
- Best of both worlds

**Recommendation:** Option A initially, Option C long-term
- Start with custom JSON (keep Phase 1 simple)
- Add serde as optional feature in Phase 5
- Users who need advanced serialization can opt in

### 5. Unit-Only Enums First?

**Question:** Should Phase 1 start even simpler with unit-only enums?

**Option A: Unit-only in Phase 1**
```rust
enum Status {
    Pending,
    Active,
    Completed,
}
// Stored as: "Pending", "Active", "Completed"
```

**Option B: Full enums in Phase 1**
```rust
enum Status {
    Pending,
    Active { since: DateTime },
    Completed { at: DateTime },
}
// Stored as JSON
```

**Recommendation:** Option B (full enums)
- Unit-only enums are trivial (just string storage)
- Full enum support is where the real value is
- JSON storage isn't much harder than string storage
- Avoids users hitting limitations immediately

## Comparison with Other ORMs

### Diesel
- Supports enums via `#[derive(DbEnum)]`
- Maps to native PostgreSQL ENUM type
- Limited to unit variants only
- No embedded struct support

### SeaORM
- Supports enums via `#[derive(EnumIter, DeriveActiveEnum)]`
- Maps to strings or integers
- Limited to unit variants only
- No embedded struct support

### Toasty Advantage
- Support for data-carrying enum variants
- Support for embedded structs
- Multiple storage strategies
- Works across SQL and NoSQL databases

## Comparison with Serde

Serde's enum representations and how they map to our design:

| Serde Strategy | Database Equivalent | Queryable? | Our Support |
|----------------|---------------------|------------|-------------|
| Externally tagged | JSON: `{"Email": "..."}` | No | Phase 1 (default) |
| Internally tagged | Flattened with discriminator | Yes | Phase 3 (opt-in) |
| Adjacently tagged | JSON: `{"t": "Email", "c": "..."}` | No | Future (opt-in) |
| Untagged | Not applicable | No | Not planned |

**Key Difference:** Serde optimizes for serialization format compatibility across different data formats. Toasty optimizes for:
1. **Schema evolution** - backward compatibility with existing database data
2. **Query performance** - indexing, filtering on embedded fields
3. **Database best practices** - native types when possible, efficient storage

## Success Criteria

### Phase 1 Success
- [ ] Can define enum types as model fields
- [ ] Enums with unit variants work
- [ ] Enums with struct variants work
- [ ] Enums serialize to JSON in database
- [ ] Can insert models with enum fields
- [ ] Can query models with enum fields
- [ ] Can filter by enum variant (basic `.is_variant()` checks)
- [ ] Works across SQLite, PostgreSQL, MySQL, DynamoDB
- [ ] Basic error handling for invalid JSON data
- [ ] Documentation with examples

### Phase 2 Success
- [ ] Can define embedded struct types
- [ ] Embedded structs serialize to JSON
- [ ] Can insert models with embedded fields
- [ ] Can query models with embedded fields
- [ ] Can nest embedded types (Address contains Location)
- [ ] Enum variants can contain embedded structs
- [ ] All drivers support embedded structs
- [ ] Documentation with examples

### Phase 3 Success
- [ ] Can opt into flattened storage with attribute
- [ ] Flattened enums generate correct schema (discriminator + field columns)
- [ ] Can filter on enum variant with flattened storage
- [ ] Can filter on embedded struct fields with flattened storage
- [ ] Can create indexes on embedded fields
- [ ] Schema migrations generate SQL for column additions
- [ ] All drivers support flattened storage
- [ ] Documentation with migration guide

### Overall Success
- [ ] Feature parity with or exceeds other Rust ORMs
- [ ] Clear migration path from JSON to flattened storage
- [ ] Real-world usage validates design decisions
- [ ] Community feedback incorporated
- [ ] Comprehensive documentation
- [ ] Example applications demonstrating usage

## References

- [Serde Enum Representations](https://serde.rs/enum-representations.html)
- [Serde Container Attributes](https://serde.rs/container-attrs.html)
- [Issue #280 - Toasty Feedback](https://github.com/tokio-rs/toasty/issues/280)
- [PostgreSQL JSONB Documentation](https://www.postgresql.org/docs/current/datatype-json.html)
- [SQLite JSON1 Extension](https://www.sqlite.org/json1.html)
- [MySQL JSON Functions](https://dev.mysql.com/doc/refman/8.0/en/json.html)
