use jiff::tz::TimeZone;

use crate::{
    stmt::{Type, Value},
    Result,
};

impl Type {
    pub fn cast_jiff(&self, value: &Value) -> Result<Option<Value>> {
        Ok(Some(match (value, self) {
            // String -> jiff
            (Value::String(value), Type::Timestamp) => {
                let v = value.clone();
                Value::Timestamp(
                    value.parse().map_err(|_| {
                        crate::Error::type_conversion(Value::String(v), "Timestamp")
                    })?,
                )
            }
            (Value::String(value), Type::Zoned) => {
                let v = value.clone();
                Value::Zoned(
                    value
                        .parse()
                        .map_err(|_| crate::Error::type_conversion(Value::String(v), "Zoned"))?,
                )
            }
            (Value::String(value), Type::Date) => {
                let v = value.clone();
                Value::Date(
                    value
                        .parse()
                        .map_err(|_| crate::Error::type_conversion(Value::String(v), "Date"))?,
                )
            }
            (Value::String(value), Type::Time) => {
                let v = value.clone();
                Value::Time(
                    value
                        .parse()
                        .map_err(|_| crate::Error::type_conversion(Value::String(v), "Time"))?,
                )
            }
            (Value::String(value), Type::DateTime) => {
                let v = value.clone();
                Value::DateTime(
                    value
                        .parse()
                        .map_err(|_| crate::Error::type_conversion(Value::String(v), "DateTime"))?,
                )
            }

            // jiff -> String
            (Value::Timestamp(value), Type::String) => Value::String(value.to_string()),
            (Value::Zoned(value), Type::String) => Value::String(value.to_string()),
            (Value::Date(value), Type::String) => Value::String(value.to_string()),
            (Value::Time(value), Type::String) => Value::String(value.to_string()),
            (Value::DateTime(value), Type::String) => Value::String(value.to_string()),

            // UTC <-> Zoned
            (Value::Timestamp(value), Type::Zoned) => Value::Zoned(value.to_zoned(TimeZone::UTC)),
            (Value::Zoned(value), Type::Timestamp) => Value::Timestamp(value.into()),

            // UTC <-> Civil
            (Value::Timestamp(value), Type::DateTime) => {
                Value::DateTime(value.to_zoned(TimeZone::UTC).into())
            }
            (Value::DateTime(value), Type::Timestamp) => Value::Timestamp(
                value
                    .to_zoned(TimeZone::UTC)
                    .expect("value was too close to minimum or maximum DateTime")
                    .into(),
            ),

            // Zoned <-> Civil
            (Value::Zoned(value), Type::DateTime) => Value::DateTime(value.into()),
            (Value::DateTime(value), Type::Zoned) => Value::Zoned(
                value
                    .to_zoned(TimeZone::UTC)
                    .expect("value was too close to minimum or maximum DateTime"),
            ),

            _ => return Ok(None),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper functions to create test values
    fn timestamp() -> jiff::Timestamp {
        jiff::Timestamp::from_second(1_609_459_200).unwrap() // 2021-01-01 00:00:00 UTC
    }

    fn zoned() -> jiff::Zoned {
        timestamp().to_zoned(TimeZone::UTC)
    }

    fn date() -> jiff::civil::Date {
        jiff::civil::date(2021, 1, 1)
    }

    fn time() -> jiff::civil::Time {
        jiff::civil::time(12, 30, 45, 0)
    }

    fn datetime() -> jiff::civil::DateTime {
        jiff::civil::date(2021, 1, 1).at(12, 30, 45, 0)
    }

    // ===== String -> jiff conversions =====

    #[test]
    fn test_string_to_timestamp() {
        let value = Value::String("2021-01-01T00:00:00Z".to_string());
        let result = Type::Timestamp.cast_jiff(&value).unwrap();
        assert!(matches!(result.unwrap(), Value::Timestamp(_)));
    }

    #[test]
    fn test_string_to_zoned() {
        let value = Value::String("2021-01-01T00:00:00Z[UTC]".to_string());
        let result = Type::Zoned.cast_jiff(&value).unwrap();
        assert!(matches!(result.unwrap(), Value::Zoned(_)));
    }

    #[test]
    fn test_string_to_date() {
        let value = Value::String("2021-01-01".to_string());
        let result = Type::Date.cast_jiff(&value).unwrap();
        assert!(matches!(result.unwrap(), Value::Date(_)));
    }

    #[test]
    fn test_string_to_time() {
        let value = Value::String("12:30:45".to_string());
        let result = Type::Time.cast_jiff(&value).unwrap();
        assert!(matches!(result.unwrap(), Value::Time(_)));
    }

    #[test]
    fn test_string_to_datetime() {
        let value = Value::String("2021-01-01T12:30:45".to_string());
        let result = Type::DateTime.cast_jiff(&value).unwrap();
        assert!(matches!(result.unwrap(), Value::DateTime(_)));
    }

    // ===== jiff -> String conversions =====

    #[test]
    fn test_timestamp_to_string() {
        let value = Value::Timestamp(timestamp());
        let result = Type::String.cast_jiff(&value).unwrap();
        match result.unwrap() {
            Value::String(s) => assert!(!s.is_empty()),
            _ => panic!("Expected String value"),
        }
    }

    #[test]
    fn test_zoned_to_string() {
        let value = Value::Zoned(zoned());
        let result = Type::String.cast_jiff(&value).unwrap();
        match result.unwrap() {
            Value::String(s) => assert!(!s.is_empty()),
            _ => panic!("Expected String value"),
        }
    }

    #[test]
    fn test_date_to_string() {
        let value = Value::Date(date());
        let result = Type::String.cast_jiff(&value).unwrap();
        match result.unwrap() {
            Value::String(s) => assert_eq!(s, "2021-01-01"),
            _ => panic!("Expected String value"),
        }
    }

    #[test]
    fn test_time_to_string() {
        let value = Value::Time(time());
        let result = Type::String.cast_jiff(&value).unwrap();
        match result.unwrap() {
            Value::String(s) => assert_eq!(s, "12:30:45"),
            _ => panic!("Expected String value"),
        }
    }

    #[test]
    fn test_datetime_to_string() {
        let value = Value::DateTime(datetime());
        let result = Type::String.cast_jiff(&value).unwrap();
        match result.unwrap() {
            Value::String(s) => assert!(!s.is_empty()),
            _ => panic!("Expected String value"),
        }
    }

    // ===== UTC <-> Zoned conversions =====

    #[test]
    fn test_timestamp_to_zoned() {
        let value = Value::Timestamp(timestamp());
        let result = Type::Zoned.cast_jiff(&value).unwrap();
        match result.unwrap() {
            Value::Zoned(z) => {
                let expected = timestamp().to_zoned(TimeZone::UTC);
                assert_eq!(z.timestamp(), expected.timestamp());
            }
            _ => panic!("Expected Zoned value"),
        }
    }

    #[test]
    fn test_zoned_to_timestamp() {
        let value = Value::Zoned(zoned());
        let result = Type::Timestamp.cast_jiff(&value).unwrap();
        match result.unwrap() {
            Value::Timestamp(ts) => {
                let expected: jiff::Timestamp = zoned().into();
                assert_eq!(ts, expected);
            }
            _ => panic!("Expected Timestamp value"),
        }
    }

    // ===== UTC <-> Civil (DateTime) conversions =====

    #[test]
    fn test_timestamp_to_datetime() {
        let value = Value::Timestamp(timestamp());
        let result = Type::DateTime.cast_jiff(&value).unwrap();
        match result.unwrap() {
            Value::DateTime(dt) => {
                let expected: jiff::civil::DateTime = timestamp().to_zoned(TimeZone::UTC).into();
                assert_eq!(dt, expected);
            }
            _ => panic!("Expected DateTime value"),
        }
    }

    #[test]
    fn test_datetime_to_timestamp() {
        let value = Value::DateTime(datetime());
        let result = Type::Timestamp.cast_jiff(&value).unwrap();
        assert!(matches!(result.unwrap(), Value::Timestamp(_)));
    }

    // ===== Zoned <-> Civil conversions =====

    #[test]
    fn test_zoned_to_datetime() {
        let value = Value::Zoned(zoned());
        let result = Type::DateTime.cast_jiff(&value).unwrap();
        match result.unwrap() {
            Value::DateTime(dt) => {
                let expected: jiff::civil::DateTime = zoned().into();
                assert_eq!(dt, expected);
            }
            _ => panic!("Expected DateTime value"),
        }
    }

    #[test]
    fn test_datetime_to_zoned() {
        let value = Value::DateTime(datetime());
        let result = Type::Zoned.cast_jiff(&value).unwrap();
        assert!(matches!(result.unwrap(), Value::Zoned(_)));
    }

    // ===== Invalid conversions (should return None) =====

    #[test]
    fn test_invalid_conversion_returns_none() {
        // Try converting a Timestamp to Date (not supported)
        let value = Value::Timestamp(timestamp());
        let result = Type::Date.cast_jiff(&value).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_invalid_conversion_date_to_time() {
        let value = Value::Date(date());
        let result = Type::Time.cast_jiff(&value).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_invalid_conversion_time_to_date() {
        let value = Value::Time(time());
        let result = Type::Date.cast_jiff(&value).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_invalid_conversion_date_to_timestamp() {
        let value = Value::Date(date());
        let result = Type::Timestamp.cast_jiff(&value).unwrap();
        assert!(result.is_none());
    }

    // ===== Round-trip conversions =====

    #[test]
    fn test_roundtrip_timestamp_string() {
        let original = Value::Timestamp(timestamp());
        let as_string = Type::String.cast_jiff(&original).unwrap().unwrap();
        let back_to_timestamp = Type::Timestamp.cast_jiff(&as_string).unwrap().unwrap();

        match (original, back_to_timestamp) {
            (Value::Timestamp(orig), Value::Timestamp(roundtrip)) => {
                assert_eq!(orig.as_second(), roundtrip.as_second());
            }
            _ => panic!("Round-trip failed"),
        }
    }

    #[test]
    fn test_roundtrip_date_string() {
        let original = Value::Date(date());
        let as_string = Type::String.cast_jiff(&original).unwrap().unwrap();
        let back_to_date = Type::Date.cast_jiff(&as_string).unwrap().unwrap();

        assert_eq!(original, back_to_date);
    }

    #[test]
    fn test_roundtrip_time_string() {
        let original = Value::Time(time());
        let as_string = Type::String.cast_jiff(&original).unwrap().unwrap();
        let back_to_time = Type::Time.cast_jiff(&as_string).unwrap().unwrap();

        assert_eq!(original, back_to_time);
    }

    #[test]
    fn test_roundtrip_timestamp_zoned() {
        let original = Value::Timestamp(timestamp());
        let as_zoned = Type::Zoned.cast_jiff(&original).unwrap().unwrap();
        let back_to_timestamp = Type::Timestamp.cast_jiff(&as_zoned).unwrap().unwrap();

        assert_eq!(original, back_to_timestamp);
    }

    #[test]
    fn test_roundtrip_timestamp_datetime() {
        let original = Value::Timestamp(timestamp());
        let as_datetime = Type::DateTime.cast_jiff(&original).unwrap().unwrap();
        let back_to_timestamp = Type::Timestamp.cast_jiff(&as_datetime).unwrap().unwrap();

        match (original, back_to_timestamp) {
            (Value::Timestamp(orig), Value::Timestamp(roundtrip)) => {
                assert_eq!(orig.as_second(), roundtrip.as_second());
            }
            _ => panic!("Round-trip failed"),
        }
    }

    // ===== Error handling =====

    #[test]
    fn test_invalid_string_to_timestamp_fails() {
        let value = Value::String("not-a-timestamp".to_string());
        let result = Type::Timestamp.cast_jiff(&value);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_string_to_date_fails() {
        let value = Value::String("invalid-date".to_string());
        let result = Type::Date.cast_jiff(&value);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_string_to_time_fails() {
        let value = Value::String("25:99:99".to_string());
        let result = Type::Time.cast_jiff(&value);
        assert!(result.is_err());
    }
}
