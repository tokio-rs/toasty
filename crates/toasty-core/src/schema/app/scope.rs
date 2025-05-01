use super::*;

#[derive(Debug, PartialEq, Clone)]
pub struct ScopedQuery {
    /// Query used for this scope
    pub id: QueryId,

    /// Name of the query. This omits the scope type.
    pub name: Name,

    /// Query args supplied by the scope
    pub scope_args: Vec<Arg>,

    /// Query args supplied by the caller
    pub caller_args: Vec<Arg>,
}

impl ScopedQuery {
    pub(crate) fn new(query: &Query) -> Self {
        Self {
            id: query.id,
            name: scoped_query_name(&query.args[1..]),
            scope_args: query.args[..1].iter().map(Clone::clone).collect(),
            caller_args: query.args[1..].iter().map(Clone::clone).collect(),
        }
    }
}

fn scoped_query_name(args: &[Arg]) -> Name {
    let mut name = "find_by".to_string();

    for (i, arg) in args.iter().enumerate() {
        name.push('_');

        if i > 0 {
            name.push_str("and_");
        }

        name.push_str(&arg.name);
    }

    Name::new(&name)
}
