use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn float_fields_column_type_override(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Measurement {
        #[key]
        #[auto]
        id: ID,
        /// f32 field stored as DOUBLE PRECISION / FLOAT8
        #[column(type = f64)]
        value_f32: f32,
        /// f64 field stored as REAL / FLOAT4
        #[column(type = f32)]
        value_f64: f64,
    }

    let mut db = t.setup_db(models!(Measurement)).await;

    let created = Measurement::create()
        .value_f32(1.5_f32)
        .value_f64(2.5_f64)
        .exec(&mut db)
        .await?;

    assert_eq!(created.value_f32, 1.5_f32);
    assert_eq!(created.value_f64, 2.5_f64);

    let read = Measurement::get_by_id(&mut db, &created.id).await?;

    assert_eq!(read.value_f32, 1.5_f32);
    assert_eq!(read.value_f64, 2.5_f64);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn float_fields_crud(t: &mut Test) -> Result<()> {
    #[allow(clippy::approx_constant)]
    const PI_F32: f32 = 3.14;
    #[allow(clippy::approx_constant)]
    const PI_F64: f64 = 3.14159265358979;

    #[derive(Debug, toasty::Model)]
    struct Measurement {
        #[key]
        #[auto]
        id: ID,
        value_f32: f32,
        value_f64: f64,
    }

    let mut db = t.setup_db(models!(Measurement)).await;

    let mut created = Measurement::create()
        .value_f32(1.5_f32)
        .value_f64(2.5_f64)
        .exec(&mut db)
        .await?;

    assert_eq!(created.value_f32, 1.5_f32);
    assert_eq!(created.value_f64, 2.5_f64);

    let read = Measurement::get_by_id(&mut db, &created.id).await?;

    assert_eq!(read.value_f32, 1.5_f32);
    assert_eq!(read.value_f64, 2.5_f64);

    // Update float fields
    created
        .update()
        .value_f32(PI_F32)
        .value_f64(PI_F64)
        .exec(&mut db)
        .await?;

    let updated = Measurement::get_by_id(&mut db, &read.id).await?;

    assert_eq!(updated.value_f32, PI_F32);
    assert_eq!(updated.value_f64, PI_F64);

    // Delete
    updated.delete().exec(&mut db).await?;
    assert_err!(Measurement::get_by_id(&mut db, &read.id).await);

    Ok(())
}
