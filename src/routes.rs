use sqlx::PgPool;
use tera::{Tera, Context};
use axum::{http::StatusCode, body::Body};
use tower_http::{trace::TraceLayer, compression::CompressionLayer, cors::CorsLayer};
use http::{Request, header::{ACCEPT, ACCEPT_ENCODING, 
                                     AUTHORIZATION, CONTENT_TYPE, ORIGIN}};
use axum::{Router, routing::{get, post}, Json, extract::{State, Path}, response::{IntoResponse, Html}, Form, debug_handler};

use serde::{Deserialize, Serialize};
use serde_json::json;
use anyhow::Result;

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
}


async fn insert_todo(payload: &CreateTodo, pool: PgPool) -> Result<Vec<Todo>> {
    sqlx::query_as::<_,Todo>("insert into todos (title) values ($1) returning *")
        .bind(&payload.title)
        .fetch_one(&pool).await?;
    let todos = sqlx::query_as::<_,Todo>("select * from todos")
                     .fetch_all(&pool).await?;
    Ok(todos)
}

async fn toggle_todo(todo_id: i32, pool: PgPool) -> Result<Todo> {
    let todo = sqlx::query_as::<_,Todo>("update todos set completed = not completed where id=$1 returning *")
        .bind(todo_id)
        .fetch_one(&pool).await?;
    Ok(todo)
}

#[debug_handler]
async fn toggle_status(
    State(app_state): State<AppState>,
    Path(id): Path<i32>, 
    ) -> (StatusCode, Html<String>) {
    let todo = toggle_todo(id, app_state.pool.clone()).await;
    match todo  {
        Ok(todo) => {
            let mut ctx = Context::new();
            ctx.insert("todo", &todo);
            render_template("todos/todo.html", &ctx, app_state.tera.clone())
        },
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from(r#"Error querying for todos"#))
        )
    }
}

#[debug_handler]
async fn create_todo(
    State(app_state): State<AppState>,
    Form(payload): Form<CreateTodo>,
) -> (StatusCode, Html<String>) {
    let local_pool = app_state.pool.clone();
    let todos = insert_todo(&payload, local_pool).await;
     match todos {
         Ok(todos) => {
            let mut ctx = Context::new();
            ctx.insert("todos", &todos);
            render_template("todos/form.html", &ctx, app_state.tera.clone())

         },
         Err(_) => (
             StatusCode::INTERNAL_SERVER_ERROR,
             Html(String::from(r#"Error querying for todos"#))
         )
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

fn render_template(name: &str, ctx: &tera::Context, tera: Tera) -> (StatusCode, Html<String>) {
    let res = tera.render(name, ctx);
    match res {
        Ok(body) => (StatusCode::OK, Html(body)),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, Html(format!(r#"
        <html>
            <head>
                <title>Oops.</title>
            </head>
            <body>
                <h1>Something really bad happened</h1>
                {:?}
            </body>
        </html>"#, err)))
    }
}

async fn show_index(State(app_state): State<AppState>) -> (StatusCode, Html<String>) {
    let local_pool = app_state.pool.clone();
    let result = sqlx::query_as::<_,Todo>("select * from todos")
                     .fetch_all(&local_pool).await;
    match result {
        Ok(todos) => {
            let mut ctx = Context::new();
            ctx.insert("todos", &todos);
            render_template("index.html", &ctx, app_state.tera.clone())
        },
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from(r#"Error querying for todos"#))
        )
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
        .route("/todo", post(create_todo))
        .route("/todos", get(get_todos))
        .route("/todos/:id/toggle", post(toggle_status))
        .route("/", get(show_index))
        .layer(cors_layer)
        .layer(trace_layer)
        .layer(CompressionLayer::new().gzip(true).deflate(true))
        .with_state(app_state)
}
