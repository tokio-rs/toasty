use crate::prelude::*;

#[driver_test]
pub async fn network_address_round_trip(test: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        cidr: IpCidr,
        inet: IpInet,
        macaddr: MacAddr6,
        macaddr8: MacAddr8,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let values = [
        (
            "192.0.2.0/24",
            "192.0.2.129/24",
            "ac:de:48:23:45:67",
            "ac:de:48:23:45:67:89:ab",
        ),
        (
            "2001:db8::/32",
            "2001:db8::1/64",
            "02:00:5e:10:00:00",
            "02:00:5e:10:00:00:00:01",
        ),
    ];

    for (cidr, inet, macaddr, macaddr8) in values {
        let cidr = cidr.parse()?;
        let inet = inet.parse()?;
        let macaddr = macaddr.parse()?;
        let macaddr8 = macaddr8.parse()?;

        let created = toasty::create!(Item {
            cidr,
            inet,
            macaddr,
            macaddr8,
        })
        .exec(&mut db)
        .await?;
        let read = Item::get_by_id(&mut db, &created.id).await?;

        assert_struct!(read, {
            cidr: == cidr,
            inet: == inet,
            macaddr: == macaddr,
            macaddr8: == macaddr8,
        });

        let filtered: Vec<Item> = Item::filter(Item::fields().inet().eq(inet))
            .exec(&mut db)
            .await?;
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, created.id);
    }

    Ok(())
}

#[driver_test]
pub async fn network_address_collection_round_trip(test: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        addresses: Vec<IpInet>,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let addresses = vec!["192.0.2.1/24".parse()?, "2001:db8::1/64".parse()?];

    let created = toasty::create!(Item {
        addresses: addresses.clone(),
    })
    .exec(&mut db)
    .await?;
    let read = Item::get_by_id(&mut db, &created.id).await?;

    assert_eq!(read.addresses, addresses);
    Ok(())
}

#[driver_test(requires(and(native_cidr, native_inet, native_macaddr, native_macaddr8)))]
pub async fn explicit_native_network_columns(test: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[column(type = cidr)]
        cidr: IpCidr,
        #[column(type = inet)]
        inet: IpInet,
        #[column(type = macaddr)]
        macaddr: MacAddr6,
        #[column(type = macaddr8)]
        macaddr8: MacAddr8,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let cidr: IpCidr = "10.0.0.0/8".parse()?;
    let inet: IpInet = "10.0.0.1/8".parse()?;
    let macaddr: MacAddr6 = "ac:de:48:23:45:67".parse()?;
    let macaddr8: MacAddr8 = "ac:de:48:23:45:67:89:ab".parse()?;
    let item = toasty::create!(Item {
        cidr,
        inet,
        macaddr,
        macaddr8,
    })
    .exec(&mut db)
    .await?;

    assert_eq!(Item::get_by_id(&mut db, &item.id).await?.inet, item.inet);
    Ok(())
}
