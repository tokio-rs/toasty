use crate::prelude::*;

#[driver_test]
pub async fn filter_data_enum(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum ContactInfo {
        #[column(variant = 1)]
        Email { address: String },
        #[column(variant = 2)]
        Phone { number: String },
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        contact: ContactInfo,
    }

    let db = t.setup_db(models!(User, ContactInfo)).await;

    User::create()
        .name("Alice")
        .contact(ContactInfo::Email {
            address: "alice@example.com".to_string(),
        })
        .exec(&db)
        .await?;

    User::create()
        .name("Bob")
        .contact(ContactInfo::Phone {
            number: "555-1234".to_string(),
        })
        .exec(&db)
        .await?;

    // Try filtering by whole enum value
    let emails = User::filter(User::fields().contact().eq(ContactInfo::Email {
        address: "alice@example.com".to_string(),
    }))
    .collect::<Vec<_>>(&db)
    .await?;

    assert_eq!(emails.len(), 1);
    assert_eq!(emails[0].name, "Alice");

    Ok(())
}
