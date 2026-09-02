//! Backend abstraction used by the HTTP routes.
//!
//! Routes only depend on the small [`Backend`] trait (plus the protocol
//! types), which keeps them unit-testable with an in-memory fake. The real
//! implementation [`ApiService`] consumes `gitnapse` as a library, reusing the
//! same provider/domain layer as the TUI and CLI. [`Backend`] mirrors the
//! full `gitnapse::provider::GitProvider` surface so no SDK capability is
//! unreachable through the protocol.

use std::sync::{Arc, RwLock};

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use gitnapse::auth::TokenSource;
use gitnapse::error::GitHubError;
use gitnapse::models::{
    CheckRun, CommitInfo, CompareResponse, Issue, MergeResponse, PullRequest, PullRequestDetail,
    PullRequestReview, Release, RepoNode, RepoSummary, ReviewComment, WorkflowRun,
};
use gitnapse::provider::{GitProvider, ProviderKind, create_provider};
use gitnapse_protocol::ErrorDto;

/// Backend-side error that maps to `409 Conflict` (e.g. token managed by env).
#[derive(Debug)]
pub struct Conflict(pub String);

impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for Conflict {}

/// Token source plus whether a token is present (never the token itself).
pub struct TokenStatus {
    pub has_token: bool,
    pub source: &'static str,
}

/// Minimal operations the HTTP layer needs. This is the seam between the
/// protocol routes and the gitnapse SDK (or any future backend).
pub trait Backend: Send + Sync {
    // ── Identity / discovery ─────────────────────────────────────────────
    fn authenticated_user(&self) -> anyhow::Result<Option<String>>;
    fn search(&self, query: &str, page: u32, per_page: u8) -> anyhow::Result<Vec<RepoSummary>>;
    fn starred_repos(&self, page: u32, per_page: u8) -> anyhow::Result<Vec<RepoSummary>>;
    fn repo_by_name(&self, full_name: &str) -> anyhow::Result<RepoSummary>;
    fn rate_limit(&self) -> (Option<u32>, Option<u64>);

    // ── Auth management (token lifecycle) ────────────────────────────────
    /// Persist a new token (validated against GitHub) and switch to it.
    fn set_token(&self, token: &str) -> anyhow::Result<()>;
    /// Forget the stored token and switch to anonymous access.
    fn clear_token(&self) -> anyhow::Result<()>;
    /// Report whether a token is active and where it comes from.
    fn token_status(&self) -> anyhow::Result<TokenStatus>;

    // ── Content ──────────────────────────────────────────────────────────
    fn branches(&self, full_name: &str) -> anyhow::Result<Vec<String>>;
    fn tree(&self, full_name: &str, git_ref: &str) -> anyhow::Result<Vec<RepoNode>>;
    fn file_content(&self, full_name: &str, path: &str, git_ref: &str) -> anyhow::Result<Vec<u8>>;

    // ── Commits / CI ─────────────────────────────────────────────────────
    fn recent_commits(
        &self,
        full_name: &str,
        branch: &str,
        per_page: u8,
    ) -> anyhow::Result<Vec<CommitInfo>>;
    fn compare(&self, full_name: &str, base: &str, head: &str) -> anyhow::Result<CompareResponse>;
    fn check_runs(&self, full_name: &str, git_ref: &str) -> anyhow::Result<Vec<CheckRun>>;
    fn workflow_runs(
        &self,
        full_name: &str,
        branch: &str,
        per_page: u8,
    ) -> anyhow::Result<Vec<WorkflowRun>>;

    // ── Issues ───────────────────────────────────────────────────────────
    fn issues(&self, full_name: &str, state: &str, per_page: u8) -> anyhow::Result<Vec<Issue>>;
    fn create_issue(
        &self,
        full_name: &str,
        title: &str,
        body: Option<&str>,
    ) -> anyhow::Result<Issue>;
    fn close_issue(&self, full_name: &str, number: u64) -> anyhow::Result<Issue>;

    // ── Pull requests ────────────────────────────────────────────────────
    fn pull_requests(
        &self,
        full_name: &str,
        state: &str,
        per_page: u8,
    ) -> anyhow::Result<Vec<PullRequest>>;
    fn pull_request_detail(
        &self,
        full_name: &str,
        number: u64,
    ) -> anyhow::Result<PullRequestDetail>;
    fn pull_request_reviews(
        &self,
        full_name: &str,
        number: u64,
    ) -> anyhow::Result<Vec<PullRequestReview>>;
    fn pull_request_comments(
        &self,
        full_name: &str,
        number: u64,
    ) -> anyhow::Result<Vec<ReviewComment>>;
    fn pull_request_commits(&self, full_name: &str, number: u64)
    -> anyhow::Result<Vec<CommitInfo>>;
    fn create_pull_request(
        &self,
        full_name: &str,
        title: &str,
        head: &str,
        base: &str,
        body: Option<&str>,
    ) -> anyhow::Result<PullRequestDetail>;
    fn merge_pull_request(
        &self,
        full_name: &str,
        number: u64,
        commit_title: Option<&str>,
        method: Option<&str>,
    ) -> anyhow::Result<MergeResponse>;
    fn update_pull_request(&self, full_name: &str, number: u64, state: &str) -> anyhow::Result<()>;
    fn create_pull_request_review(
        &self,
        full_name: &str,
        number: u64,
        body: &str,
        event: &str,
    ) -> anyhow::Result<()>;
    fn create_pull_request_comment(
        &self,
        full_name: &str,
        number: u64,
        body: &str,
    ) -> anyhow::Result<()>;

    // ── Releases / repos ─────────────────────────────────────────────────
    fn releases(&self, full_name: &str, per_page: u8) -> anyhow::Result<Vec<Release>>;
    fn create_release(
        &self,
        full_name: &str,
        tag_name: &str,
        name: Option<&str>,
        body: Option<&str>,
        prerelease: bool,
    ) -> anyhow::Result<Release>;
    fn create_repo(
        &self,
        name: &str,
        description: Option<&str>,
        private: bool,
    ) -> anyhow::Result<RepoSummary>;
}

/// Real backend powered by the gitnapse SDK (GitHub provider).
///
/// The provider is swappable at runtime so `POST /api/v1/auth/token` and
/// `DELETE /api/v1/auth/token` can take effect without a server restart.
pub struct ApiService {
    github: RwLock<Arc<dyn GitProvider>>,
}

impl ApiService {
    /// Build the service from the environment (token via env or secure store).
    pub fn from_env() -> anyhow::Result<Self> {
        gitnapse::runtime::ensure_crypto_provider();
        let token = gitnapse::auth::load_token()?;
        let github = create_provider(ProviderKind::GitHub, token.as_deref())?;
        Ok(Self {
            github: RwLock::new(github),
        })
    }

    /// Snapshot of the current provider.
    fn provider(&self) -> Arc<dyn GitProvider> {
        self.github
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Warn at startup when the server will run anonymously.
    pub fn warn_if_anonymous(&self) {
        let token = gitnapse::auth::load_token();
        if token.as_ref().map(|t| t.is_none()).unwrap_or(true) {
            log::warn!(
                "no GitHub token found — requests will run anonymously with strict rate limits; \
                 set GITHUB_TOKEN, POST /api/v1/auth/token or run `gitnapse auth set`"
            );
        }
    }
}

impl Backend for ApiService {
    fn authenticated_user(&self) -> anyhow::Result<Option<String>> {
        self.provider().fetch_authenticated_user()
    }

    fn search(&self, query: &str, page: u32, per_page: u8) -> anyhow::Result<Vec<RepoSummary>> {
        self.provider()
            .search_repositories_page(query, page, per_page)
    }

    fn starred_repos(&self, page: u32, per_page: u8) -> anyhow::Result<Vec<RepoSummary>> {
        self.provider().fetch_starred_repos(page, per_page)
    }

    fn repo_by_name(&self, full_name: &str) -> anyhow::Result<RepoSummary> {
        self.provider().fetch_repo_by_name(full_name)
    }

    fn rate_limit(&self) -> (Option<u32>, Option<u64>) {
        let github = self.provider();
        (github.rate_limit_remaining(), github.rate_limit_reset())
    }

    fn set_token(&self, token: &str) -> anyhow::Result<()> {
        let token = token.trim();
        if token.is_empty() {
            return Err(anyhow::anyhow!("token is empty"));
        }
        if gitnapse::auth::token_source()? == TokenSource::Env {
            return Err(Conflict(
                "the active token comes from the GITHUB_TOKEN environment variable; \
                 unset it (and restart the server) before storing a new one"
                    .into(),
            )
            .into());
        }
        // Validate before persisting or swapping: never lock the user out.
        let candidate = create_provider(ProviderKind::GitHub, Some(token))?;
        candidate.fetch_authenticated_user()?;
        gitnapse::auth::save_token(token)?;
        *self.github.write().unwrap_or_else(|e| e.into_inner()) = candidate;
        log::info!("GitHub token stored and activated");
        Ok(())
    }

    fn clear_token(&self) -> anyhow::Result<()> {
        if gitnapse::auth::token_source()? == TokenSource::Env {
            return Err(Conflict(
                "the token comes from the GITHUB_TOKEN environment variable; \
                 unset it and restart the server to run anonymously"
                    .into(),
            )
            .into());
        }
        gitnapse::auth::clear_token()?;
        let anonymous = create_provider(ProviderKind::GitHub, None)?;
        *self.github.write().unwrap_or_else(|e| e.into_inner()) = anonymous;
        log::info!("GitHub token cleared; running anonymously");
        Ok(())
    }

    fn token_status(&self) -> anyhow::Result<TokenStatus> {
        let source = gitnapse::auth::token_source()?;
        Ok(TokenStatus {
            has_token: source != TokenSource::None,
            source: source.label(),
        })
    }

    fn branches(&self, full_name: &str) -> anyhow::Result<Vec<String>> {
        self.provider().fetch_branches(full_name)
    }

    fn tree(&self, full_name: &str, git_ref: &str) -> anyhow::Result<Vec<RepoNode>> {
        self.provider().fetch_repo_tree(full_name, git_ref)
    }

    fn file_content(&self, full_name: &str, path: &str, git_ref: &str) -> anyhow::Result<Vec<u8>> {
        self.provider()
            .fetch_file_content_by_ref(full_name, path, git_ref)
    }

    fn recent_commits(
        &self,
        full_name: &str,
        branch: &str,
        per_page: u8,
    ) -> anyhow::Result<Vec<CommitInfo>> {
        self.provider()
            .fetch_recent_commits(full_name, branch, per_page)
    }

    fn compare(&self, full_name: &str, base: &str, head: &str) -> anyhow::Result<CompareResponse> {
        self.provider().fetch_compare(full_name, base, head)
    }

    fn check_runs(&self, full_name: &str, git_ref: &str) -> anyhow::Result<Vec<CheckRun>> {
        self.provider().fetch_check_runs(full_name, git_ref)
    }

    fn workflow_runs(
        &self,
        full_name: &str,
        branch: &str,
        per_page: u8,
    ) -> anyhow::Result<Vec<WorkflowRun>> {
        self.provider()
            .fetch_workflow_runs(full_name, branch, per_page)
    }

    fn issues(&self, full_name: &str, state: &str, per_page: u8) -> anyhow::Result<Vec<Issue>> {
        self.provider().fetch_issues(full_name, state, per_page)
    }

    fn create_issue(
        &self,
        full_name: &str,
        title: &str,
        body: Option<&str>,
    ) -> anyhow::Result<Issue> {
        self.provider().create_issue(full_name, title, body)
    }

    fn close_issue(&self, full_name: &str, number: u64) -> anyhow::Result<Issue> {
        self.provider().close_issue(full_name, number)
    }

    fn pull_requests(
        &self,
        full_name: &str,
        state: &str,
        per_page: u8,
    ) -> anyhow::Result<Vec<PullRequest>> {
        self.provider()
            .fetch_pull_requests(full_name, state, per_page)
    }

    fn pull_request_detail(
        &self,
        full_name: &str,
        number: u64,
    ) -> anyhow::Result<PullRequestDetail> {
        self.provider().fetch_pull_request_detail(full_name, number)
    }

    fn pull_request_reviews(
        &self,
        full_name: &str,
        number: u64,
    ) -> anyhow::Result<Vec<PullRequestReview>> {
        self.provider()
            .fetch_pull_request_reviews(full_name, number)
    }

    fn pull_request_comments(
        &self,
        full_name: &str,
        number: u64,
    ) -> anyhow::Result<Vec<ReviewComment>> {
        self.provider()
            .fetch_pull_request_comments(full_name, number)
    }

    fn pull_request_commits(
        &self,
        full_name: &str,
        number: u64,
    ) -> anyhow::Result<Vec<CommitInfo>> {
        self.provider()
            .fetch_pull_request_commits(full_name, number)
    }

    fn create_pull_request(
        &self,
        full_name: &str,
        title: &str,
        head: &str,
        base: &str,
        body: Option<&str>,
    ) -> anyhow::Result<PullRequestDetail> {
        self.provider()
            .create_pull_request(full_name, title, head, base, body)
    }

    fn merge_pull_request(
        &self,
        full_name: &str,
        number: u64,
        commit_title: Option<&str>,
        method: Option<&str>,
    ) -> anyhow::Result<MergeResponse> {
        self.provider()
            .merge_pull_request(full_name, number, commit_title, method)
    }

    fn update_pull_request(&self, full_name: &str, number: u64, state: &str) -> anyhow::Result<()> {
        self.provider()
            .update_pull_request(full_name, number, state)
    }

    fn create_pull_request_review(
        &self,
        full_name: &str,
        number: u64,
        body: &str,
        event: &str,
    ) -> anyhow::Result<()> {
        self.provider()
            .create_pull_request_review(full_name, number, body, event)
    }

    fn create_pull_request_comment(
        &self,
        full_name: &str,
        number: u64,
        body: &str,
    ) -> anyhow::Result<()> {
        self.provider()
            .create_pull_request_comment(full_name, number, body)
    }

    fn releases(&self, full_name: &str, per_page: u8) -> anyhow::Result<Vec<Release>> {
        self.provider().fetch_releases(full_name, per_page)
    }

    fn create_release(
        &self,
        full_name: &str,
        tag_name: &str,
        name: Option<&str>,
        body: Option<&str>,
        prerelease: bool,
    ) -> anyhow::Result<Release> {
        self.provider()
            .create_release(full_name, tag_name, name, body, prerelease)
    }

    fn create_repo(
        &self,
        name: &str,
        description: Option<&str>,
        private: bool,
    ) -> anyhow::Result<RepoSummary> {
        self.provider().create_repo(name, description, private)
    }
}

// ── Error mapping ────────────────────────────────────────────────────────
//
// The protocol promises consistent JSON errors with *semantic* HTTP status
// codes. Anything not explicitly classified is reported as a generic 500 and
// the full error is logged server-side (never sent to the client).

/// Map an upstream error to (status code, safe client message).
pub fn classify(e: &anyhow::Error) -> (StatusCode, &'static str) {
    if let Some(_c) = e.downcast_ref::<Conflict>() {
        return (
            StatusCode::CONFLICT,
            "token cannot be changed in this configuration (check the server logs)",
        );
    }
    if let Some(gh) = e.downcast_ref::<GitHubError>() {
        return match gh {
            GitHubError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "authentication required — configure a GitHub token on the server",
            ),
            GitHubError::Api { status: 401, .. } => (
                StatusCode::UNAUTHORIZED,
                "authentication required — configure a GitHub token on the server",
            ),
            GitHubError::Api { status: 403, .. } => {
                (StatusCode::FORBIDDEN, "GitHub rejected the request (403)")
            }
            GitHubError::Api { status: 404, .. } | GitHubError::NotFound(_) => (
                StatusCode::NOT_FOUND,
                "repository, branch or file not found",
            ),
            GitHubError::RateLimited { .. } => (
                StatusCode::TOO_MANY_REQUESTS,
                "GitHub API rate limit exceeded — retry later",
            ),
            GitHubError::Network(_) => (StatusCode::BAD_GATEWAY, "cannot reach GitHub right now"),
            GitHubError::FileTooLarge(_) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "file too large for the GitHub Contents API",
            ),
            GitHubError::Api { status, .. } => {
                // 4xx/5xx from upstream surfaced with its own code (422, 409, …)
                let code =
                    StatusCode::from_u16(*status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                if code.is_client_error() {
                    (code, "GitHub rejected the request — check the payload")
                } else {
                    (StatusCode::BAD_GATEWAY, "GitHub API error")
                }
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal server error"),
        };
    }
    (StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
}

/// Build the JSON error response for an upstream error. Logs the real cause.
pub fn error_response(e: &anyhow::Error) -> Response {
    let (status, message) = classify(e);
    log::error!("{status} → {e:#}");
    (
        status,
        Json(ErrorDto {
            error: message.into(),
        }),
    )
        .into_response()
}

/// Generic 500 used when a worker task panicked or the join itself failed.
pub fn task_error_response(reason: &str) -> Response {
    log::error!("background task failed: {reason}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorDto {
            error: "internal server error".into(),
        }),
    )
        .into_response()
}

/// Build a 401 JSON response for the API token guard.
pub fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorDto {
            error: "authentication required — provide a valid API token".into(),
        }),
    )
        .into_response()
}
