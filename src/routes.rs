use tower_http::{trace::TraceLayer, compression::CompressionLayer, cors::CorsLayer};
use hyper::{http::{Request, header::{ACCEPT, ACCEPT_ENCODING, 
                                     AUTHORIZATION, CONTENT_TYPE, ORIGIN}}, 
           Body, StatusCode};
use axum::{Router, routing::{get, post}, Json, extract::State, response::{IntoResponse, Html}};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::app_state::AppState;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct Todo {
    id: i32,
    title: String,
    completed: bool,
}


#[derive(Debug, Serialize, Deserialize)]
struct CreateTodo {
    title: String,
    completed: bool,
}


async fn create_todo(
    State(state): State<AppState>,
    Json(payload): Json<CreateTodo>,
) -> impl IntoResponse {
    let local_pool = state.pool.clone();
    let todo = sqlx::query_as::<_,Todo>("insert into todos (title, completed) values ($1, $2) returning *")
        .bind(&payload.title)
        .bind(&payload.completed)
        .fetch_one(&local_pool).await;
     match todo {
         Ok(todo) => (StatusCode::OK, Json(todo)).into_response(),
         Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": err.to_string()}))).into_response(),
     }
}

#[axum::debug_handler]
async fn get_todos(State(state): State<AppState>) -> impl IntoResponse {
    let local_pool = state.pool.clone();
    let todos = sqlx::query_as::<_,Todo>("select * from todos")
                     .fetch_all(&local_pool).await;
    match todos {
        Ok(todo) => (StatusCode::OK, Json(todo)).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": err.to_string()}))).into_response(),
    }
}

#[axum::debug_handler]
async fn show_index(State(app_state): State<AppState>) -> impl IntoResponse {
    let res = app_state.tera.render("index.html", &tera::Context::new());
    match res {
        Ok(body) => (StatusCode::OK, Html(body)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Html(r#"
        <html>
            <head>
                <title>Oops.</title>
            </head>
            <body>
                <h1>Something really bad happened</h1>
            </body>
        </html>"#)).into_response()
    }

}

pub fn build_router(app_state: AppState) -> Router {
    // Trace every request
    let trace_layer =
        TraceLayer::new_for_http().on_request(|_: &Request<Body>, _span: &tracing::Span| {
            tracing::info!(message = "begin request")
        });

    // Set up CORS
    let cors_layer = CorsLayer::new()
        .allow_headers(vec![
            ACCEPT,
            ACCEPT_ENCODING,
            AUTHORIZATION,
            CONTENT_TYPE,
            ORIGIN,
        ])
        .allow_methods(tower_http::cors::Any)
        .allow_origin(tower_http::cors::Any);

    // Wrap an `axum::Router` with our state, CORS, Tracing, & Compression layers
    Router::new()
        .route("/todos", post(create_todo))
        .route("/todos", get(get_todos))
        .route("/", get(show_index))
        .layer(cors_layer)
        .layer(trace_layer)
        .layer(CompressionLayer::new().gzip(true).deflate(true))
        .with_state(app_state)
}
