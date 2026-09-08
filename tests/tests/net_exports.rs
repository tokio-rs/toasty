use std::marker::PhantomData;

use toasty::stmt::{
    BigDecimal, Date, DateTime, Decimal, IpCidr, IpInet, MacAddr6, MacAddr8, Time, Timestamp, Uuid,
    Zoned,
};
use toasty_core::stmt::{
    BigDecimal as CoreBigDecimal, Date as CoreDate, DateTime as CoreDateTime,
    Decimal as CoreDecimal, IpCidr as CoreIpCidr, IpInet as CoreIpInet, MacAddr6 as CoreMacAddr6,
    MacAddr8 as CoreMacAddr8, Time as CoreTime, Timestamp as CoreTimestamp, Uuid as CoreUuid,
    Zoned as CoreZoned,
};

#[test]
fn external_types_are_reexported_from_stmt() {
    fn assert_same<T>(_: PhantomData<T>) {}

    assert_same::<Uuid>(PhantomData::<CoreUuid>);
    assert_same::<Decimal>(PhantomData::<CoreDecimal>);
    assert_same::<BigDecimal>(PhantomData::<CoreBigDecimal>);
    assert_same::<Timestamp>(PhantomData::<CoreTimestamp>);
    assert_same::<Zoned>(PhantomData::<CoreZoned>);
    assert_same::<Date>(PhantomData::<CoreDate>);
    assert_same::<Time>(PhantomData::<CoreTime>);
    assert_same::<DateTime>(PhantomData::<CoreDateTime>);
    assert_same::<IpCidr>(PhantomData::<CoreIpCidr>);
    assert_same::<IpInet>(PhantomData::<CoreIpInet>);
    assert_same::<MacAddr6>(PhantomData::<CoreMacAddr6>);
    assert_same::<MacAddr8>(PhantomData::<CoreMacAddr8>);
}
