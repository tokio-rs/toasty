use crate::prelude::*;

/// u8 as a manually-assigned primary key.
#[driver_test]
pub async fn key_u8(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: u8,
        val: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let item = toasty::create!(Item {
        id: 1_u8,
        val: "hello"
    })
    .exec(&mut db)
    .await?;
    assert_struct!(item, _ { id: 1_u8, val: "hello" });

    let read = Item::get_by_id(&mut db, &1_u8).await?;
    assert_struct!(read, _ { id: 1_u8, val: "hello" });

    item.delete().exec(&mut db).await?;
    Ok(())
}

/// u16 as a manually-assigned primary key.
#[driver_test]
pub async fn key_u16(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: u16,
        val: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let item = toasty::create!(Item {
        id: 1_u16,
        val: "hello"
    })
    .exec(&mut db)
    .await?;
    assert_struct!(item, _ { id: 1_u16, val: "hello" });

    let read = Item::get_by_id(&mut db, &1_u16).await?;
    assert_struct!(read, _ { id: 1_u16, val: "hello" });

    item.delete().exec(&mut db).await?;
    Ok(())
}

/// u32 as a manually-assigned primary key.
#[driver_test]
pub async fn key_u32(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: u32,
        val: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let item = toasty::create!(Item {
        id: 1_u32,
        val: "hello"
    })
    .exec(&mut db)
    .await?;
    assert_struct!(item, _ { id: 1_u32, val: "hello" });

    let read = Item::get_by_id(&mut db, &1_u32).await?;
    assert_struct!(read, _ { id: 1_u32, val: "hello" });

    item.delete().exec(&mut db).await?;
    Ok(())
}

/// u64 as a manually-assigned primary key.
#[driver_test]
pub async fn key_u64(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: u64,
        val: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let item = toasty::create!(Item {
        id: 42_u64,
        val: "hello"
    })
    .exec(&mut db)
    .await?;
    assert_struct!(item, _ { id: 42_u64, val: "hello" });

    let read = Item::get_by_id(&mut db, &42_u64).await?;
    assert_struct!(read, _ { id: 42_u64, val: "hello" });

    item.delete().exec(&mut db).await?;
    Ok(())
}
