# Markdown

The Markdown driver presents a directory of `.md` files as typed, read-only
Toasty models. It loads and validates the selected files when `Db` is built,
then answers queries from one immutable in-memory snapshot.

## Enabling the driver

Enable the `markdown` feature to connect with a URL:

```toml
[dependencies]
toasty = { version = "{{toasty_version}}", features = ["markdown"] }
```

```rust,ignore
let mut db = toasty::Db::builder()
    .models(toasty::models!(Post))
    .connect("markdown:./content")
    .await?;
```

Add `toasty-driver-markdown` as a dependency and construct `Markdown` directly
when the content mapping needs configuration.

## Default mapping

Each immediate child directory matches a database table name. Each lowercase
`.md` file directly in that directory is one row. YAML front-matter keys match
database columns, the remaining text maps to a string column named `body`, and
a missing single-string primary key comes from the filename without `.md`.

```text
content/
  posts/
    hello-world.md
```

```rust,ignore
#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    slug: String,
    title: String,
    published: bool,
    body: String,
}
```

```markdown
---
title: Hello, world
published: true
---
# Hello, world
```

This file has the key `hello-world`. Front matter can supply the key explicitly
instead. Missing optional fields become `None`; missing required fields and
values that do not match the compiled column type make `Db::build` fail.

## Configuring a table

`Markdown::builder` can change a table directory, rename front-matter keys,
select or disable the body column, derive one string column from the file stem
or relative path, and enable recursive file discovery:

```rust,ignore
use toasty_driver_markdown::{Markdown, Table};

let driver = Markdown::builder("content")
    .table(
        "posts",
        Table::new("articles")
            .column("date", "published_at")
            .body_column("markdown")
            .key_from_relative_path("slug")
            .recursive(true),
    )
    .strict(true)
    .build();

let mut db = toasty::Db::builder()
    .models(toasty::models!(Post))
    .build(driver)
    .await?;
```

Relative-path keys omit `.md` and always use `/` separators. Strict mode
rejects unknown root directories, unknown front-matter keys, nested directories
without recursive discovery, and non-empty bodies without a body mapping.
The loader skips symbolic links and rejects configured paths that escape the
content root.

## Queries and snapshots

Primary-key reads, filters, relations, ordering, limits, offsets, and cursor
pagination use Toasty's normal model API. The in-memory backend implements
`between`, `starts_with`, `like`, `ilike`, collection predicates, and document
path predicates. `ilike` uses locale-independent Unicode case folding.

The snapshot includes front matter and bodies. Filesystem changes are not
visible after `Db::build` returns; build another `Db` to load them.

The driver reports `db.capability().data_mutations == false`. Create, update,
and delete statements return `Error::unsupported_feature` before the engine
plans or executes them. Schema changes, migrations, reset, and raw SQL are also
unsupported.
