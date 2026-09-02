//! Mappings from the gitnapse domain models to the protocol DTOs.
//! (Free functions — `impl From` would violate the orphan rule.)

use gitnapse::models::{
    CheckRun, CommitInfo, CompareResponse, DiffFile, Issue, IssueLabel, IssueUser, MergeResponse,
    PullRequest, PullRequestDetail, PullRequestReview, Release, RepoNode, RepoSummary,
    ReviewComment, WorkflowRun,
};
use gitnapse_protocol::{
    ActorDto, CheckRunDto, CommitDto, CompareDto, DiffFileDto, IssueDto, LabelDto, MergeResultDto,
    PrBranchDto, PrCommentDto, PrDetailDto, PrReviewDto, PrSummaryDto, ReleaseDto, RepoDto,
    TreeNodeDto, WorkflowRunDto,
};

fn actor(u: &IssueUser) -> ActorDto {
    ActorDto {
        login: u.login.clone(),
    }
}

fn label(l: &IssueLabel) -> LabelDto {
    LabelDto {
        name: l.name.clone(),
        color: l.color.clone(),
    }
}

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

pub fn issue_dto(i: &Issue) -> IssueDto {
    IssueDto {
        number: i.number,
        title: i.title.clone(),
        state: i.state.clone(),
        html_url: i.html_url.clone(),
        user: actor(&i.user),
        labels: i.labels.iter().map(label).collect(),
        created_at: i.created_at.clone(),
        updated_at: i.updated_at.clone(),
        body: i.body.clone(),
        is_pr: i.pull_request.is_some(),
    }
}

fn pr_branch(b: &gitnapse::models::PrBranch) -> PrBranchDto {
    PrBranchDto {
        label: b.label.clone(),
        r#ref: b.r#ref.clone(),
        sha: b.sha.clone(),
    }
}

pub fn pr_summary_dto(p: &PullRequest) -> PrSummaryDto {
    PrSummaryDto {
        number: p.number,
        title: p.title.clone(),
        state: p.state.clone(),
        html_url: p.html_url.clone(),
        user: actor(&p.user),
        body: p.body.clone(),
        created_at: p.created_at.clone(),
        updated_at: p.updated_at.clone(),
        additions: p.additions,
        deletions: p.deletions,
        changed_files: p.changed_files,
    }
}

pub fn pr_detail_dto(p: &PullRequestDetail) -> PrDetailDto {
    PrDetailDto {
        number: p.number,
        title: p.title.clone(),
        state: p.state.clone(),
        body: p.body.clone(),
        html_url: p.html_url.clone(),
        user: actor(&p.user),
        created_at: p.created_at.clone(),
        updated_at: p.updated_at.clone(),
        merge_commit_sha: p.merge_commit_sha.clone(),
        merged: p.merged,
        merged_by: p.merged_by.as_ref().map(actor),
        additions: p.additions,
        deletions: p.deletions,
        changed_files: p.changed_files,
        commits: p.commits,
        comments: p.comments,
        review_comments: p.review_comments,
        head: pr_branch(&p.head),
        base: pr_branch(&p.base),
        labels: p.labels.iter().map(label).collect(),
    }
}

pub fn review_dto(r: &PullRequestReview) -> PrReviewDto {
    PrReviewDto {
        id: r.id,
        user: actor(&r.user),
        body: r.body.clone(),
        state: r.state.clone(),
        submitted_at: r.submitted_at.clone(),
        commit_id: r.commit_id.clone(),
    }
}

pub fn comment_dto(c: &ReviewComment) -> PrCommentDto {
    PrCommentDto {
        id: c.id,
        user: actor(&c.user),
        body: c.body.clone(),
        path: c.path.clone(),
        position: c.position,
        commit_id: c.commit_id.clone(),
        created_at: c.created_at.clone(),
        updated_at: c.updated_at.clone(),
    }
}

pub fn merge_dto(m: &MergeResponse) -> MergeResultDto {
    MergeResultDto {
        sha: m.sha.clone(),
        merged: m.merged,
        message: m.message.clone(),
    }
}

fn commit_dto(c: &CommitInfo) -> CommitDto {
    CommitDto {
        sha: c.sha.clone(),
        message: c.commit.message.clone(),
        author_name: c.commit.author.name.clone(),
        author_date: c.commit.author.date.clone(),
    }
}

pub fn commits_dto(list: &[CommitInfo]) -> Vec<CommitDto> {
    list.iter().map(commit_dto).collect()
}

fn diff_file_dto(f: &DiffFile) -> DiffFileDto {
    DiffFileDto {
        filename: f.filename.clone(),
        status: f.status.clone(),
        additions: f.additions,
        deletions: f.deletions,
        changes: f.changes,
        patch: f.patch.clone(),
    }
}

pub fn compare_dto(c: &CompareResponse) -> CompareDto {
    CompareDto {
        status: c.status.clone(),
        ahead_by: c.ahead_by,
        behind_by: c.behind_by,
        total_commits: c.total_commits,
        files: c.files.iter().map(diff_file_dto).collect(),
    }
}

pub fn check_run_dto(c: &CheckRun) -> CheckRunDto {
    CheckRunDto {
        name: c.name.clone(),
        status: c.status.clone(),
        conclusion: c.conclusion.clone(),
        html_url: c.html_url.clone(),
        started_at: c.started_at.clone(),
        completed_at: c.completed_at.clone(),
    }
}

pub fn workflow_run_dto(w: &WorkflowRun) -> WorkflowRunDto {
    WorkflowRunDto {
        name: w.name.clone(),
        status: w.status.clone(),
        conclusion: w.conclusion.clone(),
        html_url: w.html_url.clone(),
        created_at: w.created_at.clone(),
        updated_at: w.updated_at.clone(),
    }
}

pub fn release_dto(r: &Release) -> ReleaseDto {
    ReleaseDto {
        tag_name: r.tag_name.clone(),
        name: r.name.clone(),
        body: r.body.clone(),
        html_url: r.html_url.clone(),
        created_at: r.created_at.clone(),
        published_at: r.published_at.clone(),
        prerelease: r.prerelease,
    }
}
