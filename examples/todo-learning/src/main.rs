use example_todo_learning::Item;

#[tokio::main]
async fn main() -> toasty::Result<()> {
    // Initialize tracing to see the query engine pipeline
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Create database connection
    let mut db = toasty::Db::builder()
        .register::<Item>()
        .connect("sqlite::memory:")
        .await?;

    // Push the schema to the database (creates tables)
    db.push_schema().await?;

    println!("==> Inserting 100 items...");

    // Insert 100 items with sequential order values
    for i in 0..100 {
        Item::create().order(i).exec(&mut db).await?;
    }

    println!("Inserted 100 items");

    println!("\n==> First page (paginate with 10 items, descending order)...");

    // First page: should get items 90-99 (order descending)
    let items = Item::all()
        .order_by(Item::fields().order().desc())
        .paginate(10)
        .exec(&mut db)
        .await?;

    println!("First page: got {} items", items.len());
    for (i, item) in items.iter().enumerate() {
        println!("  [{i}] order={}", item.order);
    }

    // Verify we got the right items
    assert_eq!(items.len(), 10);
    for (i, expected_order) in (90..100).rev().enumerate() {
        assert_eq!(items[i].order, expected_order, "Item {i} has wrong order");
    }
    println!("✓ First page correct");

    println!("\n==> Next page...");

    // Get next page
    let items = items.next(&mut db).await?;
    if let Some(items) = items {
        println!("Next page: got {} items", items.len());
        for (i, item) in items.iter().enumerate() {
            println!("  [{i}] order={}", item.order);
        }

        // Verify we got items 80-89
        assert_eq!(items.len(), 10);
        for (i, expected_order) in (80..90).rev().enumerate() {
            assert_eq!(items[i].order, expected_order, "Item {i} has wrong order");
        }
        println!("✓ Next page correct");
    } else {
        panic!("Expected next page but got None!");
    }

    println!("\n>>> Success! <<<");

    Ok(())
}
