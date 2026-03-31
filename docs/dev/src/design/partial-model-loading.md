# Partial Model Loading

## Overview

Some model fields are expensive to load — a blog post's `body`, an image's
`blob`, a document's `content`. Partial model loading lets you skip these
columns by default and fetch them only when needed.

Fields opt in by using the `Deferred<T>` wrapper type. The derive macro
recognizes `Deferred<T>` the same way it recognizes `HasMany<T>` and
`BelongsTo<T>` — by the type path. No extra attribute is needed. Queries
exclude deferred columns unless the caller explicitly includes them.

```rust
#[derive(Debug, toasty::Model)]
struct Article {
    #[key]
    #[auto]
    id: u64,

    title: String,

    #[belongs_to]
    author: BelongsTo<User>,

    body: Deferred<String>,
}
```

## `Deferred<T>`

`Deferred<T>` wraps a value that may or may not have been loaded from the
database. It follows the same pattern as `HasMany<T>` and `BelongsTo<T>` —
an `Option` internally, with accessor methods that panic when the value is
absent.

```rust
pub struct Deferred<T> {
    value: Option<T>,
}

impl<T> Deferred<T> {
    /// Returns a reference to the loaded value.
    ///
    /// # Panics
    ///
    /// Panics if the field was not loaded.
    pub fn get(&self) -> &T { .. }

    /// Returns true if the field has not been loaded.
    pub fn is_unloaded(&self) -> bool { .. }

    /// Clear the loaded value, returning to the unloaded state.
    pub fn unload(&mut self) { .. }
}
```

`Deferred<T>` works with any type that can be a regular model field: primitive
types (`String`, `i64`, `bool`, etc.), embedded structs, and embedded enums.

### Debug output

When printed with `{:?}`, an unloaded `Deferred<T>` displays `<not loaded>`
instead of its inner value, matching the behavior of `HasMany` and `BelongsTo`.

## Defining Deferred Fields

Wrap the field type in `Deferred<T>`. The derive macro detects this and marks
the field as deferred in the schema:

```rust
#[derive(Debug, toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: u64,

    title: String,

    content: Deferred<String>,

    metadata: Deferred<DocMetadata>,
}
```

A model can have any number of deferred fields. Non-deferred fields are always
loaded.

## Querying

### Default: deferred fields are excluded

Standard queries skip deferred columns. The generated SQL selects only
non-deferred columns:

```rust
let articles = Article::all().collect(&db).await?;
// SELECT id, title, author_id FROM articles

for article in &articles {
    println!("{}", article.title);      // works
    println!("{}", article.body.get()); // panics: "deferred field `body` was not loaded"
}
```

### Loading deferred fields with `include`

Use `.include()` with the field path to load a deferred field, the same API used
for relation preloading:

```rust
let articles = Article::all()
    .include(Article::fields().body())
    .collect(&db).await?;
// SELECT id, title, author_id, body FROM articles

for article in &articles {
    println!("{}", article.body.get()); // works
}
```

Multiple deferred fields can be included in the same query:

```rust
let docs = Document::all()
    .include(Document::fields().content())
    .include(Document::fields().metadata())
    .collect(&db).await?;
// SELECT id, title, content, metadata FROM documents
```

### Combining with relation includes

Deferred field includes and relation includes compose freely:

```rust
let articles = Article::all()
    .include(Article::fields().body())
    .include(Article::fields().author())
    .collect(&db).await?;
```

### Finders and filters

`.include()` works on any query builder — `all()`, `filter_by_*()`,
`find_by_*()`:

```rust
let article = Article::find_by_id(42)
    .include(Article::fields().body())
    .get(&db).await?;

let articles = Article::filter_by_author_id(user.id)
    .include(Article::fields().body())
    .collect(&db).await?;
```

## Creating and Updating

### Creating records

`Deferred<T>` fields are set through the create builder like any other field.
The deferred behavior only affects reads — writes always include the column:

```rust
let article = Article::create()
    .title("Hello World")
    .body("This is the full article text.")
    .author(&user)
    .exec(&db).await?;
```

The returned model instance has all fields populated, including deferred ones,
because the values were just provided.

### Updating records

Update builders set deferred fields the same way as regular fields:

```rust
article.update()
    .body("Updated article text.")
    .exec(&db).await?;
```

An update does not need the deferred field to have been loaded first. You can
update a field without having read it.

## Checking Load State

Use `is_unloaded()` to check whether a deferred field was loaded before
accessing it:

```rust
if !article.body.is_unloaded() {
    println!("Body: {}", article.body.get());
}
```

## Nullable Deferred Fields

`Deferred<Option<T>>` represents a deferred field that can be `NULL` in the
database:

```rust
#[derive(Debug, toasty::Model)]
struct Article {
    #[key]
    #[auto]
    id: u64,

    title: String,

    subtitle: Deferred<Option<String>>,
}
```

When loaded, `.get()` returns `&Option<String>`. The outer `Deferred` tracks
whether the column was fetched; the inner `Option` tracks whether the database
value is `NULL`.
