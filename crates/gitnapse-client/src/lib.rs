//! Typed HTTP client for the GitNapse protocol.
//!
//! Consumes a running `gitnapse-server` over HTTP/JSON. It depends only on
//! `gitnapse-protocol` (the wire contract) — never on the `gitnapse` core — so
//! third-party apps integrate through the same stable API as the web UI.
//!
//! ```no_run
//! use gitnapse_client::Client;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = Client::new("http://127.0.0.1:8787")?;
//! let health = client.health().await?;
//! let prs = client.pull_requests("gitnapse/gitnapse", None, None).await?;
//! # Ok(())
//! # }
//! ```

use gitnapse_protocol::{
    API_PREFIX, AuthStatusDto, CheckRunDto, CommitDto, CompareDto, ContentDto, ErrorDto, HealthDto,
    IssueDto, MergeResultDto, PrCommentDto, PrDetailDto, PrReviewDto, PrSummaryDto, RateLimitDto,
    ReleaseDto, RepoDto, TreeNodeDto, UserDto, WorkflowRunDto,
};
use reqwest::StatusCode;
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("server returned {status}: {message}")]
    Api { status: StatusCode, message: String },
    #[error("server returned invalid JSON: {0}")]
    InvalidBody(#[from] serde_json::Error),
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("invalid server URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, ClientError>;

/// Client for a running GitNapse protocol server.
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    base: url::Url,
    api_token: Option<String>,
}

impl Client {
    /// Point at the base URL of a `gitnapse-server` (e.g. `http://127.0.0.1:8787`).
    pub fn new(base_url: &str) -> Result<Self> {
        Self::with_timeout(base_url, DEFAULT_TIMEOUT)
    }

    /// Like [`Self::new`] with a custom request timeout.
    pub fn with_timeout(base_url: &str, timeout: Duration) -> Result<Self> {
        let base = url::Url::parse(base_url)?;
        Ok(Self {
            http: reqwest::Client::builder().timeout(timeout).build()?,
            base,
            api_token: None,
        })
    }

    /// Enable `Authorization: Bearer <token>` when the server was started
    /// with an API token (`GITNAPSE_SERVER_TOKEN`).
    pub fn with_api_token(mut self, token: impl Into<String>) -> Self {
        self.api_token = Some(token.into());
        self
    }

    fn endpoint(&self, path: &str) -> Result<url::Url> {
        Ok(self.base.join(path)?)
    }

    async fn send(
        &self,
        method: reqwest::Method,
        url: url::Url,
        body: Option<serde_json::Value>,
    ) -> Result<reqwest::Response> {
        let mut request = self.http.request(method, url);
        if let Some(token) = &self.api_token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await?;
        if response.status().is_success() {
            Ok(response)
        } else {
            let status = response.status();
            let error: ErrorDto = match response.json().await {
                Ok(e) => e,
                Err(_) => ErrorDto {
                    error: "non-JSON error body".into(),
                },
            };
            Err(ClientError::Api {
                status,
                message: error.error,
            })
        }
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: url::Url) -> Result<T> {
        let response = self.send(reqwest::Method::GET, url, None).await?;
        Ok(response.json::<T>().await?)
    }

    /// POST a JSON body and decode the JSON response.
    async fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<T> {
        let url = self.endpoint(&format!("{API_PREFIX}{path}"))?;
        let value = serde_json::to_value(body)?;
        let response = self.send(reqwest::Method::POST, url, Some(value)).await?;
        Ok(response.json::<T>().await?)
    }

    /// POST a JSON body and discard the (empty) response body.
    async fn post_void(&self, path: &str, body: &impl serde::Serialize) -> Result<()> {
        let url = self.endpoint(&format!("{API_PREFIX}{path}"))?;
        let value = serde_json::to_value(body)?;
        let response = self.send(reqwest::Method::POST, url, Some(value)).await?;
        drop(response);
        Ok(())
    }

    /// DELETE and discard the (empty) response body.
    async fn delete_void(&self, path: &str) -> Result<()> {
        let url = self.endpoint(&format!("{API_PREFIX}{path}"))?;
        let response = self.send(reqwest::Method::DELETE, url, None).await?;
        drop(response);
        Ok(())
    }

    // ── Auth management ──────────────────────────────────────────────────

    /// `GET /api/v1/auth/status` — whether a token is active and its source.
    pub async fn auth_status(&self) -> Result<AuthStatusDto> {
        let url = self.endpoint(&format!("{API_PREFIX}/auth/status"))?;
        self.get_json(url).await
    }

    /// `POST /api/v1/auth/token` — validate, store and activate a new token.
    pub async fn set_token(&self, token: &str) -> Result<()> {
        let body = serde_json::json!({ "token": token });
        self.post_void("/auth/token", &body).await
    }

    /// `DELETE /api/v1/auth/token` — forget the stored token (anonymous).
    pub async fn clear_token(&self) -> Result<()> {
        self.delete_void("/auth/token").await
    }

    // ── Infrastructure ───────────────────────────────────────────────────

    /// `GET /health` — infrastructure status.
    pub async fn health(&self) -> Result<HealthDto> {
        let url = self.endpoint("/health")?;
        self.get_json(url).await
    }

    // ── Identity / discovery ─────────────────────────────────────────────

    /// `GET /api/v1/user` — the authenticated GitHub login (401 if anonymous).
    pub async fn user(&self) -> Result<UserDto> {
        let url = self.endpoint(&format!("{API_PREFIX}/user"))?;
        self.get_json(url).await
    }

    /// `GET /api/v1/user/starred` — repositories starred by the user.
    pub async fn starred_repos(
        &self,
        page: Option<u32>,
        per_page: Option<u8>,
    ) -> Result<Vec<RepoDto>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/user/starred"))?;
        url.query_pairs_mut()
            .append_pair("page", &page.unwrap_or(1).to_string())
            .append_pair("per_page", &per_page.unwrap_or(30).to_string());
        self.get_json(url).await
    }

    /// `GET /api/v1/rate-limit` — last known GitHub rate-limit headers.
    pub async fn rate_limit(&self) -> Result<RateLimitDto> {
        let url = self.endpoint(&format!("{API_PREFIX}/rate-limit"))?;
        self.get_json(url).await
    }

    // ── Content ──────────────────────────────────────────────────────────

    /// `GET /api/v1/search` — search repositories.
    pub async fn search(
        &self,
        query: &str,
        page: Option<u32>,
        per_page: Option<u8>,
    ) -> Result<Vec<RepoDto>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/search"))?;
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("page", &page.unwrap_or(1).to_string())
            .append_pair("per_page", &per_page.unwrap_or(30).to_string());
        self.get_json(url).await
    }

    /// `GET /api/v1/repos/detail` — repository metadata for `owner/name`.
    pub async fn repo(&self, repo: &str) -> Result<RepoDto> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/repos/detail"))?;
        url.query_pairs_mut().append_pair("repo", repo);
        self.get_json(url).await
    }

    /// `GET /api/v1/repos/branches` — list branches of `owner/name`.
    pub async fn branches(&self, repo: &str) -> Result<Vec<String>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/repos/branches"))?;
        url.query_pairs_mut().append_pair("repo", repo);
        self.get_json(url).await
    }

    /// `GET /api/v1/repos/tree` — full repository tree for a ref.
    pub async fn tree(&self, repo: &str, git_ref: Option<&str>) -> Result<Vec<TreeNodeDto>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/repos/tree"))?;
        url.query_pairs_mut().append_pair("repo", repo);
        if let Some(git_ref) = git_ref {
            url.query_pairs_mut().append_pair("ref", git_ref);
        }
        self.get_json(url).await
    }

    /// `GET /api/v1/repos/content` — file content (base64) for a ref.
    pub async fn content(
        &self,
        repo: &str,
        path: &str,
        git_ref: Option<&str>,
    ) -> Result<ContentDto> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/repos/content"))?;
        url.query_pairs_mut()
            .append_pair("repo", repo)
            .append_pair("path", path);
        if let Some(git_ref) = git_ref {
            url.query_pairs_mut().append_pair("ref", git_ref);
        }
        self.get_json(url).await
    }

    // ── Commits / CI ─────────────────────────────────────────────────────

    /// `GET /api/v1/commits` — recent commits of a branch.
    pub async fn recent_commits(
        &self,
        repo: &str,
        git_ref: Option<&str>,
        per_page: Option<u8>,
    ) -> Result<Vec<CommitDto>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/commits"))?;
        url.query_pairs_mut().append_pair("repo", repo);
        if let Some(git_ref) = git_ref {
            url.query_pairs_mut().append_pair("ref", git_ref);
        }
        if let Some(per_page) = per_page {
            url.query_pairs_mut()
                .append_pair("per_page", &per_page.to_string());
        }
        self.get_json(url).await
    }

    /// `GET /api/v1/compare` — ahead/behind comparison between two refs.
    pub async fn compare(&self, repo: &str, base: &str, head: &str) -> Result<CompareDto> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/compare"))?;
        url.query_pairs_mut()
            .append_pair("repo", repo)
            .append_pair("base", base)
            .append_pair("head", head);
        self.get_json(url).await
    }

    /// `GET /api/v1/checks` — check runs for a ref.
    pub async fn check_runs(&self, repo: &str, git_ref: &str) -> Result<Vec<CheckRunDto>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/checks"))?;
        url.query_pairs_mut()
            .append_pair("repo", repo)
            .append_pair("ref", git_ref);
        self.get_json(url).await
    }

    /// `GET /api/v1/workflows` — workflow runs filtered by branch.
    pub async fn workflow_runs(
        &self,
        repo: &str,
        branch: Option<&str>,
        per_page: Option<u8>,
    ) -> Result<Vec<WorkflowRunDto>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/workflows"))?;
        url.query_pairs_mut().append_pair("repo", repo);
        if let Some(branch) = branch {
            url.query_pairs_mut().append_pair("branch", branch);
        }
        if let Some(per_page) = per_page {
            url.query_pairs_mut()
                .append_pair("per_page", &per_page.to_string());
        }
        self.get_json(url).await
    }

    // ── Issues ───────────────────────────────────────────────────────────

    /// `GET /api/v1/issues` — list issues (`state`: open|closed|all).
    pub async fn issues(
        &self,
        repo: &str,
        state: Option<&str>,
        per_page: Option<u8>,
    ) -> Result<Vec<IssueDto>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/issues"))?;
        url.query_pairs_mut().append_pair("repo", repo);
        if let Some(state) = state {
            url.query_pairs_mut().append_pair("state", state);
        }
        if let Some(per_page) = per_page {
            url.query_pairs_mut()
                .append_pair("per_page", &per_page.to_string());
        }
        self.get_json(url).await
    }

    /// `POST /api/v1/issues` — create an issue.
    pub async fn create_issue(
        &self,
        repo: &str,
        title: &str,
        body: Option<&str>,
    ) -> Result<IssueDto> {
        let body_json = serde_json::json!({ "repo": repo, "title": title, "body": body });
        self.post_json("/issues", &body_json).await
    }

    /// `POST /api/v1/issues/close` — close an issue.
    pub async fn close_issue(&self, repo: &str, number: u64) -> Result<IssueDto> {
        let body = serde_json::json!({ "repo": repo, "number": number });
        self.post_json("/issues/close", &body).await
    }

    // ── Pull requests ────────────────────────────────────────────────────

    /// `GET /api/v1/pulls` — list pull requests (`state`: open|closed|all).
    pub async fn pull_requests(
        &self,
        repo: &str,
        state: Option<&str>,
        per_page: Option<u8>,
    ) -> Result<Vec<PrSummaryDto>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/pulls"))?;
        url.query_pairs_mut().append_pair("repo", repo);
        if let Some(state) = state {
            url.query_pairs_mut().append_pair("state", state);
        }
        if let Some(per_page) = per_page {
            url.query_pairs_mut()
                .append_pair("per_page", &per_page.to_string());
        }
        self.get_json(url).await
    }

    /// `GET /api/v1/pulls/detail` — full pull request detail.
    pub async fn pull_request(&self, repo: &str, number: u64) -> Result<PrDetailDto> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/pulls/detail"))?;
        url.query_pairs_mut()
            .append_pair("repo", repo)
            .append_pair("number", &number.to_string());
        self.get_json(url).await
    }

    /// `POST /api/v1/pulls` — open a pull request.
    pub async fn create_pull_request(
        &self,
        repo: &str,
        title: &str,
        head: &str,
        base: &str,
        body: Option<&str>,
    ) -> Result<PrDetailDto> {
        let body_json = serde_json::json!({
            "repo": repo, "title": title, "head": head, "base": base, "body": body
        });
        self.post_json("/pulls", &body_json).await
    }

    /// `POST /api/v1/pulls/merge` — merge a pull request.
    pub async fn merge_pull_request(
        &self,
        repo: &str,
        number: u64,
        commit_title: Option<&str>,
        method: Option<&str>,
    ) -> Result<MergeResultDto> {
        let body = serde_json::json!({
            "repo": repo, "number": number, "commit_title": commit_title, "method": method
        });
        self.post_json("/pulls/merge", &body).await
    }

    /// `POST /api/v1/pulls/update` — open or close a pull request.
    pub async fn update_pull_request(&self, repo: &str, number: u64, state: &str) -> Result<()> {
        let body = serde_json::json!({ "repo": repo, "number": number, "state": state });
        self.post_void("/pulls/update", &body).await
    }

    /// `GET /api/v1/pulls/reviews` — reviews submitted on a pull request.
    pub async fn pull_request_reviews(&self, repo: &str, number: u64) -> Result<Vec<PrReviewDto>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/pulls/reviews"))?;
        url.query_pairs_mut()
            .append_pair("repo", repo)
            .append_pair("number", &number.to_string());
        self.get_json(url).await
    }

    /// `POST /api/v1/pulls/reviews` — approve, request changes or comment on a PR.
    pub async fn review_pull_request(
        &self,
        repo: &str,
        number: u64,
        event: &str,
        body: Option<&str>,
    ) -> Result<()> {
        let payload = serde_json::json!({
            "repo": repo, "number": number, "event": event, "body": body
        });
        self.post_void("/pulls/reviews", &payload).await
    }

    /// `GET /api/v1/pulls/comments` — inline review comments.
    pub async fn pull_request_comments(
        &self,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PrCommentDto>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/pulls/comments"))?;
        url.query_pairs_mut()
            .append_pair("repo", repo)
            .append_pair("number", &number.to_string());
        self.get_json(url).await
    }

    /// `POST /api/v1/pulls/comments` — comment on a pull request.
    pub async fn comment_pull_request(&self, repo: &str, number: u64, body: &str) -> Result<()> {
        let payload = serde_json::json!({ "repo": repo, "number": number, "body": body });
        self.post_void("/pulls/comments", &payload).await
    }

    /// `GET /api/v1/pulls/commits` — commits of a pull request.
    pub async fn pull_request_commits(&self, repo: &str, number: u64) -> Result<Vec<CommitDto>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/pulls/commits"))?;
        url.query_pairs_mut()
            .append_pair("repo", repo)
            .append_pair("number", &number.to_string());
        self.get_json(url).await
    }

    // ── Releases / repos ─────────────────────────────────────────────────

    /// `GET /api/v1/releases` — releases of a repository.
    pub async fn releases(&self, repo: &str, per_page: Option<u8>) -> Result<Vec<ReleaseDto>> {
        let mut url = self.endpoint(&format!("{API_PREFIX}/releases"))?;
        url.query_pairs_mut().append_pair("repo", repo);
        if let Some(per_page) = per_page {
            url.query_pairs_mut()
                .append_pair("per_page", &per_page.to_string());
        }
        self.get_json(url).await
    }

    /// `POST /api/v1/releases` — create a release from an existing tag.
    pub async fn create_release(
        &self,
        repo: &str,
        tag_name: &str,
        name: Option<&str>,
        body: Option<&str>,
        prerelease: bool,
    ) -> Result<ReleaseDto> {
        let payload = serde_json::json!({
            "repo": repo, "tag_name": tag_name, "name": name, "body": body, "prerelease": prerelease
        });
        self.post_json("/releases", &payload).await
    }

    /// `POST /api/v1/repos` — create a repository for the authenticated user.
    pub async fn create_repo(
        &self,
        name: &str,
        description: Option<&str>,
        private: bool,
    ) -> Result<RepoDto> {
        let payload = serde_json::json!({
            "name": name, "description": description, "private": private
        });
        self.post_json("/repos", &payload).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_joins_under_api_prefix() {
        let client = Client::new("http://127.0.0.1:8787/").unwrap();
        let url = client.endpoint("/api/v1/search").unwrap().to_string();
        assert_eq!(url, "http://127.0.0.1:8787/api/v1/search");
    }

    #[test]
    fn trailing_slash_is_normalized() {
        let client = Client::new("http://127.0.0.1:8787").unwrap();
        let url = client.endpoint("/health").unwrap().to_string();
        assert_eq!(url, "http://127.0.0.1:8787/health");
    }

    #[test]
    fn query_pairs_are_built() {
        let client = Client::new("http://localhost:8787").unwrap();
        let mut url = client.endpoint("/api/v1/search").unwrap();
        url.query_pairs_mut()
            .append_pair("q", "rust wasm")
            .append_pair("per_page", "20");
        assert!(url.to_string().contains("q=rust+wasm"));
        assert!(url.to_string().contains("per_page=20"));
    }

    #[test]
    fn invalid_base_url_is_rejected() {
        assert!(Client::new("not a url").is_err());
    }
}
