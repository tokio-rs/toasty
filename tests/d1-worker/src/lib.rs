#![cfg(target_arch = "wasm32")]

use std::sync::Arc;

use serde::Serialize;
use toasty_core::driver::Driver;
use toasty_driver_integration_suite::{Setup, Test};
use worker::{Context, Env, Request, Response, send::IntoSendFuture};

const TESTS: &[&str] = &[
    "crud_basic::crud_one_string::id_uuid",
    "type_primitives::ty_i64",
    "type_primitives::ty_u64",
    "type_primitives::ty_uuid",
    "raw_sql::statement_and_query_on_db",
    "select_projection::select_tuple",
    "filter_like::like_basic",
    "starts_with::starts_with_case_sensitive",
    "crud_upsert::upsert_by_primary_key_creates_then_updates::id_uuid",
    "type_document::vec_struct_create_get::id_uuid",
    "batch_query::batch_same_model::id_uuid",
    "batch_rollback::batch_two_creates_rolls_back_on_second_failure::id_uuid",
];
const COMPATIBILITY_DATE: &str = "2026-07-25";

#[derive(Clone)]
struct D1Setup {
    env: Env,
}

#[async_trait::async_trait]
impl Setup for D1Setup {
    fn driver(&self) -> Box<dyn Driver> {
        let database = self.env.d1("DB").expect("DB binding is configured");
        Box::new(toasty_driver_d1::D1::new("DB", database))
    }

    async fn delete_table(&self, name: &str) {
        self.try_delete_table(name)
            .await
            .expect("D1 test table cleanup failed");
    }

    async fn try_delete_table(&self, name: &str) -> toasty::Result<()> {
        let database = self
            .env
            .d1("DB")
            .map_err(|error| driver_error("read DB binding", error))?;
        let quoted = name.replace('"', "\"\"");
        let result = database
            .prepare(format!("DROP TABLE IF EXISTS \"{quoted}\""))
            .run()
            .into_send()
            .await
            .map_err(|error| driver_error("drop test table", error))?;
        if result.success() {
            Ok(())
        } else {
            Err(driver_error(
                "drop test table",
                result.error().unwrap_or_else(|| "unknown D1 error".into()),
            ))
        }
    }
}

fn driver_error(context: &str, error: impl std::fmt::Display) -> toasty::Error {
    toasty::Error::driver_operation_failed(std::io::Error::other(error.to_string())).context(
        toasty::Error::from_args(format_args!("D1 test runner failed to {context}")),
    )
}

#[derive(Serialize)]
struct TestManifest {
    compatibility_date: &'static str,
    tests: &'static [&'static str],
}

#[derive(Serialize)]
struct RunResult<'a> {
    name: &'a str,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    operations: String,
}

#[worker::event(fetch)]
async fn fetch(request: Request, env: Env, _context: Context) -> worker::Result<Response> {
    let url = request.url()?;
    match url.path() {
        "/tests" => Response::from_json(&TestManifest {
            compatibility_date: COMPATIBILITY_DATE,
            tests: TESTS,
        }),
        "/run" => {
            let name = url
                .query_pairs()
                .find_map(|(key, value)| (key == "name").then(|| value.into_owned()))
                .ok_or_else(|| worker::Error::RustError("missing test name".into()))?;
            let outcome = run(&name, env).await;
            let response = match outcome.result {
                Ok(()) => RunResult {
                    name: &name,
                    status: "passed",
                    error: None,
                    operations: outcome.operations,
                },
                Err(error) => RunResult {
                    name: &name,
                    status: "failed",
                    error: Some(error),
                    operations: outcome.operations,
                },
            };
            Response::from_json(&response)
        }
        _ => Response::error("not found", 404),
    }
}

struct TestOutcome {
    result: Result<(), String>,
    operations: String,
}

async fn run(name: &str, env: Env) -> TestOutcome {
    use toasty_driver_integration_suite::tests;

    let setup = Arc::new(D1Setup { env });
    let mut test = Test::new_async(setup);
    let result = match name {
        "crud_basic::crud_one_string::id_uuid" => {
            test.run_async(async move |test| {
                tests::crud_basic::crud_one_string::id_uuid(test).await
            })
            .await
        }
        "type_primitives::ty_i64" => {
            test.run_async(async move |test| tests::type_primitives::ty_i64(test).await)
                .await
        }
        "type_primitives::ty_u64" => {
            test.run_async(async move |test| tests::type_primitives::ty_u64(test).await)
                .await
        }
        "type_primitives::ty_uuid" => {
            test.run_async(async move |test| tests::type_primitives::ty_uuid(test).await)
                .await
        }
        "raw_sql::statement_and_query_on_db" => {
            test.run_async(async move |test| tests::raw_sql::statement_and_query_on_db(test).await)
                .await
        }
        "select_projection::select_tuple" => {
            test.run_async(async move |test| tests::select_projection::select_tuple(test).await)
                .await
        }
        "filter_like::like_basic" => {
            test.run_async(async move |test| tests::filter_like::like_basic(test).await)
                .await
        }
        "starts_with::starts_with_case_sensitive" => {
            test.run_async(async move |test| {
                tests::starts_with::starts_with_case_sensitive(test).await
            })
            .await
        }
        "crud_upsert::upsert_by_primary_key_creates_then_updates::id_uuid" => {
            test.run_async(async move |test| {
                tests::crud_upsert::upsert_by_primary_key_creates_then_updates::id_uuid(test).await
            })
            .await
        }
        "type_document::vec_struct_create_get::id_uuid" => {
            test.run_async(async move |test| {
                tests::type_document::vec_struct_create_get::id_uuid(test).await
            })
            .await
        }
        "batch_query::batch_same_model::id_uuid" => {
            test.run_async(async move |test| {
                tests::batch_query::batch_same_model::id_uuid(test).await
            })
            .await
        }
        "batch_rollback::batch_two_creates_rolls_back_on_second_failure::id_uuid" => {
            test.run_async(async move |test| {
                tests::batch_rollback::batch_two_creates_rolls_back_on_second_failure::id_uuid(test)
                    .await
            })
            .await
        }
        _ => Err(format!("unknown test: {name}")),
    };
    TestOutcome {
        result,
        operations: format!("{:?}", test.log()),
    }
}
