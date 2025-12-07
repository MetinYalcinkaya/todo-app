use axum::{
    Json, Router,
    extract::{Path, State},
    routing::delete,
    routing::get,
    routing::patch,
};
use serde::Deserialize;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use todo_common::{Priority, Task};
use tower_http::trace::TraceLayer;
use tracing::{info, instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    pool: sqlx::SqlitePool,
}

#[derive(Deserialize, Debug)]
struct CreateTodo {
    text: String,
}

#[derive(Deserialize, Debug)]
struct UpdateTodo {
    text: Option<String>,
    done: Option<bool>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "todo_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().unwrap();
    let db_url = std::env::var("DATABASE_URL").unwrap();
    let pool = SqlitePoolOptions::new().connect(&db_url).await.unwrap();

    let state = Arc::new(AppState { pool });
    let app = Router::new()
        .route("/todos", get(list_todos).post(add_todo))
        .route("/todos/{id}", patch(update_task))
        .route("/todos/{id}", delete(delete_task))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[instrument(skip(state))]
async fn list_todos(State(state): State<Arc<AppState>>) -> Json<Vec<Task>> {
    let rows = sqlx::query_as!(
        Task,
        r#"
        SELECT id, text, done, priority as "priority: Priority" FROM tasks
        "#
    )
    .fetch_all(&state.pool)
    .await
    .unwrap();
    info!("Listing all todos");
    Json(rows)
}

#[instrument(skip(state))]
async fn add_todo(State(state): State<Arc<AppState>>, Json(payload): Json<CreateTodo>) {
    let sql = "INSERT INTO tasks (text, done, priority) values ($1, false, 'Low')";
    info!("Adding task to database: {}", payload.text);
    sqlx::query(sql)
        .bind(payload.text)
        .execute(&state.pool)
        .await
        .unwrap();
}

#[instrument(skip(state))]
async fn delete_task(State(state): State<Arc<AppState>>, Path(id): Path<i64>) {
    info!("Deleting task ID: {}", id);
    sqlx::query!("DELETE FROM tasks WHERE id = $1", id)
        .execute(&state.pool)
        .await
        .unwrap();
}

#[instrument(skip(state))]
async fn update_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateTodo>,
) {
    info!("Updating task ID: {} with {:?}", id, payload);
    // COALESCE returns first non null expression
    // so either value from payload, or the value that's already set
    sqlx::query!(
        "UPDATE tasks SET text = COALESCE($1, text), done = COALESCE($2, done) WHERE id = $3",
        payload.text,
        payload.done,
        id
    )
    .execute(&state.pool)
    .await
    .unwrap();
}
