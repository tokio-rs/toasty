use super::{Field, Load};
use crate::stmt::{Expr, List, Path};
use toasty_core::{
    Result,
    stmt::{Type, Value},
};

macro_rules! impl_jiff_field {
    ($ty:ty, $name:ident, $lit:literal) => {
        impl Load for $ty {
            type Output = Self;

            fn ty() -> Type {
                Type::$name
            }

            fn load(value: Value) -> Result<Self> {
                match value {
                    Value::$name(v) => Ok(v),
                    _ => Err(toasty_core::Error::type_conversion(value, $lit)),
                }
            }

            fn reload(target: &mut Self, value: Value) -> Result<()> {
                *target = Self::load(value)?;
                Ok(())
            }
        }

        impl Field for $ty {
            type ExprTarget = Self;
            type Path<Origin> = Path<Origin, Self>;
            type ListPath<Origin> = Path<Origin, List<Self::ExprTarget>>;
            type Update<'a> = ();
            type Inner = Self;

            fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
                path
            }

            fn new_list_path<Origin>(
                path: Path<Origin, List<Self::ExprTarget>>,
            ) -> Self::ListPath<Origin> {
                path
            }

            fn new_update<'a>(
                _assignments: &'a mut toasty_core::stmt::Assignments,
                _projection: toasty_core::stmt::Projection,
            ) -> Self::Update<'a> {
            }

            fn key_constraint<Origin>(&self, target: Path<Origin, Self::Inner>) -> Expr<bool> {
                target.eq(self)
            }
        }
    };
}

impl_jiff_field!(jiff::Timestamp, Timestamp, "jiff::Timestamp");
impl_jiff_field!(jiff::Zoned, Zoned, "jiff::Zoned");
impl_jiff_field!(jiff::civil::Date, Date, "jiff::civil::Date");
impl_jiff_field!(jiff::civil::Time, Time, "jiff::civil::Time");
impl_jiff_field!(jiff::civil::DateTime, DateTime, "jiff::civil::DateTime");

/// No backend has a time zone column type, so a `TimeZone` has no `stmt::Type`
/// of its own — it is a `String` end to end, and these two functions are the
/// only place the encoding is defined.
///
/// The encoding is the RFC 9557 form a `Zoned` carries in its bracketed
/// annotation: an IANA identifier, a fixed offset, a POSIX TZ string, or
/// `Etc/Unknown`. jiff gives `TimeZone` neither `Display` nor `FromStr`.
static PRINTER: ::jiff::fmt::temporal::DateTimePrinter =
    ::jiff::fmt::temporal::DateTimePrinter::new();

static PARSER: ::jiff::fmt::temporal::DateTimeParser = ::jiff::fmt::temporal::DateTimeParser::new();

/// Printing canonicalizes `america/NEW_YORK` to `America/New_York`, so equal
/// zones store identical text.
///
/// Fails only for a zone with no text form, which jiff produces just for a
/// `TimeZone::system()` read from a non-symlinked `/etc/localtime`.
pub(crate) fn time_zone_to_text(tz: &::jiff::tz::TimeZone) -> Result<String> {
    let mut text = String::new();

    PRINTER.print_time_zone(tz, &mut text).map_err(|_| {
        toasty_core::Error::unsupported_feature(
            "this `jiff::tz::TimeZone` has no IANA identifier, fixed offset, \
             or POSIX rule string, so it cannot be stored",
        )
    })?;

    Ok(text)
}

/// IANA identifiers resolve against jiff's global time zone database, so a
/// zone renamed between tzdb releases fails to load on a host carrying only
/// the other name.
fn time_zone_from_text(text: &str) -> Result<::jiff::tz::TimeZone> {
    PARSER
        .parse_time_zone(text)
        .map_err(|e| toasty_core::Error::from_args(format_args!("invalid time zone {text:?}: {e}")))
}

impl Load for ::jiff::tz::TimeZone {
    type Output = Self;

    fn ty() -> Type {
        Type::String
    }

    fn load(value: Value) -> Result<Self> {
        match value {
            Value::String(text) => time_zone_from_text(&text),
            _ => Err(toasty_core::Error::type_conversion(
                value,
                "jiff::tz::TimeZone",
            )),
        }
    }

    fn reload(target: &mut Self, value: Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl Field for ::jiff::tz::TimeZone {
    type ExprTarget = Self;
    type Path<Origin> = Path<Origin, Self>;
    type ListPath<Origin> = Path<Origin, List<Self::ExprTarget>>;
    type Update<'a> = ();
    type Inner = Self;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(path: Path<Origin, List<Self::ExprTarget>>) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn key_constraint<Origin>(&self, target: Path<Origin, Self::Inner>) -> Expr<bool> {
        target.eq(self)
    }
}
