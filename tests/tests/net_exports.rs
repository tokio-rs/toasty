use toasty::stmt::{IpCidr, IpInet, MacAddr6, MacAddr8};
use toasty_core::stmt::{
    IpCidr as CoreIpCidr, IpInet as CoreIpInet, MacAddr6 as CoreMacAddr6, MacAddr8 as CoreMacAddr8,
};

#[test]
fn network_types_are_reexported_from_stmt() {
    let _: IpCidr = "192.0.2.0/24".parse().unwrap();
    let _: IpInet = "192.0.2.1/24".parse().unwrap();
    let _: MacAddr6 = "ac:de:48:23:45:67".parse().unwrap();
    let _: MacAddr8 = "ac:de:48:23:45:67:89:ab".parse().unwrap();

    let _: CoreIpCidr = "192.0.2.0/24".parse().unwrap();
    let _: CoreIpInet = "192.0.2.1/24".parse().unwrap();
    let _: CoreMacAddr6 = "ac:de:48:23:45:67".parse().unwrap();
    let _: CoreMacAddr8 = "ac:de:48:23:45:67:89:ab".parse().unwrap();
}
