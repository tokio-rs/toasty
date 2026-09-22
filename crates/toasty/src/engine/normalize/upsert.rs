use toasty_core::stmt;

use super::Normalize;

impl Normalize<'_> {
    /// Routes model `#[update]` values to branches that do not have an explicit
    /// assignment. Model `#[default]` values stay separate until lower-stage
    /// upsert normalization so shared mutations can use them as their initial
    /// value.
    pub(super) fn normalize_upsert_defaults(&mut self, insert: &mut stmt::Insert) {
        let Some(upsert) = &mut insert.upsert else {
            return;
        };

        for (projection, assignment) in std::mem::take(&mut upsert.update_defaults) {
            let stmt::Assignment::Set(expr) = assignment else {
                self.record(crate::Error::invalid_statement(
                    "upsert field defaults only support value assignments",
                ));
                return;
            };
            let create = upsert.defaults.contains(&projection)
                || upsert.shared.contains(&projection)
                || upsert.create.contains(&projection);
            let update = upsert.shared.contains(&projection) || upsert.update.contains(&projection);

            match (create, update) {
                (false, false) => upsert.shared.set(projection, expr),
                (false, true) => upsert.create.set(projection, expr),
                (true, false) => upsert.update.set(projection, expr),
                (true, true) => {}
            }
        }
    }
}
