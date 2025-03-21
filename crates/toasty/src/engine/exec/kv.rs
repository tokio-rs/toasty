use super::*;

/// Key-value specific utilities
impl Exec<'_> {
    pub(super) async fn eval_using_input(
        &mut self,
        func: &eval::Func,
        input: &plan::Input,
    ) -> Result<stmt::Value> {
        let args = self.collect_input(input).await?;
        func.eval(&[args])
    }

    pub(super) async fn eval_maybe_using_input(
        &mut self,
        func: &eval::Func,
        input: &Option<plan::Input>,
    ) -> Result<stmt::Value> {
        match input {
            Some(input) => self.eval_using_input(func, input).await,
            None => func.eval(&[]),
        }
    }

    pub(super) async fn eval_keys_maybe_using_input(
        &mut self,
        func: &eval::Func,
        input: &Option<plan::Input>,
    ) -> Result<Vec<stmt::Value>> {
        match self.eval_maybe_using_input(func, input).await? {
            stmt::Value::List(keys) => Ok(keys),
            res => todo!("res={res:#?}"),
        }
    }

    pub(super) fn project_and_filter_output(
        &self,
        values: ValueStream,
        project: &eval::Func,
        filter: Option<&eval::Func>,
    ) -> ValueStream {
        // TODO: don't clone
        let project = project.clone();
        let filter = filter.cloned();

        ValueStream::from_stream(async_stream::try_stream! {
            for await value in values {
                let args = [value?];

                let select = match &filter {
                    Some(filter) if !filter.is_identity() => filter.eval_bool(&args)?,
                    _ => true,
                };

                if select {
                    let value = if project.is_identity() {
                        let [value] = args;
                        value
                    } else {
                        project.eval(&args)?
                    };

                    yield value;
                }
            }
        })
    }
}
