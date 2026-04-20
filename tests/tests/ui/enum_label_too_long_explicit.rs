#[derive(toasty::Embed)]
enum Status {
    Active,
    #[column(variant = "this_label_is_way_too_long_and_exceeds_the_sixty_three_byte_maximum_allowed")]
    Inactive,
}

fn main() {}
