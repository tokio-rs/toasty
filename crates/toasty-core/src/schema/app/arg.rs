use crate::stmt;

/// A named, typed argument used in query parameterization.
///
/// `Arg` represents a single parameter that can be passed into a query
/// statement. Each argument has a name for identification and a type that
/// determines what values it accepts.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::Arg;
/// use toasty_core::stmt::Type;
///
/// let arg = Arg {
///     name: "user_id".to_string(),
///     ty: Type::String,
/// };
/// assert_eq!(arg.name, "user_id");
/// ```
#[derive(Debug, Clone)]
pub struct Arg {
    /// The argument's name, used to identify it in queries.
    pub name: String,

    /// The statement type this argument accepts.
    pub ty: stmt::Type,
}
