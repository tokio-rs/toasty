# Enum and Embedded Struct Support

Addresses [Issue #280](https://github.com/tokio-rs/toasty/issues/280).

## Scope

Add support for:
1. Enum types as model fields (unit, tuple, struct variants)
2. Embedded structs (no separate table, stored inline)

Both use `#[derive(toasty::Embed)]`.

## Storage Strategy

**Flattened storage**:
- **Enums**: Discriminator column + nullable columns per variant field
  - INTEGER discriminator with required `#[column(variant = N)]` on each variant
  - Works uniformly across all databases (PostgreSQL, MySQL, SQLite, DynamoDB)
- **Embedded structs**: No discriminator, just flattened fields

**Unit-only enums**: No columns - stored as the INTEGER value itself.

**Post-MVP**: Native ENUM types for PostgreSQL/MySQL discriminators (optimization).

## Column Naming

Pattern: `{field}_{variant}_{name}`

```rust
#[derive(Model)]
struct User {
    #[key]
    #[auto]
    id: u64,
    critter: Creature,  // field name
}

#[derive(toasty::Embed)]
enum Creature {
    #[column(variant = 1)]
    Human { profession: String },      // variant, field
    #[column(variant = 2)]
    Lizard { habitat: String },
}

// Columns:
// - critter (discriminator)
// - critter_human_profession
// - critter_lizard_habitat
```

### Customization

**Rename field** (at enum definition):
```rust
#[derive(toasty::Embed)]
enum Creature {
    #[column(variant = 1)]
    Human { profession: String },
    #[column(variant = 2)]
    Lizard {
        #[column("lizard_env")]  // Must include variant scope
        habitat: String,
    },
}
// → critter_lizard_env (field prefix "critter" is prepended)
```

Custom column names for enum variant fields must include the variant scope. The pattern becomes `{field}_{custom_name}` where `custom_name` should include the variant portion.

**Rename field prefix** (per use):
```rust
#[derive(Model)]
struct User {
    #[key]
    #[auto]
    id: u64,
    #[column("creature_type")]
    critter: Creature,
}
// → creature_type (discriminator)
// → creature_type_human_profession (field prefix replaced for all columns)
// → creature_type_lizard_habitat
```

The `#[column("name")]` attribute on the parent struct's field replaces the field prefix for all generated columns.

**Customize discriminator type** (on enum definition):
```rust
#[derive(toasty::Embed)]
#[column(type = "bigint")]
enum Creature { ... }
```

The `#[column(type = "...")]` attribute on the enum type customizes the database type for the discriminator column (e.g., "bigint", "smallint", "tinyint").

### Tuple Variants

Numeric field naming: `{field}_{variant}_{index}`

```rust
#[derive(toasty::Embed)]
enum Contact {
    #[column(variant = 1)]
    Phone(String, String),
}
// Columns: contact, contact_phone_0, contact_phone_1
```

Customize with `#[column("...")]`:
```rust
#[derive(toasty::Embed)]
enum Contact {
    #[column(variant = 1)]
    Phone(
        #[column("phone_country")]
        String,
        #[column("phone_number")]
        String,
    ),
}
// Columns: contact, contact_phone_country, contact_phone_number
```

### Nested Types

Path flattened with underscores:

```rust
#[derive(toasty::Embed)]
enum ContactInfo {
    #[column(variant = 1)]
    Mail { address: Address },
}

#[derive(toasty::Embed)]
struct Address {
    street: String,
    city: String,
}

// → contact_mail_address_city
// → contact_mail_address_street
```

### Shared Columns Across Variants

Multiple variants can share the same column by specifying the same `#[column("name")]`:

```rust
#[derive(Model)]
struct Character {
    #[key]
    #[auto]
    id: u64,
    creature: Creature,
}

#[derive(toasty::Embed)]
enum Creature {
    #[column(variant = 1)]
    Human {
        #[column("name")]
        name: String,
        profession: String,
    },
    #[column(variant = 2)]
    Animal {
        #[column("name")]
        name: String,
        species: String,
    },
}

// Columns:
// - creature (discriminator)
// - creature_name (shared between Human and Animal)
// - creature_human_profession
// - creature_animal_species
```

**Requirements**:
- Fields sharing a column must have compatible types (validated at schema build time)
- The shared column name must be identical across variants
- Compatible types: same primitive type, or compatible type conversions
- Shared columns are still nullable at the database level (NULL when variant doesn't use that field)

## Discriminator Types

**MVP**: INTEGER discriminator for all databases
```rust
#[derive(toasty::Embed)]
enum Creature {
    #[column(variant = 1)]
    Human { profession: String },
    #[column(variant = 2)]
    Lizard { habitat: String },
}
```

All variants require `#[column(variant = N)]` with unique integer values. Compile error if missing.

**Customize discriminator type**:
```rust
#[derive(toasty::Embed)]
#[column(type = "bigint")]  // Or "smallint", "tinyint", etc.
enum Creature {
    #[column(variant = 1)]
    Human { profession: String },
    #[column(variant = 2)]
    Lizard { habitat: String },
}
```

The `#[column(type = "...")]` attribute on the enum customizes the database type for the discriminator column.

**Post-MVP**: Native ENUM types for PostgreSQL/MySQL
```sql
CREATE TYPE creature AS ENUM ('Human', 'Lizard');
```
Can customize with `#[column(variant = "name")]` on variants.

## NULL Handling

Inactive variant fields are NULL.

```sql
-- When critter = 'Human':
critter_human_profession = 'Knight'
critter_lizard_habitat = NULL
```

For `Option<T>` fields: Check discriminator first, then interpret NULL.

## Usage

```rust
#[derive(Model)]
struct User {
    #[key]
    #[auto]
    id: u64,
    address: Address,  // embedded struct
    status: Status,    // embedded enum
}

#[derive(toasty::Embed)]
struct Address {
    street: String,
    city: String,
}

#[derive(toasty::Embed)]
enum Status {
    #[column(variant = 1)]
    Pending,
    #[column(variant = 2)]
    Active { since: DateTime },
}
```

**Registration**: Automatic. `db.register::<User>()` transitively registers all nested embedded types.

**Relations**: Forbidden in embedded types (compile error).

## Examples

### Basic Enum

```rust
#[derive(Model)]
struct Task {
    #[key]
    #[auto]
    id: u64,
    status: Status,
}

#[derive(toasty::Embed)]
enum Status {
    #[column(variant = 1)]
    Pending,
    #[column(variant = 2)]
    Active,
    #[column(variant = 3)]
    Done,
}
```

Schema:
```sql
CREATE TABLE task (
    id INTEGER PRIMARY KEY,
    status INTEGER NOT NULL
);
-- 1=Pending, 2=Active, 3=Done (requires #[column(variant = N)])
```

### Data-Carrying Enum

```rust
#[derive(Model)]
struct User {
    #[key]
    #[auto]
    id: u64,
    contact: ContactMethod,
}

#[derive(toasty::Embed)]
enum ContactMethod {
    #[column(variant = 1)]
    Email { address: String },
    #[column(variant = 2)]
    Phone { country: String, number: String },
}
```

Schema:
```sql
CREATE TABLE user (
    id INTEGER PRIMARY KEY,
    contact INTEGER NOT NULL,
    contact_email_address TEXT,
    contact_phone_country TEXT,
    contact_phone_number TEXT
);
```

### Embedded Struct

```rust
#[derive(Model)]
struct User {
    #[key]
    #[auto]
    id: u64,
    address: Address,
}

#[derive(toasty::Embed)]
struct Address {
    street: String,
    city: String,
    zip: String,
}
```

Schema:
```sql
CREATE TABLE user (
    id INTEGER PRIMARY KEY,
    address_street TEXT NOT NULL,
    address_city TEXT NOT NULL,
    address_zip TEXT NOT NULL
);
```

### Nested Enum + Embedded

```rust
#[derive(toasty::Embed)]
enum ContactInfo {
    #[column(variant = 1)]
    Email { address: String },
    #[column(variant = 2)]
    Mail { address: Address },
}

#[derive(toasty::Embed)]
struct Address {
    street: String,
    city: String,
}
```

Schema:
```sql
-- contact: ContactInfo
contact INTEGER NOT NULL,
contact_email_address TEXT,
contact_mail_address_street TEXT,
contact_mail_address_city TEXT
```

## Querying

### Basic variant checks

```rust
#[derive(Model)]
struct Task {
    #[key]
    #[auto]
    id: u64,
    status: Status,
}

#[derive(toasty::Embed)]
enum Status {
    #[column(variant = 1)]
    Pending,
    #[column(variant = 2)]
    Active,
    #[column(variant = 3)]
    Done,
}

// Query by variant (shorthand)
Task::all().filter(Task::FIELDS.status().is_pending())
Task::all().filter(Task::FIELDS.status().is_active())

// Equivalent using .matches() without field conditions
Task::all().filter(
    Task::FIELDS.status().matches(Status::VARIANTS.pending())
)
```

### Field access on variant fields

```rust
#[derive(Model)]
struct User {
    #[key]
    #[auto]
    id: u64,
    contact: ContactMethod,
}

#[derive(toasty::Embed)]
enum ContactMethod {
    #[column(variant = 1)]
    Email { address: String },
    #[column(variant = 2)]
    Phone { country: String, number: String },
}

// Match specific variants and access their fields
User::all().filter(
    User::FIELDS.contact().matches(
        ContactMethod::VARIANTS.email().address().contains("@gmail")
    )
)

User::all().filter(
    User::FIELDS.contact().matches(
        ContactMethod::VARIANTS.phone().country().eq("US")
    )
)

// Shorthand for variant-only checks (no field conditions)
User::all().filter(User::FIELDS.contact().is_email())
User::all().filter(User::FIELDS.contact().is_phone())

// Equivalent using .matches()
User::all().filter(
    User::FIELDS.contact().matches(ContactMethod::VARIANTS.email())
)
```

## Implementation Plan

### High-Level Strategy

Implement in two phases:
1. **Phase 1: Embedded Structs** - Simpler, no discriminator logic
2. **Phase 2: Enums** - Builds on embedded struct patterns + discriminator

### Phase 1: Embedded Struct Support

**Step 1: Schema Type Extensions** (Current)
- Add `Embedded` variant to `FieldTy` enum
- Define `Embedded`, `EmbeddedField`, `FieldAttr` types
- Validation enforces: no relations in embedded types
- See "Schema Design" section below for details

**Step 2: Column Flattening**
- Extend schema builder to flatten embedded fields into columns
- Implement `{field}_{embedded_field}` naming pattern
- Handle `Option<EmbeddedStruct>` by making all columns nullable

**Step 3: Codegen - Parsing**
- Parse `#[derive(toasty::Embed)]` attribute
- Build `Embedded` field type in schema representation
- Validate embedded type invariants during parsing

**Step 4: Codegen - Expansion**
- Generate field accessors for embedded struct fields
- Generate struct constructors/deconstructors
- No query support yet

**Step 5: Engine - CRUD Support**
- Update insert to handle flattened fields
- Update query to reconstruct embedded structs from columns
- Handle NULL semantics for `Option<EmbeddedStruct>`

**Step 6: Testing**
- Add comprehensive tests for embedded structs
- Test nested embedded structs
- Test `Option<EmbeddedStruct>`
- Test custom column names via `#[column("...")]`

### Phase 2: Enum Support

**Step 7: Schema for Unit Enums**
- Add discriminator column representation
- Parse `#[column(variant = N)]` attributes
- Validate unique variant values

**Step 8: Unit Enum CRUD**
- Generate enum ↔ integer conversions
- Update engine for discriminator handling

**Step 9: Data-Carrying Enums**
- Combine discriminator + field flattening
- Implement `{field}_{variant}_{field}` naming
- Handle variant field nullability

**Step 10: Query Support**
- Generate `is_*()` and `matches()` methods
- Update engine to translate variant queries

**Step 11-12: Advanced Features**
- Tuple variants, shared columns, customization options

## Step 1 Detailed Design: Schema Type Extensions

### Core Schema Types (toasty-core)

Add new variant to `FieldTy` enum in `crates/toasty-core/src/schema/app/field.rs`:

```rust
pub enum FieldTy {
    Primitive(FieldPrimitive),
    Embedded(Embedded),        // NEW
    BelongsTo(BelongsTo),
    HasMany(HasMany),
    HasOne(HasOne),
}
```

Add helper methods to `FieldTy`:

```rust
impl FieldTy {
    pub fn is_embedded(&self) -> bool {
        matches!(self, FieldTy::Embedded(_))
    }
    
    pub fn as_embedded(&self) -> Option<&Embedded> {
        match self {
            FieldTy::Embedded(v) => Some(v),
            _ => None,
        }
    }
    
    pub fn expect_embedded(&self) -> &Embedded {
        self.as_embedded().expect("expected embedded")
    }
    
    pub fn expect_embedded_mut(&mut self) -> &mut Embedded {
        match self {
            FieldTy::Embedded(v) => v,
            _ => panic!("expected embedded"),
        }
    }
}
```

Define embedded type structures in `crates/toasty-core/src/schema/app/field/embedded.rs`:

```rust
use super::FieldName;
use crate::schema::Name;

#[derive(Debug, Clone)]
pub struct Embedded {
    /// Name of the embedded struct
    pub name: Name,
    
    /// Fields of the embedded struct
    pub fields: Vec<EmbeddedField>,
}

#[derive(Debug, Clone)]
pub struct EmbeddedField {
    /// Field name (includes app_name and optional storage_name from #[column])
    pub name: FieldName,
    
    /// The field's type (can be Primitive or Embedded, not relations)
    /// Relations are forbidden and checked during schema validation
    pub ty: FieldTy,
    
    /// Whether this field is nullable
    pub nullable: bool,
    
    /// Field attributes (auto, unique, index, constraints, etc.)
    /// Note: primary_key is tracked on the parent Field, not here
    pub attrs: FieldAttr,
}

#[derive(Debug, Clone)]
pub struct FieldAttr {
    /// Specifies if and how Toasty should automatically populate this field
    pub auto: Option<AutoStrategy>,
    
    /// True if the field should be unique
    pub unique: bool,
    
    /// True if the field should be indexed
    pub index: bool,
    
    /// Additional field constraints (e.g., length)
    pub constraints: Vec<Constraint>,
}
```

Update module exports in `crates/toasty-core/src/schema/app/field/mod.rs`:

```rust
mod primitive;
pub use primitive::FieldPrimitive;

mod embedded;  // NEW
pub use embedded::{Embedded, EmbeddedField, FieldAttr};  // NEW

// ... existing code
```

### Key Design Decisions

- **`EmbeddedField::name`** is `FieldName` (not `String`) to track `#[column("...")]` overrides
- **`EmbeddedField::ty`** is `FieldTy` (not `stmt::Type`) to support nested embedded structs
- **`EmbeddedField::attrs`** is `FieldAttr` to track `#[auto]`, `#[unique]`, `#[index]`, and constraints
- **`FieldAttr`** is defined in toasty-core (not just codegen) so it can be stored in the schema
- **Validation** enforces that embedded fields cannot contain relations (checked during schema building)
- **Reuse** existing types (`FieldName`, `FieldTy`) rather than creating parallel structures

Note: `#[key]` is not tracked in `FieldAttr` because embedded fields cannot be primary keys - only top-level model fields can be keys.

### Column Flattening Strategy (for future steps)

**Approach**: Flatten at schema build time
- Embedded fields expand to multiple `db::Column` entries
- Column names follow pattern: `{field_name}_{embedded_field_name}`
- Nested embedded structs flatten recursively: `{field}_{embedded}_{nested}`
- Each embedded sub-field gets its own entry in the mapping layer

### Validation Rules (for future steps)

Schema validation (`toasty-core/src/schema/verify`) must enforce:
- Embedded types cannot contain `BelongsTo`, `HasMany`, or `HasOne` fields
- Nested embedded structs are allowed (recursive flattening)
- All `#[column(variant = N)]` values are unique within an enum (Phase 2)
