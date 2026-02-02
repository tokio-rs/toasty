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
