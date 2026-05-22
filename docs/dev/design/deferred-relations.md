# Deferred-backed Relations

## Summary

Relations should use the same loaded/unloaded value model as deferred fields.
Instead of declaring relation fields with dedicated wrapper types such as
`HasMany<T>`, `HasOne<T>`, and `BelongsTo<T>`, users will declare lazy
relations with `Deferred<T>` and eager relations with the relation value
directly:

```rust
#[has_many]
foos: toasty::Deferred<Vec<Foo>>,

#[has_one]
profile: toasty::Deferred<Option<Profile>>,

#[belongs_to(key = user_id, references = id)]
user: toasty::Deferred<User>,
```

The direct forms become eager relation fields:

```rust
#[has_many]
foos: Vec<Foo>,

#[has_one]
profile: Option<Profile>,

#[belongs_to(key = user_id, references = id)]
user: User,
```

This removes the public relation wrapper types and makes relation loading
consistent with `#[deferred]` scalar and embedded fields.

## Motivation

Relation wrappers and `Deferred<T>` solve the same user problem: a field may
exist on the Rust model but not be loaded by a default query. Today those paths
are separate:

- `Deferred<T>` is used for scalar and embedded fields.
- `HasMany<T>`, `HasOne<T>`, and `BelongsTo<T>` are used for relations.

The split leaks into the API. Users must learn separate wrapper names, separate
loaded-state types, and separate rules for when `.include()` matters. It also
makes a useful concept awkward to express: a relation that is always eager
loaded. Once relations are modeled as either `Deferred<RelationValue>` or the
relation value directly, eager and lazy loading become a property of the field
type rather than a separate family of relation-specific wrappers.

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

Direct relation values are eager by default:

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: toasty::Id<Self>,

    #[has_many]
    posts: Vec<Post>,
}
```

Loading a `User` loads `posts` as part of the model's default projection. This
is useful for small, ownership-like relations that are expected by every caller.

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

## Behavior

Lazy relation fields behave like other `Deferred<T>` fields. The field is
unloaded after a default query, loaded after `.include(...)`, and can be
returned to the unloaded state with `.unload()`.

`#[has_many] Deferred<Vec<T>>` loads as an empty `Vec<T>` when no related rows
exist. `#[has_one] Deferred<Option<T>>` and
`#[belongs_to] Deferred<Option<T>>` load as `None` when the relation was loaded
and no related row exists.

Direct relation fields are eager. A default model load includes them without an
explicit `.include(...)`. Eager relation loading must be acyclic: a cycle of
eager relation fields is a schema error. Break the cycle by making at least one
edge `Deferred<_>`.

Relation accessors remain available. A `user.posts()` method still returns a
relation query builder for querying, filtering, inserting into, or removing
from the association. The field value and relation query accessor continue to
serve different purposes:

- `user.posts` is the loaded model field.
- `user.posts()` builds a statement for the association.

## Edge cases

Loaded null and unloaded must be distinct. The internal value encoding should
use one lazy-slot convention for deferred fields and relations:

- `Null` means unloaded.
- `Record([value])` means loaded.

This lets `Deferred<Option<T>>` and `Deferred<Option<Model>>` represent loaded
`None` as `Record([Null])` without a relation-specific sentinel.

Eager relation cycles are rejected. Without this, a model graph such as
`User { posts: Vec<Post> }` and `Post { user: User }` would recursively imply
loading users with posts with users with posts.

Multi-step `via` relations should remain lazy-only until include/select support
for `via` relations exists. The current relation include path already rejects
multi-step relation projection, so eager `via` would expose an unsupported
default-load path.

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

3. Done: remove the nullable single-relation sentinel. Replace the current
   loaded `None` encoding for nullable has-one and belongs-to relations with
   `Record([Null])`.

4. Split relation target traits from relation field traits. Keep the model
   target/query-builder information on a `Relation`-like trait, and introduce
   field-level traits for `has_many`, `has_one`, and `belongs_to` fields.

5. Implement the field-level relation traits for both old and new field shapes.
   The compatibility set should include `HasMany<T>`, `HasOne<T>`,
   `BelongsTo<T>`, `Deferred<Vec<T>>`, `Vec<T>`, `Deferred<T>`, `T`,
   `Deferred<Option<T>>`, and `Option<T>` where applicable.

6. Change macro parsing and expansion to use field-level relation traits.
   Relation attributes should accept `Deferred<_>` and direct value types while
   old wrapper types continue to compile.

7. Teach default returning lowering to include non-deferred relation fields.
   `Deferred<_>` relation fields stay unloaded by default; direct relation
   fields are included automatically.

8. Add schema verification for eager relation cycles and unsupported eager
   `via` relations.

9. Update tests and docs for the new syntax. Keep the old wrapper syntax as a
   compatibility path until the migration is complete.

10. Deprecate, then remove, `HasMany<T>`, `HasOne<T>`, and `BelongsTo<T>` from
    the public API.

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

- Blocking implementation: what exact trait names should replace the wrapper
  side of `Relation`?
- Blocking implementation: should eager `belongs_to` be allowed for required
  relations only, or also for nullable `Option<T>`?
- Blocking implementation: should eager relation cycle detection reject every
  cycle, or allow explicitly bounded cycles in the future?
- Deferrable: should there be a batch loader for already-loaded
  `Vec<Model>` values with lazy relation fields?
- Deferrable: when old relation wrappers are deprecated, should they become
  type aliases, compatibility structs, or disappear in one major release step?

## Out of scope

- Per-query relation projection beyond existing `.include(...)` and
  `.select(...)`; this design only changes relation field representation and
  default loading.
- Eager loading through multi-step `via` relations; this should wait for
  include/select support for `via`.
- Driver-specific join optimization; relation loading can continue to use the
  existing nested-query and nested-merge path.
