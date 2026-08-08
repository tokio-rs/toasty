# Polymorphic relations

## Summary

A model gains the ability to belong to one of several possible parent models —
an `Object` owned by a `Human`, an `Animal`, or a `Bot`, where each parent
model `has_many` objects. Rather than adding a new relation kind, the design
allows `#[belongs_to]` fields inside `#[derive(Embed)]` enum variants. The
enum's discriminant column records the owner kind, ordinary variant fields
hold the foreign keys, and the existing `#[shared]` attribute lets variants
with the same key type share one column. The set of possible owners is a
closed Rust enum, so exhaustiveness checking comes from `match`.

## Motivation

Modeling "this row belongs to one of several models" currently requires one
nullable foreign key and one `Option` relation per candidate owner:

```rust
#[derive(toasty::Model)]
struct Object {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[index]
    human_id: Option<uuid::Uuid>,
    #[belongs_to(key = human_id, references = id)]
    human: toasty::Deferred<Option<Human>>,

    #[index]
    animal_id: Option<uuid::Uuid>,
    #[belongs_to(key = animal_id, references = id)]
    animal: toasty::Deferred<Option<Animal>>,
    // ... one more pair per owner type
}
```

Nothing enforces that exactly one key is set, there is no single "owner"
value to match on, and no schema-level record that these relations are
mutually exclusive. Polymorphic associations are a roadmap item
([roadmap](../roadmap.md)).

## User-facing API

### Declaring a polymorphic relation

A polymorphic relation is an embedded enum whose variants carry a
`#[belongs_to]` field. Each variant names one possible owner model and holds
the foreign key for it:

```rust
#[derive(toasty::Embed)]
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
    Bot {
        #[index]
        serial: String,
        #[belongs_to(key = serial, references = serial)]
        bot: toasty::Deferred<Bot>,
    },
}

#[derive(toasty::Model)]
struct Object {
    #[key]
    #[auto]
    id: uuid::Uuid,

    owner: Owner,
}
```

Everything here is existing embedded-enum machinery. The table for `Object`
has:

- `owner` — the discriminant column; this is the "owner kind". The
  discriminant column always takes the embed field's name — there is no
  separate `owner_kind` column, and the guide documents this for readers
  expecting one from other ORMs. A different column name uses the ordinary
  `#[column("...")]` override on the field.
- `owner_id` — the shared key column, used by `Human` and `Animal` (the
  `#[shared(id)]` fields coalesce, following the normal
  `{enum_field}_{shared_ident}` naming rule).
- `owner_bot_serial` — `Bot`'s own key column, nullable, per the normal
  per-variant naming rule.

The `#[belongs_to]` fields map to no columns, exactly as at model level: the
relation references sibling key fields, and the key fields own the storage.

Key columns are indexed with the two forms embedded enums already define,
shown above. A shared key uses the enum-level attribute referencing the
shared logical field — `#[index(id)]` produces one index on `owner_id`
serving both `Human` and `Animal`. A per-variant key uses the field-level
attribute — `#[index]` on `serial` produces an index on `owner_bot_serial`,
which only `Bot` rows populate. As with a model-level `belongs_to`, a key
that backs a `has_many` pair must be indexed; "Index requirements" below
gives the rule.

Because `Owner` is an ordinary Rust enum, the set of possible owners is
closed and every `match` on it is checked for exhaustiveness by the
compiler. Adding a fourth owner type is a change to the enum that every
match site must acknowledge.

### Mixed key types

Variants are not required to share a key type or a column. `Bot` above is
keyed by a `String` serial number while `Human` and `Animal` share a UUID
column. Sharing follows the existing rule: `#[shared]` fields must have
identical types, and variants that do not share simply get their own
nullable column. A relation where every owner has its own key type uses no
`#[shared]` at all and stores one key column per variant.

### The owning side

Each owner model declares an ordinary `#[has_many]`. The pair resolves to
the `#[belongs_to]` inside the matching variant:

```rust
#[derive(toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[has_many]
    objects: toasty::Deferred<Vec<Object>>,
}

let objects: Vec<Object> = human.objects().collect(&mut db).await?;
```

Nothing about the field name drives this — resolution is type-directed,
and the rule is today's rule with a larger search space. A bare
`#[has_many]` collects every `belongs_to` whose target is the declaring
model, searching the target model's top-level fields and recursing through
its embed tree: each field of an embedded struct, each variant of an
embedded enum. Exactly one candidate is the pair; zero candidates is a
build error; two or more is a build error asking for an explicit `pair`.
`Human.objects` resolves to the `Human` variant because that variant holds
the only `belongs_to` on `Object` targeting `Human` — the `Animal` and
`Bot` variants target other models, so they are never candidates.
Candidates are locations, not declarations: a relation-carrying embed used
by two fields contributes one candidate per embedding.

The generated query filters on both the key and the discriminant —
`human.objects()` matches only rows whose `owner` column says `Human`, even
though `Animal` rows store their key in the same `owner_id` column. Users do
not write the discriminant predicate; it comes from the variant-scoped pair,
the same way variant-rooted filter paths already gate on the discriminant.

When inference is ambiguous, `pair` names the relation explicitly. It
accepts a dotted path — the same syntax as `via` — whose segments name
embed fields and enum variants:

```rust
#[has_many(pair = owner.human)]
objects: toasty::Deferred<Vec<Object>>,
```

An explicit path is a prefix filter over the same candidate set, and the
exactly-one rule applies to what survives the filter: bare `#[has_many]`
is the empty prefix and keeps every candidate; `pair = owner` keeps
candidates inside that embed field; `pair = owner.human` keeps the
variant's; the full path names a single field. Ambiguity after filtering
is the same build error, naming the surviving candidates.

A relation-carrying embed stays reusable like any other embedded type, and
each embedding pairs independently. With `Owner` embedded by both `Object`
and `Widget`, `Human` declares one `has_many` per embedding:

```rust
#[derive(toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[has_many]
    objects: toasty::Deferred<Vec<Object>>,
    #[has_many]
    widgets: toasty::Deferred<Vec<Widget>>,
}
```

Inference runs per target model, so both stay bare. Two embeddings on one
host model (`Object { primary_owner: Owner, secondary_owner: Owner }`) are
where explicit paths are required: `pair = primary_owner.human` versus
`pair = secondary_owner.human`. An embedding with no inverse declared is
fine — pairing is per embedding, not per type.

### Querying

The existing embedded-enum filter API applies unchanged:

```rust
// Objects owned by any bot.
Object::filter(Object::fields().owner().is_bot());

// Objects owned by a human named "Alice" — relation traversal inside the
// variant closure, gated on the discriminant.
Object::filter(
    Object::fields()
        .owner()
        .human()
        .matches(|v| v.human().name().eq("Alice")),
);

// Cross-variant: owned by *anything* keyed by this UUID. Shared logical
// fields resolve to the shared column with no variant gate.
Object::filter(Object::fields().owner().id().eq(some_id));
```

### Loading the owner

The key fields are plain data, so matching gives direct access with no new
API:

```rust
let name = match &object.owner {
    Owner::Human { id, .. } => Human::find_by_id(id).get(&mut db).await?.name,
    Owner::Animal { id, .. } => Animal::find_by_id(id).get(&mut db).await?.name,
    Owner::Bot { serial, .. } => Bot::find_by_serial(serial).get(&mut db).await?.name,
};
```

To avoid the extra round-trip, `.include()` preloads the relation field of
whichever variant each row holds:

```rust
let objects = Object::all()
    .include(Object::fields().owner())
    .collect(&mut db)
    .await?;

for object in &objects {
    if let Owner::Human { human, .. } = &object.owner {
        println!("{}", human.get().name); // loaded — `get()` does not panic
    }
}
```

### Creating and updating

In `create!` and the builders, set the owner by wrapping the parent in its
variant. Toasty fills the variant's key fields from the parent, mirroring
how `user: &alice` fills `user_id` for a model-level `belongs_to`:

```rust
toasty::create!(Object { owner: Owner::Human { human: &alice } })
    .exec(&mut db)
    .await?;
```

Supplying the key directly also works; the relation field carries no
storage, so an unloaded placeholder is valid:

```rust
toasty::create!(Object {
    owner: Owner::Bot { serial: bot_serial, bot: Deferred::default() },
})
```

Changing the owner — including changing its kind — is a whole-value
replacement of the embed, per existing embedded-enum update semantics:

```rust
object.update()
    .owner(Owner::Bot { serial: bot_serial, bot: Deferred::default() })
    .exec(&mut db)
    .await?;
```

Because the discriminant and key are one value, it is impossible to
represent a row that claims two owners or an owner kind whose key is
missing — the exclusivity that the status-quo encoding cannot enforce.

## Behavior

- `human.objects()` and other pair queries add the discriminant predicate
  automatically. On SQL backends the query has the form
  `WHERE owner = 1 AND owner_id = ?`.
- `.include(Object::fields().owner())` issues one query per variant present
  in the result set and merges the results into each row's enum value.
  Variants not present in the result set cost nothing.
- Writes set the discriminant and key columns together, as one embed value.
  Columns belonging to other variants are written NULL, per existing
  embedded-enum storage semantics.
- Optional ownership is `owner: Option<Owner>`, per existing optional-embed
  support; an ownerless row stores NULL in the discriminant column.
- Schema build fails with the existing errors when a `#[belongs_to]` key
  does not resolve to a sibling variant field, when `#[shared]` types
  disagree, or when a `#[has_many]` pair cannot be found.

## Edge cases

- **Cross-variant shared-key queries.** A filter on the un-gated shared
  field (`owner().id()`) spans owner kinds — a `Human` and an `Animal` with
  the same UUID both match. This is the documented semantics of shared
  columns, unchanged.
- **Dangling keys.** As with model-level `belongs_to`, nothing prevents a
  stored key from referencing a deleted owner; loading it yields the same
  not-found behavior as any stale foreign key.
- **Within-variant patch.** `stmt::patch` on a variant field follows the
  existing embedded-enum rules (variant-gated, SQL-only). Patching a key
  field re-points the relation without touching the kind.

## Pair resolution and lowering

This section resolves two questions the first draft left open: the scope of
relations inside embeds, and how the inverse pair is represented and
lowered.

### `pair` is a path

`via` and `pair` are duals. A `via` path steps through *relations* to reach
its target model, and resolution rejects a non-relation segment. A `pair`
path steps through *values* — embed fields and enum variants, never
relations — to reach a `belongs_to` inside the target model, and resolution
rejects a relation segment before the terminal.

`pair` reuses `via`'s resolution strategy: the derive emits the accessor
chain (`Object::fields().owner().human()`) and converts the result to a
path, so segments resolve through Rust name resolution and a bad segment is
a compile error. Variant names are accessor methods on the enum fields
struct, so the dotted-ident syntax reaches variants without new grammar.
This also upgrades `pair` from today's runtime `field_name_to_id` lookup to
compile-time checking, matching `via`.

Nothing in the mechanism is enum-specific: a `belongs_to` inside an
embedded *struct* is a pair path whose step selects no variant. Relations
are allowed in all embedded types.

### Schema representation

`Has.pair_id: FieldId` cannot express a relation inside an embed. It is
replaced by a struct that locates the paired `belongs_to`:

```rust
pub struct Pair {
    /// Embed fields descended through, outermost first. Empty for a
    /// top-level pair.
    pub steps: Vec<PairStep>,
    /// The `BelongsTo` field: on the target model when `steps` is empty,
    /// otherwise on the embedded type reached by the last step.
    pub field: FieldId,
}

pub struct PairStep {
    /// The embed field descended into.
    pub field: FieldId,
    /// For an embedded enum, the variant carrying the rest of the path: a
    /// read through this pair requires the variant's discriminant, and a
    /// write through it constructs this variant. `None` for embedded
    /// structs.
    pub variant: Option<VariantId>,
}
```

`Pair::direct(field_id)` covers the top-level case, so existing
construction sites migrate mechanically. Every level has a well-defined
`FieldId` because embedded types have their own schema entries with
flattened field lists.

A purpose-built struct is used instead of `stmt::Path`: `stmt::Path` steps
are bare indices and its variant root nests inside-out (the parent path is
boxed inside the root), so consumers walk it by recursion and lose
schema-level meaning. `Pair` reads outward-in — verify rules walk `steps`
to find the discriminant and key columns, and lowering converts it to a
projection expression directly.

### The back-link: per-embedding pair instances

An embedded type has one schema entry shared by every field that embeds
it, so a `BelongsTo` inside one cannot carry its own inverse link:
embedding the type from two fields gives one definition and two inverse
`Has` candidates. Pair linkage is per-embedding data hanging off a shared
definition — the same split the mapping already implements for columns
(one app entry per embed type; each host model's mapping resolves its own
columns).

`BelongsTo.pair` is therefore removed. Each root model instead carries one
instance record per `belongs_to` reachable on it, directly or through its
embed tree:

```rust
pub struct RelationInstance {
    /// Where the `belongs_to` sits: a top-level field (no steps) or a
    /// field inside the model's embed tree.
    pub location: Pair,
    /// The inverse `Has` on the relation's target model, when one is
    /// declared for this embedding.
    pub pair: Option<FieldId>,
}
```

A top-level `belongs_to` is the empty-steps case, so one mechanism covers
both and the old field-level back-link would be redundant. Every consumer
of the back-link — cycle avoidance in mutation lowering, via join
planning, pair verification — resolves through the host model's instance
records, and each of those sites already holds the host context because it
navigated there from a host statement.

The instance records are built by `link_relations` and stay in the app
layer, which remains authoritative for relation semantics; the mapping's
per-embedding relation node may mirror the `pair` for lowering
convenience, as an implementation choice. The linker resolves each pair
once and materializes both views from that resolution — `Has.pair` on the
owning side, the instance record on the host — so the two cannot disagree
and the old field-level back-link consistency check disappears rather than
generalizing. What remains to verify is uniqueness: at most one `Has`
pairs with a given instance, so two `has_many` claiming the same embedding
is a build error. The instance list also enumerates every `belongs_to`
reachable on a model without recursive field walks, which cascade planning
can use directly.

### Lowering: the gate and the foreign key

A reference to a relation field is already an engine-only expression:
it cannot reach SQL, and lowering eliminates it at one centralized rewrite
that expands relation references into foreign-key references. Every
consumer funnels through that rewrite — the `has_many` association filter,
includes, subquery lifting, and user-written comparisons on relation paths.

A variant-scoped pair extends that invariant rather than adding machinery.
At the same rewrite, a reference to a relation reached through `Pair` steps
lowers into two things at once:

- the foreign-key comparison, with source fields projected through the
  embed (the shared or per-variant key columns), and
- an `is_variant` gate for each step that selects a variant, ANDed into
  the enclosing boolean expression.

Fusing the gate with the key comparison at the single rewrite is a
correctness requirement, not a style choice: with a `#[shared]` key column,
`owner_id = ?` without the gate matches an `Animal` row holding the same
UUID. No consumer can obtain the key comparison without the gate, so no
expansion site can leak rows of the wrong variant.

Negation follows the existing variant-field convention: the typed layer
lowers `ne` on a variant field to `is_variant AND field != x`, and a `ne`
through a variant-scoped relation lowers the same way — false for rows of
other variants — so filter semantics do not depend on whether the gate
comes from the typed layer or the engine rewrite.

On the write side, `Pair` carries the intent mutations need: associating
through the pair (`human.objects().create(...)`) constructs the variant
value with the key fields set from the parent's key — a whole-embed write,
discriminant included, per existing embedded-enum semantics. Disassociating
writes NULL when the embed field is `Option`, and errors under the existing
nullability rules otherwise.

### Index requirements

The existing rule — a paired foreign key must be index-backed so pair
queries run fast — carries over unchanged, applied through the embed: the
verify rule requires an index prefixed by the key column(s). The
discriminant is not part of the requirement; its predicate filters outside
the index.

This is sufficient because the index never carries correctness — the
discriminant gate is fused into every lowered filter, so rows of the wrong
variant cannot match regardless of indexing. On speed, a key-only index
over-matches exactly the rows of other variants holding the same key
value, bounded by the number of sharing variants. A per-variant key column
does not even pay that: it is NULL outside its variant, so a key-only
index on it is already variant-scoped. Because the discriminant is never
required in an index, no syntax exists or is needed to name it in an index
declaration; composite `(discriminant, key)` indexes are simply not
declarable.

Declaration uses what embedded enums already define: field-level
`#[index]` on a per-variant key, and enum-level `#[index(<ident>)]` on a
shared key. The enum-level non-unique form is designed in
[enums-and-embedded-structs](enums-and-embedded-structs.md) but only
`#[unique]` has shipped — shipping it is a prerequisite of this feature.

On DynamoDB the rule guarantees a GSI on the key attribute, which is what
makes the pair query executable at all. A per-variant key yields a sparse
GSI containing only that variant's rows; a shared key yields one GSI
serving every sharing variant, with the discriminant applied as a filter
expression.

`has_one` pairs want a unique key index, and `#[unique]` on a shared key
is cross-variant per the documented shared-column semantics — a `Human`
and an `Animal` with the same key conflict. That is stronger than the
relation needs but consistent; per-variant uniqueness uses per-variant
key columns.

## Removals

The new concepts are strictly more expressive than several existing ones,
which get deleted rather than kept alongside:

- **`BelongsTo.pair: Option<FieldId>`.** Subsumed by the per-model
  instance records — a top-level `belongs_to` is the empty-steps case, so
  the field-level back-link would be a second mechanism for the same
  concept.
- **`Has.pair_id: FieldId`.** Replaced by `Has.pair: Pair`;
  `Pair::direct` is the exact old shape.
- **Runtime pair name resolution.** Explicit `pair = ...` currently emits
  a `field_name_to_id("...")` string lookup at schema construction
  (`macros/src/model/expand/schema.rs`). Accessor-chain emission deletes
  that path; `field_name_to_id` survives only for resolving
  `references = ...` on foreign keys.
- **The field-level pair consistency check** (`schema/verify.rs`). The
  linker resolves each pair once and materializes both views from it, so
  the bidirectional agreement check has nothing left to catch. Only
  per-instance uniqueness remains as a verify rule.
- **`Field::pair()`'s `BelongsTo` arm** (`schema/app/field.rs`). A shared
  definition cannot answer "what is my inverse" without host context;
  consumers move to the host-model instance lookup. The `Has` arm stays.

Checked for redundancy and deliberately kept:

- **`Via.path` vs `Pair`** — duals over disjoint step domains (relation
  steps vs. value steps) with opposite validation rules; merging them
  would turn structural rejections into runtime checks.
- **`stmt::PathRoot::Variant`** — serves typed filter paths generally;
  `Pair` is schema-layer and converts to a variant-rooted projection
  during lowering.
- **`ForeignKey` / `ForeignKeyField`** — already host-independent
  (embed-local sources, owner-model targets); instancing never touches
  them.

## Implementation order

The work ships in steps, each providing user value on its own.

1. **Enum-level `#[index(...)]`.** The non-unique form designed in
   [enums-and-embedded-structs](enums-and-embedded-structs.md); only
   `#[unique]` has shipped. Value independent of this design — non-unique
   indexes on shared columns for existing embedded-enum users — and a
   prerequisite for pair queries on shared keys passing the verify rule.
2. **Relations stored in embedded types.** `#[belongs_to]` fields are
   accepted inside embedded enums and structs: schema entries, no columns
   for the relation itself, key fields as ordinary indexed columns.
   Creating and updating supply the variant value with explicit keys;
   `match` gives direct access to the stored keys, and the owner loads
   with an ordinary `find_by_*`. No `.include()`, no inverse yet — but the
   polymorphic shape is fully modelable, storable, and queryable by key
   through the existing variant filter paths.
3. **Referencing the embedded relation with a model value.** The
   centralized rewrite: a reference to a relation reached through embed
   steps expands into the key comparison with the fused `is_variant`
   gate. In filters this enables `owner().human().eq(&alice)` and
   traversal (`.matches(|v| v.human().name().eq("Alice"))`); in `create!`
   and update assignments it enables passing the parent by reference
   (`Owner::Human { human: &alice }`) with the key fields filled in.
4. **Preloading with `.include()`.** `.include(Object::fields().owner())`
   issues one query per variant present and merges results into each
   row's enum value.
5. **Inverse pairs, queries.** `pair` paths, the `Pair` struct, the
   per-embedding instance records, linker recursion, and the removals
   listed above land together; `has_many` / `has_one` declarations on
   owner models pair into embeddings and read through them
   (`human.objects()`).
6. **Inverse pairs, mutations.** Creating, associating, and
   disassociating through the pair — `has_many` create builders, update
   assignments, and delete behavior over the paired embedding.

## Driver integration

Nothing. Drivers see ordinary discriminant, shared, and per-variant columns
and existing `Operation` variants. No new capability flags. DynamoDB
encoding and index rules for embedded enums apply as-is.

## Alternatives considered

- **Flat `owner_id` + `owner_kind` columns with a dedicated relation-enum
  derive.** The Rails layout as a first-class relation kind
  (`FieldTy::BelongsToAny` in the app schema, a new derive for the enum).
  Rejected: it hard-codes one storage layout, forces every owner to share a
  primary-key type, and duplicates discriminant machinery the embedded-enum
  support already has. The embed approach expresses the same layout with
  `#[shared]` and also handles mixed key types.
- **Per-owner top-level optional relations (status quo).** Shown in
  Motivation. No closed owner set, no exclusivity, no single value to match
  on. Remains available for cases that genuinely are independent optional
  relations.
- **A new multi-target `BelongsTo`.** Widening `BelongsTo.target` to a list
  of models. Rejected: every consumer of the app schema assumes a single
  target, and the variant encoding gives each target its own ordinary
  single-target `BelongsTo`, so the existing resolution, verification, and
  codegen apply per variant.
- **Per-expansion-site gate helper.** Each place that expands a pair
  (association filter, include, mutations) calls a helper that adds the
  `is_variant` gate. Rejected: a missed call site is a silent cross-variant
  match, not an error. The gate is fused into the single relation-reference
  rewrite instead.
- **A dedicated statement-level pair expression.** A new `stmt::Expr` node
  meaning "this row's pair matches" that lowering expands. Rejected:
  relation-field references already carry the "must be eliminated during
  lowering" contract and give the same single expansion point; a new node
  adds a variant every engine pass must traverse for no additional
  centralization.
- **Restricting relation-carrying embeds to one embedding.** A build error
  when a type containing a `belongs_to` is embedded by more than one field,
  keeping `BelongsTo.pair` single. Rejected: reuse with independent
  per-embedding pairing is worth supporting, and the instance records that
  enable it also subsume the field-level back-link for top-level relations.
- **Keying `BelongsTo.pair` by host** (a `Vec` of `(embedding, Has)`
  entries on the shared definition). Rejected: it leaves two back-link
  mechanisms — field-level for top-level relations, keyed for embedded
  ones — where per-model instance records cover both with one.

## Open questions

None. Questions raised during review are resolved in place: the scope of
relations inside embedded structs, the inverse-pair representation, embed
reuse with the `BelongsTo` back-link, and index requirements in "Pair
resolution and lowering"; discriminant column naming as documented
behavior in "Declaring a polymorphic relation".

## Out of scope

- **`via` relations through a polymorphic owner** (e.g. collecting all
  objects of all humans in a group) — composition with `via` is a separate
  design.
- **Polymorphic many-to-many** — needs a join model; a join model with a
  polymorphic `belongs_to` falls out of this design, but a dedicated API
  does not.
- **Per-variant partial indexes** — already out of scope for embedded
  enums; per-variant uniqueness uses per-variant columns.
- **Database-level foreign-key constraints on variant key columns** —
  Toasty does not emit FK constraints for model-level relations either;
  unchanged here.
