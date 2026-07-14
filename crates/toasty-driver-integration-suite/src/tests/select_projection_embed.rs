//! `.select(...)` projection through embedded sub-fields. Field paths
//! into an embed flow through the existing `Path<T, U>: IntoExpr<U>`
//! chain, so no production code change is required for this case to
//! work end-to-end; this file is the integration coverage.

use crate::prelude::*;

#[derive(Debug, toasty::Embed)]
struct Address {
    street: String,
    city: String,
}

#[derive(Debug, toasty::Model)]
struct Customer {
    #[key]
    id: i64,
    name: String,
    address: Address,
}

#[driver_test(requires(sql))]
pub async fn select_embed_subfield(test: &mut Test) -> Result<()> {
    let mut db = test.setup_db(models!(Customer)).await;

    Customer::create()
        .id(1_i64)
        .name("Alice")
        .address(Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
        })
        .exec(&mut db)
        .await?;

    let cities: Vec<String> = Customer::all()
        .select(Customer::fields().address().city())
        .exec(&mut db)
        .await?;

    assert_eq!(cities, vec!["Springfield".to_string()]);

    Ok(())
}
