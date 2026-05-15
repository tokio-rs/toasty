# Chained relation paths in field accessors

## Summary

Make `User::fields().comments().article()` — chaining typed field accessors
through a relation — a documented, first-class capability for building paths.
Use it in `#[has_many(via = a.b)]` so the macro emits the path as a method
chain that the Rust compiler validates: a typo or wrong field is a Rust type
error pointing at the attribute, not a schema-build error at runtime. The
two-phase `Via { segments, path: Option<stmt::Path> }` representation and the
linker's `resolve_via_path` pass go away. The same chaining mechanism is the
foundation a later "filter through relations" feature would build on.

## Motivation

The macro already generates relation accessors on the per-model field-list and
field structs. `User::fields().comments()` returns Comment's
`ManyField<User>`, which exposes accessors for every field on Comment —
including relations. So `User::fields().comments().article()` already
type-checks today and produces a `Path<User, List<Article>>` — but Toasty
treats this as an internal implementation detail rather than a documented
capability, and `#[has_many(via = a.b)]` does not use it.

Instead, the via implementation emits its segments as `Vec<String>` and the
schema linker walks them at link time, producing a runtime "field `b` not
found on model X" error for any typo. The macro could emit
`Self::fields().a().b()` directly and have Rust check it.

Beyond `via`, formalizing chained accessors is the foundation for relation
filtering through any depth — `User::fields().comments().article().title().eq("Rust")`
— because the chain already produces the right typed path; only the
predicate methods at the leaves need extending.

## User-facing API

### Chained field accessors

Field accessors chain through relations. Each step picks the next model's
field-accessor struct, parameterised by the original origin so the resulting
path stays rooted there:

```rust
// User → comments (HasMany<Comment>) → article (BelongsTo<Article>)
let path = User::fields().comments().article();
// path: Path<User, List<Article>>  (after `.into()`)
```

The chain works for any sequence of relation kinds — `BelongsTo`,
`HasOne`, `HasMany` — and through embedded struct fields. The result of
each step is the next model's field accessor, with the same `Origin`
type parameter, so the path stays anchored at the model the chain
started on.

**Cardinality propagation.** Once a step traverses a list-producing
relation (`HasMany`, or any relation reached from a list context), every
subsequent step returns a list-shaped accessor. So
`User::fields().comments().article()` resolves to
`Path<User, List<Article>>`, not `Path<User, Article>` — a user comments
on many articles, transitively. A pure single-cardinality chain
(`Order::fields().customer().organization()` — `BelongsTo` then `BelongsTo`)
stays single: `Path<Order, Organization>`.

### Use in `via`

`#[has_many(via = a.b)]` emits the chain directly:

```rust
#[has_many(via = comments.article)]
commented_articles: HasMany<Article>,
```

Roughly what the macro emits for the schema:

```rust
app::HasKind::Via(app::Via {
    path: User::fields().comments().article().into(),
})
```

A typo —`#[has_many(via = comments.artical)]`— becomes:

```text
error[E0599]: no method named `artical` found for struct `CommentFieldsList<User>`
   --> src/lib.rs:NN
       |
       |    #[has_many(via = comments.artical)]
       |                              ^^^^^^^ help: method `article` exists
```

instead of a `Schema build failed: ... unknown field 'artical' on Comment`
panic at `Db::builder().build().await?`.

### Path conversion

A field accessor converts to the untyped `stmt::Path` via `.into()` (existing
`Into` impls). Code that already does
`Self::fields().#field_ident().into()` to build single-step paths keeps
working unchanged; the chain just lengthens it.

## Behavior

**Macro emit.** `#[has_many(via = a.b.c)]` and `#[has_one(via = a.b.c)]`
emit `Self::fields().a().b().c().into()` (typed path → untyped path) as
the path stored on the relation's `HasKind::Via`. Each segment is a
method call on the previous step's return type; missing or wrong segments
fail to compile and the diagnostic spans the offending segment in the
attribute.

**Schema representation.** `Via` collapses to:

```rust
pub struct Via {
    pub path: stmt::Path,
}
```

No `segments`, no `Option`. The linker no longer carries a
`resolve_via_relations` pass; only direct-relation pair-linking remains.

**Embedded fields in `via` paths.** The chain works through embedded
struct fields automatically — `Order::fields().shipping().warehouse()` is
a valid chain when `shipping` is an embedded struct holding the
`warehouse` `BelongsTo`. This is observable: the
`via` design previously rejected embedded paths at the schema linker
("steps through an embedded model, which is not yet supported"). With
compile-checked emit, that rejection no longer fires — `via` through an
embedded struct field becomes available wherever the chain compiles.
See Open questions for whether this is the intended outcome.

**Cardinality rule.** A list-producing step makes every subsequent step
return a list path. The compiler enforces this through the
`ManyField<Origin>` / `OneField<Origin>` distinction in the existing
codegen.

**Errors.** The only error class users hit is "method not found" at the
attribute site when a segment doesn't exist. Type errors at the chain's
leaf — e.g. declaring `HasMany<Article>` when the path resolves to
`Path<_, Article>` (single, not list) — surface as the assertion the
`Association::many` constructor already does (`path.root.as_model_unwrap()
== T::id()`), or as a `From<Path<...>>` mismatch when the macro
constructs `Via.path`.

## Edge cases

- **All four relation-kind transitions** compile and produce the
  expected typing:
  - `BelongsTo` → `BelongsTo`: single → single.
  - `BelongsTo` → `HasMany`: single → list.
  - `HasMany` → `BelongsTo`: list → list (cardinality propagates).
  - `HasMany` → `HasMany`: list → list.
  Same for `HasOne` substituted for either side.
- **Self-referential relations** (`#[has_many] children: HasMany<Self>`)
  chain like any other; the field accessor for `Self` is the same struct
  used in the outer model.
- **Composite keys** have no impact on chain shape — they only affect
  the foreign-key wiring at each step, which is already handled by the
  per-step pair lookup at lower time.
- **Field-name collisions with chain helpers** (`any`, `all`,
  `from_path`, `path`, `eq`, `in_query`, `create`) on a field-list or
  field struct: a model field named, e.g., `any` would today silently
  shadow the `any` filter helper. This is pre-existing and out of scope
  here, but `via` paths picking up `.any()` would compile-check the
  same way, just to a non-field method.

## Driver integration

Nothing. The macro emits the same `stmt::Path` shape the linker
currently produces; drivers see only the lowered statement.

## Alternatives considered

**Status quo: string segments + linker resolution.** Keep `Via { segments,
path: Option }` and `resolve_via_path`. Rejected: typos and stale paths
become runtime schema-build errors when the existing codegen is enough to
make them compile errors.

**A custom chain trait.** Generate a dedicated `ChainStep`/`Navigate`
trait that the macro drives explicitly. Rejected: the per-model
`field_struct_ident` and `field_list_struct_ident` already expose
relation accessors that produce origin-rooted paths — adding a parallel
trait duplicates that machinery.

**Const lookup of segment names.** Generate per-model `const FIELDS`
structs and look up segments at const-eval time. Rejected: works for
single-step lookup but does not compose across models without the same
type-driven chain that direct method calls already provide.

## Open questions

- **Embedded fields in `via` paths.** Compile-checked emit lifts the
  schema-linker rejection of embedded steps as a side effect; the chain
  through `Order::fields().shipping().warehouse()` simply works.
  *Blocking implementation:* keep the rejection — by inspecting the
  macro-emitted path or by some other check — to match the
  `has-many-via` design's stated scope, or accept embedded `via` as a
  bonus that ships with this change?
- **Filter chaining through relations.** The same chain currently leaves
  primitive accessors at the leaves returning a `ListPath` rather than a
  filter-expression builder. Settling this is what would let
  `User::fields().comments().article().title().eq("Rust")` compile to a
  predicate. *Deferrable — separate feature, separate design.*
- **Diagnostic quality.** Today a typo on a relation field produces a
  generic "method not found" error pointing at the segment. Whether to
  invest in a `#[diagnostic::on_unimplemented]`-style annotation that
  reads "no field `artical` on `Comment`" instead of "method `artical`
  not found on `CommentFieldsList<User>`" is worth deciding before
  implementation. *Deferrable.*

## Out of scope

- **Filter / predicate chaining through relations.** Uses the same chain
  but extends primitive accessors at the leaves; deserves its own
  design.
- **Eliminating `Via` entirely.** With `path: stmt::Path`, the wrapper
  struct is thin; whether to inline `stmt::Path` directly into
  `HasKind::Via(stmt::Path)` is a style question, not a behavior
  change — leave it for the implementer to decide.
- **Generic relation traversal at the user-API level** beyond `via` and
  filters (e.g., `user.comments().article().exec(&mut db)` returning the
  flattened list directly). The chain produces a typed path, not a
  query; turning that into an executable query is the existing
  `Association` machinery's job and is unchanged here.
