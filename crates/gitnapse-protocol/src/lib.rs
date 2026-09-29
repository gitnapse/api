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
    // Additive enrichment (all optional so older producers stay compatible).
    /// Web URL of the repository.
    pub html_url: Option<String>,
    pub forks_count: Option<u64>,
    pub open_issues_count: Option<u64>,
    pub watchers_count: Option<u64>,
    pub private: Option<bool>,
    pub topics: Option<Vec<String>>,
    pub updated_at: Option<String>,
    pub pushed_at: Option<String>,
    /// Avatar URL of the repository owner (`owner` keeps the login).
    pub owner_avatar_url: Option<String>,
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
    /// Avatar URL when the upstream payload includes one.
    pub avatar_url: Option<String>,
}

/// A conversation comment on an issue or pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCommentDto {
    pub id: u64,
    pub user: ActorDto,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
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
    /// GitHub account matched to the commit author, when available.
    pub author: Option<ActorDto>,
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

// ── Users / profile, activity, notifications, search, insights ──────────

/// Public profile of a GitHub user (`GET /users/profile`, user search).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfileDto {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub company: Option<String>,
    pub location: Option<String>,
    pub blog: Option<String>,
    pub followers: u64,
    pub following: u64,
    pub public_repos: u64,
    pub html_url: String,
    pub created_at: Option<String>,
}

/// A compact public activity event from a user's feed.
///
/// `kind` is one of `push`, `pull_request`, `issues`, `release`, `create`,
/// `watch`, `fork` or `other`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDto {
    pub id: String,
    pub kind: String,
    pub actor: String,
    pub actor_avatar_url: Option<String>,
    pub repo: String,
    pub action: Option<String>,
    pub title: Option<String>,
    pub created_at: String,
}

/// A thread from the authenticated user's notifications inbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationDto {
    /// Thread id accepted by `POST /users/notifications/read`.
    pub id: String,
    pub unread: bool,
    pub reason: String,
    pub subject_type: String,
    pub subject_title: String,
    pub repo: Option<String>,
    pub updated_at: String,
    pub html_url: Option<String>,
}

/// A single code search hit (`repo` is the repository `full_name`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSearchResultDto {
    pub repo: String,
    pub path: String,
    pub name: String,
    pub sha: String,
    pub html_url: String,
}

/// Bytes of code per language for a repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageDto {
    pub name: String,
    pub bytes: u64,
}

/// A repository contributor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributorDto {
    pub login: String,
    pub avatar_url: Option<String>,
    pub contributions: u64,
    pub html_url: Option<String>,
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
    /// Human-readable core `TokenSource` label: `"GITHUB_TOKEN env"`,
    /// `"OAuth session"`, `"stored token"` or `"none"`.
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

#[derive(Debug, Clone, Deserialize)]
pub struct LoginRequest {
    /// GitHub login (username).
    pub login: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserReposRequest {
    pub login: String,
    /// `created`, `updated`, `pushed` or `full_name`. Defaults to `updated`.
    pub sort: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserEventsRequest {
    pub login: String,
    pub page: Option<u32>,
    pub per_page: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContributorsRequest {
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
pub struct IssueCommentCreateRequest {
    pub repo: String,
    pub number: u64,
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationReadRequest {
    /// Notification thread id, as returned in `NotificationDto.id`.
    pub id: String,
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
            html_url: Some("https://github.com/gitnapse/gitnapse".into()),
            forks_count: Some(3),
            open_issues_count: Some(1),
            watchers_count: Some(42),
            private: Some(false),
            topics: Some(vec!["rust".into(), "git".into()]),
            updated_at: Some("2026-02-01T00:00:00Z".into()),
            pushed_at: Some("2026-02-02T00:00:00Z".into()),
            owner_avatar_url: Some("https://avatars.example/gitnapse.png".into()),
        };
        let json = serde_json::to_value(&repo).unwrap();
        assert_eq!(json["full_name"], "gitnapse/gitnapse");
        assert_eq!(json["owner"], "gitnapse");
        assert!(json.get("description").unwrap().is_null());
        assert_eq!(json["forks_count"], 3);
        assert_eq!(json["private"], false);
        assert_eq!(json["topics"][0], "rust");
        assert_eq!(
            json["owner_avatar_url"],
            "https://avatars.example/gitnapse.png"
        );

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
            user: ActorDto {
                login: "x".into(),
                avatar_url: Some("https://avatars.example/x.png".into()),
            },
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
            author: Some(ActorDto {
                login: "x".into(),
                avatar_url: None,
            }),
        };
        let json = serde_json::to_value(&commit).unwrap();
        assert_eq!(json["author_name"], "x");
        assert_eq!(json["author"]["login"], "x");

        let rate = RateLimitDto {
            remaining: Some(42),
            reset: Some(1234),
        };
        let json = serde_json::to_value(&rate).unwrap();
        assert_eq!(json["remaining"], 42);
    }

    #[test]
    fn dashboard_dtos_roundtrip() {
        let comment = IssueCommentDto {
            id: 5,
            user: ActorDto {
                login: "x".into(),
                avatar_url: Some("https://avatars.example/x.png".into()),
            },
            body: "looks good".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            html_url: "https://github.com/a/b/issues/1#issuecomment-5".into(),
        };
        let json = serde_json::to_value(&comment).unwrap();
        assert_eq!(json["user"]["avatar_url"], "https://avatars.example/x.png");
        let back: IssueCommentDto = serde_json::from_value(json).unwrap();
        assert_eq!(back.id, 5);

        let profile = UserProfileDto {
            login: "octocat".into(),
            name: Some("The Octocat".into()),
            avatar_url: Some("https://avatars.example/octocat.png".into()),
            bio: None,
            company: None,
            location: Some("Earth".into()),
            blog: None,
            followers: 10,
            following: 2,
            public_repos: 8,
            html_url: "https://github.com/octocat".into(),
            created_at: Some("2011-01-25T18:44:36Z".into()),
        };
        let back: UserProfileDto =
            serde_json::from_value(serde_json::to_value(&profile).unwrap()).unwrap();
        assert_eq!(back.followers, 10);
        assert_eq!(back.name.as_deref(), Some("The Octocat"));

        let event = EventDto {
            id: "1".into(),
            kind: "push".into(),
            actor: "octocat".into(),
            actor_avatar_url: None,
            repo: "a/b".into(),
            action: None,
            title: Some("pushed 2 commits".into()),
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        let back: EventDto = serde_json::from_value(serde_json::to_value(&event).unwrap()).unwrap();
        assert_eq!(back.kind, "push");

        let notification = NotificationDto {
            id: "99".into(),
            unread: true,
            reason: "mention".into(),
            subject_type: "Issue".into(),
            subject_title: "bug".into(),
            repo: Some("a/b".into()),
            updated_at: "2026-01-01T00:00:00Z".into(),
            html_url: None,
        };
        let back: NotificationDto =
            serde_json::from_value(serde_json::to_value(&notification).unwrap()).unwrap();
        assert!(back.unread);

        let code = CodeSearchResultDto {
            repo: "a/b".into(),
            path: "src/main.rs".into(),
            name: "main.rs".into(),
            sha: "abc".into(),
            html_url: "https://github.com/a/b/blob/HEAD/src/main.rs".into(),
        };
        let back: CodeSearchResultDto =
            serde_json::from_value(serde_json::to_value(&code).unwrap()).unwrap();
        assert_eq!(back.path, "src/main.rs");

        let language = LanguageDto {
            name: "Rust".into(),
            bytes: 1234,
        };
        let back: LanguageDto =
            serde_json::from_value(serde_json::to_value(&language).unwrap()).unwrap();
        assert_eq!(back.bytes, 1234);

        let contributor = ContributorDto {
            login: "octocat".into(),
            avatar_url: None,
            contributions: 77,
            html_url: Some("https://github.com/octocat".into()),
        };
        let back: ContributorDto =
            serde_json::from_value(serde_json::to_value(&contributor).unwrap()).unwrap();
        assert_eq!(back.contributions, 77);
    }

    #[test]
    fn dashboard_requests_parse() {
        let login: LoginRequest =
            serde_json::from_value(serde_json::json!({ "login": "octocat" })).unwrap();
        assert_eq!(login.login, "octocat");

        let repos: UserReposRequest = serde_json::from_value(serde_json::json!({
            "login": "octocat", "sort": "pushed", "page": 2, "per_page": 10
        }))
        .unwrap();
        assert_eq!(repos.sort.as_deref(), Some("pushed"));
        assert_eq!(repos.page, Some(2));

        let bare: UserReposRequest =
            serde_json::from_value(serde_json::json!({ "login": "octocat" })).unwrap();
        assert!(bare.sort.is_none());
        assert!(bare.per_page.is_none());

        let events: UserEventsRequest = serde_json::from_value(serde_json::json!({
            "login": "octocat", "page": 1, "per_page": 5
        }))
        .unwrap();
        assert_eq!(events.per_page, Some(5));

        let contributors: ContributorsRequest =
            serde_json::from_value(serde_json::json!({ "repo": "a/b", "per_page": 50 })).unwrap();
        assert_eq!(contributors.per_page, Some(50));

        let comment: IssueCommentCreateRequest = serde_json::from_value(serde_json::json!({
            "repo": "a/b", "number": 7, "body": "hello"
        }))
        .unwrap();
        assert_eq!(comment.number, 7);

        let read: NotificationReadRequest =
            serde_json::from_value(serde_json::json!({ "id": "42" })).unwrap();
        assert_eq!(read.id, "42");
    }
}
