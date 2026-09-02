//! Backend abstraction used by the HTTP routes.
//!
//! Routes only depend on the small [`Backend`] trait (plus the protocol
//! types), which keeps them unit-testable with an in-memory fake. The real
//! implementation [`ApiService`] consumes `gitnapse` as a library, reusing the
//! same provider/domain layer as the TUI and CLI. [`Backend`] mirrors the
//! full `gitnapse::provider::GitProvider` surface so no SDK capability is
//! unreachable through the protocol.

use std::sync::Arc;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use gitnapse::error::GitHubError;
use gitnapse::models::{
    CheckRun, CommitInfo, CompareResponse, Issue, MergeResponse, PullRequest, PullRequestDetail,
    PullRequestReview, Release, RepoNode, RepoSummary, ReviewComment, WorkflowRun,
};
use gitnapse::provider::{GitProvider, ProviderKind, create_provider};
use gitnapse_protocol::ErrorDto;

/// Minimal operations the HTTP layer needs. This is the seam between the
/// protocol routes and the gitnapse SDK (or any future backend).
pub trait Backend: Send + Sync {
    // ── Identity / discovery ─────────────────────────────────────────────
    fn authenticated_user(&self) -> anyhow::Result<Option<String>>;
    fn search(&self, query: &str, page: u32, per_page: u8) -> anyhow::Result<Vec<RepoSummary>>;
    fn starred_repos(&self, page: u32, per_page: u8) -> anyhow::Result<Vec<RepoSummary>>;
    fn repo_by_name(&self, full_name: &str) -> anyhow::Result<RepoSummary>;
    fn rate_limit(&self) -> (Option<u32>, Option<u64>);

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
pub struct ApiService {
    github: Arc<dyn GitProvider>,
}

impl ApiService {
    /// Build the service from the environment (token via env or secure store).
    pub fn from_env() -> anyhow::Result<Self> {
        gitnapse::runtime::ensure_crypto_provider();
        let token = gitnapse::auth::load_token()?;
        let github = create_provider(ProviderKind::GitHub, token.as_deref())?;
        Ok(Self { github })
    }

    /// Warn at startup when the server will run anonymously.
    pub fn warn_if_anonymous(&self) {
        let token = gitnapse::auth::load_token();
        if token.as_ref().map(|t| t.is_none()).unwrap_or(true) {
            log::warn!(
                "no GitHub token found — requests will run anonymously with strict rate limits; \
                 set GITHUB_TOKEN or run `gitnapse auth set`"
            );
        }
    }
}

impl Backend for ApiService {
    fn authenticated_user(&self) -> anyhow::Result<Option<String>> {
        self.github.fetch_authenticated_user()
    }

    fn search(&self, query: &str, page: u32, per_page: u8) -> anyhow::Result<Vec<RepoSummary>> {
        self.github.search_repositories_page(query, page, per_page)
    }

    fn starred_repos(&self, page: u32, per_page: u8) -> anyhow::Result<Vec<RepoSummary>> {
        self.github.fetch_starred_repos(page, per_page)
    }

    fn repo_by_name(&self, full_name: &str) -> anyhow::Result<RepoSummary> {
        self.github.fetch_repo_by_name(full_name)
    }

    fn rate_limit(&self) -> (Option<u32>, Option<u64>) {
        (
            self.github.rate_limit_remaining(),
            self.github.rate_limit_reset(),
        )
    }

    fn branches(&self, full_name: &str) -> anyhow::Result<Vec<String>> {
        self.github.fetch_branches(full_name)
    }

    fn tree(&self, full_name: &str, git_ref: &str) -> anyhow::Result<Vec<RepoNode>> {
        self.github.fetch_repo_tree(full_name, git_ref)
    }

    fn file_content(&self, full_name: &str, path: &str, git_ref: &str) -> anyhow::Result<Vec<u8>> {
        self.github
            .fetch_file_content_by_ref(full_name, path, git_ref)
    }

    fn recent_commits(
        &self,
        full_name: &str,
        branch: &str,
        per_page: u8,
    ) -> anyhow::Result<Vec<CommitInfo>> {
        self.github
            .fetch_recent_commits(full_name, branch, per_page)
    }

    fn compare(&self, full_name: &str, base: &str, head: &str) -> anyhow::Result<CompareResponse> {
        self.github.fetch_compare(full_name, base, head)
    }

    fn check_runs(&self, full_name: &str, git_ref: &str) -> anyhow::Result<Vec<CheckRun>> {
        self.github.fetch_check_runs(full_name, git_ref)
    }

    fn workflow_runs(
        &self,
        full_name: &str,
        branch: &str,
        per_page: u8,
    ) -> anyhow::Result<Vec<WorkflowRun>> {
        self.github.fetch_workflow_runs(full_name, branch, per_page)
    }

    fn issues(&self, full_name: &str, state: &str, per_page: u8) -> anyhow::Result<Vec<Issue>> {
        self.github.fetch_issues(full_name, state, per_page)
    }

    fn create_issue(
        &self,
        full_name: &str,
        title: &str,
        body: Option<&str>,
    ) -> anyhow::Result<Issue> {
        self.github.create_issue(full_name, title, body)
    }

    fn close_issue(&self, full_name: &str, number: u64) -> anyhow::Result<Issue> {
        self.github.close_issue(full_name, number)
    }

    fn pull_requests(
        &self,
        full_name: &str,
        state: &str,
        per_page: u8,
    ) -> anyhow::Result<Vec<PullRequest>> {
        self.github.fetch_pull_requests(full_name, state, per_page)
    }

    fn pull_request_detail(
        &self,
        full_name: &str,
        number: u64,
    ) -> anyhow::Result<PullRequestDetail> {
        self.github.fetch_pull_request_detail(full_name, number)
    }

    fn pull_request_reviews(
        &self,
        full_name: &str,
        number: u64,
    ) -> anyhow::Result<Vec<PullRequestReview>> {
        self.github.fetch_pull_request_reviews(full_name, number)
    }

    fn pull_request_comments(
        &self,
        full_name: &str,
        number: u64,
    ) -> anyhow::Result<Vec<ReviewComment>> {
        self.github.fetch_pull_request_comments(full_name, number)
    }

    fn pull_request_commits(
        &self,
        full_name: &str,
        number: u64,
    ) -> anyhow::Result<Vec<CommitInfo>> {
        self.github.fetch_pull_request_commits(full_name, number)
    }

    fn create_pull_request(
        &self,
        full_name: &str,
        title: &str,
        head: &str,
        base: &str,
        body: Option<&str>,
    ) -> anyhow::Result<PullRequestDetail> {
        self.github
            .create_pull_request(full_name, title, head, base, body)
    }

    fn merge_pull_request(
        &self,
        full_name: &str,
        number: u64,
        commit_title: Option<&str>,
        method: Option<&str>,
    ) -> anyhow::Result<MergeResponse> {
        self.github
            .merge_pull_request(full_name, number, commit_title, method)
    }

    fn update_pull_request(&self, full_name: &str, number: u64, state: &str) -> anyhow::Result<()> {
        self.github.update_pull_request(full_name, number, state)
    }

    fn create_pull_request_review(
        &self,
        full_name: &str,
        number: u64,
        body: &str,
        event: &str,
    ) -> anyhow::Result<()> {
        self.github
            .create_pull_request_review(full_name, number, body, event)
    }

    fn create_pull_request_comment(
        &self,
        full_name: &str,
        number: u64,
        body: &str,
    ) -> anyhow::Result<()> {
        self.github
            .create_pull_request_comment(full_name, number, body)
    }

    fn releases(&self, full_name: &str, per_page: u8) -> anyhow::Result<Vec<Release>> {
        self.github.fetch_releases(full_name, per_page)
    }

    fn create_release(
        &self,
        full_name: &str,
        tag_name: &str,
        name: Option<&str>,
        body: Option<&str>,
        prerelease: bool,
    ) -> anyhow::Result<Release> {
        self.github
            .create_release(full_name, tag_name, name, body, prerelease)
    }

    fn create_repo(
        &self,
        name: &str,
        description: Option<&str>,
        private: bool,
    ) -> anyhow::Result<RepoSummary> {
        self.github.create_repo(name, description, private)
    }
}

// ── Error mapping ────────────────────────────────────────────────────────
//
// The protocol promises consistent JSON errors with *semantic* HTTP status
// codes. Anything not explicitly classified is reported as a generic 500 and
// the full error is logged server-side (never sent to the client).

/// Map an upstream error to (status code, safe client message).
pub fn classify(e: &anyhow::Error) -> (StatusCode, &'static str) {
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
