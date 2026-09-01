//! Mappings from the gitnapse domain models to the protocol DTOs.
//! (Free functions — `impl From` would violate the orphan rule.)

use gitnapse::models::{RepoNode, RepoSummary};
use gitnapse_protocol::{RepoDto, TreeNodeDto};

pub fn repo_dto(r: &RepoSummary) -> RepoDto {
    RepoDto {
        full_name: r.full_name.clone(),
        name: r.name.clone(),
        owner: r.owner.login.clone(),
        description: r.description.clone(),
        stargazers_count: r.stargazers_count,
        language: r.language.clone(),
        default_branch: r.default_branch.clone(),
        clone_url: r.clone_url.clone(),
    }
}

pub fn node_dto(n: &RepoNode) -> TreeNodeDto {
    TreeNodeDto {
        path: n.path.clone(),
        name: n.name.clone(),
        depth: n.depth,
        is_dir: n.is_dir,
    }
}
