//! The GitNapse communication protocol.
//!
//! This crate defines the stable wire contract between the GitNapse core and
//! ANY interface (web UI, desktop GUI, CLI wrappers, automation, third-party
//! apps). It is deliberately independent of `gitnapse` itself so any client in
//! any language can implement it.
//!
//! The operations are documented in `docs/PROTOCOL.md`. The reference server
//! implementation lives in the `gitnapse-server` crate.
//!
//! This crate has no runtime dependencies other than `serde`, so request and
//! response types can be reused by servers (for validation) and clients (for
//! parsing) alike.

use serde::{Deserialize, Serialize};

/// URL prefix of the current protocol version.
///
/// Breaking changes to the protocol bump this value (e.g. to `/api/v2`),
/// while the previous version keeps serving until deprecated.
pub const API_PREFIX: &str = "/api/v1";

// ── Responses ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthDto {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoDto {
    pub full_name: String,
    pub name: String,
    pub owner: String,
    pub description: Option<String>,
    pub stargazers_count: u64,
    pub language: Option<String>,
    pub default_branch: String,
    pub clone_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNodeDto {
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentDto {
    pub path: String,
    /// File content base64-encoded (binary-safe).
    pub content: String,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDto {
    pub error: String,
}

// ── Operations beyond the core four (full GitProvider surface) ──────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDto {
    pub login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelDto {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorDto {
    pub login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDto {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub html_url: String,
    pub user: ActorDto,
    pub labels: Vec<LabelDto>,
    pub created_at: String,
    pub updated_at: String,
    pub body: Option<String>,
    /// Present when the issue is actually a pull request.
    pub is_pr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrSummaryDto {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub html_url: String,
    pub user: ActorDto,
    pub body: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
    pub changed_files: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrBranchDto {
    pub label: String,
    pub r#ref: String,
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrDetailDto {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub body: Option<String>,
    pub html_url: String,
    pub user: ActorDto,
    pub created_at: String,
    pub updated_at: String,
    pub merge_commit_sha: Option<String>,
    pub merged: Option<bool>,
    pub merged_by: Option<ActorDto>,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
    pub changed_files: Option<u32>,
    pub commits: Option<u32>,
    pub comments: Option<u32>,
    pub review_comments: Option<u32>,
    pub head: PrBranchDto,
    pub base: PrBranchDto,
    pub labels: Vec<LabelDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrReviewDto {
    pub id: u64,
    pub user: ActorDto,
    pub body: Option<String>,
    pub state: String,
    pub submitted_at: Option<String>,
    pub commit_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrCommentDto {
    pub id: u64,
    pub user: ActorDto,
    pub body: String,
    pub path: Option<String>,
    pub position: Option<u64>,
    pub commit_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResultDto {
    pub sha: String,
    pub merged: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitDto {
    pub sha: String,
    pub message: String,
    pub author_name: String,
    pub author_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffFileDto {
    pub filename: String,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub changes: u32,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareDto {
    pub status: String,
    pub ahead_by: u32,
    pub behind_by: u32,
    pub total_commits: u32,
    pub files: Vec<DiffFileDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRunDto {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub html_url: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRunDto {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub html_url: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseDto {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub html_url: String,
    pub created_at: String,
    pub published_at: Option<String>,
    pub prerelease: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitDto {
    pub remaining: Option<u32>,
    pub reset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatusDto {
    /// Whether a GitHub token is currently active on the server.
    pub has_token: bool,
    /// `env`, `oauth`, `stored` or `none`.
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenSetRequest {
    /// GitHub personal access token (or OAuth token) to store and activate.
    pub token: String,
}

// ── Requests (query parameters, URL-encoded) ────────────────────────────
//
// These structs are deserialized directly from the URL query string by any
// HTTP implementation (axum `Query`, `serde_urlencoded`, ...), so they are
// part of the wire contract and must not drift from the server routes.

#[derive(Debug, Default, Clone, Deserialize)]
pub struct SearchRequest {
    pub q: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoRequest {
    /// Repository in `owner/name` form.
    pub repo: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TreeRequest {
    pub repo: String,
    /// Branch/tag/commit. Defaults to the repository default branch.
    pub r#ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentRequest {
    pub repo: String,
    /// Path inside the repository.
    pub path: String,
    pub r#ref: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PageRequest {
    pub page: Option<u32>,
    pub per_page: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateRepoRequest {
    pub repo: String,
    /// `open`, `closed` or `all`. Defaults to `open`.
    pub state: Option<String>,
    pub per_page: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NumberRepoRequest {
    pub repo: String,
    pub number: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommitListRequest {
    pub repo: String,
    /// Branch/tag/commit. Defaults to the repository default branch.
    pub r#ref: Option<String>,
    pub per_page: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompareRequest {
    pub repo: String,
    pub base: String,
    pub head: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefRepoRequest {
    pub repo: String,
    pub r#ref: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowRunsRequest {
    pub repo: String,
    /// Branch to filter by. Defaults to `main`.
    pub branch: Option<String>,
    pub per_page: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleasesRequest {
    pub repo: String,
    pub per_page: Option<u8>,
}

// ── Requests (JSON bodies) ──────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct IssueCreateRequest {
    pub repo: String,
    pub title: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrCreateRequest {
    pub repo: String,
    pub title: String,
    /// Source branch.
    pub head: String,
    /// Target branch.
    pub base: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrMergeRequest {
    pub repo: String,
    pub number: u64,
    pub commit_title: Option<String>,
    /// `merge`, `squash` or `rebase`. Defaults to `merge`.
    pub method: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrUpdateRequest {
    pub repo: String,
    pub number: u64,
    /// `open` or `closed`.
    pub state: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrCommentRequest {
    pub repo: String,
    pub number: u64,
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrReviewRequest {
    pub repo: String,
    pub number: u64,
    /// `approve`, `request_changes` or `comment`.
    pub event: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseCreateRequest {
    pub repo: String,
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoCreateRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub private: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip<T: Serialize + for<'de> Deserialize<'de>>(value: &T) -> T {
        let json = serde_json::to_string(value).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn response_types_roundtrip() {
        let health = HealthDto {
            status: "ok".into(),
            version: "0.1.0".into(),
        };
        assert_eq!(roundtrip(&health).status, "ok");

        let err = ErrorDto {
            error: "boom".into(),
        };
        assert_eq!(roundtrip(&err).error, "boom");
    }

    #[test]
    fn response_dtos_serialize_expected_shapes() {
        let repo = RepoDto {
            full_name: "gitnapse/gitnapse".into(),
            name: "gitnapse".into(),
            owner: "gitnapse".into(),
            description: None,
            stargazers_count: 1,
            language: Some("Rust".into()),
            default_branch: "main".into(),
            clone_url: "https://github.com/gitnapse/gitnapse.git".into(),
        };
        let json = serde_json::to_value(&repo).unwrap();
        assert_eq!(json["full_name"], "gitnapse/gitnapse");
        assert_eq!(json["owner"], "gitnapse");
        assert!(json.get("description").unwrap().is_null());

        let node = TreeNodeDto {
            path: "src/main.rs".into(),
            name: "main.rs".into(),
            depth: 1,
            is_dir: false,
        };
        let json = serde_json::to_value(&node).unwrap();
        assert_eq!(json["is_dir"], false);

        let content = ContentDto {
            path: "README.md".into(),
            content: "aGVsbG8=".into(),
            size: 5,
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["content"], "aGVsbG8=");
    }

    #[test]
    fn search_request_parses_partial_fields() {
        let full: SearchRequest =
            serde_json::from_value(serde_json::json!({ "q": "rust", "page": 3, "per_page": 50 }))
                .unwrap();
        assert_eq!(full.q.as_deref(), Some("rust"));
        assert_eq!(full.page, Some(3));
        assert_eq!(full.per_page, Some(50));

        let empty: SearchRequest = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(empty.q.is_none());
        assert!(empty.page.is_none());
    }

    #[test]
    fn repo_and_tree_requests_parse_optional_ref() {
        let repo: RepoRequest =
            serde_json::from_value(serde_json::json!({ "repo": "a/b" })).unwrap();
        assert_eq!(repo.repo, "a/b");

        let tree: TreeRequest =
            serde_json::from_value(serde_json::json!({ "repo": "a/b", "ref": "dev" })).unwrap();
        assert_eq!(tree.r#ref.as_deref(), Some("dev"));

        let content: ContentRequest =
            serde_json::from_value(serde_json::json!({ "repo": "a/b", "path": "x/y.rs" })).unwrap();
        assert_eq!(content.path, "x/y.rs");
        assert!(content.r#ref.is_none());
    }

    #[test]
    fn missing_required_field_is_rejected() {
        let err = serde_json::from_value::<RepoRequest>(serde_json::json!({})).unwrap_err();
        assert!(err.is_data());
    }

    #[test]
    fn api_prefix_is_versioned() {
        assert_eq!(API_PREFIX, "/api/v1");
    }

    #[test]
    fn extended_requests_parse_query_and_bodies() {
        let page: PageRequest = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(page.page.is_none());

        let state: StateRepoRequest =
            serde_json::from_value(serde_json::json!({ "repo": "a/b" })).unwrap();
        assert_eq!(state.state.as_deref(), None);

        let compare: CompareRequest = serde_json::from_value(
            serde_json::json!({ "repo": "a/b", "base": "main", "head": "dev" }),
        )
        .unwrap();
        assert_eq!(compare.head, "dev");

        let review: PrReviewRequest = serde_json::from_value(serde_json::json!({
            "repo": "a/b", "number": 7, "event": "approve", "body": "lgtm"
        }))
        .unwrap();
        assert_eq!(review.event, "approve");

        let release: ReleaseCreateRequest = serde_json::from_value(serde_json::json!({
            "repo": "a/b", "tag_name": "v1.0", "prerelease": true
        }))
        .unwrap();
        assert!(release.prerelease);
        assert!(release.name.is_none());

        let repo: RepoCreateRequest =
            serde_json::from_value(serde_json::json!({ "name": "x", "private": true })).unwrap();
        assert!(repo.private);

        let _merge: PrMergeRequest = serde_json::from_value(serde_json::json!({
            "repo": "a/b", "number": 1, "method": "squash"
        }))
        .unwrap();
    }

    #[test]
    fn extended_dtos_roundtrip() {
        let issue = IssueDto {
            number: 1,
            title: "bug".into(),
            state: "open".into(),
            html_url: "https://github.com/a/b/issues/1".into(),
            user: ActorDto { login: "x".into() },
            labels: vec![LabelDto {
                name: "bug".into(),
                color: "d73a4a".into(),
            }],
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-02T00:00:00Z".into(),
            body: None,
            is_pr: false,
        };
        let back: IssueDto = serde_json::from_value(serde_json::to_value(&issue).unwrap()).unwrap();
        assert_eq!(back.number, 1);
        assert_eq!(back.labels[0].name, "bug");

        let release = ReleaseDto {
            tag_name: "v0.1.0".into(),
            name: Some("v0.1.0".into()),
            body: None,
            html_url: "https://github.com/a/b/releases/tag/v0.1.0".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            published_at: None,
            prerelease: false,
        };
        let back: ReleaseDto =
            serde_json::from_value(serde_json::to_value(&release).unwrap()).unwrap();
        assert_eq!(back.tag_name, "v0.1.0");

        let commit = CommitDto {
            sha: "abc".into(),
            message: "fix".into(),
            author_name: "x".into(),
            author_date: "2026-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_value(&commit).unwrap();
        assert_eq!(json["author_name"], "x");

        let rate = RateLimitDto {
            remaining: Some(42),
            reset: Some(1234),
        };
        let json = serde_json::to_value(&rate).unwrap();
        assert_eq!(json["remaining"], 42);
    }
}
