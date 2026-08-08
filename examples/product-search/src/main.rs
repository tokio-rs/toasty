//! product-search: a storefront catalog/search endpoint — filter, sort, page, and project,
//! the operations a product list or search screen runs on every request.
//!
//! Run it cold (`cargo run -p example-product-search`). Uses in-memory SQLite by default;
//! set `TOASTY_CONNECTION_URL` for another backend. `.like()` is SQL-only; the portable
//! prefix matcher `.starts_with()` works everywhere (see the note inline).

#[derive(Debug, toasty::Model)]
// A composite index over (category, price). It also generates the leftmost-prefix lookup
// `filter_by_category`, so `category` needs no separate field-level `#[index]` — adding one
// would emit duplicate methods and fail to compile.
#[index(category, price)]
struct Product {
    #[key]
    #[auto]
    id: uuid::Uuid,
    name: String,
    // An open, extensible taxonomy — new categories appear without code changes — so this is a
    // `String` (a real app might make it its own model). Contrast a small *closed* set of
    // states, which is better modeled as an embedded enum (see store-operations' OrderStatus).
    category: String,
    price: i64, // cents
    in_stock: bool,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let url =
        std::env::var("TOASTY_CONNECTION_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let mut db = toasty::Db::builder()
        .models(toasty::models!(crate::*))
        .connect(&url)
        .await?;
    db.push_schema().await?;

    // --- seed: create_many bulk-inserts many rows of ONE model in one round-trip -----
    // `.item(..)` takes a finished create builder; `.with_item(|c| ..)` configures one inline.
    Product::create_many()
        .item(toasty::create!(Product {
            name: "Pro Keyboard",
            category: "input",
            price: 7999,
            in_stock: true,
        }))
        .with_item(|c| c.name("Mouse").category("input").price(2999).in_stock(true))
        .with_item(|c| {
            c.name("4K Monitor")
                .category("display")
                .price(39999)
                .in_stock(true)
        })
        .with_item(|c| {
            c.name("Webcam")
                .category("input")
                .price(4999)
                .in_stock(false)
        })
        .with_item(|c| {
            c.name("USB Cable")
                .category("input")
                .price(999)
                .in_stock(true)
        })
        .exec(&mut db)
        .await?;

    // --- filtering: predicates built from fields() accessors -------------------------
    // Precedence is LEFT-TO-RIGHT method chaining, NOT SQL precedence: `a.or(b).and(c)`
    // parses as `(a OR b) AND c`. Group differently by nesting the arguments.
    let cheap_input = Product::filter(
        Product::fields()
            .category()
            .eq("input")
            .and(Product::fields().price().lt(3000)),
    )
    .exec(&mut db)
    .await?;
    println!("input items under $30: {}", cheap_input.len());

    let two_cats = Product::filter(Product::fields().category().in_list(["input", "display"]))
        .exec(&mut db)
        .await?;
    println!("items in input or display: {}", two_cats.len());

    // `starts_with` is a portable, case-sensitive prefix match (works on every backend).
    let pro = Product::filter(Product::fields().name().starts_with("Pro"))
        .exec(&mut db)
        .await?;
    println!("names starting with \"Pro\": {}", pro.len());
    // `.like()` is SQL-only and its case-sensitivity varies per backend (postgres-directory
    // demonstrates PostgreSQL's case-insensitive `.ilike()`).
    let cables = Product::filter(Product::fields().name().like("%Cable%"))
        .exec(&mut db)
        .await?;
    println!("names matching %Cable%: {}", cables.len());

    // --- generated lookups from the composite index ----------------------------------
    // One method per leftmost prefix: `filter_by_category` and `filter_by_category_and_price`.
    let input = Product::filter_by_category("input").exec(&mut db).await?;
    let exact = Product::filter_by_category_and_price("input", 2999)
        .exec(&mut db)
        .await?;
    println!(
        "category=input: {}; category=input & price=2999: {}",
        input.len(),
        exact.len()
    );

    // --- sorting ---------------------------------------------------------------------
    // A tuple is a multi-field sort: (primary, tie-breaker).
    let sorted = Product::all()
        .order_by((
            Product::fields().price().asc(),
            Product::fields().name().desc(),
        ))
        .exec(&mut db)
        .await?;
    let names: Vec<&str> = sorted.iter().map(|p| p.name.as_str()).collect();
    println!("cheapest first: {names:?}");

    // --- limit & offset --------------------------------------------------------------
    // `offset` requires a `limit` first. Fine for small data; doesn't scale (use a cursor).
    let page2 = Product::all()
        .order_by(Product::fields().price().asc())
        .limit(2)
        .offset(2)
        .exec(&mut db)
        .await?;
    let page2_names: Vec<&str> = page2.iter().map(|p| p.name.as_str()).collect();
    println!("rows 3-4 by ascending price: {page2_names:?}");

    // --- cursor pagination (requires order_by) ---------------------------------------
    // `per_page` is an UPPER BOUND — walk `.next()` to the end rather than counting rows.
    let mut page = Product::all()
        .order_by(Product::fields().price().desc())
        .paginate(2)
        .exec(&mut db)
        .await?;
    let mut total = 0;
    loop {
        total += page.len();
        match page.next(&mut db).await? {
            Some(next) => page = next,
            None => break,
        }
    }
    println!("cursor-paginated through {total} products in pages of 2");

    // --- projection: read only the columns you need ----------------------------------
    // The return type follows the projection: one field -> `Vec<Field>`, a tuple -> `Vec<(..)>`.
    let just_names: Vec<String> = Product::all()
        .select(Product::fields().name())
        .exec(&mut db)
        .await?;
    let id_price: Vec<(uuid::Uuid, i64)> = Product::all()
        .select((Product::fields().id(), Product::fields().price()))
        .exec(&mut db)
        .await?;
    println!(
        "projected {} names and {} (id, price) pairs",
        just_names.len(),
        id_price.len()
    );

    Ok(())
}
