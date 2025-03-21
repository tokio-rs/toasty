use toasty_core::stmt;

pub trait Params {
    fn push(&mut self, param: &stmt::Value);
}

impl Params for Vec<stmt::Value> {
    fn push(&mut self, value: &stmt::Value) {
        self.push(value.clone());
    }
}