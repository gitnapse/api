# GitNapse API — protocol repo

The communication protocol between the GitNapse core and **any** interface.
Modular by design: the contract (types) is separate from its HTTP
implementation, and the packaged app lives outside this repo.

## Repo layout

```
api/
  crates/
    gitnapse-protocol/   # protocol types + query contracts (the wire contract)
    gitnapse-server/     # reference HTTP (axum) implementation over the gitnapse SDK
    gitnapse-client/     # typed HTTP client for third-party apps (no core dependency)
  docs/PROTOCOL.md       # the protocol documentation (operations, errors, versioning)
  scripts/ci.sh          # local CI gate (fmt, clippy, tests, audit)
```

## Crates

- **gitnapse-protocol**: stable wire types (request + response DTOs, versioned
  `API_PREFIX`). Depends only on `serde`; language/transport agnostic.
- **gitnapse-server**: implements the protocol over the `gitnapse` SDK (built
  with the TUI disabled — no ratatui/crossterm in the dependency tree). Embeds
  a minimal web UI (served at `/`).
- **gitnapse-client**: reference typed client for third-party apps. Talks only
  to the protocol; never links the core.

## Running the server

```sh
cargo run -p gitnapse-server -- --host 127.0.0.1 --port 8787
```

Security posture (local-first):

- Only loopback `Host` headers are accepted (DNS-rebinding guard).
- Optional API token: `GITNAPSE_SERVER_TOKEN=secret cargo run -p gitnapse-server`
  (or `--api-token secret`) -> every `/api/*` request must send
  `Authorization: Bearer secret`.
- The GitHub token comes from `GITHUB_TOKEN` or `gitnapse auth set`; the server
  warns at startup when it will run anonymously.

Try it:

```sh
curl http://127.0.0.1:8787/health
curl "http://127.0.0.1:8787/api/v1/search?q=language:rust&per_page=5"
curl "http://127.0.0.1:8787/api/v1/repos/tree?repo=gitnapse/api"
curl "http://127.0.0.1:8787/api/v1/issues?repo=gitnapse/api&state=open"
curl -X POST http://127.0.0.1:8787/api/v1/issues \
  -H "content-type: application/json" \
  -d '{"repo":"gitnapse/api","title":"Hello from curl"}'
```

The catalog mirrors the full GitNapse SDK: identity (`/user`, starred,
rate-limit), content (search, repo detail, branches, tree, content), commits/
CI (commits, compare, checks, workflows), issues and pull requests
(list/create/close/merge/review/comment) and releases/repo creation.

## SDK dependency

During development the crates use `gitnapse = { path = "../gitnapse",
default-features = false }`. Once `gitnapse` is published to crates.io, switch
to a published/git dependency so the repo is self-contained.

See `docs/PROTOCOL.md` for the operation catalog and error semantics.
