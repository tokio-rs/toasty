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

### Embedded struct field constraints

Embedded struct fields can be accessed directly for filtering, ordering, and other query operations:

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

// Filter by embedded struct fields
User::all().filter(User::FIELDS.address().city().eq("Seattle"))
User::all().filter(User::FIELDS.address().zip().like("98%"))

// Multiple constraints on embedded struct
User::all().filter(
    User::FIELDS.address().city().eq("Seattle")
        .and(User::FIELDS.address().zip().like("98%"))
)

// Order by embedded struct fields
User::all().order_by(User::FIELDS.address().city().asc())

// Select embedded struct fields (projection)
User::all()
    .select(User::FIELDS.id())
    .select(User::FIELDS.address().city())
```

### Nested embedded structs

For nested embedded types, continue chaining field accessors:

```rust
#[derive(Model)]
struct Company {
    #[key]
    #[auto]
    id: u64,
    headquarters: Office,
}

#[derive(toasty::Embed)]
struct Office {
    name: String,
    location: Address,
}

#[derive(toasty::Embed)]
struct Address {
    street: String,
    city: String,
    zip: String,
}

// Access nested embedded struct fields
Company::all().filter(
    Company::FIELDS.headquarters().location().city().eq("Seattle")
)

Company::all().filter(
    Company::FIELDS.headquarters().name().eq("Main Office")
        .and(Company::FIELDS.headquarters().location().zip().like("98%"))
)
```

### Combining enum and embedded struct constraints

When an enum variant contains an embedded struct, use `.matches()` to specify the variant, then access the embedded struct's fields:

```rust
#[derive(Model)]
struct User {
    #[key]
    #[auto]
    id: u64,
    contact: ContactInfo,
}

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

// Filter by embedded struct fields within enum variant
User::all().filter(
    User::FIELDS.contact().matches(
        ContactInfo::VARIANTS.mail().address().city().eq("Seattle")
    )
)

// Multiple constraints on embedded struct within variant
User::all().filter(
    User::FIELDS.contact().matches(
        ContactInfo::VARIANTS.mail()
            .address().city().eq("Seattle")
            .address().street().contains("Main")
    )
)
```

### Constraints with shared columns

When enum variants share columns, constraints apply based on the variant being matched:

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

// Query the shared "name" field for a specific variant
Character::all().filter(
    Character::FIELDS.creature().matches(
        Creature::VARIANTS.human().name().eq("Alice")
    )
)

// Query across variants using the shared column
// (finds any creature with this name, regardless of variant)
Character::all().filter(
    Character::FIELDS.creature().name().eq("Bob")
)

// Variant-specific field
Character::all().filter(
    Character::FIELDS.creature().matches(
        Creature::VARIANTS.human().profession().eq("Knight")
    )
)
```

## Updating

Update builders provide two methods per field:
- `.field(value)` - Direct value assignment
- `.with_field(|f| ...)` - Closure-based update

The `.with_*` methods provide a uniform API across all field types and enable:
- **Embedded types**: Partial updates (only set specific nested fields)
- **Primitives**: Future type-specific operations (e.g., `NumericUpdate::increment()`)
- **Enums**: Update variant fields without changing the discriminator

### Whole replacement

Setting an embedded struct field on an update replaces all of its columns:

```rust
// Loaded model update — sets address_street, address_city, address_zip
user.update()
    .address(Address { street: "123 Main", city: "Seattle", zip: "98101" })
    .exec(&db).await?;

// Query-based update — same behavior, no model loaded
User::filter_by_id(id).update()
    .address(Address { street: "123 Main", city: "Seattle", zip: "98101" })
    .exec(&db).await?;
```

### Partial updates

Each field (primitive or embedded) generates a companion `{Type}Update<'a>` type that
provides a view into the update statement's assignments. These update types hold a
reference to the statement and a projection path, allowing them to directly mutate
the statement as fields are set. This enables efficient nested updates without intermediate
allocations.

```rust
#[derive(toasty::Embed)]
struct Address {
    street: String,
    city: String,
    zip: String,
}

// AddressUpdate<'a> is generated automatically by `#[derive(toasty::Embed)]`
// StringUpdate<'a> is generated for primitive String fields
```

**Embedded types:**

```rust
// Whole replacement — sets all address columns
user.update()
    .address(Address { street: "123 Main", city: "Seattle", zip: "98101" })
    .exec(&db).await?;

// Partial update — only address_city is SET
user.update()
    .with_address(|a| {
        a.set_city("Seattle");
    })
    .exec(&db).await?;

// Multiple sub-fields — only address_city and address_zip are SET
user.update()
    .with_address(|a| {
        a.set_city("Seattle");
        a.set_zip("98101");
    })
    .exec(&db).await?;

// Query-based partial update
User::filter_by_id(id).update()
    .with_address(|a| a.set_city("Seattle"))
    .exec(&db).await?;
```

**Primitive types:**

```rust
// Direct value
user.update()
    .name("Alice")
    .exec(&db).await?;

// Via closure (enables future type-specific operations)
user.update()
    .with_name(|n| {
        n.set("Alice");
    })
    .exec(&db).await?;
```

For now, primitive update builders only provide `.set()`. Future enhancements could add
type-specific operations like `NumericUpdate::increment()`, `StringUpdate::append()`, etc.

### Partial updates with nested embedded structs

Nested embedded structs also generate `{Type}Update<'a>` types. The `.with_*` methods
can be nested naturally:

```rust
#[derive(toasty::Embed)]
struct Office {
    name: String,
    location: Address,
}

// Update only headquarters_location_city
company.update()
    .with_headquarters(|h| {
        h.with_location(|a| {
            a.set_city("Seattle");
        });
    })
    .exec(&db).await?;

// Update headquarters_name and headquarters_location_zip
company.update()
    .with_headquarters(|h| {
        h.with_name(|n| n.set("West Coast HQ"));
        h.with_location(|a| {
            a.set_zip("98101");
        });
    })
    .exec(&db).await?;
```

### Enum updates

Enums use whole-variant replacement. Setting an enum field replaces the discriminator and
all variant columns:

```rust
// Replace the entire enum value — sets discriminator + variant fields,
// NULLs out fields from the previous variant
user.update()
    .contact(ContactMethod::Email { address: "new@example.com".into() })
    .exec(&db).await?;
```

For data-carrying variants, use `.with_contact()` to update fields within the current
variant without changing the discriminator:

```rust
#[derive(toasty::Embed)]
enum ContactMethod {
    #[column(variant = 1)]
    Email { address: String },
    #[column(variant = 2)]
    Phone { country: String, number: String },
}

// Update only the phone number, leave country and discriminator unchanged
user.update()
    .with_contact(|c| {
        c.phone(|p| {
            p.with_number(|n| n.set("555-1234"));
        });
    })
    .exec(&db).await?;

// Update email variant
User::filter_by_id(id).update()
    .with_contact(|c| {
        c.email(|e| {
            e.with_address(|a| a.set("new@example.com"));
        });
    })
    .exec(&db).await?;
```

`ContactMethodUpdate<'a>` has one method per variant (e.g., `.phone()`, `.email()`). Each
method accepts a closure that receives a builder scoped to that variant's fields. The
discriminator is not changed by partial updates.
