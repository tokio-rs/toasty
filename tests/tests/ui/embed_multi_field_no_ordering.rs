// Ordering comparisons (`ge`, `lt`, …) exist only on canonical newtype
// embeds, where they pass through to the single inner column. A
// multi-field embed has no shared cross-backend ordering, so its fields
// struct stays eq-only and `.ge(...)` is a compile error.

#[derive(Debug, toasty::Embed)]
struct Point {
    x: i64,
    y: i64,
}

#[derive(Debug, toasty::Model)]
struct Pin {
    #[key]
    #[auto]
    id: uuid::Uuid,
    location: Point,
}

fn main() {
    let _ = Pin::filter(Pin::fields().location().ge(Point { x: 0, y: 0 }));
}
