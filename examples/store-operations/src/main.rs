//! store-operations: a store's checkout and maintenance backend, and the home for choosing
//! the right WRITE tool — an interactive transaction, a batch, a query-update, or raw SQL.
//!
//! Run it cold (`cargo run -p example-store-operations`). In-memory SQLite by default; set
//! `TOASTY_CONNECTION_URL` for another SQL backend (interactive transactions are SQL-only).

// IsolationLevel/TransactionMode are NOT re-exported from `toasty` — import them from toasty-core.
use toasty_core::driver::operation::{IsolationLevel, TransactionMode};

#[derive(Debug, toasty::Model)]
struct Account {
    #[key]
    #[auto]
    id: uuid::Uuid,
    #[unique]
    owner: String,
    #[default(0)]
    balance_cents: i64,
}

// A fixed set of order states — an embedded enum, not a `String`, so typos are compile errors.
// With no `#[column(variant)]`, each variant is stored as its snake_case name ("open", "paid",
// ...) in one column, which also keeps the raw-SQL GROUP BY below human-readable.
#[derive(Debug, PartialEq, toasty::Embed)]
enum OrderStatus {
    Open,
    Paid,
    Stale,
    Cancelled,
}

#[derive(Debug, toasty::Model)]
#[index(status)] // index the enum's discriminant column for status lookups
struct Order {
    #[key]
    #[auto]
    id: uuid::Uuid,
    status: OrderStatus,
    total_cents: i64,
}

// Order line items, stored partitioned by their order via a composite (partition, local) key:
// every item for one order shares a partition, and `id` distinguishes items within it.
#[derive(Debug, toasty::Model)]
#[key(partition = order_id, local = id)]
struct OrderItem {
    order_id: uuid::Uuid,
    #[auto]
    id: uuid::Uuid,
    sku: String,
    qty: i64,
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

    let buyer = toasty::create!(Account {
        owner: "buyer",
        balance_cents: 5_000
    })
    .exec(&mut db)
    .await?;
    let seller = toasty::create!(Account { owner: "seller" })
        .exec(&mut db)
        .await?;

    // --- interactive transaction: read, branch, then write — atomically --------------
    // Reach for a transaction (not a batch) when a write depends on a value you just read.
    let order_total = 3_000;
    let mut tx = db.transaction().await?;
    let mut buyer_acct = Account::get_by_id(&mut tx, &buyer.id).await?;
    if buyer_acct.balance_cents < order_total {
        // `Error::from_args` is the real constructor (there is no `Error::msg`). Returning Err
        // drops `tx`, which auto-rolls-back — no explicit rollback needed.
        return Err(toasty::Error::from_args(format_args!("insufficient funds")));
    }
    // Relative updates are atomic against the stored value — no read-modify-write race.
    toasty::update!(buyer_acct { balance_cents.subtract(order_total) })
        .exec(&mut tx)
        .await?;
    let mut seller_acct = Account::get_by_id(&mut tx, &seller.id).await?;
    toasty::update!(seller_acct { balance_cents.add(order_total) })
        .exec(&mut tx)
        .await?;
    let order = toasty::create!(Order {
        status: OrderStatus::Paid,
        total_cents: order_total
    })
    .exec(&mut tx)
    .await?;

    // A nested transaction is a SAVEPOINT: a tentative coupon we roll back without losing
    // the order recorded in the outer transaction.
    {
        let mut coupon = tx.transaction().await?;
        toasty::update!(buyer_acct { balance_cents.add(500) })
            .exec(&mut coupon)
            .await?;
        coupon.rollback().await?; // discard the coupon; the outer tx (and the order) survive
    }
    tx.commit().await?;
    println!(
        "checkout committed: order {} is {:?}",
        order.id, order.status
    );

    // --- composite (partition/local) key: line items live under their order ----------
    OrderItem::create_many()
        .with_item(|c| c.order_id(order.id).sku("WIDGET").qty(2))
        .with_item(|c| c.order_id(order.id).sku("GADGET").qty(1))
        .exec(&mut db)
        .await?;
    // The partition key alone is a cheap, scoped lookup of one order's items.
    let items = OrderItem::filter_by_order_id(order.id)
        .exec(&mut db)
        .await?;
    println!("order has {} line items", items.len());

    // --- transaction options: isolation and lock-mode are separate axes --------------
    // SQLite supports only Serializable isolation; Immediate is a SQLite lock-timing mode
    // (PostgreSQL/MySQL reject it with Error::UnsupportedFeature).
    let mut snap = db
        .transaction_builder()
        .isolation(IsolationLevel::Serializable)
        .mode(TransactionMode::Immediate)
        .begin()
        .await?;
    // Compare an enum field against a variant with `.eq(..)`.
    let _paid = Order::filter(Order::fields().status().eq(OrderStatus::Paid))
        .exec(&mut snap)
        .await?;
    snap.commit().await?;

    // --- batch: one atomic round-trip for heterogeneous creates ----------------------
    // A tuple batch preserves shape: each element is a single create, so you get a tuple back.
    let (_warehouse, _draft): (Account, Order) = toasty::batch((
        toasty::create!(Account { owner: "warehouse" }),
        toasty::create!(Order {
            status: OrderStatus::Open,
            total_cents: 100
        }),
    ))
    .exec(&mut db)
    .await?;

    // For MANY rows of ONE model, create_many is the bulk-insert tool.
    let opened: Vec<Order> = Order::create_many()
        .with_item(|c| c.status(OrderStatus::Open).total_cents(200))
        .with_item(|c| c.status(OrderStatus::Open).total_cents(300))
        .exec(&mut db)
        .await?;
    println!("opened {} more orders", opened.len());

    // --- bulk update / delete by query: no rows are loaded first ---------------------
    toasty::update!(Order::filter(Order::fields().status().eq(OrderStatus::Open)) { status: OrderStatus::Stale })
        .exec(&mut db)
        .await?;
    Order::filter(Order::fields().status().eq(OrderStatus::Cancelled))
        .delete()
        .exec(&mut db)
        .await?;

    // --- instance delete (consumes self) ---------------------------------------------
    if let Some(order) = Order::filter(Order::fields().status().eq(OrderStatus::Stale))
        .first()
        .exec(&mut db)
        .await?
    {
        order.delete().exec(&mut db).await?; // takes `self`
    }

    // --- raw SQL for an aggregate the builder doesn't express -------------------------
    // `query()` returns `Vec<Value>`, each row a `Value::Record` in selected-column order.
    // Because `status` stores as its snake_case variant name, raw SQL sees readable values.
    let rows = toasty::sql::query("SELECT status, COUNT(*) FROM orders GROUP BY status")
        .exec(&mut db)
        .await?;
    println!("orders grouped by status:");
    for row in rows {
        let toasty::stmt::Value::Record(cols) = row else {
            unreachable!()
        };
        println!("  status={:?} count={:?}", cols[0], cols[1]);
    }

    Ok(())
}
