//! Domain service used by the HTTP routes. Consumes `gitnapse` as a library,
//! reusing the same provider/domain layer as the TUI and CLI, and exposes the
//! results through the protocol types.

use std::sync::Arc;

use gitnapse::models::{RepoNode, RepoSummary};
use gitnapse::provider::{GitProvider, ProviderKind, create_provider};

pub struct ApiService {
    pub github: Arc<dyn GitProvider>,
}

impl ApiService {
    /// Build the service from the environment (token via env or secure store).
    pub fn from_env() -> anyhow::Result<Self> {
        gitnapse::runtime::ensure_crypto_provider();
        let token = gitnapse::auth::load_token()?;
        let github = create_provider(ProviderKind::GitHub, token.as_deref())?;
        Ok(Self { github })
    }

    pub fn search_repos(
        &self,
        query: &str,
        page: u32,
        per_page: u8,
    ) -> anyhow::Result<Vec<RepoSummary>> {
        self.github.search_repositories_page(query, page, per_page)
    }

    pub fn branches(&self, full_name: &str) -> anyhow::Result<Vec<String>> {
        self.github.fetch_branches(full_name)
    }

    pub fn tree(&self, full_name: &str, git_ref: &str) -> anyhow::Result<Vec<RepoNode>> {
        self.github.fetch_repo_tree(full_name, git_ref)
    }

    pub fn file_content(
        &self,
        full_name: &str,
        path: &str,
        git_ref: &str,
    ) -> anyhow::Result<Vec<u8>> {
        self.github
            .fetch_file_content_by_ref(full_name, path, git_ref)
    }
}
