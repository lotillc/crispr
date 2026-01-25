//! Local repository management
//!
//! Clones and maintains local copies of repositories for faster access
//! and to support the CRISPR.md context file.

use anyhow::{anyhow, Context, Result};
use git2::{Cred, FetchOptions, RemoteCallbacks, Repository};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Manages local repository clones
pub struct RepoManager {
    repos_dir: PathBuf,
    github_token: Option<String>,
}

impl RepoManager {
    pub fn new(repos_dir: impl Into<PathBuf>, github_token: Option<String>) -> Self {
        Self {
            repos_dir: repos_dir.into(),
            github_token,
        }
    }

    /// Get the local path for a repository
    pub fn repo_path(&self, owner: &str, repo: &str) -> PathBuf {
        self.repos_dir.join(owner).join(repo)
    }

    /// Ensure a repository is cloned and up to date
    /// Returns the path to the local repository
    pub fn ensure_repo(&self, owner: &str, repo: &str) -> Result<PathBuf> {
        let path = self.repo_path(owner, repo);

        if path.exists() {
            // Pull latest changes
            self.pull(&path)?;
        } else {
            // Clone the repository
            self.clone(owner, repo, &path)?;
        }

        Ok(path)
    }

    /// Clone a repository
    fn clone(&self, owner: &str, repo: &str, path: &Path) -> Result<()> {
        info!(owner, repo, path = %path.display(), "Cloning repository");

        // Create parent directories
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let url = format!("https://github.com/{}/{}.git", owner, repo);

        let mut callbacks = RemoteCallbacks::new();
        if let Some(token) = &self.github_token {
            let token = token.clone();
            callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
                Cred::userpass_plaintext("x-access-token", &token)
            });
        }

        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);

        builder
            .clone(&url, path)
            .with_context(|| format!("Failed to clone {}/{}", owner, repo))?;

        info!(owner, repo, "Repository cloned successfully");
        Ok(())
    }

    /// Pull latest changes from origin
    fn pull(&self, path: &Path) -> Result<()> {
        debug!(path = %path.display(), "Pulling latest changes");

        let repo = Repository::open(path)?;

        // Fetch from origin
        let mut callbacks = RemoteCallbacks::new();
        if let Some(token) = &self.github_token {
            let token = token.clone();
            callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
                Cred::userpass_plaintext("x-access-token", &token)
            });
        }

        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        let mut remote = repo.find_remote("origin")?;
        remote.fetch(&["refs/heads/*:refs/remotes/origin/*"], Some(&mut fetch_opts), None)?;

        // Get the default branch
        let head = repo.head()?;
        let branch_name = head
            .shorthand()
            .ok_or_else(|| anyhow!("Could not get branch name"))?;

        // Fast-forward merge
        let fetch_head = repo.find_reference(&format!("refs/remotes/origin/{}", branch_name))?;
        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;

        let (analysis, _) = repo.merge_analysis(&[&fetch_commit])?;

        if analysis.is_up_to_date() {
            debug!("Repository is up to date");
            return Ok(());
        }

        if analysis.is_fast_forward() {
            let mut reference = repo.find_reference(&format!("refs/heads/{}", branch_name))?;
            reference.set_target(fetch_commit.id(), "fast-forward")?;
            repo.set_head(&format!("refs/heads/{}", branch_name))?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
            debug!("Fast-forward merge completed");
        } else {
            warn!("Non-fast-forward merge required, skipping pull");
        }

        Ok(())
    }

    /// Read a file from the repository
    pub fn read_file(&self, owner: &str, repo: &str, file_path: &str) -> Result<String> {
        let repo_path = self.ensure_repo(owner, repo)?;
        let full_path = repo_path.join(file_path);

        std::fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read {}", full_path.display()))
    }

    /// Read a file if it exists, return None otherwise
    pub fn read_file_optional(&self, owner: &str, repo: &str, file_path: &str) -> Result<Option<String>> {
        let repo_path = self.ensure_repo(owner, repo)?;
        let full_path = repo_path.join(file_path);

        if full_path.exists() {
            Ok(Some(std::fs::read_to_string(&full_path)?))
        } else {
            Ok(None)
        }
    }

    /// Write a file to the repository (does not commit)
    pub fn write_file(&self, owner: &str, repo: &str, file_path: &str, content: &str) -> Result<()> {
        let repo_path = self.ensure_repo(owner, repo)?;
        let full_path = repo_path.join(file_path);

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&full_path, content)
            .with_context(|| format!("Failed to write {}", full_path.display()))
    }

    /// List files in a directory
    pub fn list_files(&self, owner: &str, repo: &str, dir_path: &str) -> Result<Vec<String>> {
        let repo_path = self.ensure_repo(owner, repo)?;
        let full_path = repo_path.join(dir_path);

        if !full_path.exists() {
            return Ok(vec![]);
        }

        let mut files = vec![];
        for entry in std::fs::read_dir(&full_path)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(name) = path.file_name() {
                files.push(name.to_string_lossy().to_string());
            }
        }

        Ok(files)
    }

    /// Check if a file exists
    pub fn file_exists(&self, owner: &str, repo: &str, file_path: &str) -> Result<bool> {
        let repo_path = self.ensure_repo(owner, repo)?;
        Ok(repo_path.join(file_path).exists())
    }

    /// Create a new branch from the default branch
    pub fn create_branch(&self, owner: &str, repo: &str, branch_name: &str) -> Result<()> {
        let repo_path = self.ensure_repo(owner, repo)?;
        let git_repo = Repository::open(&repo_path)?;

        let head = git_repo.head()?;
        let commit = head.peel_to_commit()?;

        git_repo.branch(branch_name, &commit, false)?;

        // Checkout the new branch
        let refname = format!("refs/heads/{}", branch_name);
        git_repo.set_head(&refname)?;
        git_repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

        info!(owner, repo, branch_name, "Created and checked out new branch");
        Ok(())
    }

    /// Stage and commit changes
    pub fn commit(&self, owner: &str, repo: &str, message: &str, files: &[&str]) -> Result<git2::Oid> {
        let repo_path = self.ensure_repo(owner, repo)?;
        let git_repo = Repository::open(&repo_path)?;

        let mut index = git_repo.index()?;

        for file in files {
            index.add_path(Path::new(file))?;
        }
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = git_repo.find_tree(tree_id)?;

        let head = git_repo.head()?;
        let parent = head.peel_to_commit()?;

        let sig = git_repo.signature()?;

        let oid = git_repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            message,
            &tree,
            &[&parent],
        )?;

        info!(owner, repo, commit = %oid, "Created commit");
        Ok(oid)
    }

    /// Push a branch to origin
    pub fn push(&self, owner: &str, repo: &str, branch_name: &str) -> Result<()> {
        let repo_path = self.ensure_repo(owner, repo)?;
        let git_repo = Repository::open(&repo_path)?;

        let mut callbacks = RemoteCallbacks::new();
        if let Some(token) = &self.github_token {
            let token = token.clone();
            callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
                Cred::userpass_plaintext("x-access-token", &token)
            });
        }

        let mut push_opts = git2::PushOptions::new();
        push_opts.remote_callbacks(callbacks);

        let mut remote = git_repo.find_remote("origin")?;
        let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);

        remote.push(&[&refspec], Some(&mut push_opts))?;

        info!(owner, repo, branch_name, "Pushed branch to origin");
        Ok(())
    }

    /// Checkout the default branch
    pub fn checkout_default(&self, owner: &str, repo: &str) -> Result<()> {
        let repo_path = self.ensure_repo(owner, repo)?;
        let git_repo = Repository::open(&repo_path)?;

        // Find the default branch (usually main or master)
        let head_ref = git_repo.find_reference("refs/remotes/origin/HEAD")?;
        let resolved = head_ref.resolve()?;
        let branch_name = resolved
            .shorthand()
            .ok_or_else(|| anyhow!("Could not determine default branch"))?
            .strip_prefix("origin/")
            .unwrap_or("main");

        let refname = format!("refs/heads/{}", branch_name);
        git_repo.set_head(&refname)?;
        git_repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

        Ok(())
    }

    /// Get the current HEAD SHA
    pub async fn get_head_sha(&self) -> Result<String> {
        // Note: This is a placeholder - actual implementation would use the stored repo context
        // For now we return a placeholder that would be replaced with actual git operations
        Ok("HEAD".to_string())
    }

    /// Get files changed between two commits
    pub async fn get_changed_files_since(&self, base_sha: &str, head_sha: &str) -> Result<Vec<String>> {
        // Placeholder implementation
        // In production, this would diff the two commits
        Ok(vec![])
    }

    /// Count commits between two SHAs
    pub async fn count_commits_between(&self, base_sha: &str, head_sha: &str) -> Result<i32> {
        // Placeholder implementation
        Ok(0)
    }

    /// Get file content at a specific commit
    pub async fn get_file_at_commit(&self, file_path: &str, commit_sha: &str) -> Result<Option<String>> {
        // Placeholder implementation
        // In production, this would use git show to get file at specific commit
        Ok(None)
    }
}
