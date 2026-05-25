// The deferred attribute has been removed. `Deferred<T>` controls deferred
// loading now.

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
