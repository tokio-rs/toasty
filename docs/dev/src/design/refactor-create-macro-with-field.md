# Refactor `create!` Macro: Replace `with_$field` with Field-Based Builders

Replace the generated `with_$field` closure methods on create builders with a
field-based approach that resolves nested create builders through the model's
fields infrastructure. This eliminates a class of name conflicts and reduces the
generated code surface.

## Problem

The `create!` macro expands nested struct/list values using `with_$field`
methods on the create builder. For example:

```rust
// Input:
toasty::create!(User {
    name: "Carl",
    todos: [{ title: "first" }, { title: "second" }],
})

// Expands to:
User::create()
    .name("Carl")
    .with_todos(|b| b.with_item(|b| b.title("first")).with_item(|b| b.title("second")))
```

Each relation field on the create builder gets a `with_$field` method generated
by `toasty-codegen`. This creates two problems:

1. **Name conflicts.** A model with fields `foo` and `with_foo` produces two
   methods named `with_foo` — one for the primitive field `with_foo` and one for
   the closure-based setter for the relation `foo`. The codegen currently stores
   a `with_ident` on every field (`schema/field.rs:38`) and generates it
   unconditionally.

2. **Code bloat.** Every BelongsTo, HasOne, and HasMany relation generates an
   extra closure-based method alongside the direct setter. These methods exist
   only to serve the `create!` macro's expansion — user code rarely calls them
   directly.

## Proposal

Route nested create builder construction through the model's existing fields
infrastructure instead of generating `with_$field` methods.

### Core idea

Each relation field accessor on the fields struct already knows its target model
type. Add a `create()` method to `ManyField` and `OneField` that returns the
appropriate builder type, so the macro can expand to:

```rust
// BelongsTo / HasOne nested create:
User::fields().profile().create()

// HasMany nested create:
User::fields().todos().create_many()
```

The `create!` macro expansion changes from closure-based `with_$field` calls to
field-path-based builder construction.

### Typed creation expansion

```rust
// Input:
toasty::create!(User {
    name: "Carl",
    todos: [{ title: "first" }, { title: "second" }],
})

// Current expansion:
User::create()
    .name("Carl")
    .with_todos(|b| b.with_item(|b| b.title("first")).with_item(|b| b.title("second")))

// Proposed expansion:
User::create()
    .name("Carl")
    .todos(
        User::fields().todos().create_many()
            .with_item(|b| b.title("first"))
            .with_item(|b| b.title("second"))
    )
```

For a BelongsTo nested struct:

```rust
// Input:
toasty::create!(Todo {
    title: "buy milk",
    user: { name: "Carl" },
})

// Current expansion:
Todo::create()
    .title("buy milk")
    .with_user(|b| b.name("Carl"))

// Proposed expansion:
Todo::create()
    .title("buy milk")
    .user(User::fields().user().create().name("Carl"))
```

Wait — that doesn't quite work. The macro sees field name `user` but doesn't
know the relation's model type at macro expansion time. The macro is purely
syntactic; it doesn't resolve types.

This is exactly why `with_$field` uses closures today — the closure parameter
type is inferred from the method signature, so the macro never needs to name
the related model's create builder.

### Revised approach: use the fields struct for builder construction

The macro doesn't know the target model type, but it does know the *source*
model type (from the `create!` invocation). The fields struct accessor for a
relation field returns a typed `ManyField<Origin>` or `OneField<Origin>`, and
the relation type information is embedded in those types.

Add a `create()` method to `OneField` and a `create_many()` method to
`ManyField`:

```rust
// Generated on OneField<__Origin> (for BelongsTo/HasOne):
impl<__Origin> OneField<__Origin> {
    pub fn create(self) -> <TargetRelation as Relation>::Create {
        Default::default()
    }
}

// Generated on ManyField<__Origin> (for HasMany):
impl<__Origin> ManyField<__Origin> {
    pub fn create_many(self) -> CreateMany<<TargetRelation as Relation>::Model> {
        CreateMany::new()
    }
}
```

The macro expansion becomes:

```rust
// BelongsTo nested struct:
// Input:  user: { name: "Carl" }
// Output: .user(Todo::fields().user().create().name("Carl"))

// HasMany nested list:
// Input:  todos: [{ title: "first" }, { title: "second" }]
// Output: .todos(User::fields().todos().create_many().with_item(|b| b.title("first")).with_item(|b| b.title("second")))
```

The macro knows the root model type (e.g., `Todo`, `User`) from the
`CreateItem::Typed { path, .. }` variant, so it can emit
`<path>::fields().<field_name>().create()`.

### Scoped creation

Scoped creation (`create!(in user.todos() { title: "..." })`) does not use
`with_$field` at all today — it expands to `user.todos().create().title("...")`.
The scope expression is opaque to the macro.

The problem arises when a scoped create has *nested* relation fields:

```rust
// Input:
toasty::create!(in user.todos() {
    title: "buy milk",
    steps: [{ description: "go to store" }],
})

// Current expansion:
user.todos().create()
    .title("buy milk")
    .with_steps(|b| b.with_item(|b| b.description("go to store")))
```

Here, the macro doesn't know the model type — it only has the expression
`user.todos()`. It can't emit `Todo::fields().steps().create_many()` because it
doesn't know `Todo` is the model.

#### Solution: a `Fields` trait

Introduce a trait that any scope or create builder can implement to expose the
fields struct:

```rust
pub trait HasFields {
    type Fields;
    fn fields() -> Self::Fields;
}
```

The `#[derive(Model)]` macro implements this for the model itself:

```rust
impl HasFields for User {
    type Fields = UserFields<User>;
    fn fields() -> UserFields<User> {
        User::fields()  // delegates to the existing method
    }
}
```

And for the create builder:

```rust
impl HasFields for UserCreate {
    type Fields = UserFields<User>;
    fn fields() -> UserFields<User> {
        User::fields()
    }
}
```

The macro can then resolve fields from the create builder's type without knowing
the model name:

```rust
// For scoped creation, the macro has a create builder (from .create()):
//   let builder = user.todos().create();
// The builder type is TodoCreate, which implements HasFields.

// Nested field expansion uses the builder type to get fields:
//   <_ as HasFields>::fields().steps().create_many()
```

But the macro can't reference the builder's type either — it only has a chain of
method calls. We need a different approach.

#### Revised solution: `create_$relation` methods on the create builder

Instead of routing through the fields struct, generate lightweight factory
methods on the create builder itself:

```rust
impl TodoCreate {
    // Existing direct setter (unchanged):
    pub fn steps(mut self, steps: impl IntoExpr<List<Step>>) -> Self { ... }

    // Factory method — returns a CreateMany for the relation:
    pub fn create_steps() -> CreateMany<Step> {
        CreateMany::new()
    }
}
```

This has the same name-conflict risk as `with_$field` — a model with a field
named `create_steps` would collide. However, `create_` is a less common prefix
than `with_` for field names in practice.

#### Better revised solution: associated function on the fields struct accessor

Put the factory on the field accessor type itself, which lives in the relation's
module namespace and cannot conflict with user field names:

```rust
// In the Todo relation module:
impl<__Origin> ManyField<__Origin> {
    pub fn create_many(&self) -> CreateMany<Step> {
        CreateMany::new()
    }
}

impl<__Origin> OneField<__Origin> {
    pub fn create(&self) -> <TargetRelation as Relation>::Create {
        Default::default()
    }
}
```

For typed creates, the macro emits `<RootModel>::fields().<field>().create()`.

For scoped creates, the macro needs a way to get the fields struct from the
scope expression. Since `user.todos().create()` returns a `TodoCreate`, and we
know it implements `IntoInsert<Model = Todo>`, we can add:

```rust
pub trait IntoInsert {
    type Model: Model;
    fn into_insert(self) -> Insert<Self::Model>;

    // New: get the fields struct for the target model
    fn model_fields() -> <Self::Model as HasFields>::Fields
    where
        Self::Model: HasFields,
    {
        <Self::Model as HasFields>::fields()
    }
}
```

But the macro doesn't have the builder type available syntactically either.

#### Pragmatic solution for scoped creates

The macro expansion for scoped creates builds incrementally. When expanding
`create!(in user.todos() { title: "buy milk", steps: [{ description: "..." }] })`,
the macro currently produces:

```rust
user.todos().create().title("buy milk").with_steps(|b| ...)
```

The simplest field-based replacement is to keep `with_item` closures for the
*innermost* nesting (since `CreateMany::with_item` already uses closures and
those don't conflict), and only eliminate `with_$field` on the create builder.

For the create builder, the macro can use a helper trait:

```rust
/// Implemented by create builders. Provides access to the model's fields struct
/// for constructing nested builders without naming the model type.
pub trait CreateBuilder: IntoInsert {
    type Fields;
    fn builder_fields() -> Self::Fields;
}
```

Generated for each model:

```rust
impl CreateBuilder for TodoCreate {
    type Fields = TodoFields<Todo>;
    fn builder_fields() -> TodoFields<Todo> {
        Todo::fields()
    }
}
```

The macro expansion for nested relations in scoped creates would then use a
turbofish-free approach. Since the builder is always the last expression in a
chain, the macro can bind it to a variable:

```rust
// Scoped create with nested relation:
{
    let __builder = user.todos().create().title("buy milk");
    let __steps = <_ as CreateBuilder>::builder_fields(&__builder).steps().create_many()
        .with_item(|b| b.description("go to store"));
    __builder.steps(__steps)
}
```

Wait — `builder_fields` shouldn't take `&self` since it's just an associated
function. But we need the compiler to infer the type. We can use a helper
function:

```rust
// In toasty crate:
pub fn builder_fields<B: CreateBuilder>(_builder: &B) -> B::Fields {
    B::builder_fields()
}
```

Then the macro emits:

```rust
{
    let __builder = user.todos().create().title("buy milk");
    let __steps = toasty::builder_fields(&__builder).steps().create_many()
        .with_item(|b| b.description("go to store"));
    __builder.steps(__steps)
}
```

This works: `builder_fields(&__builder)` infers `B = TodoCreate`, returns
`TodoFields<Todo>`, and `.steps().create_many()` returns `CreateMany<Step>`.

## Proposed Design

### New trait: `CreateBuilder`

```rust
/// Implemented by generated create builder types. Provides access to the
/// model's fields struct for constructing nested relation builders.
pub trait CreateBuilder: IntoInsert {
    type Fields;
    fn builder_fields() -> Self::Fields;
}
```

### New helper function: `builder_fields`

```rust
/// Returns the fields struct for a create builder's model. Used by the
/// `create!` macro to resolve relation types without naming the model.
pub fn builder_fields<B: CreateBuilder>(_: &B) -> B::Fields {
    B::builder_fields()
}
```

### New methods on `ManyField` and `OneField`

Generated in `expand/relation.rs`:

```rust
impl<__Origin> ManyField<__Origin> {
    // Existing methods...

    /// Construct a `CreateMany` for building nested HasMany items.
    pub fn create_many(self) -> CreateMany<<TargetRelation as Relation>::Model> {
        CreateMany::new()
    }
}

impl<__Origin> OneField<__Origin> {
    // Existing methods...

    /// Construct the create builder for the related model.
    pub fn create(self) -> <TargetRelation as Relation>::Create {
        Default::default()
    }
}
```

### Macro expansion changes

#### Typed creation with nested BelongsTo

```rust
// Input:
toasty::create!(Todo { title: "buy milk", user: { name: "Carl" } })

// Expansion:
{
    let __builder = Todo::create().title("buy milk");
    let __user = Todo::fields().user().create().name("Carl");
    __builder.user(__user)
}
```

The macro knows the root type (`Todo`) and the field name (`user`), so it emits
`Todo::fields().user().create()` to get a `UserCreate` builder without naming
`User`.

#### Typed creation with nested HasMany

```rust
// Input:
toasty::create!(User { name: "Carl", todos: [{ title: "first" }, { title: "second" }] })

// Expansion:
{
    let __builder = User::create().name("Carl");
    let __todos = User::fields().todos().create_many()
        .with_item(|b| b.title("first"))
        .with_item(|b| b.title("second"));
    __builder.todos(__todos)
}
```

#### Scoped creation with nested relations

```rust
// Input:
toasty::create!(in user.todos() { title: "buy milk", steps: [{ desc: "go to store" }] })

// Expansion:
{
    let __builder = user.todos().create().title("buy milk");
    let __steps = toasty::builder_fields(&__builder).steps().create_many()
        .with_item(|b| b.description("go to store"));
    __builder.steps(__steps)
}
```

The `builder_fields` helper infers the create builder type from `&__builder` and
returns the correct fields struct.

#### Flat creates (no nesting)

Flat creates are unchanged. No `with_$field` methods are involved:

```rust
// Input:
toasty::create!(User { name: "Carl" })

// Expansion (same as today):
User::create().name("Carl")
```

#### Deeply nested creates

For deeper nesting (e.g., a nested BelongsTo that itself has a nested HasMany),
the macro applies the pattern recursively:

```rust
// Input:
toasty::create!(Todo {
    title: "buy milk",
    user: {
        name: "Carl",
        profile: { bio: "hi" },
    },
})

// Expansion:
{
    let __builder = Todo::create().title("buy milk");
    let __user = {
        let __builder = Todo::fields().user().create().name("Carl");
        let __profile = toasty::builder_fields(&__builder).profile().create().bio("hi");
        __builder.profile(__profile)
    };
    __builder.user(__user)
}
```

Each nesting level introduces a new block. The inner `__builder` shadows the
outer one, which is fine since each block is self-contained.

### `CreateMany::with_item` closures stay

`CreateMany::with_item` uses closures (`FnOnce(M::Create) -> M::Create`) and
lives in the `toasty` crate, not in generated code. It has no name-conflict
risk. The `with_item` pattern stays as-is.

If a nested list item itself contains nested relations, the macro uses
`builder_fields` inside the closure:

```rust
// Input:
toasty::create!(User {
    name: "Carl",
    todos: [{ title: "first", category: { name: "work" } }],
})

// Expansion:
{
    let __builder = User::create().name("Carl");
    let __todos = User::fields().todos().create_many()
        .with_item(|b| {
            let __category = toasty::builder_fields(&b).category().create().name("work");
            b.title("first").category(__category)
        });
    __builder.todos(__todos)
}
```

## What gets removed

1. **`with_ident` field** on `codegen::schema::Field` (`schema/field.rs:38`).
   No longer needed — the `with_$field` identifier is not generated.

2. **`with_$field` closure methods** on create builders. The `expand_create_methods`
   function in `expand/create.rs` drops the `#with_ident` arms for BelongsTo
   (lines 145–150), HasMany (lines 170–175), and HasOne (lines 189–194).

3. **`with_name` field** on `FieldEntry` in the macro parser
   (`create/parse.rs:35`). The macro no longer computes or uses `with_$field`
   identifiers.

4. **Closure-based expansion** in `create/expand.rs` for `FieldValue::Single`
   and `FieldValue::List` (lines 47–54). Replaced with field-path-based builder
   construction.

## What gets added

1. **`CreateBuilder` trait** in `toasty/src/stmt/` or `toasty/src/lib.rs`.

2. **`builder_fields` helper function** in `toasty/src/lib.rs`.

3. **`CreateBuilder` impl** generated for each model's create builder in
   `expand/create.rs`.

4. **`create()` method on `OneField`** and **`create_many()` method on
   `ManyField`** in `expand/relation.rs`.

5. **Updated macro expansion** in `create/expand.rs` that emits block-scoped
   variable bindings with field-path-based builder construction.

## Migration

This is a breaking change for any user code that calls `with_$field` methods
directly (outside of `create!`). Such calls are uncommon — the methods exist
primarily to serve the macro — but they are public API.

Users calling `with_user(|b| b.name("Carl"))` directly would migrate to:

```rust
// Before:
Todo::create().title("buy milk").with_user(|b| b.name("Carl"))

// After:
let user = Todo::fields().user().create().name("Carl");
Todo::create().title("buy milk").user(user)
```

The `create!` macro syntax is unchanged. Only the expansion changes, so all
existing `create!` invocations continue to work without modification.

## Open Questions

1. **Naming: `create()` vs `build()` on field accessors.** `create()` on
   `OneField` might be confused with the model's `create()` associated function,
   but they return the same type. `build()` is an alternative but less clear
   about intent.

2. **`CreateMany::with_item` and deeply nested relations.** The `builder_fields`
   approach works inside `with_item` closures because the closure parameter `b`
   has a known type (`M::Create`). But this introduces a let-binding inside
   the closure, making the expansion slightly more complex. An alternative is to
   add `with_item_fields` to `CreateMany` that passes both the builder and a
   fields struct to the closure.

3. **Scoped batch creates.** `create!(in user.todos()::[{ ... }, { ... }])` is
   not currently supported. If added later, the `builder_fields` approach
   extends naturally — each item in the batch can use the same pattern.
