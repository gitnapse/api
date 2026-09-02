//! HTTP routes implementing the GitNapse protocol (v1) + embedded web UI.
//!
//! The operation contract is defined in `docs/PROTOCOL.md` and the request
//! types live in `gitnapse-protocol` so they cannot drift from the wire.
//! The route surface mirrors the full `gitnapse` SDK (`Backend` trait):
//! identity/discovery, content, commits/CI, issues, pull requests, releases
//! and repo creation — reads as GET, mutations as POST with JSON bodies.
//!
//! Security posture (local-first server):
//! - Binds to `127.0.0.1` by default and rejects any request whose `Host`
//!   header is not a loopback address (DNS-rebinding protection).
//! - Optionally requires `Authorization: Bearer <token>` on `/api/*` when a
//!   token is configured (env `GITNAPSE_SERVER_TOKEN`).
//! - Cross-origin browsers cannot reach POST endpoints: the server sends no
//!   CORS headers, so preflight fails outside same-origin.
//! - Every request is subject to an overall timeout and payload guards.

use axum::Router;
use axum::error_handling::HandleErrorLayer;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Json, Query, Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{Next, from_fn, from_fn_with_state};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use base64::Engine;
use gitnapse_protocol::{
    API_PREFIX, AuthStatusDto, CommitListRequest, CompareRequest, ContentDto, ContentRequest,
    ErrorDto, HealthDto, IssueCreateRequest, NumberRepoRequest, PageRequest, PrCommentRequest,
    PrCreateRequest, PrMergeRequest, PrReviewRequest, PrUpdateRequest, RateLimitDto,
    RefRepoRequest, ReleaseCreateRequest, ReleasesRequest, RepoCreateRequest, RepoRequest,
    SearchRequest, StateRepoRequest, TokenSetRequest, TreeRequest, UserDto, WorkflowRunsRequest,
};
use std::sync::Arc;
use std::time::Duration;
use tower::{ServiceBuilder, timeout::TimeoutLayer};

use crate::convert::{
    check_run_dto, comment_dto, commits_dto, compare_dto, issue_dto, merge_dto, node_dto,
    pr_detail_dto, pr_summary_dto, release_dto, repo_dto, review_dto, workflow_run_dto,
};
use crate::service::{Backend, error_response, task_error_response, unauthorized_response};
use crate::webui::INDEX_HTML;

/// Upper bound for a single file response (16 MiB, ~2x the GitHub blob limit).
const MAX_CONTENT_BYTES: usize = 16 * 1024 * 1024;
/// Upper bound for a full tree response (pre-order node count).
const MAX_TREE_NODES: usize = 200_000;
/// Overall request timeout (the gitnapse HTTP client already times out at 30s
/// per upstream call; this guards the whole handler).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);

/// Loopback host values accepted by the DNS-rebinding guard.
const ALLOWED_HOSTS: &[&str] = &[
    "127.0.0.1",
    "localhost",
    "::1",
    "[::1]",
    "[0:0:0:0:0:0:0:1]",
];

#[derive(Clone)]
pub struct ServerState {
    pub backend: Arc<dyn Backend>,
    /// Optional shared secret required on every `/api/*` request.
    pub api_token: Option<String>,
}

pub fn router(backend: Arc<dyn Backend>, api_token: Option<String>) -> Router {
    let state = Arc::new(ServerState { backend, api_token });

    let routes = Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        // NOTE: keep these paths in sync with gitnapse_protocol::API_PREFIX.
        // Identity / discovery
        .route("/api/v1/user", get(user))
        .route("/api/v1/user/starred", get(user_starred))
        .route("/api/v1/rate-limit", get(rate_limit))
        // Auth management (token lifecycle)
        .route(
            "/api/v1/auth/token",
            get(auth_status).post(auth_set).delete(auth_clear),
        )
        .route("/api/v1/auth/status", get(auth_status))
        // Content
        .route("/api/v1/search", get(search))
        .route("/api/v1/repos/detail", get(repo_detail))
        .route("/api/v1/repos/branches", get(branches))
        .route("/api/v1/repos/tree", get(tree))
        .route("/api/v1/repos/content", get(content))
        // Commits / CI
        .route("/api/v1/commits", get(commits))
        .route("/api/v1/compare", get(compare))
        .route("/api/v1/checks", get(checks))
        .route("/api/v1/workflows", get(workflows))
        // Issues
        .route("/api/v1/issues", get(issues_list).post(issues_create))
        .route("/api/v1/issues/close", post(issues_close))
        // Pull requests
        .route("/api/v1/pulls", get(pulls_list).post(pulls_create))
        .route("/api/v1/pulls/detail", get(pulls_detail))
        .route(
            "/api/v1/pulls/reviews",
            get(pulls_reviews).post(pulls_review),
        )
        .route(
            "/api/v1/pulls/comments",
            get(pulls_comments).post(pulls_comment),
        )
        .route("/api/v1/pulls/commits", get(pulls_commits))
        .route("/api/v1/pulls/merge", post(pulls_merge))
        .route("/api/v1/pulls/update", post(pulls_update))
        // Releases / repos
        .route("/api/v1/releases", get(releases_list).post(releases_create))
        .route("/api/v1/repos", post(repos_create))
        .fallback(not_found)
        .with_state(state.clone());

    let timeout_layer = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(handle_timeout))
        .layer(TimeoutLayer::new(REQUEST_TIMEOUT));

    routes
        .layer(from_fn(host_guard))
        .layer(from_fn_with_state(state, api_auth))
        .layer(timeout_layer)
}

fn is_allowed_host(host: Option<&HeaderValue>) -> bool {
    let Some(host) = host else { return false };
    let Ok(host) = host.to_str() else {
        return false;
    };
    // Host may carry a port: `127.0.0.1:8787` or `[::1]:8787`.
    let hostname = if let Some(rest) = host.strip_prefix('[') {
        rest.split_once(']').map(|(h, _)| h).unwrap_or(host)
    } else {
        host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host)
    };
    ALLOWED_HOSTS.contains(&hostname)
}

/// Reject requests that do not target a loopback host (DNS-rebinding guard).
async fn host_guard(request: Request, next: Next) -> Response {
    if !is_allowed_host(request.headers().get("host")) {
        return json_error(
            StatusCode::FORBIDDEN,
            "server only accepts loopback connections",
        );
    }
    next.run(request).await
}

/// Require `Authorization: Bearer <token>` on protocol routes when configured.
async fn api_auth(State(state): State<Arc<ServerState>>, request: Request, next: Next) -> Response {
    let Some(expected) = &state.api_token else {
        return next.run(request).await;
    };
    if !request.uri().path().starts_with(API_PREFIX) {
        return next.run(request).await;
    }
    let provided = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    match provided {
        Some(given) if constant_time_eq(given, expected) => next.run(request).await,
        _ => unauthorized_response(),
    }
}

/// Constant-time string comparison (no timing side channel on the token).
fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn handle_timeout(error: Box<dyn std::error::Error + Send + Sync>) -> Response {
    log::warn!("request timed out: {error}");
    json_error(StatusCode::GATEWAY_TIMEOUT, "request timed out")
}

// ── Shared helpers ──────────────────────────────────────────────────────

fn json_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(ErrorDto {
            error: message.into(),
        }),
    )
        .into_response()
}

fn json_bad_request(message: &str) -> Response {
    json_error(StatusCode::BAD_REQUEST, message)
}

/// Validate `owner/name` form.
fn is_valid_repo(full_name: &str) -> bool {
    let t = full_name.trim();
    !t.is_empty() && t.contains('/')
}

fn too_large(message: &str) -> Response {
    json_error(StatusCode::PAYLOAD_TOO_LARGE, message)
}

fn no_content() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

/// Run a blocking backend call on the thread pool and map the outcome.
async fn run_task<T, F>(job: F, to_response: impl FnOnce(T) -> Response) -> Response
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    match tokio::task::spawn_blocking(job).await {
        Ok(Ok(value)) => to_response(value),
        Ok(Err(e)) => error_response(&e),
        Err(e) => task_error_response(&e.to_string()),
    }
}

fn dto_list<T: serde::Serialize>(list: Vec<T>) -> Response {
    Json(list).into_response()
}

// ── Identity / discovery ────────────────────────────────────────────────

async fn user(State(state): State<Arc<ServerState>>) -> Response {
    let backend = state.backend.clone();
    run_task(
        move || backend.authenticated_user(),
        |login| match login {
            Some(login) => Json(UserDto { login }).into_response(),
            None => json_error(
                StatusCode::UNAUTHORIZED,
                "not authenticated — configure a GitHub token on the server",
            ),
        },
    )
    .await
}

async fn user_starred(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<PageRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(30).clamp(1, 100);
    let backend = state.backend.clone();
    run_task(
        move || backend.starred_repos(page, per_page),
        |repos| dto_list(repos.iter().map(repo_dto).collect::<Vec<_>>()),
    )
    .await
}

async fn rate_limit(State(state): State<Arc<ServerState>>) -> Response {
    let (remaining, reset) = state.backend.rate_limit();
    Json(RateLimitDto { remaining, reset }).into_response()
}

// ── Auth management ─────────────────────────────────────────────────────

async fn auth_status(State(state): State<Arc<ServerState>>) -> Response {
    let backend = state.backend.clone();
    run_task(
        move || backend.token_status(),
        |status| {
            Json(AuthStatusDto {
                has_token: status.has_token,
                source: status.source.to_string(),
            })
            .into_response()
        },
    )
    .await
}

async fn auth_set(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<TokenSetRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(req)) = body else {
        return json_bad_request("invalid JSON body");
    };
    if req.token.trim().is_empty() {
        return json_bad_request("token is required");
    }
    let token = req.token;
    let backend = state.backend.clone();
    run_task(move || backend.set_token(&token), |()| no_content()).await
}

async fn auth_clear(State(state): State<Arc<ServerState>>) -> Response {
    let backend = state.backend.clone();
    run_task(move || backend.clear_token(), |()| no_content()).await
}

// ── Content ─────────────────────────────────────────────────────────────

async fn index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

async fn health() -> Json<HealthDto> {
    Json(HealthDto {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn not_found() -> Response {
    json_error(StatusCode::NOT_FOUND, "endpoint not found")
}

async fn search(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<SearchRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    let query = params.q.unwrap_or_default();
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(30).clamp(1, 100);
    let backend = state.backend.clone();
    run_task(
        move || backend.search(&query, page, per_page),
        |repos| dto_list(repos.iter().map(repo_dto).collect::<Vec<_>>()),
    )
    .await
}

async fn repo_detail(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<RepoRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) {
        return json_bad_request("repo parameter required as <owner/name>");
    }
    let repo = params.repo;
    let backend = state.backend.clone();
    run_task(
        move || backend.repo_by_name(&repo),
        |r| Json(repo_dto(&r)).into_response(),
    )
    .await
}

async fn branches(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<RepoRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) {
        return json_bad_request("repo parameter required as <owner/name>");
    }
    let repo = params.repo;
    let backend = state.backend.clone();
    run_task(move || backend.branches(&repo), dto_list).await
}

async fn tree(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<TreeRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) {
        return json_bad_request("repo parameter required as <owner/name>");
    }
    let repo = params.repo;
    let git_ref = params.r#ref.unwrap_or_else(|| "HEAD".to_string());
    let backend = state.backend.clone();
    run_task(
        move || backend.tree(&repo, &git_ref),
        |nodes| {
            if nodes.len() > MAX_TREE_NODES {
                return too_large(&format!(
                    "repository tree too large ({} nodes, limit {MAX_TREE_NODES})",
                    nodes.len()
                ));
            }
            dto_list(nodes.iter().map(node_dto).collect::<Vec<_>>())
        },
    )
    .await
}

async fn content(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<ContentRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) {
        return json_bad_request("repo parameter required as <owner/name>");
    }
    if params.path.trim().is_empty() {
        return json_bad_request("path parameter is required");
    }
    let repo = params.repo;
    let path = params.path;
    let git_ref = params.r#ref.unwrap_or_else(|| "HEAD".to_string());
    let backend = state.backend.clone();
    let path_for_dto = path.clone();
    run_task(
        move || backend.file_content(&repo, &path, &git_ref),
        |bytes| {
            if bytes.len() > MAX_CONTENT_BYTES {
                return too_large(&format!(
                    "file too large ({} bytes, limit {MAX_CONTENT_BYTES})",
                    bytes.len()
                ));
            }
            let size = bytes.len();
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            Json(ContentDto {
                path: path_for_dto,
                content: encoded,
                size,
            })
            .into_response()
        },
    )
    .await
}

// ── Commits / CI ────────────────────────────────────────────────────────

async fn commits(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<CommitListRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) {
        return json_bad_request("repo parameter required as <owner/name>");
    }
    let repo = params.repo;
    let branch = params.r#ref.unwrap_or_else(|| "HEAD".to_string());
    let per_page = params.per_page.unwrap_or(30).clamp(1, 100);
    let backend = state.backend.clone();
    run_task(
        move || backend.recent_commits(&repo, &branch, per_page),
        |list| dto_list(commits_dto(&list)),
    )
    .await
}

async fn compare(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<CompareRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) {
        return json_bad_request("repo parameter required as <owner/name>");
    }
    if params.base.trim().is_empty() || params.head.trim().is_empty() {
        return json_bad_request("base and head parameters are required");
    }
    let (repo, base, head) = (params.repo, params.base, params.head);
    let backend = state.backend.clone();
    run_task(
        move || backend.compare(&repo, &base, &head),
        |c| Json(compare_dto(&c)).into_response(),
    )
    .await
}

async fn checks(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<RefRepoRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) {
        return json_bad_request("repo parameter required as <owner/name>");
    }
    let (repo, git_ref) = (params.repo, params.r#ref);
    let backend = state.backend.clone();
    run_task(
        move || backend.check_runs(&repo, &git_ref),
        |runs| dto_list(runs.iter().map(check_run_dto).collect::<Vec<_>>()),
    )
    .await
}

async fn workflows(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<WorkflowRunsRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) {
        return json_bad_request("repo parameter required as <owner/name>");
    }
    let repo = params.repo;
    let branch = params.branch.unwrap_or_else(|| "main".to_string());
    let per_page = params.per_page.unwrap_or(30).clamp(1, 100);
    let backend = state.backend.clone();
    run_task(
        move || backend.workflow_runs(&repo, &branch, per_page),
        |runs| dto_list(runs.iter().map(workflow_run_dto).collect::<Vec<_>>()),
    )
    .await
}

// ── Issues ──────────────────────────────────────────────────────────────

fn parse_repo_state(params: &StateRepoRequest) -> Option<(String, String, u8)> {
    if !is_valid_repo(&params.repo) {
        return None;
    }
    let state = params.state.as_deref().unwrap_or("open");
    if !matches!(state, "open" | "closed" | "all") {
        return None;
    }
    let per_page = params.per_page.unwrap_or(30).clamp(1, 100);
    Some((params.repo.clone(), state.to_string(), per_page))
}

async fn issues_list(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<StateRepoRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    let Some((repo, issue_state, per_page)) = parse_repo_state(&params) else {
        return json_bad_request(
            "repo parameter required as <owner/name>; state is open|closed|all",
        );
    };
    let backend = state.backend.clone();
    run_task(
        move || backend.issues(&repo, &issue_state, per_page),
        |issues| dto_list(issues.iter().map(issue_dto).collect::<Vec<_>>()),
    )
    .await
}

async fn issues_create(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<IssueCreateRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(req)) = body else {
        return json_bad_request("invalid JSON body");
    };
    if !is_valid_repo(&req.repo) || req.title.trim().is_empty() {
        return json_bad_request("repo (owner/name) and title are required");
    }
    let (repo, title, text) = (req.repo, req.title, req.body);
    let backend = state.backend.clone();
    run_task(
        move || backend.create_issue(&repo, &title, text.as_deref()),
        |issue| (StatusCode::CREATED, Json(issue_dto(&issue))).into_response(),
    )
    .await
}

async fn issues_close(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<NumberRepoRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(req)) = body else {
        return json_bad_request("invalid JSON body");
    };
    if !is_valid_repo(&req.repo) || req.number == 0 {
        return json_bad_request("repo (owner/name) and a positive number are required");
    }
    let (repo, number) = (req.repo, req.number);
    let backend = state.backend.clone();
    run_task(
        move || backend.close_issue(&repo, number),
        |issue| Json(issue_dto(&issue)).into_response(),
    )
    .await
}

// ── Pull requests ───────────────────────────────────────────────────────

async fn pulls_list(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<StateRepoRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    let Some((repo, issue_state, per_page)) = parse_repo_state(&params) else {
        return json_bad_request(
            "repo parameter required as <owner/name>; state is open|closed|all",
        );
    };
    let backend = state.backend.clone();
    run_task(
        move || backend.pull_requests(&repo, &issue_state, per_page),
        |prs| dto_list(prs.iter().map(pr_summary_dto).collect::<Vec<_>>()),
    )
    .await
}

async fn pulls_create(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<PrCreateRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(req)) = body else {
        return json_bad_request("invalid JSON body");
    };
    if !is_valid_repo(&req.repo)
        || req.title.trim().is_empty()
        || req.head.trim().is_empty()
        || req.base.trim().is_empty()
    {
        return json_bad_request("repo (owner/name), title, head and base are required");
    }
    let (repo, title, head, base, text) = (req.repo, req.title, req.head, req.base, req.body);
    let backend = state.backend.clone();
    run_task(
        move || backend.create_pull_request(&repo, &title, &head, &base, text.as_deref()),
        |pr| (StatusCode::CREATED, Json(pr_detail_dto(&pr))).into_response(),
    )
    .await
}

async fn pulls_detail(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<NumberRepoRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) || params.number == 0 {
        return json_bad_request("repo (owner/name) and a positive number are required");
    }
    let (repo, number) = (params.repo, params.number);
    let backend = state.backend.clone();
    run_task(
        move || backend.pull_request_detail(&repo, number),
        |pr| Json(pr_detail_dto(&pr)).into_response(),
    )
    .await
}

async fn pulls_reviews(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<NumberRepoRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) || params.number == 0 {
        return json_bad_request("repo (owner/name) and a positive number are required");
    }
    let (repo, number) = (params.repo, params.number);
    let backend = state.backend.clone();
    run_task(
        move || backend.pull_request_reviews(&repo, number),
        |reviews| dto_list(reviews.iter().map(review_dto).collect::<Vec<_>>()),
    )
    .await
}

async fn pulls_comments(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<NumberRepoRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) || params.number == 0 {
        return json_bad_request("repo (owner/name) and a positive number are required");
    }
    let (repo, number) = (params.repo, params.number);
    let backend = state.backend.clone();
    run_task(
        move || backend.pull_request_comments(&repo, number),
        |comments| dto_list(comments.iter().map(comment_dto).collect::<Vec<_>>()),
    )
    .await
}

async fn pulls_commits(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<NumberRepoRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) || params.number == 0 {
        return json_bad_request("repo (owner/name) and a positive number are required");
    }
    let (repo, number) = (params.repo, params.number);
    let backend = state.backend.clone();
    run_task(
        move || backend.pull_request_commits(&repo, number),
        |commits| dto_list(commits_dto(&commits)),
    )
    .await
}

async fn pulls_merge(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<PrMergeRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(req)) = body else {
        return json_bad_request("invalid JSON body");
    };
    if !is_valid_repo(&req.repo) || req.number == 0 {
        return json_bad_request("repo (owner/name) and a positive number are required");
    }
    if let Some(m) = req.method.as_deref()
        && !matches!(m, "merge" | "squash" | "rebase")
    {
        return json_bad_request("method is merge|squash|rebase");
    }
    let (repo, number, commit_title, method) = (req.repo, req.number, req.commit_title, req.method);
    let backend = state.backend.clone();
    run_task(
        move || {
            backend.merge_pull_request(&repo, number, commit_title.as_deref(), method.as_deref())
        },
        |m| Json(merge_dto(&m)).into_response(),
    )
    .await
}

async fn pulls_update(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<PrUpdateRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(req)) = body else {
        return json_bad_request("invalid JSON body");
    };
    if !is_valid_repo(&req.repo) || req.number == 0 {
        return json_bad_request("repo (owner/name) and a positive number are required");
    }
    if !matches!(req.state.as_str(), "open" | "closed") {
        return json_bad_request("state is open|closed");
    }
    let (repo, number, pr_state) = (req.repo, req.number, req.state);
    let backend = state.backend.clone();
    run_task(
        move || backend.update_pull_request(&repo, number, &pr_state),
        |()| no_content(),
    )
    .await
}

async fn pulls_review(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<PrReviewRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(req)) = body else {
        return json_bad_request("invalid JSON body");
    };
    if !is_valid_repo(&req.repo) || req.number == 0 {
        return json_bad_request("repo (owner/name) and a positive number are required");
    }
    if !matches!(
        req.event.as_str(),
        "approve" | "request_changes" | "comment"
    ) {
        return json_bad_request("event is approve|request_changes|comment");
    }
    let (repo, number, event, text) = (req.repo, req.number, req.event, req.body);
    let backend = state.backend.clone();
    run_task(
        move || {
            backend.create_pull_request_review(&repo, number, text.as_deref().unwrap_or(""), &event)
        },
        |()| no_content(),
    )
    .await
}

async fn pulls_comment(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<PrCommentRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(req)) = body else {
        return json_bad_request("invalid JSON body");
    };
    if !is_valid_repo(&req.repo) || req.number == 0 || req.body.trim().is_empty() {
        return json_bad_request("repo (owner/name), number and non-empty body are required");
    }
    let (repo, number, comment) = (req.repo, req.number, req.body);
    let backend = state.backend.clone();
    run_task(
        move || backend.create_pull_request_comment(&repo, number, &comment),
        |()| no_content(),
    )
    .await
}

// ── Releases / repos ────────────────────────────────────────────────────

async fn releases_list(
    State(state): State<Arc<ServerState>>,
    params: Result<Query<ReleasesRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return json_bad_request("invalid query parameters");
    };
    if !is_valid_repo(&params.repo) {
        return json_bad_request("repo parameter required as <owner/name>");
    }
    let repo = params.repo;
    let per_page = params.per_page.unwrap_or(30).clamp(1, 100);
    let backend = state.backend.clone();
    run_task(
        move || backend.releases(&repo, per_page),
        |releases| dto_list(releases.iter().map(release_dto).collect::<Vec<_>>()),
    )
    .await
}

async fn releases_create(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<ReleaseCreateRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(req)) = body else {
        return json_bad_request("invalid JSON body");
    };
    if !is_valid_repo(&req.repo) || req.tag_name.trim().is_empty() {
        return json_bad_request("repo (owner/name) and tag_name are required");
    }
    let (repo, tag, name, text, prerelease) =
        (req.repo, req.tag_name, req.name, req.body, req.prerelease);
    let backend = state.backend.clone();
    run_task(
        move || backend.create_release(&repo, &tag, name.as_deref(), text.as_deref(), prerelease),
        |r| (StatusCode::CREATED, Json(release_dto(&r))).into_response(),
    )
    .await
}

async fn repos_create(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<RepoCreateRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(req)) = body else {
        return json_bad_request("invalid JSON body");
    };
    if req.name.trim().is_empty() {
        return json_bad_request("name is required");
    }
    let (name, description, private) = (req.name, req.description, req.private);
    let backend = state.backend.clone();
    run_task(
        move || backend.create_repo(&name, description.as_deref(), private),
        |r| (StatusCode::CREATED, Json(repo_dto(&r))).into_response(),
    )
    .await
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use gitnapse::error::GitHubError;
    use gitnapse::models::{
        CheckRun, CommitAuthor, CommitDetails, CommitInfo, CompareResponse, DiffFile, Issue,
        IssueLabel, IssueUser, MergeResponse, PrBranch, PullRequest, PullRequestDetail,
        PullRequestReview, Release, RepoNode, RepoOwner, RepoSummary, ReviewComment, WorkflowRun,
    };
    use gitnapse_protocol::{IssueDto, PrDetailDto, RepoDto};
    use tower::ServiceExt;

    #[derive(Clone, Copy, Default)]
    enum FailMode {
        #[default]
        None,
        NotFound,
        Unauthorized,
    }

    struct FakeBackend {
        fail: FailMode,
    }

    impl Default for FakeBackend {
        fn default() -> Self {
            Self {
                fail: FailMode::None,
            }
        }
    }

    fn repo_summary() -> RepoSummary {
        RepoSummary {
            name: "gitnapse".into(),
            full_name: "gitnapse/gitnapse".into(),
            description: Some("a TUI".into()),
            stargazers_count: 42,
            language: Some("Rust".into()),
            clone_url: "https://github.com/gitnapse/gitnapse.git".into(),
            owner: RepoOwner {
                login: "gitnapse".into(),
            },
            default_branch: "main".into(),
        }
    }

    impl FakeBackend {
        fn fail_error(&self) -> Option<anyhow::Error> {
            match self.fail {
                FailMode::None => None,
                FailMode::NotFound => Some(anyhow::Error::new(GitHubError::Api {
                    status: 404,
                    body: "Not Found".into(),
                })),
                FailMode::Unauthorized => Some(anyhow::Error::new(GitHubError::Unauthorized)),
            }
        }
    }

    impl Backend for FakeBackend {
        fn authenticated_user(&self) -> anyhow::Result<Option<String>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(Some("xscriptor".into()))
        }

        fn search(&self, _q: &str, _p: u32, _pp: u8) -> anyhow::Result<Vec<RepoSummary>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(vec![repo_summary()])
        }

        fn starred_repos(&self, _p: u32, _pp: u8) -> anyhow::Result<Vec<RepoSummary>> {
            self.search("", 1, 10)
        }

        fn repo_by_name(&self, _full_name: &str) -> anyhow::Result<RepoSummary> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(repo_summary())
        }

        fn rate_limit(&self) -> (Option<u32>, Option<u64>) {
            (Some(99), Some(1_700_000_000))
        }

        fn set_token(&self, token: &str) -> anyhow::Result<()> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            if token.trim().is_empty() {
                return Err(anyhow::anyhow!("token is empty"));
            }
            Ok(())
        }

        fn clear_token(&self) -> anyhow::Result<()> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(())
        }

        fn token_status(&self) -> anyhow::Result<crate::service::TokenStatus> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(crate::service::TokenStatus {
                has_token: true,
                source: "stored",
            })
        }

        fn branches(&self, full_name: &str) -> anyhow::Result<Vec<String>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(vec![format!("branch-of-{full_name}")])
        }

        fn tree(&self, _full_name: &str, _git_ref: &str) -> anyhow::Result<Vec<RepoNode>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(vec![RepoNode {
                path: "src/main.rs".into(),
                name: "main.rs".into(),
                depth: 1,
                is_dir: false,
            }])
        }

        fn file_content(&self, _f: &str, _p: &str, _r: &str) -> anyhow::Result<Vec<u8>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(b"hello".to_vec())
        }

        fn recent_commits(
            &self,
            _full_name: &str,
            _branch: &str,
            _per_page: u8,
        ) -> anyhow::Result<Vec<CommitInfo>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(vec![CommitInfo {
                sha: "abc123".into(),
                commit: CommitDetails {
                    message: "fix things".into(),
                    author: CommitAuthor {
                        name: "x".into(),
                        date: "2026-01-01T00:00:00Z".into(),
                    },
                },
            }])
        }

        fn compare(
            &self,
            _full_name: &str,
            _base: &str,
            _head: &str,
        ) -> anyhow::Result<CompareResponse> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(CompareResponse {
                status: "ahead".into(),
                ahead_by: 2,
                behind_by: 0,
                total_commits: 2,
                files: vec![DiffFile {
                    filename: "README.md".into(),
                    status: "modified".into(),
                    additions: 1,
                    deletions: 1,
                    changes: 2,
                    patch: Some("@@ -1 +1 @@".into()),
                }],
            })
        }

        fn check_runs(&self, _f: &str, _r: &str) -> anyhow::Result<Vec<CheckRun>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(vec![CheckRun {
                name: "ci".into(),
                status: "completed".into(),
                conclusion: Some("success".into()),
                html_url: "https://github.com".into(),
                started_at: Some("2026-01-01T00:00:00Z".into()),
                completed_at: Some("2026-01-01T00:01:00Z".into()),
            }])
        }

        fn workflow_runs(&self, _f: &str, _b: &str, _pp: u8) -> anyhow::Result<Vec<WorkflowRun>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(vec![WorkflowRun {
                name: "release".into(),
                status: "completed".into(),
                conclusion: Some("success".into()),
                html_url: "https://github.com".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:01:00Z".into(),
            }])
        }

        fn issues(&self, _f: &str, _s: &str, _pp: u8) -> anyhow::Result<Vec<Issue>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(vec![issue_fixture(7, "open")])
        }

        fn create_issue(
            &self,
            _f: &str,
            _title: &str,
            _body: Option<&str>,
        ) -> anyhow::Result<Issue> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(issue_fixture(99, "open"))
        }

        fn close_issue(&self, _f: &str, _number: u64) -> anyhow::Result<Issue> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(issue_fixture(7, "closed"))
        }

        fn pull_requests(&self, _f: &str, _s: &str, _pp: u8) -> anyhow::Result<Vec<PullRequest>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(vec![PullRequest {
                number: 7,
                title: "feat".into(),
                state: "open".into(),
                html_url: "https://github.com/gitnapse/gitnapse/pull/7".into(),
                user: user_fixture(),
                body: None,
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
                additions: Some(1),
                deletions: Some(1),
                changed_files: Some(1),
            }])
        }

        fn pull_request_detail(&self, _f: &str, _number: u64) -> anyhow::Result<PullRequestDetail> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(pr_detail_fixture())
        }

        fn pull_request_reviews(
            &self,
            _f: &str,
            _number: u64,
        ) -> anyhow::Result<Vec<PullRequestReview>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(vec![PullRequestReview {
                id: 1,
                user: user_fixture(),
                body: Some("lgtm".into()),
                state: "APPROVED".into(),
                submitted_at: Some("2026-01-01T00:00:00Z".into()),
                commit_id: Some("abc".into()),
            }])
        }

        fn pull_request_comments(
            &self,
            _f: &str,
            _number: u64,
        ) -> anyhow::Result<Vec<ReviewComment>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(vec![ReviewComment {
                id: 1,
                user: user_fixture(),
                body: "comment".into(),
                path: Some("src/main.rs".into()),
                position: Some(1),
                commit_id: Some("abc".into()),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            }])
        }

        fn pull_request_commits(&self, _f: &str, _number: u64) -> anyhow::Result<Vec<CommitInfo>> {
            self.recent_commits("", "", 1)
        }

        fn create_pull_request(
            &self,
            _f: &str,
            _title: &str,
            _head: &str,
            _base: &str,
            _body: Option<&str>,
        ) -> anyhow::Result<PullRequestDetail> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(pr_detail_fixture())
        }

        fn merge_pull_request(
            &self,
            _f: &str,
            _n: u64,
            _t: Option<&str>,
            _m: Option<&str>,
        ) -> anyhow::Result<MergeResponse> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(MergeResponse {
                sha: "merged-sha".into(),
                merged: true,
                message: "Pull Request successfully merged".into(),
            })
        }

        fn update_pull_request(&self, _f: &str, _n: u64, _s: &str) -> anyhow::Result<()> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(())
        }

        fn create_pull_request_review(
            &self,
            _f: &str,
            _n: u64,
            _b: &str,
            _e: &str,
        ) -> anyhow::Result<()> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(())
        }

        fn create_pull_request_comment(&self, _f: &str, _n: u64, _b: &str) -> anyhow::Result<()> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(())
        }

        fn releases(&self, _f: &str, _pp: u8) -> anyhow::Result<Vec<Release>> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(vec![Release {
                tag_name: "v0.1.0".into(),
                name: Some("v0.1.0".into()),
                body: None,
                html_url: "https://github.com/gitnapse/gitnapse/releases/tag/v0.1.0".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                published_at: None,
                prerelease: false,
            }])
        }

        fn create_release(
            &self,
            _f: &str,
            _tag: &str,
            _n: Option<&str>,
            _b: Option<&str>,
            _pr: bool,
        ) -> anyhow::Result<Release> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(Release {
                tag_name: "v9.9.9".into(),
                name: Some("v9.9.9".into()),
                body: None,
                html_url: "https://github.com".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                published_at: None,
                prerelease: false,
            })
        }

        fn create_repo(
            &self,
            _name: &str,
            _description: Option<&str>,
            _private: bool,
        ) -> anyhow::Result<RepoSummary> {
            if let Some(e) = self.fail_error() {
                return Err(e);
            }
            Ok(repo_summary())
        }
    }

    fn user_fixture() -> IssueUser {
        IssueUser {
            login: "xscriptor".into(),
        }
    }

    fn issue_fixture(number: u64, state: &str) -> Issue {
        Issue {
            number,
            title: "an issue".into(),
            state: state.into(),
            html_url: format!("https://github.com/gitnapse/gitnapse/issues/{number}"),
            user: user_fixture(),
            labels: vec![IssueLabel {
                name: "bug".into(),
                color: "d73a4a".into(),
            }],
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            body: None,
            pull_request: None,
        }
    }

    fn pr_detail_fixture() -> PullRequestDetail {
        let branch = || PrBranch {
            label: "gitnapse:main".into(),
            r#ref: "main".into(),
            sha: "abc".into(),
        };
        PullRequestDetail {
            number: 7,
            title: "feat".into(),
            state: "open".into(),
            body: None,
            html_url: "https://github.com/gitnapse/gitnapse/pull/7".into(),
            user: user_fixture(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            merge_commit_sha: None,
            merged: Some(false),
            merged_by: None,
            additions: Some(1),
            deletions: Some(1),
            changed_files: Some(1),
            commits: Some(1),
            comments: Some(0),
            review_comments: Some(0),
            head: branch(),
            base: branch(),
            labels: vec![],
        }
    }

    fn test_app(backend: Arc<dyn Backend>, api_token: Option<String>) -> Router {
        router(backend, api_token)
    }

    async fn get(app: Router, uri: &str) -> axum::response::Response {
        app.oneshot(
            Request::builder()
                .uri(uri)
                .header("host", "127.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    }

    async fn post(app: Router, uri: &str, body: serde_json::Value) -> axum::response::Response {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("host", "127.0.0.1")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
    }

    async fn json_body(response: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    // ── Core read routes ────────────────────────────────────────────────

    #[tokio::test]
    async fn health_returns_ok() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        let response = get(app, "/health").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let health: HealthDto = serde_json::from_slice(&body).unwrap();
        assert_eq!(health.status, "ok");
    }

    #[tokio::test]
    async fn index_serves_embedded_ui() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        let response = get(app, "/").await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn search_returns_repo_json() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        let response = get(app, "/api/v1/search?q=rust").await;
        assert_eq!(response.status(), StatusCode::OK);
        let repos: Vec<RepoDto> = serde_json::from_value(json_body(response).await).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].full_name, "gitnapse/gitnapse");
        assert_eq!(repos[0].owner, "gitnapse");
    }

    #[tokio::test]
    async fn branches_and_content_work() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        let response = get(app.clone(), "/api/v1/repos/branches?repo=a/b").await;
        assert_eq!(response.status(), StatusCode::OK);
        let list: Vec<String> = serde_json::from_value(json_body(response).await).unwrap();
        assert_eq!(list, ["branch-of-a/b"]);

        let response = get(app, "/api/v1/repos/content?repo=a/b&path=Cargo.toml").await;
        assert_eq!(response.status(), StatusCode::OK);
        let content: ContentDto = serde_json::from_value(json_body(response).await).unwrap();
        assert_eq!(content.size, 5);
        assert_eq!(content.content, "aGVsbG8=");
    }

    #[tokio::test]
    async fn identity_and_rate_limit_routes() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        let response = get(app.clone(), "/api/v1/user").await;
        assert_eq!(response.status(), StatusCode::OK);
        let user: UserDto = serde_json::from_value(json_body(response).await).unwrap();
        assert_eq!(user.login, "xscriptor");

        let response = get(app, "/api/v1/rate-limit").await;
        assert_eq!(response.status(), StatusCode::OK);
        let rate: RateLimitDto = serde_json::from_value(json_body(response).await).unwrap();
        assert_eq!(rate.remaining, Some(99));
    }

    #[tokio::test]
    async fn full_surface_reads_respond() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        for uri in [
            "/api/v1/user/starred",
            "/api/v1/repos/detail?repo=a/b",
            "/api/v1/commits?repo=a/b",
            "/api/v1/compare?repo=a/b&base=main&head=dev",
            "/api/v1/checks?repo=a/b&ref=main",
            "/api/v1/workflows?repo=a/b",
            "/api/v1/issues?repo=a/b",
            "/api/v1/pulls?repo=a/b",
            "/api/v1/pulls/detail?repo=a/b&number=7",
            "/api/v1/pulls/reviews?repo=a/b&number=7",
            "/api/v1/pulls/comments?repo=a/b&number=7",
            "/api/v1/pulls/commits?repo=a/b&number=7",
            "/api/v1/releases?repo=a/b",
        ] {
            let response = get(app.clone(), uri).await;
            assert_eq!(response.status(), StatusCode::OK, "GET {uri}");
        }
    }

    // ── Mutations ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn create_issue_returns_201() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        let response = post(
            app,
            "/api/v1/issues",
            serde_json::json!({ "repo": "a/b", "title": "bug", "body": null }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let issue: IssueDto = serde_json::from_value(json_body(response).await).unwrap();
        assert_eq!(issue.number, 99);
    }

    #[tokio::test]
    async fn create_pull_request_and_merge() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        let response = post(
            app.clone(),
            "/api/v1/pulls",
            serde_json::json!({ "repo": "a/b", "title": "feat", "head": "f", "base": "main" }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let pr: PrDetailDto = serde_json::from_value(json_body(response).await).unwrap();
        assert_eq!(pr.number, 7);

        let response = post(
            app.clone(),
            "/api/v1/pulls/merge",
            serde_json::json!({ "repo": "a/b", "number": 7, "method": "squash" }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let json = json_body(response).await;
        assert_eq!(json["merged"], true);
    }

    #[tokio::test]
    async fn void_mutations_return_204() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        for (uri, body) in [
            (
                "/api/v1/pulls/update",
                serde_json::json!({ "repo": "a/b", "number": 7, "state": "closed" }),
            ),
            (
                "/api/v1/pulls/comments",
                serde_json::json!({ "repo": "a/b", "number": 7, "body": "nice" }),
            ),
            (
                "/api/v1/pulls/reviews",
                serde_json::json!({ "repo": "a/b", "number": 7, "event": "approve" }),
            ),
        ] {
            let response = post(app.clone(), uri, body).await;
            assert_eq!(response.status(), StatusCode::NO_CONTENT, "POST {uri}");
        }
    }

    #[tokio::test]
    async fn auth_status_and_token_lifecycle() {
        let app = test_app(Arc::new(FakeBackend::default()), None);

        let response = get(app.clone(), "/api/v1/auth/status").await;
        assert_eq!(response.status(), StatusCode::OK);
        let status: AuthStatusDto = serde_json::from_value(json_body(response).await).unwrap();
        assert!(status.has_token);
        assert_eq!(status.source, "stored");

        let response = post(
            app.clone(),
            "/api/v1/auth/token",
            serde_json::json!({ "token": "ghp_newtoken" }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/auth/token")
                    .header("host", "127.0.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn empty_token_is_400() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        let response = post(
            app,
            "/api/v1/auth/token",
            serde_json::json!({ "token": "   " }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn close_issue_returns_updated_issue() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        let response = post(
            app,
            "/api/v1/issues/close",
            serde_json::json!({ "repo": "a/b", "number": 7 }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let issue: IssueDto = serde_json::from_value(json_body(response).await).unwrap();
        assert_eq!(issue.state, "closed");
    }

    #[tokio::test]
    async fn releases_and_repo_creation_return_201() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        let response = post(
            app.clone(),
            "/api/v1/releases",
            serde_json::json!({ "repo": "a/b", "tag_name": "v1.0" }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let json = json_body(response).await;
        assert_eq!(json["tag_name"], "v9.9.9");

        let response = post(
            app,
            "/api/v1/repos",
            serde_json::json!({ "name": "newrepo", "private": true }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    // ── Error contract ──────────────────────────────────────────────────

    #[tokio::test]
    async fn repo_not_found_maps_to_404_json() {
        let backend = FakeBackend {
            fail: FailMode::NotFound,
        };
        let app = test_app(Arc::new(backend), None);
        let response = get(app, "/api/v1/repos/tree?repo=a/b").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let err: ErrorDto = serde_json::from_value(json_body(response).await).unwrap();
        assert!(err.error.contains("not found"));
    }

    #[tokio::test]
    async fn missing_token_maps_to_401_json() {
        let backend = FakeBackend {
            fail: FailMode::Unauthorized,
        };
        let app = test_app(Arc::new(backend), None);
        let response = get(app, "/api/v1/issues?repo=a/b").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let err: ErrorDto = serde_json::from_value(json_body(response).await).unwrap();
        assert!(err.error.contains("authentication required"));
    }

    #[tokio::test]
    async fn api_token_guard_enforced_when_configured() {
        let app = test_app(Arc::new(FakeBackend::default()), Some("s3cret".to_string()));

        let denied = get(app.clone(), "/api/v1/issues?repo=a/b").await;
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

        let denied_post = post(
            app.clone(),
            "/api/v1/issues",
            serde_json::json!({ "repo": "a/b", "title": "x" }),
        )
        .await;
        assert_eq!(denied_post.status(), StatusCode::UNAUTHORIZED);

        let allowed = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/issues?repo=a/b")
                    .header("host", "127.0.0.1")
                    .header("authorization", "Bearer s3cret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(allowed.status(), StatusCode::OK);

        // Health stays open (infrastructure probe).
        let health = get(app, "/health").await;
        assert_eq!(health.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn non_loopback_host_is_rejected() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/search?q=x")
                    .header("host", "evil.example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn unknown_endpoint_returns_json_404() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        let response = get(app, "/api/v1/nope").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn invalid_inputs_are_400() {
        let app = test_app(Arc::new(FakeBackend::default()), None);
        // Missing repo
        let response = get(app.clone(), "/api/v1/repos/branches?repo=").await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        // Invalid state
        let response = get(app.clone(), "/api/v1/issues?repo=a/b&state=bogus").await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        // Malformed JSON body
        let response = post(
            app.clone(),
            "/api/v1/issues",
            serde_json::json!({ "title": 42 }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        // Invalid merge method
        let response = post(
            app,
            "/api/v1/pulls/merge",
            serde_json::json!({ "repo": "a/b", "number": 7, "method": "nuke" }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
