#[derive(Debug)]
pub(crate) struct ErrorSet {
    errors: Vec<syn::Error>,
}

impl ErrorSet {
    pub(crate) fn new() -> Self {
        Self { errors: vec![] }
    }

    pub(crate) fn push(&mut self, err: syn::Error) {
        self.errors.push(err);
    }

    pub(crate) fn collect(self) -> Option<syn::Error> {
        self.errors.into_iter().reduce(|mut acc, err| {
            acc.combine(err);
            acc
        })
    }
}
