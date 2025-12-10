//! Repository scanner for Python bindings
//!
//! This is a pure Rust scanner similar to the CLI's scanner, adapted for Python bindings.

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::path::Path;

use infiniloom_engine::types::{
    LanguageStats, RepoFile, RepoMetadata, Repository, TokenCounts,
};

/// Configuration for repository scanning
pub struct ScanConfig {
    /// Include hidden files (starting with .)
    pub include_hidden: bool,
    /// Respect .gitignore files
    pub respect_gitignore: bool,
    /// Read file contents
    pub read_contents: bool,
    /// Maximum file size to include (bytes)
    pub max_file_size: u64,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            include_hidden: false,
            respect_gitignore: true,
            read_contents: false,
            max_file_size: 50 * 1024 * 1024, // 50MB
        }
    }
}

/// Scan a repository and return a Repository struct
pub fn scan_repository(path: &Path, config: ScanConfig) -> Result<Repository> {
    let path = path.canonicalize().context("Invalid repository path")?;

    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_string();

    let mut files = Vec::new();
    let mut language_counts: HashMap<String, u32> = HashMap::new();
    let mut total_lines: u64 = 0;

    // Build walker with ignore support
    let walker = WalkBuilder::new(&path)
        .hidden(!config.include_hidden)
        .git_ignore(config.respect_gitignore)
        .git_global(config.respect_gitignore)
        .git_exclude(config.respect_gitignore)
        .build();

    for entry in walker.flatten() {
        let entry_path = entry.path();

        // Skip directories
        if !entry_path.is_file() {
            continue;
        }

        // Check file size
        let metadata = entry_path.metadata().ok();
        let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

        if size_bytes > config.max_file_size {
            continue;
        }

        // Skip binary files
        if is_binary_extension(entry_path) {
            continue;
        }

        // Get relative path
        let relative_path = entry_path
            .strip_prefix(&path)
            .unwrap_or(entry_path)
            .to_string_lossy()
            .to_string();

        // Detect language
        let language = detect_language(entry_path);

        // Update language counts
        if let Some(ref lang) = language {
            *language_counts.entry(lang.clone()).or_insert(0) += 1;
        }

        // Read content if requested
        let content = if config.read_contents {
            std::fs::read_to_string(entry_path).ok()
        } else {
            None
        };

        // Count lines
        let lines = content
            .as_ref()
            .map(|c| c.lines().count() as u64)
            .unwrap_or_else(|| estimate_lines(size_bytes));
        total_lines += lines;

        // Estimate token counts
        let token_count = estimate_tokens(size_bytes, content.as_deref());

        files.push(RepoFile {
            path: entry_path.to_path_buf(),
            relative_path,
            language,
            size_bytes,
            token_count,
            symbols: Vec::new(), // Would need AST parsing
            importance: 0.5,     // Default importance
            content,
        });
    }

    // Calculate language statistics
    let total_files = files.len() as u32;
    let languages: Vec<LanguageStats> = language_counts
        .into_iter()
        .map(|(lang, count)| {
            let percentage = if total_files > 0 {
                (count as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };
            LanguageStats {
                language: lang,
                files: count,
                lines: 0, // Would need per-language line counting
                percentage,
            }
        })
        .collect();

    // Calculate total tokens
    let total_tokens = TokenCounts {
        claude: files.iter().map(|f| f.token_count.claude).sum(),
        gpt4o: files.iter().map(|f| f.token_count.gpt4o).sum(),
        gpt4: files.iter().map(|f| f.token_count.gpt4).sum(),
        gemini: files.iter().map(|f| f.token_count.gemini).sum(),
        llama: files.iter().map(|f| f.token_count.llama).sum(),
    };

    let branch = detect_git_branch(&path);
    let commit = detect_git_commit(&path);

    Ok(Repository {
        name: repo_name,
        path,
        files,
        metadata: RepoMetadata {
            total_files,
            total_lines,
            total_tokens,
            languages,
            framework: None,
            description: None,
            branch,
            commit,
            directory_structure: None,
            external_dependencies: Vec::new(),
            git_history: None,
        },
    })
}

/// Estimate tokens from file size
fn estimate_tokens(size_bytes: u64, content: Option<&str>) -> TokenCounts {
    let size = size_bytes as f32;

    // If we have content, count more accurately
    if let Some(text) = content {
        let len = text.len() as f32;
        return TokenCounts {
            claude: (len / 3.5) as u32,
            gpt4o: (len / 4.0) as u32,
            gpt4: (len / 3.7) as u32,
            gemini: (len / 3.8) as u32,
            llama: (len / 3.5) as u32,
        };
    }

    // Otherwise estimate from file size
    TokenCounts {
        claude: (size / 3.5) as u32,
        gpt4o: (size / 4.0) as u32,
        gpt4: (size / 3.7) as u32,
        gemini: (size / 3.8) as u32,
        llama: (size / 3.5) as u32,
    }
}

/// Estimate lines from file size
fn estimate_lines(size_bytes: u64) -> u64 {
    // Average ~40 characters per line
    size_bytes / 40
}

/// Detect programming language from file extension
fn detect_language(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?;

    let lang = match ext.to_lowercase().as_str() {
        "py" | "pyi" | "pyx" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "rs" => "rust",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "cxx" | "hxx" => "cpp",
        "cs" => "csharp",
        "rb" | "rake" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "sh" | "bash" => "bash",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "md" | "markdown" => "markdown",
        "sql" => "sql",
        "zig" => "zig",
        _ => return None,
    };

    Some(lang.to_string())
}

/// Check if file has a binary extension
fn is_binary_extension(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return false,
    };

    matches!(
        ext.as_str(),
        "exe" | "dll" | "so" | "dylib" | "a" | "o" | "obj" | "lib" |
        "pyc" | "pyo" | "class" | "jar" | "war" | "ear" |
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" |
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" | "svg" |
        "mp3" | "mp4" | "avi" | "mov" | "wav" |
        "pdf" | "doc" | "docx" | "xls" | "xlsx" |
        "woff" | "woff2" | "ttf" | "eot" |
        "db" | "sqlite" | "sqlite3"
    )
}

/// Detect current git branch
fn detect_git_branch(path: &Path) -> Option<String> {
    let head_path = path.join(".git/HEAD");
    let content = std::fs::read_to_string(head_path).ok()?;

    if content.starts_with("ref: refs/heads/") {
        Some(content.trim_start_matches("ref: refs/heads/").trim().to_owned())
    } else {
        // Detached HEAD - safely take first 7 characters
        Some(content.trim().chars().take(7).collect())
    }
}

/// Detect current git commit
fn detect_git_commit(path: &Path) -> Option<String> {
    let head_path = path.join(".git/HEAD");
    let content = std::fs::read_to_string(head_path).ok()?;

    if content.starts_with("ref: ") {
        // Follow ref
        let ref_path = content.trim_start_matches("ref: ").trim();
        let full_path = path.join(".git").join(ref_path);
        std::fs::read_to_string(full_path)
            .ok()
            .map(|s| s.trim().chars().take(7).collect())
    } else {
        // Detached HEAD - safely take first 7 characters
        Some(content.trim().chars().take(7).collect())
    }
}
