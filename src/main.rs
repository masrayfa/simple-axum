use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use axum::{
    error_handling::HandleErrorLayer,
    extract::{Query, State, Path},
    http::StatusCode,
    response::IntoResponse,
    BoxError, Router, Json,
};
use serde::{Deserialize, Serialize};
use tower::{ServiceBuilder};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    // tracing_subscriber::registry()
    //     .with(
    //         tracing_subscriber::EnvFilter::try_from_default_env()
    //             .unwrap_or_else(|_| "example_todos=debug, tower_http=debug".into()),
    //     )
    //     .with(tracing_subscriber::fmt::layer())
    //     .init();

    let db = DB::default();

    let app: Router = Router::new()
        .route("/todos", axum::routing::get(todos_index).post(create_todo))
        .route("/:id", axum::routing::get(get_todo_by_id))
        .route("/:id", axum::routing::patch(update_todo))
        .route("/:id", axum::routing::delete(delete_todo))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|error: BoxError| async move {
                    if error.is::<tower::timeout::error::Elapsed>() {
                        Ok(StatusCode::REQUEST_TIMEOUT)
                    } else {
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Unhandled internal error: {}", error),
                        ))
                    }
                }))
                .timeout(Duration::from_secs(10))
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        )
        .with_state(db);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Deserialize)]
pub struct CreateTodo {
    pub title: String,
}

pub async fn create_todo(State(db): State<DB>,Json(payload): Json<CreateTodo>) -> impl IntoResponse {
    let id = Uuid::new_v4();

    let todo = Todo {
        id,
        title: payload.title,
        completed: false,
    };

    db.write().unwrap().insert(id, todo.clone());

    (StatusCode::CREATED, Json(todo))
}

#[derive(Debug, Deserialize, Default)]
pub struct Pagination {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}
async fn todos_index(
    pagination: Option<Query<Pagination>>,
    State(db): State<DB>,
) -> impl IntoResponse {

    let todos = db.read().unwrap();

    let Query(pagination) = pagination.unwrap_or_default();

    // let todos = todos.values().cloned().collect::<Vec<_>>();
    // let todos = todos.values().skip(pagination.offset.unwrap_or_default()).take(pagination.limit.unwrap_or_default()).cloned().collect::<Vec<_>>();
     let todos = todos
        .values()
        .skip(pagination.offset.unwrap_or(0))
        .take(pagination.limit.unwrap_or(usize::MAX))
        .cloned()
        .collect::<Vec<_>>();

    Json(todos)
}

async fn get_todo_by_id(
    Path(id): Path<Uuid>,
    State(db): State<DB>,
) -> Result<impl IntoResponse, StatusCode> { 
    let todos = db.read().unwrap();

    let todo = todos.get(&id).cloned().ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(todo))
}

#[derive(Debug, Deserialize)]
struct UpdateTodo {
    title: Option<String>,
    completed: Option<bool>,
}

async fn update_todo(
    Path(id): Path<Uuid>,
    State(db): State<DB>,
    Json(input): Json<UpdateTodo>,
) -> impl IntoResponse {
    let mut todos = db.write().unwrap();

    let todo = todos.get_mut(&id).unwrap();

    if let Some(title) = input.title {
        todo.title = title;
    }

    if let Some(completed) = input.completed {
        todo.completed = completed;
    }
}

async fn delete_todo(
    Path(id): Path<Uuid>,
    State(db): State<DB>,
) -> impl IntoResponse {
    let mut todos = db.write().unwrap();

    todos.remove(&id);
}

type DB = Arc<RwLock<HashMap<uuid::Uuid, Todo>>>;

#[derive(Debug, Serialize, Clone)]
pub struct Todo {
    id: Uuid,
    title: String,
    completed: bool,
}
