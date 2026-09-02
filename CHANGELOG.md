# Changelog

## Unreleased

### Added

- **Full SDK surface in the protocol**: the route catalog now mirrors every
  `gitnapse` provider capability — identity (`/user`, `/user/starred`,
  `/rate-limit`), content (`/repos/detail`), commits/CI (`/commits`,
  `/compare`, `/checks`, `/workflows`), issues (list/create/close), pull
  requests (list/detail/reviews/comments/commits/create/merge/update/review/
  comment), releases (list/create) and repo creation. Reads are `GET`;
  mutations are `POST` with JSON bodies (201/204 responses). (`gitnapse-protocol`, `gitnapse-server`)
- **Request types in the protocol contract**: every query string and JSON body
  maps to a struct in `gitnapse-protocol` so the wire format cannot drift from
  the server routes. Response DTOs are `Deserialize` too. (`crates/gitnapse-protocol`)
- **`gitnapse-client` crate**: typed HTTP client with a method per operation
  (identity, content, commits/CI, issues, PRs, releases, repo creation) and
  optional bearer token. No dependency on the gitnapse core. (`crates/gitnapse-client`)
- **HTTP tests with an in-memory backend**: routes only depend on a small
  `Backend` trait, so the suite runs without keyring/OAuth/network and covers
  the full surface, error mapping (404/401), the API-token guard,
  DNS-rebinding guard, JSON 404 fallback and 400 validation. (`crates/gitnapse-server/src/routes.rs`)
- **Token lifecycle endpoints**: `GET /api/v1/auth/status`
  (`{ has_token, source }`, never the token), `POST /api/v1/auth/token`
  (validates against GitHub, stores it, swaps the provider at runtime, no
  restart) and `DELETE /api/v1/auth/token` (back to anonymous). When the token
  is managed by `GITHUB_TOKEN`, mutations return `409`. Backend methods and
  `gitnapse-client` helpers (`auth_status`, `set_token`, `clear_token`)
  included. The core exposes a typed `TokenSource`/`token_source()` for this.
  (`crates/gitnapse-server`, `crates/gitnapse-client`, `../gitnapse`)

### Changed

- **Semantic errors**: JSON error bodies now use meaningful status codes
  (400/401/403/404/413/429/502/504) instead of always 500, and never leak
  internal error details to clients. Full causes are logged server-side.
  (`crates/gitnapse-server/src/service.rs`)
- **Security posture**: requests whose `Host` header is not loopback are
  rejected (DNS-rebinding protection); an optional `GITNAPSE_SERVER_TOKEN`
  enables `Authorization: Bearer` on `/api/*` routes; overall request timeout
  added. (`crates/gitnapse-server/src/routes.rs`)
- **Boundaries and limits**: `/content` and `/tree` enforce size/node caps;
  `ApiService::warn_if_anonymous` warns at startup when no token is found.
- **Server UX**: `info`-level default logging, `log::info!` startup banner with
  a warning when binding a non-loopback interface, and graceful shutdown on
  Ctrl+C/SIGTERM. (`crates/gitnapse-server/src/main.rs`)
- **SDK build cost**: `gitnapse` is consumed with `default-features = false`
  (no ratatui/crossterm/TUI modules), cutting the dependency tree and build
  time of the server drastically.
- **Web UI**: results are rendered with `textContent` node construction
  instead of `innerHTML`, removing a DOM-XSS vector from repo metadata
  returned by the public GitHub API.

## Init

- `gitnapse-protocol` + `gitnapse-server` workspace extracted from the
  gitnapse monorepo: stable wire DTOs and a reference axum HTTP server over
  the `gitnapse` SDK.
