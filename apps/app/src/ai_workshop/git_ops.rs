// === AI-WORKSHOP START ===
// Git 版本控制原子工具（流 C.4）：init / status / log / commit / checkout / branch / diff。
// 仓库根 = 实例根目录（{root}/.git），git2 同步 API，工具 execute 内直接调用。
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use git2::build::CheckoutBuilder;
use git2::{
    BranchType, IndexAddOption, Patch, Repository, Signature, Status,
    StatusOptions,
};
use serde_json::{Value, json};

use crate::ai_workshop::tools::context::ExecutionContext;
use crate::ai_workshop::tools::registry::{Tool, ToolDomain, ToolInfo};

/// 从 arguments 中读取字符串参数；缺失或类型不符返回错误。
fn string_arg(arguments: &Value, key: &str) -> Result<String, String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("缺少参数: {key}"))
}

/// 打开实例根的 git 仓库；未初始化返回错误。
fn open_repo(root: &Path) -> Result<Repository, String> {
    Repository::open(root)
        .map_err(|e| format!("无法打开 git 仓库 {}: {e}", root.display()))
}

/// 判断实例根是否已是 git 仓库（存在 .git 目录）。
fn repo_is_initialized(root: &Path) -> bool {
    root.join(".git").exists()
}

/// 将 git2 status 位映射为统一的状态字符串。
fn status_label(status: Status) -> &'static str {
    if status.contains(Status::CONFLICTED) {
        return "conflicted";
    }
    if status.contains(Status::WT_NEW) && !status.contains(Status::INDEX_NEW) {
        return "untracked";
    }
    if status.contains(Status::INDEX_NEW) {
        return "new";
    }
    if status.contains(Status::INDEX_DELETED)
        || status.contains(Status::WT_DELETED)
    {
        return "deleted";
    }
    if status.contains(Status::INDEX_RENAMED)
        || status.contains(Status::WT_RENAMED)
    {
        return "renamed";
    }
    "modified"
}

/// 确保仓库有 user.name / user.email；缺失时写入仓库级默认值并返回签名。
fn ensure_signature(repo: &Repository) -> Result<Signature<'_>, String> {
    let mut config = repo.config().map_err(|e| e.to_string())?;
    let name = config
        .get_string("user.name")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let _ = config.set_str("user.name", "AI Workshop");
            "AI Workshop".to_string()
        });
    let email = config
        .get_string("user.email")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let _ = config.set_str("user.email", "ai@local");
            "ai@local".to_string()
        });
    Signature::now(&name, &email).map_err(|e| e.to_string())
}

/// git_init 核心：仓库不存在时 init，已存在返回提示。返回 { initialized, exists }。
fn git_init_impl(root: &Path) -> Result<Value, String> {
    if repo_is_initialized(root) {
        return Ok(json!({ "initialized": false, "exists": true }));
    }
    Repository::init(root).map_err(|e| format!("git init 失败: {e}"))?;
    Ok(json!({ "initialized": true, "exists": false }))
}

/// git_status 核心：返回文件变更列表 [{ path, status }]。
fn git_status_impl(root: &Path) -> Result<Value, String> {
    let repo = open_repo(root)?;
    let statuses = repo
        .statuses(Some(
            StatusOptions::new()
                .include_untracked(true)
                .include_ignored(false),
        ))
        .map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for entry in statuses.iter() {
        let status = entry.status();
        if status.is_empty() || status.contains(Status::IGNORED) {
            continue;
        }
        let path = entry.path().unwrap_or("").to_string();
        files.push(json!({ "path": path, "status": status_label(status) }));
    }
    files.sort_by(|a, b| {
        a["path"]
            .as_str()
            .unwrap_or("")
            .cmp(b["path"].as_str().unwrap_or(""))
    });
    Ok(json!({ "files": files, "count": files.len() }))
}

/// git_log 核心：返回 [{ hash(short), message, author, timestamp }]，新提交在前。
fn git_log_impl(root: &Path, limit: usize) -> Result<Value, String> {
    let repo = open_repo(root)?;
    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;
    revwalk
        .push_head()
        .map_err(|e| format!("仓库没有 HEAD: {e}"))?;
    let mut commits = Vec::new();
    for oid in revwalk {
        let oid = oid.map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
        let full = oid.to_string();
        let short = full[..full.len().min(7)].to_string();
        commits.push(json!({
            "hash": short,
            "message": commit.message().unwrap_or("").trim_end().to_string(),
            "author": commit.author().name().unwrap_or("").to_string(),
            "timestamp": commit.time().seconds(),
        }));
        if commits.len() >= limit {
            break;
        }
    }
    Ok(json!({ "commits": commits, "count": commits.len() }))
}

/// git_commit 核心：全部暂存 + commit；无变更返回 Err("无变更可提交")。
fn git_commit_impl(root: &Path, message: &str) -> Result<Value, String> {
    let repo = open_repo(root)?;
    let statuses = repo
        .statuses(Some(StatusOptions::new().include_untracked(true)))
        .map_err(|e| e.to_string())?;
    let has_changes = statuses.iter().any(|e| {
        let s = e.status();
        !s.is_empty() && !s.contains(Status::IGNORED)
    });
    if !has_changes {
        return Err("无变更可提交".to_string());
    }

    let mut index = repo.index().map_err(|e| e.to_string())?;
    index
        .add_all(["*"], IndexAddOption::DEFAULT, None)
        .map_err(|e| e.to_string())?;
    index.write().map_err(|e| e.to_string())?;
    let tree_id = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;

    let sig = ensure_signature(&repo)?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let oid = match parent {
        Some(p) => repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&p])
            .map_err(|e| e.to_string())?,
        None => repo
            .commit(None, &sig, &sig, message, &tree, &[])
            .map_err(|e| e.to_string())?,
    };
    let full = oid.to_string();
    let short = full[..full.len().min(7)].to_string();
    Ok(json!({ "hash": short, "message": message }))
}

/// git_checkout 核心：接受分支名或 commit（revparse）。分支走普通 checkout，其余 detached。
fn git_checkout_impl(root: &Path, target: &str) -> Result<Value, String> {
    let repo = open_repo(root)?;
    if repo.find_branch(target, BranchType::Local).is_ok() {
        repo.set_head(&format!("refs/heads/{target}"))
            .map_err(|e| e.to_string())?;
        repo.checkout_head(Some(&mut CheckoutBuilder::new().force()))
            .map_err(|e| e.to_string())?;
        return Ok(json!({ "checked_out": target, "detached": false }));
    }
    let obj = repo
        .revparse_single(target)
        .map_err(|_| format!("找不到 commit 或分支: {target}"))?;
    repo.checkout_tree(&obj, Some(&mut CheckoutBuilder::new().force()))
        .map_err(|e| e.to_string())?;
    repo.set_head_detached(obj.id())
        .map_err(|e| e.to_string())?;
    Ok(json!({ "checked_out": target, "detached": true }))
}

/// git_branch 核心：action ∈ list | create | delete | checkout。
fn git_branch_impl(
    root: &Path,
    action: &str,
    branch_name: Option<&str>,
) -> Result<Value, String> {
    let repo = open_repo(root)?;
    match action {
        "list" => {
            let branches = repo
                .branches(Some(BranchType::Local))
                .map_err(|e| e.to_string())?;
            let mut names = Vec::new();
            for branch in branches {
                let (branch, _) = branch.map_err(|e| e.to_string())?;
                let name =
                    branch.name().ok().flatten().unwrap_or("").to_string();
                names
                    .push(json!({ "name": name, "current": branch.is_head() }));
            }
            Ok(json!({ "branches": names, "count": names.len() }))
        }
        "create" => {
            let name = branch_name.ok_or("缺少参数: branch_name")?;
            let head = repo.head().map_err(|e| e.to_string())?;
            let commit = head.peel_to_commit().map_err(|e| e.to_string())?;
            repo.branch(name, &commit, false)
                .map_err(|e| e.to_string())?;
            Ok(json!({ "created": name }))
        }
        "delete" => {
            let name = branch_name.ok_or("缺少参数: branch_name")?;
            let mut branch = repo
                .find_branch(name, BranchType::Local)
                .map_err(|_| format!("分支不存在: {name}"))?;
            branch.delete().map_err(|e| e.to_string())?;
            Ok(json!({ "deleted": name }))
        }
        "checkout" => {
            let name = branch_name.ok_or("缺少参数: branch_name")?;
            repo.set_head(&format!("refs/heads/{name}"))
                .map_err(|e| e.to_string())?;
            repo.checkout_head(Some(&mut CheckoutBuilder::new().force()))
                .map_err(|e| e.to_string())?;
            Ok(json!({ "checked_out": name }))
        }
        other => Err(format!("未知分支操作: {other}")),
    }
}

/// git_diff 核心：commit 缺省 = 工作区 vs HEAD；返回 [{ path, additions, deletions }]。
fn git_diff_impl(root: &Path, commit: Option<&str>) -> Result<Value, String> {
    let repo = open_repo(root)?;
    let base_tree = match commit {
        Some(c) => {
            let obj = repo
                .revparse_single(c)
                .map_err(|_| format!("找不到 commit: {c}"))?;
            Some(obj.peel_to_tree().map_err(|e| e.to_string())?)
        }
        None => repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok())
            .map(|c| c.tree().map_err(|e| e.to_string()))
            .transpose()?,
    };
    // 无基准（空仓库且未指定 commit）时没有可比对的差异。
    let Some(base_tree) = base_tree else {
        return Ok(json!({ "files": [], "count": 0 }));
    };
    let diff = repo
        .diff_tree_to_workdir_with_index(Some(&base_tree), None)
        .map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for (i, delta) in diff.deltas().enumerate() {
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut additions = 0usize;
        let mut deletions = 0usize;
        if let Some(patch) =
            Patch::from_diff(&diff, i).map_err(|e| e.to_string())?
        {
            for h in 0..patch.num_hunks() {
                let (_, lines) = patch.hunk(h).map_err(|e| e.to_string())?;
                for l in 0..lines {
                    let line =
                        patch.line_in_hunk(h, l).map_err(|e| e.to_string())?;
                    match line.origin() {
                        '+' => additions += 1,
                        '-' => deletions += 1,
                        _ => {}
                    }
                }
            }
        }
        files.push(json!({
            "path": path,
            "additions": additions,
            "deletions": deletions,
        }));
    }
    files.sort_by(|a, b| {
        a["path"]
            .as_str()
            .unwrap_or("")
            .cmp(b["path"].as_str().unwrap_or(""))
    });
    Ok(json!({ "files": files, "count": files.len() }))
}

/// git_init 工具。参数：instance_id 必填。仓库不存在时 init，已存在返回提示。
pub struct GitInitTool;

#[async_trait]
impl Tool for GitInitTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "git_init".to_string(),
            description: "在实例根目录初始化 git 仓库（已存在则提示）。"
                .to_string(),
            domain: ToolDomain::Git,
            requires_confirmation: false,
            is_readonly: false,
            params_schema: json!({
                "type": "object",
                "properties": {
                    "instance_id": { "type": "string", "description": "目标实例 ID" }
                },
                "required": ["instance_id"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        ctx: &ExecutionContext,
    ) -> Result<Value, String> {
        let instance_id = string_arg(&arguments, "instance_id")?;
        // git_init 是写操作（初始化 .git），与其余写工具一致获取实例写锁。
        let _lock = ctx
            .instance_lock_manager
            .acquire_write_lock(
                &instance_id,
                std::time::Duration::from_secs(30),
            )
            .await?;
        let root = theseus::instance::get_full_path(&instance_id)
            .await
            .map_err(|e| e.to_string())?;
        git_init_impl(&root)
    }
}

/// git_status（readonly）。参数：instance_id。返回文件变更列表。
pub struct GitStatusTool;

#[async_trait]
impl Tool for GitStatusTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "git_status".to_string(),
            description: "查看实例 git 仓库当前的文件变更状态。".to_string(),
            domain: ToolDomain::Git,
            requires_confirmation: false,
            is_readonly: true,
            params_schema: json!({
                "type": "object",
                "properties": {
                    "instance_id": { "type": "string", "description": "目标实例 ID" }
                },
                "required": ["instance_id"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        _ctx: &ExecutionContext,
    ) -> Result<Value, String> {
        let instance_id = string_arg(&arguments, "instance_id")?;
        let root = theseus::instance::get_full_path(&instance_id)
            .await
            .map_err(|e| e.to_string())?;
        git_status_impl(&root)
    }
}

/// git_log（readonly）。参数：instance_id、limit 可选(默认20)。
pub struct GitLogTool;

#[async_trait]
impl Tool for GitLogTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "git_log".to_string(),
            description: "查看实例 git 仓库的提交历史。".to_string(),
            domain: ToolDomain::Git,
            requires_confirmation: false,
            is_readonly: true,
            params_schema: json!({
                "type": "object",
                "properties": {
                    "instance_id": { "type": "string", "description": "目标实例 ID" },
                    "limit": { "type": "integer", "default": 20, "description": "返回条数，默认 20" }
                },
                "required": ["instance_id"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        _ctx: &ExecutionContext,
    ) -> Result<Value, String> {
        let instance_id = string_arg(&arguments, "instance_id")?;
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .min(200) as usize;
        let root = theseus::instance::get_full_path(&instance_id)
            .await
            .map_err(|e| e.to_string())?;
        git_log_impl(&root, limit)
    }
}

/// git_commit（confirm）。参数：instance_id、message 必填。全部暂存并提交。
pub struct GitCommitTool;

#[async_trait]
impl Tool for GitCommitTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "git_commit".to_string(),
            description: "暂存实例内全部变更并创建一次 git 提交。".to_string(),
            domain: ToolDomain::Git,
            requires_confirmation: true,
            is_readonly: false,
            params_schema: json!({
                "type": "object",
                "properties": {
                    "instance_id": { "type": "string", "description": "目标实例 ID" },
                    "message": { "type": "string", "description": "提交信息" }
                },
                "required": ["instance_id", "message"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        ctx: &ExecutionContext,
    ) -> Result<Value, String> {
        let instance_id = string_arg(&arguments, "instance_id")?;
        let message = string_arg(&arguments, "message")?;
        let _lock = ctx
            .instance_lock_manager
            .acquire_write_lock(
                &instance_id,
                std::time::Duration::from_secs(30),
            )
            .await?;
        let root = theseus::instance::get_full_path(&instance_id)
            .await
            .map_err(|e| e.to_string())?;
        git_commit_impl(&root, &message)
    }
}

/// git_checkout（confirm）。参数：instance_id、commit_hash 必填（接受 commit 或分支名）。
pub struct GitCheckoutTool;

#[async_trait]
impl Tool for GitCheckoutTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "git_checkout".to_string(),
            description:
                "检出指定 commit 或分支（非分支时进入 detached HEAD）。"
                    .to_string(),
            domain: ToolDomain::Git,
            requires_confirmation: true,
            is_readonly: false,
            params_schema: json!({
                "type": "object",
                "properties": {
                    "instance_id": { "type": "string", "description": "目标实例 ID" },
                    "commit_hash": { "type": "string", "description": "commit 哈希或分支名" }
                },
                "required": ["instance_id", "commit_hash"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        ctx: &ExecutionContext,
    ) -> Result<Value, String> {
        let instance_id = string_arg(&arguments, "instance_id")?;
        let target = string_arg(&arguments, "commit_hash")?;
        let _lock = ctx
            .instance_lock_manager
            .acquire_write_lock(
                &instance_id,
                std::time::Duration::from_secs(30),
            )
            .await?;
        let root = theseus::instance::get_full_path(&instance_id)
            .await
            .map_err(|e| e.to_string())?;
        git_checkout_impl(&root, &target)
    }
}

/// git_branch（confirm）。参数：instance_id、action 必填，branch_name 在 action!=list 时必填。
pub struct GitBranchTool;

#[async_trait]
impl Tool for GitBranchTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "git_branch".to_string(),
            description: "管理 git 分支：list | create | delete | checkout。"
                .to_string(),
            domain: ToolDomain::Git,
            requires_confirmation: true,
            is_readonly: false,
            params_schema: json!({
                "type": "object",
                "properties": {
                    "instance_id": { "type": "string", "description": "目标实例 ID" },
                    "action": { "type": "string", "description": "list | create | delete | checkout" },
                    "branch_name": { "type": "string", "description": "action 非 list 时必填的分支名" }
                },
                "required": ["instance_id", "action"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        ctx: &ExecutionContext,
    ) -> Result<Value, String> {
        let instance_id = string_arg(&arguments, "instance_id")?;
        let action = string_arg(&arguments, "action")?;
        let branch_name = arguments.get("branch_name").and_then(Value::as_str);
        let _lock = ctx
            .instance_lock_manager
            .acquire_write_lock(
                &instance_id,
                std::time::Duration::from_secs(30),
            )
            .await?;
        let root = theseus::instance::get_full_path(&instance_id)
            .await
            .map_err(|e| e.to_string())?;
        git_branch_impl(&root, &action, branch_name)
    }
}

/// git_diff（readonly）。参数：instance_id、commit 可选（缺省=工作区 vs HEAD）。
pub struct GitDiffTool;

#[async_trait]
impl Tool for GitDiffTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "git_diff".to_string(),
            description: "查看 git 变更的增删行统计（缺省为工作区 vs HEAD）。"
                .to_string(),
            domain: ToolDomain::Git,
            requires_confirmation: false,
            is_readonly: true,
            params_schema: json!({
                "type": "object",
                "properties": {
                    "instance_id": { "type": "string", "description": "目标实例 ID" },
                    "commit": { "type": "string", "description": "可选：对比的 commit，缺省为工作区 vs HEAD" }
                },
                "required": ["instance_id"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        _ctx: &ExecutionContext,
    ) -> Result<Value, String> {
        let instance_id = string_arg(&arguments, "instance_id")?;
        let commit = arguments.get("commit").and_then(Value::as_str);
        let root = theseus::instance::get_full_path(&instance_id)
            .await
            .map_err(|e| e.to_string())?;
        git_diff_impl(&root, commit)
    }
}

/// 构造并注册全部 Git 版本控制工具。
pub fn register_git_ops_tools(
    registry: &Arc<super::tools::registry::ToolRegistry>,
) {
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(GitInitTool),
        Arc::new(GitStatusTool),
        Arc::new(GitLogTool),
        Arc::new(GitCommitTool),
        Arc::new(GitCheckoutTool),
        Arc::new(GitBranchTool),
        Arc::new(GitDiffTool),
    ];
    for tool in tools {
        registry.register(tool);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{IndexAddOption, Repository};

    /// 建一个临时目录并初始化为 git 仓库，返回根路径。
    fn temp_repo() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "git_ops_test_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        Repository::init(&root).unwrap();
        root
    }

    /// 在临时仓库中创建一个已提交的文件（真实 git 前置）。
    fn commit_file(
        repo: &Repository,
        path: &str,
        content: &str,
        message: &str,
    ) {
        let root = repo.workdir().unwrap();
        std::fs::write(root.join(path), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_all(["*"], IndexAddOption::DEFAULT, None).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        // CI 无全局 git 身份；测试用仓库级身份，避免依赖环境配置。
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "test").unwrap();
            config.set_str("user.email", "test@packrinth.local").unwrap();
        }
        let sig = repo.signature().unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        match parent {
            Some(p) => {
                repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&p])
                    .unwrap();
            }
            None => {
                repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[])
                    .unwrap();
            }
        }
    }

    #[test]
    fn init_creates_new_repo() {
        let root = std::env::temp_dir().join(format!(
            "git_ops_init_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let out = git_init_impl(&root).unwrap();
        assert_eq!(out["initialized"], true);
        assert!(repo_is_initialized(&root));
    }

    #[test]
    fn init_idempotent() {
        let root = temp_repo();
        let out = git_init_impl(&root).unwrap();
        assert_eq!(out["initialized"], false);
        assert_eq!(out["exists"], true);
    }

    #[test]
    fn commit_no_changes_errors() {
        let root = temp_repo();
        let repo = open_repo(&root).unwrap();
        commit_file(&repo, "a.txt", "hello", "first");
        let err = git_commit_impl(&root, "noop").unwrap_err();
        assert!(err.contains("无变更"), "应报无变更，实际: {err}");
    }

    #[test]
    fn commit_untracked_only_succeeds() {
        let root = temp_repo();
        std::fs::write(root.join("new.txt"), "hello").unwrap();
        let out = git_commit_impl(&root, "first commit").unwrap();
        assert_eq!(out["message"], "first commit");
        assert_eq!(out["hash"].as_str().unwrap().len(), 7);
    }

    #[test]
    fn commit_and_log_order() {
        let root = temp_repo();
        let repo = open_repo(&root).unwrap();
        commit_file(&repo, "a.txt", "one", "first");
        commit_file(&repo, "a.txt", "two", "second");

        let out = git_log_impl(&root, 20).unwrap();
        let commits = out["commits"].as_array().unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0]["message"], "second");
        assert_eq!(commits[1]["message"], "first");
        assert_eq!(commits[0]["hash"].as_str().unwrap().len(), 7);
    }

    #[test]
    fn status_detects_changes() {
        let root = temp_repo();
        let repo = open_repo(&root).unwrap();
        commit_file(&repo, "a.txt", "one", "first");

        // 未跟踪文件
        std::fs::write(root.join("new.txt"), "x").unwrap();
        let out = git_status_impl(&root).unwrap();
        let files = out["files"].as_array().unwrap();
        let untracked = files.iter().find(|f| f["path"] == "new.txt").unwrap();
        assert_eq!(untracked["status"], "untracked");

        // 修改已跟踪文件
        std::fs::write(root.join("a.txt"), "two").unwrap();
        let out = git_status_impl(&root).unwrap();
        let files = out["files"].as_array().unwrap();
        let modified = files.iter().find(|f| f["path"] == "a.txt").unwrap();
        assert_eq!(modified["status"], "modified");
    }

    #[test]
    fn branch_list_and_create() {
        let root = temp_repo();
        let repo = open_repo(&root).unwrap();
        commit_file(&repo, "a.txt", "one", "first");

        // 初始分支应为 master 或 main，且是 current
        let out = git_branch_impl(&root, "list", None).unwrap();
        let branches = out["branches"].as_array().unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0]["current"], true);

        // 新建分支后 list 应含 2 条
        git_branch_impl(&root, "create", Some("feature")).unwrap();
        let out = git_branch_impl(&root, "list", None).unwrap();
        assert_eq!(out["branches"].as_array().unwrap().len(), 2);

        // 删除分支
        git_branch_impl(&root, "delete", Some("feature")).unwrap();
        let out = git_branch_impl(&root, "list", None).unwrap();
        assert_eq!(out["branches"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn diff_counts_changes() {
        let root = temp_repo();
        let repo = open_repo(&root).unwrap();
        commit_file(&repo, "a.txt", "line1\nline2\n", "first");
        std::fs::write(root.join("a.txt"), "line1\nline2\nline3\n").unwrap();

        let out = git_diff_impl(&root, None).unwrap();
        let files = out["files"].as_array().unwrap();
        let a = files.iter().find(|f| f["path"] == "a.txt").unwrap();
        assert_eq!(a["additions"], 1);
        assert_eq!(a["deletions"], 0);
    }
}
// === AI-WORKSHOP END ===
