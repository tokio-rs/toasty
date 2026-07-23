type Payload = toasty::Json<Vec<String>>;

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    #[auto]
    id: i64,

    #[column(type = text)]
    payload: Payload,

    #[column("optional_payload", type = text)]
    optional: Option<Payload>,

    #[column(type = "jsonb")]
    deferred: toasty::Deferred<Payload>,

    #[column(type = json)]
    native: Payload,
}

fn main() {}
