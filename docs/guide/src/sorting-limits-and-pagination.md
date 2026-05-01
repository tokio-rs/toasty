# Sorting, Limits, and Pagination

Queries return results in an unspecified order by default. Use `.order_by()` to
sort results, `.limit()` and `.offset()` to restrict the result set, and
`.paginate()` to walk through results one page at a time.

## Sorting with `order_by`

Call `.order_by()` on a query with a field path and a direction — `.asc()` for
ascending or `.desc()` for descending:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Item {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     order: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Ascending: smallest order first
let items = Item::all()
    .order_by(Item::fields().order().asc())
    .exec(&mut db)
    .await?;

// Descending: largest order first
let items = Item::all()
    .order_by(Item::fields().order().desc())
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

`Item::fields().order()` returns a field path. Calling `.asc()` or `.desc()` on
it produces an ordering expression that `.order_by()` accepts.

Sorting works with filters too:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Item {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     order: i64,
#     category: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let items = Item::filter(Item::fields().category().eq("books"))
    .order_by(Item::fields().order().desc())
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

## Limiting results

`.limit(n)` caps the number of records returned:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Item {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     order: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// At most 5 items
let items = Item::all().limit(5).exec(&mut db).await?;
# Ok(())
# }
```

If the query matches fewer records than the limit, all matching records are
returned.

`.limit(n)` is an upper bound, not a guarantee. Toasty applies the limit to
the database query, but it may filter the returned rows further before
producing the final result set. When that happens, a query can return fewer
than `n` records even if more than `n` rows match the filter expression. Use
[cursor-based pagination](#cursor-based-pagination) to walk every matching
record.

Combine `.order_by()` with `.limit()` to get the top or bottom N records:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Item {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     order: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Top 7 items by order (highest first)
let items = Item::all()
    .order_by(Item::fields().order().desc())
    .limit(7)
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

## Offset

`.offset(n)` skips the first `n` results. It requires `.limit()` to be called
first:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Item {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     order: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Skip the first 5, then return the next 7
let items = Item::all()
    .order_by(Item::fields().order().asc())
    .limit(7)
    .offset(5)
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

Limit and offset work for simple cases, but cursor-based pagination (below) is a
better fit for paging through large result sets. Offset-based pagination gets
slower as the offset increases because the database still reads and discards the
skipped rows. It can also produce inconsistent results when rows are inserted or
deleted between page fetches. See Markus Winand's
["No Offset"](https://use-the-index-luke.com/no-offset) for an in-depth
explanation.

## Cursor-based pagination

`.paginate(per_page)` splits results into pages. It requires `.order_by()` and
returns a `Page` instead of a `Vec`:

```rust
# use toasty::Model;
# use toasty::stmt::Page;
# #[derive(Debug, toasty::Model)]
# struct Item {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     order: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let page: Page<_> = Item::all()
    .order_by(Item::fields().order().desc())
    .paginate(10)
    .exec(&mut db)
    .await?;

// Access items in the page
for item in page.iter() {
    println!("order: {}", item.order);
}

// Check how many items are in this page
println!("items: {}", page.len());
# Ok(())
# }
```

A `Page` dereferences to a slice, so you can index into it, iterate over it, and
call slice methods like `.len()` and `.iter()` directly.

`per_page` is an upper bound on the page size, not a guarantee. Toasty
applies it to the database query, but it may filter the returned rows further
before producing the page. A page can therefore contain fewer than `per_page`
items even when more results exist. Check `.has_next()` (or follow
`.next()` until it returns `None`) to detect the end of the result set rather
than relying on the size of any individual page.

### Navigating pages

`Page` provides `.next()` and `.prev()` methods that fetch the next or previous
page. Both return `Option<Page>` — `None` when there are no more results in that
direction:

```rust
# use toasty::Model;
# use toasty::stmt::Page;
# #[derive(Debug, toasty::Model)]
# struct Item {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     order: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let first_page: Page<_> = Item::all()
    .order_by(Item::fields().order().asc())
    .paginate(10)
    .exec(&mut db)
    .await?;

// Move to the next page
if let Some(second_page) = first_page.next(&mut db).await? {
    println!("page 2 has {} items", second_page.len());

    // Go back
    if let Some(back) = second_page.prev(&mut db).await? {
        println!("back to page 1: {} items", back.len());
    }
}
# Ok(())
# }
```

Use `.has_next()` and `.has_prev()` to check whether more pages exist without
fetching them:

```rust,ignore
if page.has_next() {
    let next = page.next(&mut db).await?.unwrap();
}
```

### Walking all pages

To process every record in pages:

```rust
# use toasty::Model;
# use toasty::stmt::Page;
# #[derive(Debug, toasty::Model)]
# struct Item {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     order: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let mut page: Page<_> = Item::all()
    .order_by(Item::fields().order().asc())
    .paginate(10)
    .exec(&mut db)
    .await?;

loop {
    for item in page.iter() {
        println!("order: {}", item.order);
    }

    match page.next(&mut db).await? {
        Some(next) => page = next,
        None => break,
    }
}
# Ok(())
# }
```

### Starting from a cursor position

Use `.after()` to start pagination after a specific value in the sort field:

```rust
# use toasty::Model;
# use toasty::stmt::Page;
# #[derive(Debug, toasty::Model)]
# struct Item {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     order: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Start after order=90 (descending), so the first item will be order=89
let page: Page<_> = Item::all()
    .order_by(Item::fields().order().desc())
    .paginate(10)
    .after(90)
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

The value passed to `.after()` corresponds to the field used in `.order_by()`.

## Method summary

Methods available on query builders:

| Method | Description |
|---|---|
| `.order_by(field.asc())` | Sort ascending by field |
| `.order_by(field.desc())` | Sort descending by field |
| `.limit(n)` | Return at most `n` records |
| `.offset(n)` | Skip first `n` records (requires `.limit()`) |
| `.paginate(per_page)` | Cursor-based pagination (requires `.order_by()`) |

Methods available on `Page`:

| Method | Returns | Description |
|---|---|---|
| `.next(&mut db)` | `Result<Option<Page>>` | Fetch next page |
| `.prev(&mut db)` | `Result<Option<Page>>` | Fetch previous page |
| `.has_next()` | `bool` | Whether a next page exists |
| `.has_prev()` | `bool` | Whether a previous page exists |
| `.items` | `Vec<M>` | The records in this page |
| `.len()` | `usize` | Number of items (via `Deref` to slice) |
| `.iter()` | iterator | Iterate items (via `Deref` to slice) |
