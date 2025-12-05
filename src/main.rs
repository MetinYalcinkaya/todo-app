use axum::{Json, Router, extract::State, routing::get};
use serde::Deserialize;
use std::sync::Arc;
use todo_server::model::{Task, TodoList};
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use tracing::{debug, info, instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Deserialize, Debug)]
struct CreateTodo {
    text: String,
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

    let state = Arc::new(RwLock::new(TodoList::default()));
    let app = Router::new()
        .route("/todos", get(list_todos).post(add_todo))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[instrument(skip(state))]
async fn list_todos(State(state): State<Arc<RwLock<TodoList>>>) -> Json<Vec<Task>> {
    let cloned_state = Arc::clone(&state);
    let read_guard = cloned_state.read().await;
    debug!("Listing all todos: {:?}", state);
    Json(read_guard.get_list())
}

#[instrument]
async fn add_todo(State(state): State<Arc<RwLock<TodoList>>>, Json(payload): Json<CreateTodo>) {
    let cloned_state = Arc::clone(&state);
    let mut write_guard = cloned_state.write().await;
    info!("Adding task: {}", payload.text);
    write_guard.add(payload.text);
}
