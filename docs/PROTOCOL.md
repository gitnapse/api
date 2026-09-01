# GitNapse Communication Protocol

The **protocol** defines how any interface (web UI, desktop GUI, CLI wrappers,
automation, third-party apps) communicates with the GitNapse core. It is a
stable, transport-oriented contract, independent of the `gitnapse` internals.

- **Types** (request/response DTOs): crate `gitnapse-protocol` (no gitnapse
  dependency, any language can reimplement it).
- **Reference server**: crate `gitnapse-server` (HTTP/JSON over axum) which
  implements the operations on top of the `gitnapse` SDK.
- **In-process use**: a desktop GUI (e.g. a Tauri app in `gitnapseapp`) can
  reuse the same protocol types and call the SDK directly, or talk to the
  server over HTTP for remote/embedded scenarios.

## Versioning

The protocol is versioned in the URL path: `/api/v1/...`. Breaking changes bump
the version (`/api/v2/...`). `GET /health` is infrastructure, not versioned.

## Transport

- HTTP/JSON. Server binds `127.0.0.1:8787` by default.
- The token is resolved server-side from the environment (`GITHUB_TOKEN`) or the
  secure store, exactly like the app. Future versions may accept a per-request
  `Authorization: Bearer <token>`.

## Errors

Errors use a consistent JSON body with `500 Internal Server Error`:

```json
{ "error": "search failed: ..." }
```

## Operations (v1)

### `GET /health`

Infrastructure status.

```json
{ "status": "ok", "version": "0.1.0" }
```

### `GET /api/v1/search?q=&page=&per_page=`

Search repositories. `page` defaults to 1, `per_page` defaults to 30 (max 100).

```json
[
  {
    "full_name": "gitnapse/gitnapse",
    "name": "gitnapse",
    "owner": "gitnapse",
    "description": "...",
    "stargazers_count": 42,
    "language": "Rust",
    "default_branch": "main",
    "clone_url": "https://github.com/gitnapse/gitnapse.git"
  }
]
```

### `GET /api/v1/repos/branches?repo=owner/name`

```json
["main", "dev", "feature/x"]
```

### `GET /api/v1/repos/tree?repo=&ref=`

`ref` defaults to `HEAD`. Full tree, pre-order.

```json
[
  { "path": "src/main.rs", "name": "main.rs", "depth": 1, "is_dir": false }
]
```

### `GET /api/v1/repos/content?repo=&path=&ref=`

File content base64-encoded (binary-safe).

```json
{ "path": "README.md", "content": "IyBHaXROYXBzZQo=", "size": 12 }
```

## Extending the protocol

1. Add the request/response types to `gitnapse-protocol` (keep it dependency-free).
2. Add the route to `gitnapse-server` and map it to the SDK.
3. Document the operation in this file.

## Repo layout

```
api/
  crates/gitnapse-protocol/   # protocol types (the contract)
  crates/gitnapse-server/     # reference HTTP implementation over the SDK
  docs/PROTOCOL.md            # this document
```

The packaged application (Tauri desktop GUI: TS/CSS frontend + Rust backend)
lives in its own repo (`gitnapseapp`) and consumes this protocol and/or the SDK.
