//! `.select(...)` with a wide tuple projection. Tuple `Load` and `IntoExpr`
//! impls are generated up to 16 elements; this file pins the upper bound so
//! a regression back to a smaller arity fails to compile here.

use crate::prelude::*;

#[derive(Debug, toasty::Model)]
struct Wide {
    #[key]
    id: i64,
    f1: String,
    f2: String,
    f3: String,
    f4: String,
    f5: String,
    f6: String,
    f7: String,
    f8: String,
    f9: String,
    f10: String,
    f11: String,
    f12: String,
    f13: String,
    f14: String,
    f15: String,
}

/// Selecting sixteen fields as a single tuple loads every column in order.
#[driver_test]
pub async fn select_sixteen_field_tuple(test: &mut Test) -> Result<()> {
    let mut db = test.setup_db(models!(Wide)).await;

    Wide::create()
        .id(1_i64)
        .f1("v1")
        .f2("v2")
        .f3("v3")
        .f4("v4")
        .f5("v5")
        .f6("v6")
        .f7("v7")
        .f8("v8")
        .f9("v9")
        .f10("v10")
        .f11("v11")
        .f12("v12")
        .f13("v13")
        .f14("v14")
        .f15("v15")
        .exec(&mut db)
        .await?;

    let f = Wide::fields();
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        i64,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    )> = Wide::all()
        .select((
            f.id(),
            f.f1(),
            f.f2(),
            f.f3(),
            f.f4(),
            f.f5(),
            f.f6(),
            f.f7(),
            f.f8(),
            f.f9(),
            f.f10(),
            f.f11(),
            f.f12(),
            f.f13(),
            f.f14(),
            f.f15(),
        ))
        .exec(&mut db)
        .await?;

    // `PartialEq` and `Debug` for std tuples stop at 12 elements, so
    // destructure rather than compare the whole tuple.
    assert_eq!(rows.len(), 1);
    let (id, f1, f2, f3, f4, f5, f6, f7, f8, f9, f10, f11, f12, f13, f14, f15) =
        rows.into_iter().next().unwrap();
    assert_eq!(id, 1_i64);
    assert_eq!(
        [
            f1, f2, f3, f4, f5, f6, f7, f8, f9, f10, f11, f12, f13, f14, f15
        ],
        [
            "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9", "v10", "v11", "v12", "v13",
            "v14", "v15"
        ]
    );

    Ok(())
}
