//! crm-embedded: a contact card composed from EMBEDDED value types — a typed id, a
//! flattened address, and a tagged-union contact method. None of these gets its own table;
//! their columns are flattened into the `contacts` table.
//!
//! Run it cold (`cargo run -p example-crm-embedded`). In-memory SQLite by default; set
//! `TOASTY_CONNECTION_URL` for another backend.

// A newtype embed maps to ONE un-prefixed column. As a `#[key]` with `#[auto]`, generation
// proxies to the inner type — here a time-ordered UUID v7.
#[derive(Debug, toasty::Embed)]
struct ContactId(uuid::Uuid);

// A multi-field embed flattens into prefixed columns: `address_street`, `address_city`,
// `address_postal_code`. An `#[index]` on an embedded field indexes the flattened column.
#[derive(Debug, toasty::Embed)]
struct Address {
    street: String,
    city: String,
    #[index]
    postal_code: String,
}

// An embedded enum stores a discriminant column plus one nullable column per data field. With
// no `#[column(variant)]`, the discriminant is the variant name in snake_case ("email",
// "phone"); override it with `#[column(variant = "...")]` or an integer. Variants may carry
// their own fields.
#[derive(Debug, PartialEq, toasty::Embed)]
enum ContactMethod {
    Email { address: String },
    Phone { number: String },
}

#[derive(Debug, toasty::Model)]
struct Contact {
    #[key]
    #[auto]
    id: ContactId,
    name: String,
    address: Address,
    method: ContactMethod,
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

    // Build contacts entirely from embedded values — no extra tables involved.
    let mut dana = toasty::create!(Contact {
        name: "Dana",
        address: Address {
            street: "1 Main St".into(),
            city: "Seattle".into(),
            postal_code: "98101".into(),
        },
        method: ContactMethod::Email {
            address: "dana@example.com".into(),
        },
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Contact {
        name: "Ed",
        address: Address {
            street: "5 Oak Ave".into(),
            city: "Portland".into(),
            postal_code: "97201".into(),
        },
        method: ContactMethod::Phone {
            number: "555-0100".into(),
        },
    })
    .exec(&mut db)
    .await?;

    // Filter by a flattened sub-field of the embedded struct.
    let seattle = Contact::filter(Contact::fields().address().city().eq("Seattle"))
        .exec(&mut db)
        .await?;
    println!("contacts in Seattle: {}", seattle.len());

    // Embedded enums generate `is_<variant>()` predicates, plus `.matches()` to reach into a
    // data variant's fields from a query.
    let emailable = Contact::filter(Contact::fields().method().is_email())
        .exec(&mut db)
        .await?;
    println!("contacts reachable by email: {}", emailable.len());
    let exact = Contact::filter(
        Contact::fields()
            .method()
            .email()
            .matches(|e| e.address().eq("dana@example.com")),
    )
    .exec(&mut db)
    .await?;
    println!("contacts with that exact email: {}", exact.len());

    // PATCH one embedded sub-field; the others (street, postal_code) are preserved. Passing a
    // whole `Address` value instead would REPLACE the entire embed.
    toasty::update!(dana {
        address: toasty::stmt::patch(Address::fields().city(), "Bellevue"),
    })
    .exec(&mut db)
    .await?;
    // Patch several fields at once with `stmt::apply`.
    toasty::update!(dana {
        address: toasty::stmt::apply([
            toasty::stmt::patch(Address::fields().street(), "456 Oak Ave"),
            toasty::stmt::patch(Address::fields().postal_code(), "98004"),
        ]),
    })
    .exec(&mut db)
    .await?;

    let reloaded = Contact::get_by_id(&mut db, &dana.id).await?;
    println!(
        "Dana's patched address: {} {} {}",
        reloaded.address.street, reloaded.address.city, reloaded.address.postal_code
    );

    Ok(())
}
