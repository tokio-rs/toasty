use crate::{
    stmt::{Type, Value},
    Result,
};

impl Type {
    pub(crate) fn cast_chrono(&self, value: &Value) -> Result<Option<Value>> {
        Ok(Some(match (value, self) {
            // String -> chrono
            (Value::String(value), Type::ChronoDateTimeUtc) => {
                Value::ChronoDateTimeUtc(value.parse()?)
            }
            (Value::String(value), Type::ChronoNaiveDateTime) => {
                Value::ChronoNaiveDateTime(value.parse()?)
            }
            (Value::String(value), Type::ChronoNaiveDate) => Value::ChronoNaiveDate(value.parse()?),
            (Value::String(value), Type::ChronoNaiveTime) => Value::ChronoNaiveTime(value.parse()?),

            // chrono -> String
            (Value::ChronoDateTimeUtc(value), Type::String) => Value::String(value.to_string()),
            (Value::ChronoNaiveDateTime(value), Type::String) => {
                Value::String(value.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
            }
            (Value::ChronoNaiveDate(value), Type::String) => Value::String(value.to_string()),
            (Value::ChronoNaiveTime(value), Type::String) => Value::String(value.to_string()),

            _ => return Ok(None),
        }))
    }
}

#[cfg(test)]
mod chrono_cast_tests {
    use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

    use crate::stmt::{Type, Value};

    fn datetime_utc() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0).unwrap()
    }

    fn naive_datetime() -> NaiveDateTime {
        datetime_utc().naive_utc()
    }

    fn naive_date() -> NaiveDate {
        naive_datetime().date()
    }

    fn naive_time() -> NaiveTime {
        naive_datetime().time()
    }

    #[test]
    fn date_time_utc_roundtrip() {
        let dt = Value::ChronoDateTimeUtc(datetime_utc());
        let string = Type::String.cast_chrono(&dt).unwrap().unwrap();

        assert!(matches!(string, Value::String(_)));

        let dt_after = Type::ChronoDateTimeUtc
            .cast_chrono(&string)
            .unwrap()
            .unwrap();

        assert_eq!(dt, dt_after);
    }

    #[test]
    fn naive_date_time_roundtrip() {
        let dt = Value::ChronoNaiveDateTime(naive_datetime());
        let string = Type::String.cast_chrono(&dt).unwrap().unwrap();

        assert!(matches!(string, Value::String(_)));

        let dt_after = Type::ChronoNaiveDateTime
            .cast_chrono(&string)
            .unwrap()
            .unwrap();

        assert_eq!(dt, dt_after);
    }

    #[test]
    fn naive_date_roundtrip() {
        let dt = Value::ChronoNaiveDate(naive_date());
        let string = Type::String.cast_chrono(&dt).unwrap().unwrap();

        assert!(matches!(string, Value::String(_)));

        let dt_after = Type::ChronoNaiveDate.cast_chrono(&string).unwrap().unwrap();

        assert_eq!(dt, dt_after);
    }

    #[test]
    fn naive_time_roundtrip() {
        let dt = Value::ChronoNaiveTime(naive_time());
        let string = Type::String.cast_chrono(&dt).unwrap().unwrap();

        assert!(matches!(string, Value::String(_)));

        let dt_after = Type::ChronoNaiveTime.cast_chrono(&string).unwrap().unwrap();

        assert_eq!(dt, dt_after);
    }
}
