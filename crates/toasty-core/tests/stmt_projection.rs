use toasty_core::stmt::Projection;

#[test]
fn test_resolves_to() {
    let identity1 = Projection::identity();
    let identity2 = Projection::identity();
    assert!(identity1.resolves_to(identity2));

    let single1 = Projection::from([0]);
    let single2 = Projection::from([0]);
    let single3 = Projection::from([1]);
    assert!(single1.resolves_to(single2.clone()));
    assert!(!single1.resolves_to(single3));

    let multi1 = Projection::from([0, 1, 2]);
    let multi2 = Projection::from([0, 1, 2]);
    let multi3 = Projection::from([0, 1, 3]);
    assert!(multi1.resolves_to(multi2.clone()));
    assert!(!multi1.resolves_to(multi3));

    // Different types should not resolve to each other
    let identity3 = Projection::identity();
    let single4 = Projection::from([0]);
    let multi4 = Projection::from([0, 1]);
    assert!(!identity3.resolves_to(single4));
    assert!(!single2.resolves_to(multi4));
}
