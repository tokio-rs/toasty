use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
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
        .route("/todo/{id}/update", put(update_todo))
        .route("/todo/{id}/delete", delete(delete_todo))
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

async fn update_todo(
    State(mut db): State<toasty::Db>,
    Path(id): Path<uuid::Uuid>,
    Json(updated_todo): Json<NewTodo>,
) -> Result<Json<Todo>, (StatusCode, String)> {
    let mut todo = Todo::get_by_id(&mut db, id).await.map_err(internal_error)?;
    todo.update()
        .title(&updated_todo.title)
        .exec(&mut db)
        .await
        .map_err(internal_error)?;

    Ok(Json(Todo {
        id: todo.id,
        title: updated_todo.title,
    }))
}

async fn delete_todo(
    State(mut db): State<toasty::Db>,
    Path(id): Path<uuid::Uuid>,
) -> Result<(), (StatusCode, String)> {
    let todo = Todo::get_by_id(&mut db, id).await.map_err(internal_error)?;
    todo.delete().exec(&mut db).await.map_err(internal_error)?;

    Ok(())
}

/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
