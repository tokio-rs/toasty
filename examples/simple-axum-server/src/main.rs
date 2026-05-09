use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use std::net::SocketAddr;

#[derive(Debug, toasty::Model, serde::Serialize)]
struct Todo {
    #[key]
    #[auto]
    id: uuid::Uuid,

    title: String,
}

#[derive(Debug, serde::Deserialize)]
struct NewTodo {
    title: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = toasty::Db::builder()
        .models(toasty::models!(crate::*))
        .connect(
            std::env::var("TOASTY_CONNECTION_URL")
                .as_deref()
                .unwrap_or("sqlite::memory:"),
        )
        .await?;

    db.push_schema().await?;

    // build our application with some routes
    let app = Router::new()
        .route("/todo/list", get(list_todos))
        .route("/todo/create", post(create_todo))
        .with_state(db);

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await?;

    Ok(())
}

#[axum::debug_handler]
async fn create_todo(
    State(mut db): State<toasty::Db>,
    Json(new_todo): Json<NewTodo>,
) -> Result<Json<Todo>, (StatusCode, String)> {
    let res = toasty::create!(Todo {
        title: new_todo.title,
    })
    .exec(&mut db)
    .await
    .map_err(internal_error)?;

    Ok(Json(res))
}

#[axum::debug_handler]
async fn list_todos(
    State(mut db): State<toasty::Db>,
) -> Result<Json<Vec<Todo>>, (StatusCode, String)> {
    let res = Todo::all().exec(&mut db).await.map_err(internal_error)?;
    Ok(Json(res))
}

/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
