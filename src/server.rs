use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tokio::net::TcpListener;

use crate::consts::CHAT_MODELS;
use crate::upstream;

pub struct AppState {
    pub http_client: reqwest::Client,
    pub ustc_token: Option<String>,
    pub local_api_keys: Vec<String>,
}

fn json_error(status: StatusCode, message: impl Into<String>, error_type: &str) -> Response {
    let body = serde_json::json!({
        "error": {
            "message": message.into(),
            "type": error_type,
            "param": null,
            "code": null,
        }
    });
    (status, Json(body)).into_response()
}

async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    if !state.local_api_keys.is_empty() {
        let Some(key) = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|auth| auth.strip_prefix("Bearer "))
        else {
            return json_error(
                StatusCode::UNAUTHORIZED,
                "Missing API key. Provide it via Authorization: Bearer <key>.",
                "authentication_error",
            );
        };

        if !state.local_api_keys.iter().any(|k| k == key) {
            return json_error(
                StatusCode::UNAUTHORIZED,
                "Invalid API key.",
                "authentication_error",
            );
        }
    }

    next.run(request).await
}

#[derive(Debug, Deserialize)]
struct ChatCompletionRequest {
    model: String,
    messages: serde_json::Value,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    tools: Vec<serde_json::Value>,
}

async fn list_models() -> Json<serde_json::Value> {
    let models = CHAT_MODELS
        .iter()
        .map(|(id, show, reasoning, allow_tools)| {
            serde_json::json!({
                "id": id,
                "name": show,
                "object": "model",
                "created": 0,
                "owned_by": "ustc",
                "show": show,
                "supports_reasoning": reasoning,
                "supports_tools": allow_tools,
                "capabilities": {
                    "input": ["text"],
                    "output": ["text"],
                    "tools": allow_tools,
                    "reasoning": reasoning,
                    "streaming": true,
                },
                "context_window": 128000,
                "max_output_tokens": 32768,
            })
        })
        .collect::<Vec<_>>();

    Json(serde_json::json!({
        "object": "list",
        "data": models,
    }))
}

async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    let Some(token) = &state.ustc_token else {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "USTChat token not configured. Login required.",
            "server_error",
        );
    };

    let Some((model_name, _, _, _)) = CHAT_MODELS
        .iter()
        .find(|(id, _, _, _)| id.eq_ignore_ascii_case(&req.model))
    else {
        let available: Vec<&str> = CHAT_MODELS.iter().map(|(id, _, _, _)| *id).collect();
        return json_error(
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid model '{}'. Available: {}",
                req.model.trim(),
                available.join(", ")
            ),
            "invalid_request_error",
        );
    };

    let resp = upstream::request_chat(
        &state.http_client,
        token,
        model_name,
        &req.messages,
        req.tools,
    )
    .await;
    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            return json_error(
                StatusCode::BAD_GATEWAY,
                format!("Upstream request failed: {e}"),
                "server_error",
            );
        }
    };

    if req.stream {
        let byte_stream = resp.bytes_stream();

        Response::builder()
            .header(header::CONTENT_TYPE, "text/event-stream")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from_stream(byte_stream))
            .unwrap()
    } else {
        let body_text = match resp.text().await {
            Ok(text) => text,
            Err(e) => {
                return json_error(
                    StatusCode::BAD_GATEWAY,
                    format!("Failed to read response: {e}"),
                    "server_error",
                );
            }
        };

        Response::builder()
            .header(header::CONTENT_TYPE, "application/json")
            .body(body_text.into())
            .unwrap()
    }
}

pub async fn run(state: Arc<AppState>, host: &str, port: u16) -> std::io::Result<()> {
    let app = Router::new()
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    let addr = format!("{host}:{port}");
    let listener = TcpListener::bind(&addr).await?;

    println!("Proxy server listening on http://{addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
            eprintln!("Shutting down...");
        })
        .await
}
