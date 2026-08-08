// A `bool` field cannot be stored in an `int` column.

#[derive(Debug, toasty::Model)]
struct Flag {
    #[key]
    #[auto]
    id: i64,

    #[column(type = i64)]
    enabled: bool,
}

fn main() {}
