// `#[deferred]` is not yet supported on embedded enum variant fields.
// Toasty must reject the attribute at macro time rather than silently
// dropping it.

#[derive(toasty::Embed)]
enum ContactInfo {
    Email {
        #[deferred]
        address: toasty::Deferred<String>,
    },
    Phone {
        number: String,
    },
}

fn main() {}
