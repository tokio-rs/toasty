#![cfg(feature = "serde")]

#[test]
fn serde_round_trip() {
    let value = toasty::JsonValue(serde_json::json!({
        "request": {
            "method": "POST",
            "body": [1, null, true],
        },
    }));

    let encoded = serde_json::to_string(&value).unwrap();
    let decoded: toasty::JsonValue = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, value);
}
