# GitNapse API — protocol repo

The communication protocol between the GitNapse core and **any** interface.
Modular by design: the contract (types) is separate from its HTTP implementation,
and the packaged app lives outside this repo.

## Repo layout

```
api/
  crates/
    gitnapse-protocol/   # protocol types (the contract) — no gitnapse dependency
    gitnapse-server/     # reference HTTP (axum) implementation over the gitnapse SDK
  docs/PROTOCOL.md       # the protocol documentation (operations, errors, versioning)
```

## Crates

- **gitnapse-protocol**: stable wire types (RepoDto, TreeNodeDto, ContentDto,
  HealthDto, ErrorDto). Language/transport agnostic.
- **gitnapse-server**: implements the protocol over the `gitnapse` SDK. Embeds a
  minimal web UI (served at `/`). Run with:

```sh
cargo run -p gitnapse-server -- --host 127.0.0.1 --port 8787
```

See `docs/PROTOCOL.md` for the operation catalog.

## The packaged app (`gitnapseapp`)

The desktop GUI (Tauri: TS/CSS frontend + Rust backend) is a **separate repo**,
kept out of `api` for modularity. It consumes this protocol and/or the `gitnapse`
SDK directly (in-process), so it works offline without the server.

## SDK dependency

During development both crates use `gitnapse = { path = "../gitnapse" }`. Once
`gitnapse` is published to crates.io, switch to a published/git dependency so the
repo is self-contained.
