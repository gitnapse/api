//! The GitNapse communication protocol.
//!
//! This crate defines the stable wire contract between the GitNapse core and
//! ANY interface (web UI, desktop GUI, CLI wrappers, automation, third-party
//! apps). It is deliberately independent of `gitnapse` itself so any client in
//! any language can implement it.
//!
//! The operations are documented in `docs/PROTOCOL.md`. The reference server
//! implementation lives in the `gitnapse-server` crate.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthDto {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
pub struct TreeNodeDto {
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
}

#[derive(Debug, Serialize)]
pub struct ContentDto {
    pub path: String,
    /// File content base64-encoded (binary-safe).
    pub content: String,
    pub size: usize,
}

#[derive(Debug, Serialize)]
pub struct ErrorDto {
    pub error: String,
}
