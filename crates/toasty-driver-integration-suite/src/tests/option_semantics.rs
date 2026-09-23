//! Application Option semantics, using Rust comparisons as the oracle.

use crate::prelude::*;
use toasty::stmt::{Expr, IntoExpr};

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    #[auto]
    id: uuid::Uuid,
    lhs: Option<String>,
    rhs: Option<String>,
}

async fn setup(t: &mut Test) -> Result<(toasty::Db, Vec<Item>)> {
    let mut db = t.setup_db(models!(Item)).await;
    let mut items = Vec::new();
    for lhs in [None, Some("hello"), Some("world")] {
        for rhs in [None, Some("hello"), Some("world")] {
            items.push(
                toasty::create!(Item {
                    lhs: lhs.map(str::to_owned),
                    rhs: rhs.map(str::to_owned),
                })
                .exec(&mut db)
                .await?,
            );
        }
    }
    Ok((db, items))
}

async fn assert_filter(
    db: &mut toasty::Db,
    items: &[Item],
    filter: Expr<bool>,
    matches: impl Fn(&Item) -> bool,
) -> Result<()> {
    let mut expected: Vec<_> = items
        .iter()
        .filter(|item| matches(item))
        .map(|i| i.id)
        .collect();
    let mut actual: Vec<_> = Item::filter(filter.clone())
        .exec(db)
        .await?
        .into_iter()
        .map(|i| i.id)
        .collect();
    expected.sort();
    actual.sort();
    assert_eq!(actual, expected, "predicate: {filter:?}");
    Ok(())
}

#[driver_test(requires(native_float_nan))]
pub async fn option_float_nan(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Number {
        #[key]
        #[auto]
        id: uuid::Uuid,
        lhs: Option<f64>,
        rhs: Option<f64>,
    }
    let mut db = t.setup_db(models!(Number)).await;
    let mut rows = Vec::new();
    for lhs in [None, Some(1.0), Some(f64::NAN)] {
        for rhs in [None, Some(1.0), Some(f64::NAN)] {
            rows.push(toasty::create!(Number { lhs, rhs }).exec(&mut db).await?);
        }
    }
    let predicates = [
        Number::fields().lhs().eq(Number::fields().rhs()),
        Number::fields().lhs().ne(Number::fields().rhs()),
        Number::fields().lhs().lt(Number::fields().rhs()),
        Number::fields().lhs().eq(Some(f64::NAN)),
        Number::fields().lhs().in_list([Some(f64::NAN)]),
    ];
    for (index, predicate) in predicates.into_iter().enumerate() {
        let expected = |row: &Number| match index {
            0 => row.lhs == row.rhs,
            1 => row.lhs != row.rhs,
            2 => row.lhs < row.rhs,
            _ => false,
        };
        let actual = Number::filter(predicate.clone()).exec(&mut db).await?;
        let mut actual: Vec<_> = actual.iter().map(|r| r.id).collect();
        let mut expected_ids: Vec<_> = rows.iter().filter(|r| expected(r)).map(|r| r.id).collect();
        actual.sort();
        expected_ids.sort();
        assert_eq!(actual, expected_ids);
        let mut projected: Vec<(uuid::Uuid, bool)> = Number::all()
            .select((Number::fields().id(), predicate))
            .exec(&mut db)
            .await?;
        let mut expected_rows: Vec<_> = rows.iter().map(|r| (r.id, expected(r))).collect();
        projected.sort();
        expected_rows.sort();
        assert_eq!(projected, expected_rows);
    }
    Ok(())
}

#[driver_test(requires(not(native_float_nan)))]
pub async fn option_nan_storage_rejected(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Number {
        #[key]
        #[auto]
        id: uuid::Uuid,
        value: Option<f64>,
    }
    let mut db = t.setup_db(models!(Number)).await;
    assert!(
        Number::create()
            .value(Some(f64::NAN))
            .exec(&mut db)
            .await
            .is_err()
    );
    Ok(())
}

#[driver_test(requires(and(native_array, native_float_nan)))]
pub async fn option_payload_membership_nan(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Numbers {
        #[key]
        #[auto]
        id: uuid::Uuid,
        values: Vec<f64>,
    }
    let mut db = t.setup_db(models!(Numbers)).await;
    toasty::create!(Numbers {
        values: vec![f64::NAN, 1.0]
    })
    .exec(&mut db)
    .await?;
    assert!(
        Numbers::filter(Numbers::fields().values().contains(f64::NAN))
            .exec(&mut db)
            .await?
            .is_empty()
    );
    assert_eq!(
        Numbers::filter(Numbers::fields().values().contains(1.0))
            .exec(&mut db)
            .await?
            .len(),
        1
    );
    Ok(())
}

#[driver_test(requires(scan))]
pub async fn option_presence(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    for filter in [
        Item::fields().lhs().is_none(),
        Item::fields().lhs().is_some().not(),
    ] {
        assert_filter(&mut db, &items, filter, |i| i.lhs.is_none()).await?;
    }
    for filter in [
        Item::fields().lhs().is_some(),
        Item::fields().lhs().is_none().not(),
    ] {
        assert_filter(&mut db, &items, filter, |i| i.lhs.is_some()).await?;
    }
    Ok(())
}

#[driver_test(requires(scan))]
pub async fn option_eq_ne_none(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    assert_filter(
        &mut db,
        &items,
        Item::fields().lhs().eq(None::<String>),
        |i| i.lhs.is_none(),
    )
    .await?;
    assert_filter(
        &mut db,
        &items,
        Item::fields().lhs().ne(None::<String>),
        |i| i.lhs.is_some(),
    )
    .await
}

#[driver_test(requires(scan))]
pub async fn option_eq_some_elision(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    for filter in [
        Item::fields().lhs().eq("hello"),
        Item::fields().lhs().eq(Some("hello".to_owned())),
    ] {
        assert_filter(&mut db, &items, filter, |i| {
            i.lhs.as_deref() == Some("hello")
        })
        .await?;
    }
    Ok(())
}

#[driver_test(requires(scan))]
pub async fn option_ne_some(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    assert_filter(&mut db, &items, Item::fields().lhs().ne("hello"), |i| {
        i.lhs.as_deref() != Some("hello")
    })
    .await
}

#[driver_test(requires(scan))]
pub async fn option_negated_eq_some(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    assert_filter(
        &mut db,
        &items,
        Item::fields().lhs().eq("hello").not(),
        |i| i.lhs.as_deref() != Some("hello"),
    )
    .await
}

#[driver_test(requires(sql))]
pub async fn option_field_eq(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    assert_filter(
        &mut db,
        &items,
        Item::fields().lhs().eq(Item::fields().rhs()),
        |i| i.lhs == i.rhs,
    )
    .await
}

#[driver_test(requires(sql))]
pub async fn option_field_ne(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    assert_filter(
        &mut db,
        &items,
        Item::fields().lhs().ne(Item::fields().rhs()),
        |i| i.lhs != i.rhs,
    )
    .await
}

#[driver_test(requires(sql))]
pub async fn option_comparison_projection(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    let mut actual: Vec<(uuid::Uuid, bool)> = Item::all()
        .select((Item::fields().id(), Item::fields().lhs().eq("hello")))
        .exec(&mut db)
        .await?;
    let mut expected: Vec<_> = items
        .iter()
        .map(|i| (i.id, i.lhs.as_deref() == Some("hello")))
        .collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    Ok(())
}

#[driver_test(requires(scan))]
pub async fn option_in_list(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    for needles in [
        vec![],
        vec![None],
        vec![Some("hello".to_owned())],
        vec![None, Some("hello".to_owned())],
        vec![None, None, Some("hello".to_owned())],
    ] {
        assert_filter(
            &mut db,
            &items,
            Item::fields().lhs().in_list(needles.clone()),
            |i| needles.contains(&i.lhs),
        )
        .await?;
    }
    Ok(())
}

#[driver_test(requires(scan))]
pub async fn option_not_in_list(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    for needles in [
        vec![],
        vec![Some("hello".to_owned())],
        vec![None],
        vec![None, Some("hello".to_owned())],
    ] {
        assert_filter(
            &mut db,
            &items,
            Item::fields().lhs().in_list(needles.clone()).not(),
            |i| !needles.contains(&i.lhs),
        )
        .await?;
    }
    Ok(())
}

#[driver_test(requires(scan))]
pub async fn option_in_query(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    for needle in [None, Some("hello"), Some("world")] {
        let query = Item::filter(Item::fields().lhs().eq(needle.map(str::to_owned)))
            .select(Item::fields().lhs());
        let predicate = Item::fields().lhs().in_query(query);
        assert_filter(&mut db, &items, predicate.clone(), |i| {
            i.lhs.as_deref() == needle
        })
        .await?;
        assert_filter(&mut db, &items, predicate.not(), |i| {
            i.lhs.as_deref() != needle
        })
        .await?;
    }
    let empty = Item::filter(false.into_expr()).select(Item::fields().lhs());
    assert_filter(
        &mut db,
        &items,
        Item::fields().lhs().in_query(empty),
        |_| false,
    )
    .await
}

#[driver_test(requires(sql))]
pub async fn option_in_limited_query(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    let query = Item::all()
        .order_by(Item::fields().lhs().asc())
        .limit(1)
        .select(Item::fields().lhs());
    assert_filter(&mut db, &items, Item::fields().lhs().in_query(query), |i| {
        i.lhs.is_none()
    })
    .await
}

#[driver_test(requires(scan))]
pub async fn option_boolean_comparison(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Flags {
        #[key]
        #[auto]
        id: uuid::Uuid,
        lhs: Option<bool>,
        rhs: Option<bool>,
    }
    let mut db = t.setup_db(models!(Flags)).await;
    let mut rows = Vec::new();
    for lhs in [None, Some(false), Some(true)] {
        for rhs in [None, Some(false), Some(true)] {
            rows.push(toasty::create!(Flags { lhs, rhs }).exec(&mut db).await?);
        }
    }
    for (predicate, equal) in [
        (Flags::fields().lhs().eq(Flags::fields().rhs()), true),
        (Flags::fields().lhs().ne(Flags::fields().rhs()), false),
    ] {
        let actual = Flags::filter(predicate).exec(&mut db).await?;
        let mut actual: Vec<_> = actual.iter().map(|r| r.id).collect();
        let mut expected: Vec<_> = rows
            .iter()
            .filter(|r| (r.lhs == r.rhs) == equal)
            .map(|r| r.id)
            .collect();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
    }
    Ok(())
}

#[driver_test(requires(sql))]
pub async fn option_enum_predicates(t: &mut Test) -> Result<()> {
    #[derive(Debug, Clone, Copy, PartialEq, toasty::Embed)]
    enum Status {
        Active,
        Paused,
    }
    #[derive(Debug, toasty::Model)]
    struct Entry {
        #[key]
        #[auto]
        id: uuid::Uuid,
        status: Option<Status>,
    }
    let mut db = t.setup_db(models!(Entry)).await;
    let mut expected = Vec::new();
    for status in [None, Some(Status::Active), Some(Status::Paused)] {
        let entry = toasty::create!(Entry { status }).exec(&mut db).await?;
        expected.push((entry.id, status == Some(Status::Active)));
    }
    let predicate = Entry::fields().status().eq(Status::Active);
    assert_eq!(
        Entry::filter(predicate.clone().not())
            .exec(&mut db)
            .await?
            .len(),
        2
    );
    let mut actual: Vec<(uuid::Uuid, bool)> = Entry::all()
        .select((Entry::fields().id(), predicate))
        .exec(&mut db)
        .await?;
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    Ok(())
}

#[driver_test]
pub async fn option_defaults_and_assignments(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Entry {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[default(Some("default".to_string()))]
        value: Option<String>,
        other: Option<String>,
    }
    let mut db = t.setup_db(models!(Entry)).await;
    let defaulted = Entry::create().exec(&mut db).await?;
    assert_eq!(defaulted.value.as_deref(), Some("default"));
    let mut absent = Entry::create().value(None::<String>).exec(&mut db).await?;
    assert_eq!(absent.value, None);
    absent.update().other("other").exec(&mut db).await?;
    assert_eq!(absent.value, None);
    absent.update().value("set").exec(&mut db).await?;
    assert_eq!(absent.value.as_deref(), Some("set"));
    absent.update().value(None::<String>).exec(&mut db).await?;
    assert_eq!(Entry::get_by_id(&mut db, absent.id).await?.value, None);
    Ok(())
}

#[driver_test(requires(scan))]
pub async fn option_literal_equality(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    for lhs in [None, Some("hello".to_owned()), Some("world".to_owned())] {
        for rhs in [None, Some("hello".to_owned()), Some("world".to_owned())] {
            let expr: Expr<Option<String>> = lhs.clone().into_expr();
            assert_filter(&mut db, &items, expr.clone().eq(rhs.clone()), |_| {
                lhs == rhs
            })
            .await?;
            assert_filter(&mut db, &items, expr.ne(rhs.clone()), |_| lhs != rhs).await?;
        }
    }
    Ok(())
}

#[driver_test(requires(scan))]
pub async fn option_excluded_middle(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    let p = Item::fields().lhs().eq("hello");
    assert_filter(&mut db, &items, p.clone().or(p.not()), |_| true).await
}

#[driver_test(requires(sql))]
pub async fn option_tuple_membership(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    let needles = vec![(None::<String>, Some("hello".to_owned()))];
    assert_filter(
        &mut db,
        &items,
        toasty::stmt::in_list(
            (Item::fields().lhs(), Item::fields().rhs()),
            needles.clone(),
        ),
        |i| needles.contains(&(i.lhs.clone(), i.rhs.clone())),
    )
    .await
}

#[driver_test(requires(sql))]
pub async fn option_first_projection_preserves_presence(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    for item in &items {
        let actual: Option<Option<String>> = Item::filter(Item::fields().id().eq(item.id))
            .select(Item::fields().lhs())
            .first()
            .exec(&mut db)
            .await?;
        assert_eq!(actual, Some(item.lhs.clone()));
    }
    let missing: Option<Option<String>> = Item::filter(false.into_expr())
        .select(Item::fields().lhs())
        .first()
        .exec(&mut db)
        .await?;
    assert_eq!(missing, None);
    Ok(())
}

#[driver_test(requires(sql))]
pub async fn option_nested_expression_presence(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    let nested = Item::fields().lhs().into_expr().some();
    let predicate = nested.clone().eq(Some(None::<String>));
    assert_filter(&mut db, &items, predicate, |i| i.lhs.is_none()).await?;
    for item in &items {
        let value: Option<Option<Option<String>>> = Item::filter_by_id(item.id)
            .select(nested.clone())
            .first()
            .exec(&mut db)
            .await?;
        assert_eq!(value, Some(Some(item.lhs.clone())));
    }
    let literal: Expr<Option<Option<String>>> = Some(None::<String>).into_expr();
    assert_filter(&mut db, &items, literal.eq(Some(None::<String>)), |_| true).await
}

#[driver_test(requires(sql))]
pub async fn option_ordered_comparisons(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    for rhs in [None, Some("hello".to_owned()), Some("world".to_owned())] {
        for (filter, op) in [
            (Item::fields().lhs().lt(rhs.clone()), 0),
            (Item::fields().lhs().le(rhs.clone()), 1),
            (Item::fields().lhs().gt(rhs.clone()), 2),
            (Item::fields().lhs().ge(rhs.clone()), 3),
        ] {
            assert_filter(&mut db, &items, filter, |i| match op {
                0 => i.lhs < rhs,
                1 => i.lhs <= rhs,
                2 => i.lhs > rhs,
                _ => i.lhs >= rhs,
            })
            .await?;
        }
    }
    Ok(())
}

#[driver_test(requires(sql))]
pub async fn option_string_predicates(t: &mut Test) -> Result<()> {
    let (mut db, items) = setup(t).await?;
    for filter in [
        Item::fields().lhs().starts_with("hel"),
        Item::fields().lhs().like("hel%"),
        Item::fields().lhs().like_with_escape("hel%", '!'),
    ] {
        assert_filter(&mut db, &items, filter.clone(), |i| {
            i.lhs.as_deref() == Some("hello")
        })
        .await?;
        assert_filter(&mut db, &items, filter.clone().not(), |i| {
            i.lhs.as_deref() != Some("hello")
        })
        .await?;
        let mut actual: Vec<(uuid::Uuid, bool)> = Item::all()
            .select((Item::fields().id(), filter))
            .exec(&mut db)
            .await?;
        let mut expected: Vec<_> = items
            .iter()
            .map(|i| (i.id, i.lhs.as_deref() == Some("hello")))
            .collect();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
    }
    Ok(())
}

#[driver_test(requires(and(sql, backward_pagination)))]
pub async fn option_ordering_and_pagination(t: &mut Test) -> Result<()> {
    let (mut db, mut items) = setup(t).await?;
    for value in ["HELLO", "hello ", "é"] {
        items.push(
            toasty::create!(Item {
                lhs: Some(value.to_owned()),
                rhs: None::<String>,
            })
            .exec(&mut db)
            .await?,
        );
    }
    let mut expected: Vec<_> = items.iter().map(|i| (i.lhs.clone(), i.id)).collect();
    expected.sort();
    for descending in [false, true] {
        let order = if descending {
            Item::fields().lhs().desc()
        } else {
            Item::fields().lhs().asc()
        };
        let mut page = Item::all()
            .order_by(order)
            .paginate(2)
            .exec(&mut db)
            .await?;
        let mut actual = Vec::new();
        loop {
            actual.extend(page.iter().map(|i| (i.lhs.clone(), i.id)));
            let Some(next) = page.next(&mut db).await? else {
                break;
            };
            page = next;
        }
        assert_eq!(actual, expected);
        while let Some(previous) = page.prev(&mut db).await? {
            if previous.is_empty() {
                break;
            }
            page = previous;
        }
        assert_eq!(
            page.iter()
                .map(|i| (i.lhs.clone(), i.id))
                .collect::<Vec<_>>(),
            expected[..2]
        );
        expected.reverse();
    }
    Ok(())
}

#[driver_test(requires(sql))]
pub async fn option_update_and_delete(t: &mut Test) -> Result<()> {
    let (mut db, _) = setup(t).await?;
    Item::filter(Item::fields().lhs().ne("hello"))
        .update()
        .rhs(None::<String>)
        .exec(&mut db)
        .await?;
    let items = Item::all().exec(&mut db).await?;
    assert_eq!(items.iter().filter(|i| i.rhs.is_none()).count(), 7);
    Item::filter(Item::fields().lhs().eq(None::<String>))
        .delete()
        .exec(&mut db)
        .await?;
    assert_eq!(Item::all().exec(&mut db).await?.len(), 6);
    Ok(())
}

#[driver_test(requires(sql))]
pub async fn option_relation_presence(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[has_one(pair = user)]
        profile: toasty::Deferred<Option<Profile>>,
    }
    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        user_id: Option<uuid::Uuid>,
        #[belongs_to(key = user_id)]
        user: toasty::Deferred<Option<User>>,
    }
    let mut db = t.setup_db(models!(User, Profile)).await;
    let user = toasty::create!(User {}).exec(&mut db).await?;
    let empty = toasty::create!(User {}).exec(&mut db).await?;
    let linked = toasty::create!(Profile { user_id: user.id })
        .exec(&mut db)
        .await?;
    let unlinked = toasty::create!(Profile {}).exec(&mut db).await?;
    for filter in [
        User::fields().profile().is_none(),
        User::fields().profile().eq(None::<Profile>),
    ] {
        let users = User::filter(filter).exec(&mut db).await?;
        assert_eq!(users.iter().map(|u| u.id).collect::<Vec<_>>(), [empty.id]);
    }
    let profiles = Profile::filter(Profile::fields().user().is_none())
        .exec(&mut db)
        .await?;
    assert_eq!(
        profiles.iter().map(|p| p.id).collect::<Vec<_>>(),
        [unlinked.id]
    );
    let profiles = Profile::filter(Profile::fields().user().in_query(User::all()))
        .exec(&mut db)
        .await?;
    assert_eq!(
        profiles.iter().map(|p| p.id).collect::<Vec<_>>(),
        [linked.id]
    );
    Ok(())
}

#[driver_test(requires(sql))]
pub async fn option_relation_identity_comparison(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[has_one(pair = user)]
        profile: toasty::Deferred<Option<Profile>>,
        favorite_id: Option<uuid::Uuid>,
        #[belongs_to(key = favorite_id)]
        favorite: toasty::Deferred<Option<Profile>>,
    }
    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        user_id: uuid::Uuid,
        #[belongs_to(key = user_id)]
        user: toasty::Deferred<User>,
    }
    let mut db = t.setup_db(models!(User, Profile)).await;
    let mut owner = toasty::create!(User {}).exec(&mut db).await?;
    let empty = toasty::create!(User {}).exec(&mut db).await?;
    let profile = toasty::create!(Profile { user_id: owner.id })
        .exec(&mut db)
        .await?;
    owner.update().favorite_id(profile.id).exec(&mut db).await?;
    let other = toasty::create!(User {
        favorite_id: profile.id
    })
    .exec(&mut db)
    .await?;
    let equal = User::fields().profile().eq(User::fields().favorite());
    let mut actual: Vec<_> = User::filter(equal.clone())
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|u| u.id)
        .collect();
    let mut expected = vec![owner.id, empty.id];
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    let unequal = User::filter(equal.not()).exec(&mut db).await?;
    assert_eq!(unequal.len(), 1);
    assert_eq!(unequal[0].id, other.id);
    Ok(())
}

#[driver_test(requires(sql))]
pub async fn option_string_equality_is_exact(t: &mut Test) -> Result<()> {
    let mut db = t.setup_db(models!(Item)).await;
    let mut items = Vec::new();
    for value in [None, Some("hello"), Some("HELLO"), Some("hello ")] {
        items.push(
            toasty::create!(Item {
                lhs: value.map(str::to_owned),
                rhs: "hello"
            })
            .exec(&mut db)
            .await?,
        );
    }
    for predicate in [
        Item::fields().lhs().eq("hello"),
        Item::fields().lhs().eq(Item::fields().rhs()),
        Item::fields().lhs().in_list(["hello", "absent"]),
    ] {
        assert_filter(&mut db, &items, predicate, |i| {
            i.lhs.as_deref() == Some("hello")
        })
        .await?;
    }
    Ok(())
}

#[driver_test(requires(sql))]
pub async fn option_embedded_equality(t: &mut Test) -> Result<()> {
    #[derive(Debug, Clone, PartialEq, toasty::Embed)]
    struct Detail {
        text: Option<String>,
        flag: bool,
    }
    #[derive(Debug, toasty::Model)]
    struct Record {
        #[key]
        #[auto]
        id: uuid::Uuid,
        lhs: Option<Detail>,
        rhs: Option<Detail>,
    }
    let mut db = t.setup_db(models!(Record)).await;
    let values = [
        None,
        Some(Detail {
            text: None,
            flag: false,
        }),
        Some(Detail {
            text: Some("hello".into()),
            flag: false,
        }),
    ];
    let mut records = Vec::new();
    for lhs in &values {
        for rhs in &values {
            records.push(
                toasty::create!(Record {
                    lhs: lhs.clone(),
                    rhs: rhs.clone()
                })
                .exec(&mut db)
                .await?,
            );
        }
    }
    for rhs in &values {
        let mut actual: Vec<_> = Record::filter(Record::fields().lhs().eq(rhs.clone()))
            .exec(&mut db)
            .await?
            .iter()
            .map(|r| r.id)
            .collect();
        let mut expected: Vec<_> = records
            .iter()
            .filter(|r| &r.lhs == rhs)
            .map(|r| r.id)
            .collect();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
    }
    let mut actual: Vec<_> = Record::filter(Record::fields().lhs().eq(Record::fields().rhs()))
        .exec(&mut db)
        .await?
        .iter()
        .map(|r| r.id)
        .collect();
    let mut expected: Vec<_> = records
        .iter()
        .filter(|r| r.lhs == r.rhs)
        .map(|r| r.id)
        .collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    Ok(())
}

#[driver_test(requires(sql))]
pub async fn option_relation_non_primary_reference(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        name: String,
    }
    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        user_name: Option<String>,
        #[belongs_to(key = user_name, references = name)]
        user: toasty::Deferred<Option<User>>,
    }
    let mut db = t.setup_db(models!(User, Post)).await;
    let user = toasty::create!(User { name: "alice" })
        .exec(&mut db)
        .await?;
    let linked = toasty::create!(Post { user_name: "alice" })
        .exec(&mut db)
        .await?;
    let unlinked = toasty::create!(Post {}).exec(&mut db).await?;
    let posts = Post::filter(Post::fields().user().eq(&user))
        .exec(&mut db)
        .await?;
    assert_eq!(posts.iter().map(|p| p.id).collect::<Vec<_>>(), [linked.id]);
    let posts = Post::filter(Post::fields().user().ne(&user))
        .exec(&mut db)
        .await?;
    assert_eq!(
        posts.iter().map(|p| p.id).collect::<Vec<_>>(),
        [unlinked.id]
    );
    Ok(())
}
