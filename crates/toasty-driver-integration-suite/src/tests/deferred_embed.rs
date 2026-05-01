use crate::prelude::*;

// ---------- Deferred<Embed> on a struct embed ----------

#[driver_test(id(ID))]
pub async fn deferred_embed_struct(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        author: String,
        notes: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[deferred]
        metadata: toasty::Deferred<Metadata>,
    }

    let mut db = t.setup_db(models!(Document, Metadata)).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        metadata: Metadata {
            author: "Alice".to_string(),
            notes: "Important".to_string(),
        },
    })
    .exec(&mut db)
    .await?;

    // Created records have the deferred embed loaded with the value the caller
    // just supplied.
    assert_eq!("Hello", created.title);
    assert_eq!("Alice", created.metadata.get().author);
    assert_eq!("Important", created.metadata.get().notes);

    // A separate query leaves the deferred embed unloaded.
    let read = Document::filter_by_id(created.id).get(&mut db).await?;
    assert_eq!("Hello", read.title);
    assert!(read.metadata.is_unloaded());

    // The per-field accessor loads the embed by value.
    let metadata: Metadata = read.metadata().exec(&mut db).await?;
    assert_eq!("Alice", metadata.author);
    assert_eq!("Important", metadata.notes);
    // `.exec()` does not mutate the in-memory record.
    assert!(read.metadata.is_unloaded());

    // `.include()` preloads the embed onto the parent query.
    let read_with = Document::filter_by_id(created.id)
        .include(Document::fields().metadata())
        .get(&mut db)
        .await?;
    assert!(!read_with.metadata.is_unloaded());
    assert_eq!("Alice", read_with.metadata.get().author);
    assert_eq!("Important", read_with.metadata.get().notes);

    Ok(())
}

// `Deferred<Option<EmbeddedType>>` is not covered here. `Option<Embed>`
// itself isn't yet supported as a model field — the schema layer has no
// representation for a nullable embedded type, so the column-type lowering
// errors out with "type Model(...) is not supported by this database".
// Tracking that gap is orthogonal to deferred fields.

// ---------- #[deferred] inside an embed struct that's nested in an enum variant ----------
//
// `#[deferred]` on a variant field directly is rejected at the macro layer,
// but a struct embedded as a variant field is allowed to carry its own
// deferred sub-fields. The lowering has to descend through the enum's
// `Match` expression to mask / wrap those sub-fields.

#[driver_test(id(ID))]
pub async fn deferred_inside_embed_in_enum_variant(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        author: String,

        #[deferred]
        notes: toasty::Deferred<String>,
    }

    #[derive(Debug, toasty::Embed)]
    enum ContactInfo {
        Email { address: String, metadata: Metadata },
        Phone { number: String },
    }

    #[derive(Debug, toasty::Model)]
    struct Person {
        #[key]
        #[auto]
        id: ID,

        name: String,

        contact: ContactInfo,
    }

    let mut db = t.setup_db(models!(Person, ContactInfo, Metadata)).await;

    // INSERT...RETURNING must echo back the deferred sub-field nested two
    // levels deep (through the enum variant and through the embed struct).
    let created = toasty::create!(Person {
        name: "Alice".to_string(),
        contact: ContactInfo::Email {
            address: "alice@example.com".to_string(),
            metadata: Metadata {
                author: "Alice".to_string(),
                notes: "Important".to_string().into(),
            },
        },
    })
    .exec(&mut db)
    .await?;

    let ContactInfo::Email {
        address, metadata, ..
    } = &created.contact
    else {
        panic!("expected Email variant");
    };
    assert_eq!("alice@example.com", address);
    assert_eq!("Alice", metadata.author);
    assert_eq!("Important", metadata.notes.get());

    // Default load: contact loaded, but the deferred sub-field nested inside
    // the variant's Metadata embed is unloaded.
    let read = Person::filter_by_id(created.id).get(&mut db).await?;
    let ContactInfo::Email { metadata, .. } = &read.contact else {
        panic!("expected Email variant");
    };
    assert_eq!("Alice", metadata.author);
    assert!(metadata.notes.is_unloaded());

    Ok(())
}

// `.include()` reaching a deferred sub-field that lives inside a struct embed
// nested inside an enum variant. The variant handle exposes the same field
// accessors as a struct embed, returning variant-rooted Paths that the
// engine flattens into `[contact_idx, variant_idx, …]` projections and
// dispatches into the matching arm of the embed enum's `Match`.

#[driver_test(id(ID))]
pub async fn include_deferred_inside_embed_in_enum_variant(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        author: String,

        #[deferred]
        notes: toasty::Deferred<String>,
    }

    #[derive(Debug, toasty::Embed)]
    enum ContactInfo {
        Email { address: String, metadata: Metadata },
        Phone { number: String },
    }

    #[derive(Debug, toasty::Model)]
    struct Person {
        #[key]
        #[auto]
        id: ID,

        name: String,

        contact: ContactInfo,
    }

    let mut db = t.setup_db(models!(Person, ContactInfo, Metadata)).await;

    let alice = toasty::create!(Person {
        name: "Alice".to_string(),
        contact: ContactInfo::Email {
            address: "alice@example.com".to_string(),
            metadata: Metadata {
                author: "Alice".to_string(),
                notes: "Important".to_string().into(),
            },
        },
    })
    .exec(&mut db)
    .await?;

    // Bob's contact is a different variant, used to verify that an include
    // routed through Email doesn't activate anything for him.
    let bob = toasty::create!(Person {
        name: "Bob".to_string(),
        contact: ContactInfo::Phone {
            number: "555-0100".to_string(),
        },
    })
    .exec(&mut db)
    .await?;

    // Alice's variant matches the path — `notes` arrives loaded.
    let alice_read = Person::filter_by_id(alice.id)
        .include(Person::fields().contact().email().metadata().notes())
        .get(&mut db)
        .await?;
    let ContactInfo::Email { metadata, .. } = &alice_read.contact else {
        panic!("expected Email variant");
    };
    assert_eq!("Alice", metadata.author);
    assert!(!metadata.notes.is_unloaded());
    assert_eq!("Important", metadata.notes.get());

    // Bob's variant doesn't match the include path's arm — the include is a
    // no-op for him: the row still loads cleanly with the Phone variant.
    let bob_read = Person::filter_by_id(bob.id)
        .include(Person::fields().contact().email().metadata().notes())
        .get(&mut db)
        .await?;
    let ContactInfo::Phone { number } = &bob_read.contact else {
        panic!("expected Phone variant");
    };
    assert_eq!("555-0100", number);

    Ok(())
}

// ---------- Deferred<UnitEnum> ----------

#[driver_test(id(ID))]
pub async fn deferred_embed_unit_enum(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Status {
        Draft,
        Published,
        Archived,
    }

    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[deferred]
        status: toasty::Deferred<Status>,
    }

    let mut db = t.setup_db(models!(Document, Status)).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        status: Status::Published,
    })
    .exec(&mut db)
    .await?;

    assert_eq!(&Status::Published, created.status.get());

    let read = Document::filter_by_id(created.id).get(&mut db).await?;
    assert!(read.status.is_unloaded());

    let status: Status = read.status().exec(&mut db).await?;
    assert_eq!(Status::Published, status);

    let inc = Document::filter_by_id(created.id)
        .include(Document::fields().status())
        .get(&mut db)
        .await?;
    assert!(!inc.status.is_unloaded());
    assert_eq!(&Status::Published, inc.status.get());

    Ok(())
}

// ---------- Deferred<DataCarryingEnum> ----------

#[driver_test(id(ID))]
pub async fn deferred_embed_data_enum(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum ContactInfo {
        Email { address: String },
        Phone { number: String },
        Mail,
    }

    #[derive(Debug, toasty::Model)]
    struct Person {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[deferred]
        contact: toasty::Deferred<ContactInfo>,
    }

    let mut db = t.setup_db(models!(Person, ContactInfo)).await;

    let alice = toasty::create!(Person {
        name: "Alice".to_string(),
        contact: ContactInfo::Email {
            address: "alice@example.com".to_string(),
        },
    })
    .exec(&mut db)
    .await?;

    let bob = toasty::create!(Person {
        name: "Bob".to_string(),
        contact: ContactInfo::Mail,
    })
    .exec(&mut db)
    .await?;

    assert_eq!(
        &ContactInfo::Email {
            address: "alice@example.com".to_string()
        },
        alice.contact.get()
    );

    let read = Person::filter_by_id(alice.id).get(&mut db).await?;
    assert!(read.contact.is_unloaded());

    let contact: ContactInfo = read.contact().exec(&mut db).await?;
    assert_eq!(
        ContactInfo::Email {
            address: "alice@example.com".to_string()
        },
        contact
    );

    let read_bob = Person::filter_by_id(bob.id).get(&mut db).await?;
    let contact: ContactInfo = read_bob.contact().exec(&mut db).await?;
    assert_eq!(ContactInfo::Mail, contact);

    let inc = Person::filter_by_id(alice.id)
        .include(Person::fields().contact())
        .get(&mut db)
        .await?;
    assert!(!inc.contact.is_unloaded());
    assert_eq!(
        &ContactInfo::Email {
            address: "alice@example.com".to_string()
        },
        inc.contact.get()
    );

    Ok(())
}

// ---------- Updating a deferred embed reloads with the new value ----------

#[driver_test(id(ID))]
pub async fn deferred_embed_update_reloads(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        author: String,
        notes: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[deferred]
        metadata: toasty::Deferred<Metadata>,
    }

    let mut db = t.setup_db(models!(Document, Metadata)).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        metadata: Metadata {
            author: "Alice".to_string(),
            notes: "old".to_string(),
        },
    })
    .exec(&mut db)
    .await?;

    let mut doc = Document::filter_by_id(created.id).get(&mut db).await?;
    assert!(doc.metadata.is_unloaded());

    doc.update()
        .metadata(Metadata {
            author: "Bob".to_string(),
            notes: "new".to_string(),
        })
        .exec(&mut db)
        .await?;

    // The caller supplied the value, so the field becomes loaded post-update.
    assert!(!doc.metadata.is_unloaded());
    assert_eq!("Bob", doc.metadata.get().author);
    assert_eq!("new", doc.metadata.get().notes);

    Ok(())
}

// ---------- Deferred<Embed> with #[deferred] inside the embed ----------
//
// The combined shape: the embed itself is deferred at the parent, AND the
// embed has its own deferred sub-field. `.include(metadata())` loads the
// outer wrapper but leaves the inner deferred sub-field unloaded;
// `.include(metadata().notes())` loads both.

#[driver_test(id(ID))]
pub async fn deferred_embed_with_deferred_sub_field(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        author: String,

        #[deferred]
        notes: toasty::Deferred<String>,
    }

    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[deferred]
        metadata: toasty::Deferred<Metadata>,
    }

    let mut db = t.setup_db(models!(Document, Metadata)).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        metadata: Metadata {
            author: "Alice".to_string(),
            notes: "Important".to_string().into(),
        },
    })
    .exec(&mut db)
    .await?;

    // INSERT...RETURNING returns everything loaded.
    assert_eq!("Alice", created.metadata.get().author);
    assert_eq!("Important", created.metadata.get().notes.get());

    // Default load: metadata itself unloaded.
    let read = Document::filter_by_id(created.id).get(&mut db).await?;
    assert!(read.metadata.is_unloaded());

    // Including just the outer embed loads `author` but leaves the inner
    // deferred sub-field unloaded.
    let inc_outer = Document::filter_by_id(created.id)
        .include(Document::fields().metadata())
        .get(&mut db)
        .await?;
    assert!(!inc_outer.metadata.is_unloaded());
    assert_eq!("Alice", inc_outer.metadata.get().author);
    assert!(inc_outer.metadata.get().notes.is_unloaded());

    // Including the inner deferred sub-field implies the outer is loaded too,
    // and the inner sub-field arrives loaded.
    let inc_inner = Document::filter_by_id(created.id)
        .include(Document::fields().metadata().notes())
        .get(&mut db)
        .await?;
    assert!(!inc_inner.metadata.is_unloaded());
    assert_eq!("Alice", inc_inner.metadata.get().author);
    assert!(!inc_inner.metadata.get().notes.is_unloaded());
    assert_eq!("Important", inc_inner.metadata.get().notes.get());

    Ok(())
}

// ---------- #[deferred] inside an Embed (per-column) ----------

#[driver_test(id(ID))]
pub async fn deferred_field_inside_embed(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        author: String,

        #[deferred]
        notes: toasty::Deferred<String>,
    }

    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: ID,

        title: String,

        metadata: Metadata,
    }

    let mut db = t.setup_db(models!(Document, Metadata)).await;

    let created = toasty::create!(Document {
        title: "Hello".to_string(),
        metadata: Metadata {
            author: "Alice".to_string(),
            notes: "Important".to_string().into(),
        },
    })
    .exec(&mut db)
    .await?;

    // Created records carry the just-supplied value loaded.
    assert_eq!("Alice", created.metadata.author);
    assert_eq!("Important", created.metadata.notes.get());

    // Default load: embed eager fields are loaded, deferred sub-field is not.
    let read = Document::filter_by_id(created.id).get(&mut db).await?;
    assert_eq!("Alice", read.metadata.author);
    assert!(read.metadata.notes.is_unloaded());

    // Including the deferred sub-field loads it on the same query.
    let inc = Document::filter_by_id(created.id)
        .include(Document::fields().metadata().notes())
        .get(&mut db)
        .await?;
    assert!(!inc.metadata.notes.is_unloaded());
    assert_eq!("Important", inc.metadata.notes.get());

    Ok(())
}
