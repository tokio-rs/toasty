#![allow(dead_code)]

#[derive(toasty::Model)]
struct ExternalStatementTypes {
    #[key]
    #[auto]
    id: toasty::stmt::Uuid,

    decimal: toasty::stmt::Decimal,
    big_decimal: toasty::stmt::BigDecimal,

    #[auto]
    created_at: toasty::stmt::Timestamp,
    #[auto]
    updated_at: toasty::stmt::Timestamp,
    zoned: toasty::stmt::Zoned,
    date: toasty::stmt::Date,
    time: toasty::stmt::Time,
    date_time: toasty::stmt::DateTime,

    cidr: toasty::stmt::IpCidr,
    inet: toasty::stmt::IpInet,
    mac_addr: toasty::stmt::MacAddr6,
    mac_addr_8: toasty::stmt::MacAddr8,
}
