//! Remote repository support
//!
//! Supports cloning and fetching from remote Git repositories (GitHub, GitLab, Bitbucket, etc.)

use std::path::{Path, PathBuf};
use std::process::Command;
use url::Url;

/// Supported Git providers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitProvider {
    GitHub,
    GitLab,
    Bitbucket,
    Generic,
}

/// Parsed remote repository URL
#[derive(Debug, Clone)]
pub struct RemoteRepo {
    /// Original URL
    pub url: String,
    /// Git provider
    pub provider: GitProvider,
    /// Repository owner/organization
    pub owner: Option<String>,
    /// Repository name
    pub name: String,
    /// Branch to clone (None = default branch)
    pub branch: Option<String>,
    /// Specific commit/tag to checkout
    pub reference: Option<String>,
    /// Subdirectory to extract (sparse checkout)
    pub subdir: Option<String>,
}

impl RemoteRepo {
    /// Parse a remote URL into a RemoteRepo
    /// Supports formats:
    /// - https://github.com/owner/repo
    /// - https://github.com/owner/repo/tree/branch
    /// - https://github.com/owner/repo/tree/branch/subdir
    /// - github:owner/repo
    /// - owner/repo (assumes GitHub)
    /// - git@github.com:owner/repo.git
    pub fn parse(input: &str) -> Result<Self, RemoteError> {
        let input = input.trim();

        // Handle shorthand formats
        if let Some(rest) = input.strip_prefix("github:") {
            return Self::parse_shorthand(rest, GitProvider::GitHub);
        }
        if let Some(rest) = input.strip_prefix("gitlab:") {
            return Self::parse_shorthand(rest, GitProvider::GitLab);
        }
        if let Some(rest) = input.strip_prefix("bitbucket:") {
            return Self::parse_shorthand(rest, GitProvider::Bitbucket);
        }

        // Handle owner/repo shorthand (assumes GitHub)
        if !input.contains("://") && !input.contains('@') && input.contains('/') {
            return Self::parse_shorthand(input, GitProvider::GitHub);
        }

        // Handle SSH URLs (git@github.com:owner/repo.git)
        if input.starts_with("git@") {
            return Self::parse_ssh_url(input);
        }

        // Handle HTTPS URLs
        Self::parse_https_url(input)
    }

    fn parse_shorthand(input: &str, provider: GitProvider) -> Result<Self, RemoteError> {
        let parts: Vec<&str> = input.split('/').collect();
        if parts.len() < 2 {
            return Err(RemoteError::InvalidUrl(format!("Invalid shorthand: {}", input)));
        }

        let owner = parts[0].to_owned();
        let name = parts[1].trim_end_matches(".git").to_owned();

        let (branch, subdir) = if parts.len() > 2 {
            // Check if "tree" or "blob" is in path (GitHub URL format)
            if parts.get(2) == Some(&"tree") || parts.get(2) == Some(&"blob") {
                let branch = parts.get(3).map(|s| s.to_string());
                let subdir = if parts.len() > 4 {
                    Some(parts[4..].join("/"))
                } else {
                    None
                };
                (branch, subdir)
            } else {
                // Assume rest is subdir
                (None, Some(parts[2..].join("/")))
            }
        } else {
            (None, None)
        };

        Ok(Self {
            url: Self::build_clone_url(provider, &owner, &name),
            provider,
            owner: Some(owner),
            name,
            branch,
            reference: None,
            subdir,
        })
    }

    fn parse_ssh_url(input: &str) -> Result<Self, RemoteError> {
        // git@github.com:owner/repo.git
        let provider = if input.contains("github.com") {
            GitProvider::GitHub
        } else if input.contains("gitlab.com") {
            GitProvider::GitLab
        } else if input.contains("bitbucket.org") {
            GitProvider::Bitbucket
        } else {
            GitProvider::Generic
        };

        // Extract owner/repo from path
        let path_start = input
            .find(':')
            .ok_or_else(|| RemoteError::InvalidUrl("Invalid SSH URL format".to_owned()))?
            + 1;
        let path = &input[path_start..];

        Self::parse_shorthand(path, provider)
    }

    fn parse_https_url(input: &str) -> Result<Self, RemoteError> {
        let url = Url::parse(input).map_err(|e| RemoteError::InvalidUrl(e.to_string()))?;

        let host = url.host_str().unwrap_or("");
        let provider = if host.contains("github.com") {
            GitProvider::GitHub
        } else if host.contains("gitlab.com") {
            GitProvider::GitLab
        } else if host.contains("bitbucket.org") {
            GitProvider::Bitbucket
        } else {
            GitProvider::Generic
        };

        let path = url.path().trim_start_matches('/');
        Self::parse_shorthand(path, provider)
    }

    fn build_clone_url(provider: GitProvider, owner: &str, name: &str) -> String {
        match provider {
            GitProvider::GitHub => format!("https://github.com/{}/{}.git", owner, name),
            GitProvider::GitLab => format!("https://gitlab.com/{}/{}.git", owner, name),
            GitProvider::Bitbucket => format!("https://bitbucket.org/{}/{}.git", owner, name),
            GitProvider::Generic => format!("https://example.com/{}/{}.git", owner, name),
        }
    }

    /// Clone the repository to a temporary directory
    pub fn clone(&self, target_dir: Option<&Path>) -> Result<PathBuf, RemoteError> {
        let target = target_dir.map(PathBuf::from).unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "infiniloom-{}-{}",
                self.owner.as_deref().unwrap_or("repo"),
                self.name
            ))
        });

        // Clean up existing directory
        if target.exists() {
            std::fs::remove_dir_all(&target).map_err(|e| RemoteError::IoError(e.to_string()))?;
        }

        // Build git clone command
        let mut cmd = Command::new("git");
        cmd.arg("clone");

        // Shallow clone for faster download
        cmd.arg("--depth").arg("1");

        // Branch if specified
        if let Some(ref branch) = self.branch {
            cmd.arg("--branch").arg(branch);
        }

        // Single branch for speed
        cmd.arg("--single-branch");

        cmd.arg(&self.url);
        cmd.arg(&target);

        let output = cmd
            .output()
            .map_err(|e| RemoteError::GitError(format!("Failed to run git: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RemoteError::GitError(format!("git clone failed: {}", stderr)));
        }

        // Checkout specific reference if provided
        if let Some(ref reference) = self.reference {
            let mut checkout = Command::new("git");
            checkout.current_dir(&target);
            checkout.args(["checkout", reference]);

            let output = checkout
                .output()
                .map_err(|e| RemoteError::GitError(format!("Failed to checkout: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(RemoteError::GitError(format!("git checkout failed: {}", stderr)));
            }
        }

        // If subdir specified, return path to subdir
        if let Some(ref subdir) = self.subdir {
            let subdir_path = target.join(subdir);
            if subdir_path.exists() {
                return Ok(subdir_path);
            }
        }

        Ok(target)
    }

    /// Clone with sparse checkout (only fetch specified paths)
    pub fn sparse_clone(
        &self,
        paths: &[&str],
        target_dir: Option<&Path>,
    ) -> Result<PathBuf, RemoteError> {
        let target = target_dir.map(PathBuf::from).unwrap_or_else(|| {
            std::env::temp_dir().join(format!("infiniloom-sparse-{}", self.name))
        });

        // Clean up
        if target.exists() {
            std::fs::remove_dir_all(&target).map_err(|e| RemoteError::IoError(e.to_string()))?;
        }

        // Initialize empty repo
        let mut init = Command::new("git");
        init.args(["init", &target.to_string_lossy()]);
        init.output()
            .map_err(|e| RemoteError::GitError(e.to_string()))?;

        // Configure sparse checkout
        let mut config = Command::new("git");
        config.current_dir(&target);
        config.args(["config", "core.sparseCheckout", "true"]);
        config
            .output()
            .map_err(|e| RemoteError::GitError(e.to_string()))?;

        // Add remote
        let mut remote = Command::new("git");
        remote.current_dir(&target);
        remote.args(["remote", "add", "origin", &self.url]);
        remote
            .output()
            .map_err(|e| RemoteError::GitError(e.to_string()))?;

        // Write sparse checkout config
        let sparse_dir = target.join(".git/info");
        std::fs::create_dir_all(&sparse_dir).map_err(|e| RemoteError::IoError(e.to_string()))?;

        let sparse_file = sparse_dir.join("sparse-checkout");
        let sparse_content = paths.join("\n");
        std::fs::write(&sparse_file, sparse_content)
            .map_err(|e| RemoteError::IoError(e.to_string()))?;

        // Fetch and checkout
        let branch = self.branch.as_deref().unwrap_or("HEAD");
        let mut fetch = Command::new("git");
        fetch.current_dir(&target);
        fetch.args(["fetch", "--depth", "1", "origin", branch]);
        let output = fetch
            .output()
            .map_err(|e| RemoteError::GitError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RemoteError::GitError(format!("git fetch failed: {}", stderr)));
        }

        let mut checkout = Command::new("git");
        checkout.current_dir(&target);
        checkout.args(["checkout", "FETCH_HEAD"]);
        checkout
            .output()
            .map_err(|e| RemoteError::GitError(e.to_string()))?;

        Ok(target)
    }

    /// Check if a URL is a remote repository URL
    pub fn is_remote_url(input: &str) -> bool {
        input.contains("://") ||
        input.starts_with("git@") ||
        input.starts_with("github:") ||
        input.starts_with("gitlab:") ||
        input.starts_with("bitbucket:") ||
        // Simple owner/repo format (not starting with / or .)
        (input.contains('/') && !input.starts_with('/') && !input.starts_with('.') && input.matches('/').count() == 1)
    }
}

/// Remote repository errors
#[derive(Debug)]
pub enum RemoteError {
    InvalidUrl(String),
    GitError(String),
    IoError(String),
    NotFound(String),
}

impl std::fmt::Display for RemoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            Self::GitError(msg) => write!(f, "Git error: {}", msg),
            Self::IoError(msg) => write!(f, "I/O error: {}", msg),
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
        }
    }
}

impl std::error::Error for RemoteError {}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_url() {
        let repo = RemoteRepo::parse("https://github.com/rust-lang/rust").unwrap();
        assert_eq!(repo.provider, GitProvider::GitHub);
        assert_eq!(repo.owner, Some("rust-lang".to_string()));
        assert_eq!(repo.name, "rust");
    }

    #[test]
    fn test_parse_shorthand() {
        let repo = RemoteRepo::parse("rust-lang/rust").unwrap();
        assert_eq!(repo.provider, GitProvider::GitHub);
        assert_eq!(repo.name, "rust");

        let repo = RemoteRepo::parse("github:rust-lang/rust").unwrap();
        assert_eq!(repo.provider, GitProvider::GitHub);
    }

    #[test]
    fn test_parse_ssh_url() {
        let repo = RemoteRepo::parse("git@github.com:rust-lang/rust.git").unwrap();
        assert_eq!(repo.provider, GitProvider::GitHub);
        assert_eq!(repo.owner, Some("rust-lang".to_string()));
        assert_eq!(repo.name, "rust");
    }

    #[test]
    fn test_parse_with_branch() {
        let repo = RemoteRepo::parse("https://github.com/rust-lang/rust/tree/master").unwrap();
        assert_eq!(repo.branch, Some("master".to_string()));
    }

    #[test]
    fn test_is_remote_url() {
        assert!(RemoteRepo::is_remote_url("https://github.com/foo/bar"));
        assert!(RemoteRepo::is_remote_url("git@github.com:foo/bar.git"));
        assert!(RemoteRepo::is_remote_url("github:foo/bar"));
        assert!(!RemoteRepo::is_remote_url("/path/to/local/repo"));
    }
}
