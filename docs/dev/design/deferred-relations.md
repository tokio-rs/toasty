# Deferred-backed Relations

## Summary

Relations use ordinary Rust field types to control loading behavior. A relation
wrapped in `Deferred<T>` is lazy, and a direct relation value is eager. Instead
of declaring relation fields with dedicated wrapper types such as `HasMany<T>`,
`HasOne<T>`, and `BelongsTo<T>`, users declare lazy relations with
`Deferred<T>`:

```rust
#[has_many]
foos: toasty::Deferred<Vec<Foo>>,

#[has_one]
profile: toasty::Deferred<Option<Profile>>,

#[belongs_to(key = user_id, references = id)]
user: toasty::Deferred<User>,
```

They declare eager relations with the relation value directly:

```rust
#[has_many]
foos: Vec<Foo>,

#[has_one]
profile: Option<Profile>,

#[belongs_to(key = user_id, references = id)]
user: User,
```

This removes the public relation wrapper types and makes the field type describe
whether Toasty loads the relation on demand or with every query that returns the
model.

## Motivation

Relation wrappers and `Deferred<T>` solve the same user problem: a field may
exist on the Rust model but not be loaded by a default query. Today those paths
are separate:

- `Deferred<T>` is used for scalar and embedded fields.
- `HasMany<T>`, `HasOne<T>`, and `BelongsTo<T>` are used for relations.

The split leaks into the API. Users must learn separate wrapper names, separate
loaded-state types, and separate rules for when `.include()` matters. Once lazy
relations are modeled as `Deferred<RelationValue>`, relation fields use the
same loaded-state API as other deferred fields. Direct relation fields cover the
other common case: a relation that every query for the model needs.

## User-facing API

Lazy relations are declared by wrapping the relation's value type in
`Deferred<T>`.

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: toasty::Id<Self>,

    #[has_many]
    posts: toasty::Deferred<Vec<Post>>,
}

#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    id: toasty::Id<Self>,

    user_id: toasty::Id<User>,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::Deferred<User>,
}
```

A normal query leaves those fields unloaded:

```rust
let user = User::filter_by_id(id).get(&mut db).await?;
assert!(user.posts.is_unloaded());
```

Including the relation loads the `Deferred<T>` value:

```rust
let user = User::filter_by_id(id)
    .include(User::fields().posts())
    .get(&mut db)
    .await?;

for post in user.posts.get() {
    println!("{}", post.title);
}
```

Eager relations are declared by using the relation value directly. Toasty loads
an eager relation with every query that returns the model, as if the query had
included the relation path.

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: toasty::Id<Self>,

    #[has_many]
    posts: Vec<Post>,
}

let user = User::filter_by_id(id).get(&mut db).await?;

for post in &user.posts {
    println!("{}", post.title);
}
```

Before:

```rust
#[has_many]
posts: toasty::HasMany<Post>,

#[belongs_to(key = user_id, references = id)]
user: toasty::BelongsTo<User>,
```

After:

```rust
#[has_many]
posts: toasty::Deferred<Vec<Post>>,

#[belongs_to(key = user_id, references = id)]
user: toasty::Deferred<User>,
```

Or, when the relation should load with every model query:

```rust
#[has_many]
posts: Vec<Post>,

#[belongs_to(key = user_id, references = id)]
user: User,
```

## Behavior

Lazy relation fields behave like other `Deferred<T>` fields. The field is
unloaded after a default query, loaded after `.include(...)`, and can be
returned to the unloaded state with `.unload()`.

`#[has_many] Deferred<Vec<T>>` loads as an empty `Vec<T>` when no related rows
exist. `#[has_one] Deferred<Option<T>>` and
`#[belongs_to] Deferred<Option<T>>` load as `None` when the relation was loaded
and no related row exists.

Direct relation fields are eager. `#[has_many] Vec<T>` loads as an empty
`Vec<T>` when no related rows exist. `#[has_one] Option<T>` and
`#[belongs_to] Option<T>` load as `None` when the relation is optional and no
related row exists. Required direct single relations use `T`.

Toasty rejects schemas with eager-load cycles. A schema such as
`User { posts: Vec<Post> }` and `Post { user: User }` would recurse forever, so
one side must use `Deferred<_>`.

Relation accessors remain available. A `user.posts()` method still returns a
relation query builder for querying, filtering, inserting into, or removing
from the association. The field value and relation query accessor continue to
serve different purposes:

- `user.posts` is the loaded model field for eager relations, or the
  loaded/unloaded slot for lazy relations.
- `user.posts()` builds a statement for the association.

## Edge cases

Loaded null and unloaded must be distinct. The internal value encoding should
use one lazy-slot convention for deferred fields and relations:

- `Null` means unloaded.
- `Record([value])` means loaded.

This lets `Deferred<Option<T>>` and `Deferred<Option<Model>>` represent loaded
`None` as `Record([Null])` without a relation-specific special value.

Eager relation cycles are schema errors. Cycles are checked across eager
relation fields only. A cycle that contains a `Deferred<_>` edge is allowed
because the lazy edge does not load by default.

Multi-step `via` relations may be eager once include/select support for `via`
relations exists. The eager-cycle check must treat an eager `via` relation like
the relation path it follows.

## Driver integration

No new driver operation should be required. Relation loading remains an engine
concern built from existing query, nested-merge, and statement-lowering
machinery. Drivers should continue to see ordinary statement plans and ordinary
`stmt::Value` records/lists.

The internal value contract does matter for out-of-tree engine integrations:
lazy slots should use the unified `Null` versus `Record([value])` shape before
the public relation syntax changes.

## Incremental Plan

1. Done: add explicit internal helpers for lazy-slot encoding and decoding.
   Preserve current behavior, but route `Deferred<T>` through shared helpers.

2. Done: update relation wrappers to accept the lazy-slot shape. `HasMany<T>`,
   `HasOne<T>`, and `BelongsTo<T>` should decode `Null` as unloaded and
   `Record([value])` as loaded.

3. Done: remove the nullable single-relation special case. Encode loaded
   `None` for nullable has-one and belongs-to relations as `Record([Null])`.

4. Done: split relation target traits from relation field traits. Model
   target/query-builder information stays on `Relation`; field schema
   construction moved to `HasManyField`, `HasOneField`, and `BelongsToField`.

5. Done: implement the field-level relation traits for the deferred field
   shapes. The accepted set is `Deferred<Vec<T>>`, `Deferred<T>`, and
   `Deferred<Option<T>>` where applicable.

6. Done: change macro parsing and expansion to use field-level relation traits.
   Relation attributes accept `Deferred<_>` for lazy relations and direct value
   types for eager relations. Old wrapper types are rejected as relation fields.

7. Done: teach default returning lowering to include non-deferred relation
   fields. `Deferred<_>` relation fields stay unloaded by default; direct
   relation fields are included automatically.

8. Done: add schema verification for eager relation cycles.

9. Done: update tests and docs for the new syntax.

10. Done: remove `HasMany<T>`, `HasOne<T>`, and `BelongsTo<T>` from the public
    API.

11. Cleanup: investigate folding some or all `Relation` associated types into
    the relation field traits. The step 4 split currently requires generated
    code to hop from a field type through `HasManyField::Target`,
    `HasOneField::Target`, or `BelongsToField::Target` before reaching
    `Relation` associated types such as `Model`, `Expr`, `Query`, `Many`, and
    `One`. Also investigate whether generated local type aliases such as
    `type __RelationTarget = <#target_ty as #field_trait>::Target;` are enough
    to keep the expansion readable without changing trait ownership. Compare
    the options by checking generated rustdocs and compiler error messages.
    TODO: investigate whether `Relation` is still needed at all, or whether
    generated code can use `Model` plus the relation field traits directly.
    TODO: consider renaming the shared `lazy_slot` helper module to `lazy`.

## Alternatives considered

Keep the existing wrappers and add eager wrapper variants. This would avoid a
large refactor, but it would expand the wrapper family and keep relations
conceptually separate from deferred fields.

Make every relation eager by default and add an attribute for lazy loading.
This reads nicely for some models, but it would make relation cycles too easy
to create and would be a major behavioral change for existing code.

Use `Option<T>` as the lazy marker for single relations. This conflicts with
the semantic difference between "the relation was not loaded" and "the relation
was loaded and no row exists." `Deferred<Option<T>>` represents both states
without overloading `Option`.

## Open questions

- Deferrable: should there be a batch loader for already-loaded
  `Vec<Model>` values with lazy relation fields?
- Deferrable: when old relation wrappers are deprecated, should they become
  type aliases, compatibility structs, or disappear in one major release step?
- Deferrable: should eager relation cycle detection ever allow explicitly
  bounded cycles?

## Out of scope

- Per-query relation projection beyond existing `.include(...)` and
  `.select(...)`; this design only changes relation field representation and
  default loading.
- Driver-specific join optimization; relation loading can continue to use the
  existing nested-query and nested-merge path.
