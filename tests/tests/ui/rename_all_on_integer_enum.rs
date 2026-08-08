// `rename_all` derives string labels, so it is rejected on an enum that stores
// integer discriminants.

#[derive(toasty::Embed)]
#[column(rename_all = "PascalCase")]
enum Priority {
    #[column(variant = 1)]
    Low,
    #[column(variant = 2)]
    High,
}

fn main() {}
