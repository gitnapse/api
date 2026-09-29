# GitNapse Communication Protocol

The **protocol** defines how any interface (web UI, desktop GUI, CLI wrappers,
automation, third-party apps) communicates with the GitNapse core. It is a
stable, transport-oriented contract, independent of the `gitnapse` internals.

- **Types** (request/response DTOs): crate `gitnapse-protocol` (only depends on
  `serde`; any language can reimplement it). Request structs are part of the
  contract too — the URL query parameters and JSON bodies deserialize directly
  into them.
- **Reference server**: crate `gitnapse-server` (HTTP/JSON over axum) which
  implements the operations on top of the `gitnapse` SDK (TUI disabled). The
  route surface mirrors the full SDK: no provider capability is unreachable.
- **Typed client**: crate `gitnapse-client` — the reference implementation for
  third-party apps; it only depends on `gitnapse-protocol`.
- **In-process use**: a desktop GUI (e.g. a Tauri app in `gitnapseapp`) can
  reuse the same protocol types and call the SDK directly, or talk to the
  server over HTTP for remote/embedded scenarios.

## Versioning

The protocol is versioned in the URL path: `/api/v1/...`. Breaking changes bump
the version (`/api/v2/...`). The current prefix is exported by the protocol
crate as `gitnapse_protocol::API_PREFIX`. `GET /health` is infrastructure, not
versioned.

**Additive changes do not bump the version**: new operations, new optional DTO
fields and new enum-ish values (e.g. a new `state` accepted by an existing
parameter) keep serving under `/api/v1`. Clients must ignore unknown fields and
treat new response fields as optional; producers must only add fields, never
rename or remove them within a version.

## Transport

- HTTP/JSON. Server binds `127.0.0.1:8787` by default and **rejects requests
  whose `Host` is not a loopback address** (`127.0.0.1`, `localhost`, `::1`),
  which also neutralizes DNS-rebinding attacks.
- Reads are `GET`; mutations are `POST` with a JSON body. The server sends no
  CORS headers, so cross-origin browsers cannot reach the API (preflight
  fails); mutations are therefore only callable by same-origin pages or
  non-browser clients.
- The GitHub token is resolved server-side from the environment
  (`GITHUB_TOKEN`) or the secure store, exactly like the app. Requests run
  anonymously when no token is configured (the server warns at startup).
- Optional API token: start the server with `--api-token <secret>` (or env
  `GITNAPSE_SERVER_TOKEN`) to require `Authorization: Bearer <secret>` on every
  `/api/*` request. Clients opt in with `Client::with_api_token("…")`.
  `GET /` and `GET /health` stay open.
- The GitHub token can also be managed at runtime through the
  [auth endpoints](#auth-management): `gitnapseapp` (or any interface) can
  store/clear the token without a server restart. New tokens are validated
  against GitHub before being persisted.

## Errors

Errors always use JSON `{ "error": "safe client message" }` with a *semantic*
HTTP status code. The server logs the full underlying cause; client messages
never leak internals:

| Status | Meaning |
|---|---|
| `400` | Missing/invalid query parameter or JSON body (e.g. `repo` not `owner/name`) |
| `401` | Missing/invalid API token, GitHub auth required, or `/user` with no identity |
| `403` | Request not from a loopback host, or GitHub rejected the request |
| `404` | Endpoint, repository, branch, file, issue or PR not found |
| `409`/`422` | Upstream rejection surfaced with GitHub's own code (e.g. merge conflict) |
| `409` | Token cannot be changed because it is managed by the `GITHUB_TOKEN` env |
| `413` | Response above server limits (file/tree too large) |
| `429` | GitHub rate limit exceeded |
| `502` | Upstream (GitHub) unreachable |
| `504` | Request exceeded the server timeout |
| `500` | Internal error (generic, no details exposed) |

Example:

```json
{ "error": "repository, branch or file not found" }
```

## Operations (v1)

### Infrastructure (open, not bearer-protected)

`GET /health` -> `{ "status": "ok", "version": "0.1.0" }`

### Identity / discovery

| Endpoint | Description |
|---|---|
| `GET /api/v1/user` | Authenticated login (`{ "login": "x" }`; `401` when anonymous) |
| `GET /api/v1/user/starred?page=&per_page=` | Starred repos of the user |
| `GET /api/v1/rate-limit` | `{ "remaining", "reset" }` from the last responses (nullable) |

### Users / profile

| Endpoint | Description |
|---|---|
| `GET /api/v1/users/profile?login=` | Public profile (`UserProfileDto`) |
| `GET /api/v1/users/repos?login=&sort=&page=&per_page=` | Public repos; `sort` is `created`\|`updated`\|`pushed`\|`full_name` (default `updated`) |
| `GET /api/v1/users/events?login=&page=&per_page=` | Public activity feed (`EventDto[]`, newest first) |
| `GET /api/v1/users/notifications?page=&per_page=` | Authenticated user's notification inbox (`NotificationDto[]`; `401` when anonymous) |
| `POST /api/v1/users/notifications/read` | Body `{ "id" }` (thread id from `NotificationDto`) -> `204` |

### Auth management

| Endpoint | Description |
|---|---|
| `GET /api/v1/auth/status` | `{ "has_token": bool, "source": "GITHUB_TOKEN env"\|"OAuth session"\|"stored token"\|"none" }` (never the token itself; `source` is the core `TokenSource` label) |
| `POST /api/v1/auth/token` | Body `{ "token" }` → validates against GitHub (`401` if rejected), stores it in the secure store and switches the server to it → `204` |
| `DELETE /api/v1/auth/token` | Forgets the stored token and switches to anonymous → `204` |

Notes: when the token comes from the `GITHUB_TOKEN` environment variable,
`POST`/`DELETE` return `409` (unset the env var and restart first). These
endpoints are guarded exactly like the rest of the API (loopback host,
optional bearer) and are reachable by non-browser clients only.

### Search

| Endpoint | Description |
|---|---|
| `GET /api/v1/search?q=&page=&per_page=` | Search repositories (max `per_page` 100) |
| `GET /api/v1/search/users?q=&page=&per_page=` | Search users (`UserProfileDto[]`; reduced objects, counts may be `0`) |
| `GET /api/v1/search/code?q=&page=&per_page=` | Search code (`CodeSearchResultDto[]`; `q` is required by GitHub) |

### Content

| Endpoint | Description |
|---|---|
| `GET /api/v1/repos/detail?repo=owner/name` | Repository metadata (counts, topics, timestamps, owner avatar) |
| `GET /api/v1/repos/branches?repo=` | Branch names |
| `GET /api/v1/repos/tree?repo=&ref=` | Full tree (pre-order; `ref` -> `HEAD`; `413` > 200k nodes) |
| `GET /api/v1/repos/content?repo=&path=&ref=` | File content, base64 (`413` > 16 MiB) |
| `GET /api/v1/repos/languages?repo=` | Bytes per language (`LanguageDto[]`, sorted by bytes desc) |
| `GET /api/v1/repos/contributors?repo=&per_page=` | Contributors (`ContributorDto[]`) |

### Commits / CI

| Endpoint | Description |
|---|---|
| `GET /api/v1/commits?repo=&ref=&per_page=` | Recent commits (`ref` -> `HEAD`) |
| `GET /api/v1/compare?repo=&base=&head=` | Ahead/behind + changed files (+ patch) |
| `GET /api/v1/checks?repo=&ref=` | Check runs for a ref |
| `GET /api/v1/workflows?repo=&branch=&per_page=` | Workflow runs (`branch` -> `main`) |

### Issues

| Endpoint | Description |
|---|---|
| `GET /api/v1/issues?repo=&state=&per_page=` | List (`state` open\|closed\|all -> open) |
| `GET /api/v1/issues/detail?repo=&number=` | Full detail of one issue (`IssueDto`) |
| `GET /api/v1/issues/comments?repo=&number=` | Conversation comments (`IssueCommentDto[]`) |
| `POST /api/v1/issues` | `{ "repo", "title", "body"? }` -> `201` issue |
| `POST /api/v1/issues/comment` | `{ "repo", "number", "body" }` -> `201` comment |
| `POST /api/v1/issues/close` | `{ "repo", "number" }` -> closed issue |
| `POST /api/v1/issues/reopen` | `{ "repo", "number" }` -> reopened issue |

### Pull requests

| Endpoint | Description |
|---|---|
| `GET /api/v1/pulls?repo=&state=&per_page=` | List (`state` -> open) |
| `GET /api/v1/pulls/detail?repo=&number=` | Full detail (branches, merge info, counts) |
| `GET /api/v1/pulls/files?repo=&number=` | Changed files with patches (`DiffFileDto[]`, first page ≤ 100) |
| `GET /api/v1/pulls/conversation?repo=&number=` | Conversation comments (`IssueCommentDto[]`) |
| `GET /api/v1/pulls/reviews?repo=&number=` | Reviews |
| `GET /api/v1/pulls/comments?repo=&number=` | Inline review comments |
| `GET /api/v1/pulls/commits?repo=&number=` | Commits |
| `POST /api/v1/pulls` | `{ "repo", "title", "head", "base", "body"? }` -> `201` detail |
| `POST /api/v1/pulls/merge` | `{ "repo", "number", "commit_title"?, "method"? }` (merge\|squash\|rebase -> merge) -> result |
| `POST /api/v1/pulls/update` | `{ "repo", "number", "state" }` (open\|closed) -> `204` |
| `POST /api/v1/pulls/reviews` | `{ "repo", "number", "event", "body"? }` (approve\|request_changes\|comment) -> `204` |
| `POST /api/v1/pulls/comments` | `{ "repo", "number", "body" }` -> `204` |

### Releases / repos

| Endpoint | Description |
|---|---|
| `GET /api/v1/releases?repo=&per_page=` | Releases |
| `POST /api/v1/releases` | `{ "repo", "tag_name", "name"?, "body"?, "prerelease"? }` -> `201` release |
| `POST /api/v1/repos` | `{ "name", "description"?, "private"? }` -> `201` repo |

## Client reference

```rust,no_run
use gitnapse_client::Client;

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let client = Client::new("http://127.0.0.1:8787")?
    .with_api_token("server-secret");

let me = client.user().await?;                                  // GET /user
let repos = client.search("language:rust", Some(1), Some(20)).await?;
let branches = client.branches("gitnapse/gitnapse").await?;
let issues = client.issues("gitnapse/gitnapse", Some("open"), None).await?;
let issue = client.issue_detail("gitnapse/gitnapse", 7).await?;
let comments = client.issue_comments("gitnapse/gitnapse", 7).await?;
let profile = client.user_profile("xscriptor").await?;
let inbox = client.notifications(Some(1), Some(20)).await?;
let hits = client.search_code("repo:gitnapse/api fn main", None, None).await?;
let langs = client.languages("gitnapse/gitnapse").await?;
let prs = client.pull_requests("gitnapse/gitnapse", None, None).await?;
let files = client.pr_files("gitnapse/gitnapse", 7).await?;
let pr = client.create_pull_request("a/b", "t", "feat", "main", None).await?;
client.merge_pull_request("a/b", pr.number, None, Some("squash")).await?;
let release = client.create_release("a/b", "v1.0", None, None, false).await?;
# Ok(())
# }
```

## Extending the protocol

1. Add the request/response types to `gitnapse-protocol` (keep it
   `serde`-only), with serde round-trip tests.
2. Add the route to `gitnapse-server` using the protocol request types and the
   `Backend` trait; extend `gitnapse-client` and its docs.
3. Document the operation and its error semantics in this file.

## Limits and guarantees

- Content: max 16 MiB per file; tree: max 200 000 nodes (both `413`).
- Per-request timeout: 90 s (`504` on expiry).
- Server-side state mutations only via GitHub's API through the authenticated
  identity; nothing is stored or written by the server itself.
