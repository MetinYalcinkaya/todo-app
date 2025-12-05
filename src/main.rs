use axum::{Json, Router, extract::State, routing::get};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use todo_server::model::{Task, TodoList};

struct AppState {
    todo_list: Arc<RwLock<TodoList>>,
}

#[tokio::main]
async fn main() {
    // let state = Arc::new(RwLock::new(TodoList::default()));
    // let cloned = Arc::clone(&state);
    // let mut write_guard = cloned.write().unwrap();
    // write_guard.add(String::from("testing"));
    // drop(write_guard);
    // let read_guard = cloned.read().unwrap();
    // let list = read_guard.get_list();
    // dbg!(list);
    // drop(read_guard);

    let state = Arc::new(RwLock::new(TodoList::default()));
    let app = Router::new()
        .route("/todos", get(list_todos).post(add_todo))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn list_todos(State(state): State<Arc<RwLock<TodoList>>>) -> Json<Vec<Task>> {
    let cloned_state = Arc::clone(&state);
    let read_guard = cloned_state.read().unwrap();
    read_guard.get_list()
}

async fn add_todo(State(state): State<Arc<RwLock<TodoList>>>, Json(payload): Json<String>) {
    let cloned_state = Arc::clone(&state);
    let mut write_guard = cloned_state.write().unwrap();
    write_guard.add(payload);
}
