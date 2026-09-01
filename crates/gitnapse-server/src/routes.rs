//! HTTP routes implementing the GitNapse protocol (v1) + embedded web UI.
//!
//! The operation contract is defined in `docs/PROTOCOL.md`.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use base64::Engine;
use gitnapse_protocol::{ContentDto, ErrorDto, HealthDto};
use serde::Deserialize;
use std::sync::Arc;

use crate::convert::{node_dto, repo_dto};
use crate::service::ApiService;
use crate::webui::INDEX_HTML;

pub fn router(service: Arc<ApiService>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/api/v1/search", get(search))
        .route("/api/v1/repos/branches", get(branches))
        .route("/api/v1/repos/tree", get(tree))
        .route("/api/v1/repos/content", get(content))
        .with_state(service)
}

async fn index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

async fn health() -> Json<HealthDto> {
    Json(HealthDto {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

fn err_response(message: String) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorDto { error: message }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
struct SearchParams {
    q: Option<String>,
    page: Option<u32>,
    per_page: Option<u8>,
}

async fn search(
    State(service): State<Arc<ApiService>>,
    Query(params): Query<SearchParams>,
) -> Response {
    let query = params.q.unwrap_or_default();
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(30).clamp(1, 100);
    let service = service.clone();
    let result =
        tokio::task::spawn_blocking(move || service.search_repos(&query, page, per_page)).await;
    match result {
        Ok(Ok(repos)) => Json(repos.iter().map(repo_dto).collect::<Vec<_>>()).into_response(),
        Ok(Err(e)) => err_response(format!("search failed: {e}")),
        Err(e) => err_response(format!("task failed: {e}")),
    }
}

#[derive(Debug, Deserialize)]
struct RepoQuery {
    repo: String,
}

async fn branches(
    State(service): State<Arc<ApiService>>,
    Query(params): Query<RepoQuery>,
) -> Response {
    let service = service.clone();
    let result = tokio::task::spawn_blocking(move || service.branches(&params.repo)).await;
    match result {
        Ok(Ok(branches)) => Json(branches).into_response(),
        Ok(Err(e)) => err_response(format!("branches failed: {e}")),
        Err(e) => err_response(format!("task failed: {e}")),
    }
}

#[derive(Debug, Deserialize)]
struct TreeQuery {
    repo: String,
    r#ref: Option<String>,
}

async fn tree(State(service): State<Arc<ApiService>>, Query(params): Query<TreeQuery>) -> Response {
    let git_ref = params.r#ref.unwrap_or_else(|| "HEAD".to_string());
    let service = service.clone();
    let result = tokio::task::spawn_blocking(move || service.tree(&params.repo, &git_ref)).await;
    match result {
        Ok(Ok(nodes)) => Json(nodes.iter().map(node_dto).collect::<Vec<_>>()).into_response(),
        Ok(Err(e)) => err_response(format!("tree failed: {e}")),
        Err(e) => err_response(format!("task failed: {e}")),
    }
}

#[derive(Debug, Deserialize)]
struct ContentQuery {
    repo: String,
    path: String,
    r#ref: Option<String>,
}

async fn content(
    State(service): State<Arc<ApiService>>,
    Query(params): Query<ContentQuery>,
) -> Response {
    let git_ref = params.r#ref.unwrap_or_else(|| "HEAD".to_string());
    let path = params.path.clone();
    let path_for_call = path.clone();
    let service = service.clone();
    let result = tokio::task::spawn_blocking(move || {
        service.file_content(&params.repo, &path_for_call, &git_ref)
    })
    .await;
    match result {
        Ok(Ok(bytes)) => {
            let size = bytes.len();
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            Json(ContentDto {
                path,
                content: encoded,
                size,
            })
            .into_response()
        }
        Ok(Err(e)) => err_response(format!("content failed: {e}")),
        Err(e) => err_response(format!("task failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_returns_ok() {
        let service = Arc::new(ApiService::from_env().expect("service"));
        let app = router(service);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let health: HealthDto = serde_json::from_slice(&body).unwrap();
        assert_eq!(health.status, "ok");
    }

    #[tokio::test]
    async fn index_serves_embedded_ui() {
        let service = Arc::new(ApiService::from_env().expect("service"));
        let app = router(service);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("GitNapse Web"));
    }
}
