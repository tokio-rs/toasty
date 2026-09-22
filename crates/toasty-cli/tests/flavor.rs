//! Tests for the mapping from [`Flavor`] to the capability used to lower a
//! schema. The mapping lives in `toasty::schema_dump::capability_for_flavor`
//! and is keyed by [`Flavor::as_str`], so the two can drift apart silently.

use clap::ValueEnum;
use toasty_cli::Flavor;

#[test]
fn every_flavor_lowers_to_a_sql_dialect() {
    for flavor in Flavor::value_variants() {
        // `capability()` panics when `as_str()` names a flavor the mapping
        // does not know, which is the drift this guards against.
        let capability = flavor.capability();

        assert!(
            capability.sql.is_some(),
            "flavor `{flavor}` must name a SQL dialect"
        );
    }
}

#[test]
fn flavor_names_round_trip_through_clap_and_serde() {
    for flavor in Flavor::value_variants() {
        let name = flavor.as_str();

        assert_eq!(Flavor::from_str(name, false).as_ref(), Ok(flavor));
        assert_eq!(
            serde_json::to_string(flavor).unwrap(),
            format!("\"{name}\"")
        );
    }
}
