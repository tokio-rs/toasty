# Polymorphic Relations

An embedded enum can hold a `#[belongs_to]` relation in each variant. This
lets one record belong to one of several model types. Rust checks every
`match` against the enum's complete set of owner types.

```rust
#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: uuid::Uuid,
    name: String,
    #[has_many]
    objects: toasty::Deferred<Vec<Object>>,
}

#[derive(Debug, toasty::Model)]
struct Animal {
    #[key]
    #[auto]
    id: uuid::Uuid,
    name: String,
    #[has_many]
    objects: toasty::Deferred<Vec<Object>>,
}

#[derive(Debug, toasty::Embed)]
#[index(id)]
enum Owner {
    Human {
        #[shared(id)]
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        human: toasty::Deferred<Human>,
    },
    Animal {
        #[shared(id)]
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        animal: toasty::Deferred<Animal>,
    },
}

#[derive(Debug, toasty::Model)]
struct Object {
    #[key]
    #[auto]
    id: uuid::Uuid,
    owner: Owner,
}
```

The `objects` table stores the discriminant in `owner` and the shared key
in `owner_id`. There is no separate `owner_kind` column. The relation fields
have no columns of their own. Variant names use snake_case in storage;
`#[column(variant = "...")]` changes a variant's stored value.

Variants can use different key types. A field without `#[shared]` gets its
own column, such as `owner_bot_serial` for a `Bot { serial: String, ... }`
variant. Put `#[index]` on a per-variant key. Shared keys use the enum-level
`#[index(id)]` shown above. An inverse relation requires an index prefixed
by its foreign-key columns on every backend, including DynamoDB.

## Creating and replacing an owner

Pass a parent reference inside a variant literal to fill its key fields:

```rust,ignore
let mut object = toasty::create!(Object {
    owner: Owner::Human { human: &alice },
})
.exec(&mut db)
.await?;

let another = alice.objects().create().exec(&mut db).await?;
```

You can also supply the key explicitly and leave the relation unloaded
with `Deferred::default()`. Change ownership by replacing the whole enum:

```rust,ignore
object.update()
    .owner(Owner::Animal {
        id: cat.id,
        animal: toasty::Deferred::default(),
    })
    .exec(&mut db)
    .await?;
```

The discriminant and key change together. Columns used only by other
variants become NULL. `stmt::patch` cannot enter an enum variant; such an
update returns `unsupported_feature` before writing anything.

Use `Option<Owner>` for optional ownership. Removing an association clears
an optional owner. Removing a required embedded owner returns an error;
replace it with another owner instead.

When the owner is nested inside an embedded struct, changing or clearing
the association preserves the struct's other fields. Replacing an owner
with the same owner preserves its inverse relation, including a required
`has_one`.

## Querying and loading

An inverse query automatically filters by both key and variant. A human
and an animal can have the same UUID without seeing each other's objects.
A query on the shared field `Object::fields().owner().id()` intentionally
matches both kinds.

```rust,ignore
let objects = Object::all()
    .include(Object::fields().owner())
    .exec(&mut db)
    .await?;

for object in objects {
    match object.owner {
        Owner::Human { human, .. } => println!("{}", human.get().name),
        Owner::Animal { animal, .. } => println!("{}", animal.get().name),
    }
}
```

Preloading groups owners by variant and queries only variants present in
the result. A plain relation field, such as `human: Human`, loads eagerly
without `.include()`.

An empty result does not query any owners. Loading a dangling owner key
returns `record_not_found`. Including a `via` relation through an embedded
inverse pair is unsupported and returns `unsupported_feature`.

## Pair paths and reuse

Pair inference searches the target model's fields and its embedded structs
and enum variants. Exactly one `belongs_to` must target the declaring
model. If several match, specify a prefix with `pair`, for example
`#[has_many(pair = owner.human)]`. A prefix must still identify exactly one
relation; the full path can end at the relation field itself.

The same embedded type can appear on several models or several fields of
one model. Each embedding pairs independently. For two optional fields
named `primary_owner` and `secondary_owner`, use
`pair = primary_owner` and `pair = secondary_owner` to distinguish them.
An embedding without an inverse declaration is also valid.
