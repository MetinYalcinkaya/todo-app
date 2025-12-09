use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::delete,
    routing::get,
    routing::patch,
};
use serde::Deserialize;
use sqlx::query_builder::QueryBuilder;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use todo_common::{Priority, Task, TaskQuery};
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
    priority: Option<Priority>,
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
        .route("/todos", get(fetch_todos).post(add_todo))
        .route("/todos/{id}", patch(update_task))
        .route("/todos/{id}", delete(delete_task))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[instrument(skip(state))]
async fn fetch_todos(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TaskQuery>,
) -> Json<Vec<Task>> {
    let mut query = QueryBuilder::new("SELECT id, text, done, priority FROM tasks");

    let mut has_where = false;

    if let Some(done) = params.done {
        query.push(" WHERE done = ");
        query.push_bind(done);
        has_where = true;
    }

    if let Some(priority) = params.priority {
        if has_where {
            query.push(" AND ");
        } else {
            query.push(" WHERE ");
        }
        query.push("priority = ");
        query.push_bind(priority);
    }

    // let query = query.build().sql();

    let rows = query
        .build_query_as::<Task>()
        .fetch_all(&state.pool)
        .await
        .unwrap();

    info!("Fetching filtered todos");
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
        "UPDATE tasks SET text = COALESCE($1, text), done = COALESCE($2, done), priority = COALESCE($3, priority) WHERE id = $4",
        payload.text,
        payload.done,
        payload.priority,
        id
    )
    .execute(&state.pool)
    .await
    .unwrap();
}
