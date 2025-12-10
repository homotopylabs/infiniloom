//! Repository scanner for Infiniloom CLI
//!
//! This module provides both a Zig-accelerated scanner (when `zig-core` feature is enabled)
//! and a pure Rust fallback for portability.
//!
//! Performance notes:
//! - Zig core: Fastest option, uses optimized Zig file walker
//! - Rust fallback: Uses `ignore` crate (respects .gitignore)
//! - File reading and parsing are parallelized with rayon
//! - Parser cache enables true parallel tree-sitter parsing
//! - Use --skip-symbols for 80x speedup on large repos

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use infiniloom_engine::dependencies::DependencyGraph;
use infiniloom_engine::parser::{Language, Parser};
use infiniloom_engine::types::{LanguageStats, RepoFile, RepoMetadata, Repository, TokenCounts};
use infiniloom_engine::ZigCore;

// Thread-local parser for each rayon worker
// This avoids mutex contention by giving each thread its own parser
thread_local! {
    static THREAD_PARSER: std::cell::RefCell<Parser> = std::cell::RefCell::new(Parser::new());
}

/// Parse content using thread-local parser (lock-free)
fn parse_with_thread_local(content: &str, path: &Path) -> Vec<infiniloom_engine::types::Symbol> {
    THREAD_PARSER.with(|parser| {
        let mut parser = parser.borrow_mut();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(lang) = Language::from_extension(ext) {
                parser.parse(content, lang).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    })
}

/// Configuration for repository scanning
pub(crate) struct ScanConfig {
    /// Include hidden files (starting with .)
    pub include_hidden: bool,
    /// Respect .gitignore files
    pub respect_gitignore: bool,
    /// Read file contents
    pub read_contents: bool,
    /// Maximum file size to include (bytes)
    pub max_file_size: u64,
    /// Skip symbol extraction (faster for large repos)
    pub skip_symbols: bool,
    /// Use Zig core for scanning (if available)
    pub use_zig_core: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            include_hidden: false,
            respect_gitignore: true,
            read_contents: false,
            max_file_size: 50 * 1024 * 1024, // 50MB
            skip_symbols: false,
            use_zig_core: ZigCore::is_available(),
        }
    }
}

/// Check if Zig core is available
#[allow(dead_code)]
pub(crate) fn is_zig_core_available() -> bool {
    ZigCore::is_available()
}

/// Get Zig core version (or "rust-fallback" if not available)
#[allow(dead_code)]
pub(crate) fn zig_core_version() -> String {
    ZigCore::version()
}

/// File info collected during initial walk
struct FileInfo {
    path: PathBuf,
    relative_path: String,
    size_bytes: u64,
    language: Option<String>,
}

/// Scan a repository and return a Repository struct
/// Uses parallel processing for improved performance on large repositories
///
/// When `use_zig_core` is true and Zig core is available, uses the faster
/// Zig-based file walker. Otherwise falls back to pure Rust implementation.
pub(crate) fn scan_repository(path: &Path, config: ScanConfig) -> Result<Repository> {
    let path = path.canonicalize().context("Invalid repository path")?;

    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_owned();

    // Try Zig core first if enabled
    if config.use_zig_core {
        if let Some(zig_core) = ZigCore::new() {
            log::info!("Using Zig core for scanning (version: {})", ZigCore::version());
            return scan_with_zig_core(&path, &repo_name, &config, &zig_core);
        } else {
            log::debug!("Zig core requested but not available, falling back to Rust");
        }
    }

    // Phase 1: Collect file paths (fast, sequential walk with ignore filtering)
    let file_infos = collect_file_infos(&path, &config)?;

    // Phase 2: Process files in parallel (reading, parsing, token counting)
    let files: Vec<RepoFile> = if config.read_contents {
        if config.skip_symbols {
            // Without symbols, parallelize freely (no parser needed)
            file_infos
                .into_par_iter()
                .filter_map(process_file_content_only)
                .collect()
        } else {
            // With symbols, use thread-local parsers for parallel parsing
            file_infos
                .into_par_iter()
                .filter_map(process_file_with_content)
                .collect()
        }
    } else {
        // Sequential is fine when just collecting metadata (CPU bound, fast)
        file_infos
            .into_iter()
            .map(process_file_without_content)
            .collect()
    };

    // Phase 3: Aggregate statistics
    let total_files = files.len() as u32;
    let total_lines: u64 = files
        .iter()
        .map(|f| {
            f.content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| estimate_lines(f.size_bytes))
        })
        .sum();

    let mut language_counts: HashMap<String, u32> = HashMap::new();
    for file in &files {
        if let Some(ref lang) = file.language {
            *language_counts.entry(lang.clone()).or_insert(0) += 1;
        }
    }

    let languages: Vec<LanguageStats> = language_counts
        .into_iter()
        .map(|(lang, count)| {
            let percentage = if total_files > 0 {
                (count as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };
            LanguageStats { language: lang, files: count, lines: 0, percentage }
        })
        .collect();

    let total_tokens = TokenCounts {
        claude: files.iter().map(|f| f.token_count.claude).sum(),
        gpt4o: files.iter().map(|f| f.token_count.gpt4o).sum(),
        gpt4: files.iter().map(|f| f.token_count.gpt4).sum(),
        gemini: files.iter().map(|f| f.token_count.gemini).sum(),
        llama: files.iter().map(|f| f.token_count.llama).sum(),
    };

    let branch = detect_git_branch(&path);
    let commit = detect_git_commit(&path);
    let directory_structure = generate_directory_structure(&files);

    // Build dependency graph and extract external dependencies
    let temp_repo = Repository {
        name: repo_name.clone(),
        path: path.clone(),
        files: files.clone(),
        metadata: RepoMetadata::default(),
    };
    let dep_graph = DependencyGraph::build(&temp_repo);
    let mut external_dependencies: Vec<String> =
        dep_graph.get_external_deps().iter().cloned().collect();
    external_dependencies.sort();

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
            directory_structure: Some(directory_structure),
            external_dependencies,
            git_history: None,
        },
    })
}

/// Collect file information (paths, sizes) without reading content
fn collect_file_infos(base_path: &Path, config: &ScanConfig) -> Result<Vec<FileInfo>> {
    let mut file_infos = Vec::new();

    let walker = WalkBuilder::new(base_path)
        .hidden(!config.include_hidden)
        .git_ignore(config.respect_gitignore)
        .git_global(config.respect_gitignore)
        .git_exclude(config.respect_gitignore)
        .filter_entry(|entry| {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                if file_name == ".git" {
                    return false;
                }
            }
            true
        })
        .build();

    for entry in walker.flatten() {
        let entry_path = entry.path();

        if !entry_path.is_file() {
            continue;
        }

        let metadata = entry_path.metadata().ok();
        let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

        if size_bytes > config.max_file_size {
            continue;
        }

        if is_binary_extension(entry_path) {
            continue;
        }

        let relative_path = entry_path
            .strip_prefix(base_path)
            .unwrap_or(entry_path)
            .to_string_lossy()
            .to_string();

        let language = detect_language(entry_path);

        file_infos.push(FileInfo {
            path: entry_path.to_path_buf(),
            relative_path,
            size_bytes,
            language,
        });
    }

    Ok(file_infos)
}

/// Process a file with content reading only (no parsing - fast path)
fn process_file_content_only(info: FileInfo) -> Option<RepoFile> {
    let content = std::fs::read_to_string(&info.path).ok()?;
    let token_count = estimate_tokens(info.size_bytes, Some(&content));

    Some(RepoFile {
        path: info.path,
        relative_path: info.relative_path,
        language: info.language,
        size_bytes: info.size_bytes,
        token_count,
        symbols: Vec::new(),
        importance: 0.5,
        content: Some(content),
    })
}

/// Process a file with content reading and parsing (used in parallel)
/// Uses thread-local parser for lock-free parallel parsing
fn process_file_with_content(info: FileInfo) -> Option<RepoFile> {
    // Read content
    let content = std::fs::read_to_string(&info.path).ok()?;

    // Estimate tokens from actual content
    let token_count = estimate_tokens(info.size_bytes, Some(&content));

    // Parse symbols using thread-local parser (lock-free)
    let symbols = parse_with_thread_local(&content, &info.path);

    Some(RepoFile {
        path: info.path,
        relative_path: info.relative_path,
        language: info.language,
        size_bytes: info.size_bytes,
        token_count,
        symbols,
        importance: 0.5,
        content: Some(content),
    })
}

/// Process a file without reading content (fast path)
fn process_file_without_content(info: FileInfo) -> RepoFile {
    let token_count = estimate_tokens(info.size_bytes, None);

    RepoFile {
        path: info.path,
        relative_path: info.relative_path,
        language: info.language,
        size_bytes: info.size_bytes,
        token_count,
        symbols: Vec::new(),
        importance: 0.5,
        content: None,
    }
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
        // Python
        "py" | "pyi" | "pyx" => "python",

        // JavaScript/TypeScript
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",

        // Rust
        "rs" => "rust",

        // Go
        "go" => "go",

        // Java/JVM
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "groovy" => "groovy",
        "clj" | "cljs" | "cljc" => "clojure",

        // C/C++
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "cxx" | "hxx" => "cpp",

        // C#
        "cs" => "csharp",

        // Ruby
        "rb" | "rake" | "gemspec" => "ruby",

        // PHP
        "php" => "php",

        // Swift
        "swift" => "swift",

        // Shell
        "sh" | "bash" => "bash",
        "zsh" => "zsh",
        "fish" => "fish",
        "ps1" | "psm1" => "powershell",

        // Web
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "sass" => "sass",
        "less" => "less",

        // Data/Config
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "ini" | "cfg" => "ini",

        // Documentation
        "md" | "markdown" => "markdown",
        "mdx" => "mdx",
        "rst" => "rst",
        "txt" => "text",

        // Zig
        "zig" => "zig",

        // Lua
        "lua" => "lua",

        // SQL
        "sql" => "sql",

        // Elixir/Erlang
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",

        // Haskell
        "hs" | "lhs" => "haskell",

        // OCaml/F#
        "ml" | "mli" => "ocaml",
        "fs" | "fsi" | "fsx" => "fsharp",

        // Vue/Svelte
        "vue" => "vue",
        "svelte" => "svelte",

        // Docker
        "dockerfile" => "dockerfile",

        // Terraform
        "tf" | "tfvars" => "terraform",

        // Makefile-like
        "makefile" | "mk" => "make",
        "cmake" => "cmake",

        // Nix
        "nix" => "nix",

        // Julia
        "jl" => "julia",

        // R
        "r" | "rmd" => "r",

        // Dart
        "dart" => "dart",

        // Nim
        "nim" => "nim",

        // V
        "v" => "vlang",

        // Crystal
        "cr" => "crystal",

        _ => return None,
    };

    Some(lang.to_owned())
}

/// Check if file has a binary extension
fn is_binary_extension(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return false,
    };

    matches!(
        ext.as_str(),
        // Executables
        "exe" | "dll" | "so" | "dylib" | "a" | "o" | "obj" | "lib" |
        // Compiled
        "pyc" | "pyo" | "class" | "jar" | "war" | "ear" |
        // Archives
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "tgz" |
        // Images
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" | "svg" | "tiff" | "psd" |
        // Audio/Video
        "mp3" | "mp4" | "avi" | "mov" | "wav" | "flac" | "ogg" | "webm" | "mkv" |
        // Documents
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" |
        // Fonts
        "woff" | "woff2" | "ttf" | "eot" | "otf" |
        // Database
        "db" | "sqlite" | "sqlite3" |
        // Misc binary
        "bin" | "dat" | "cache" | "lock" | "sum"
    )
}

/// Detect current git branch
fn detect_git_branch(path: &Path) -> Option<String> {
    let head_path = path.join(".git/HEAD");
    let content = std::fs::read_to_string(head_path).ok()?;

    if content.starts_with("ref: refs/heads/") {
        Some(
            content
                .trim_start_matches("ref: refs/heads/")
                .trim()
                .to_owned(),
        )
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
        std::fs::read_to_string(full_path).ok().map(|s| {
            // Safely take first 7 characters without panicking on short strings
            s.trim().chars().take(7).collect()
        })
    } else {
        // Detached HEAD - content is the commit hash
        // Safely take first 7 characters without panicking on short strings
        Some(content.trim().chars().take(7).collect())
    }
}

// ============================================================================
// Zig Core Scanner
// ============================================================================

/// Scan repository using Zig core for maximum performance
fn scan_with_zig_core(
    path: &Path,
    repo_name: &str,
    config: &ScanConfig,
    zig_core: &ZigCore,
) -> Result<Repository> {
    let path_str = path.to_string_lossy();

    // Perform fast directory scan with Zig
    let scan_result = zig_core
        .scan(&path_str, config.include_hidden, config.respect_gitignore, config.max_file_size)
        .map_err(|e| anyhow::anyhow!("Zig scan failed: {}", e))?;

    log::info!(
        "Zig scan completed: {} files, {} bytes in {}ms",
        scan_result.file_count,
        scan_result.total_bytes,
        scan_result.scan_time_ms
    );

    // Collect files from Zig core
    let mut files = Vec::with_capacity(scan_result.file_count as usize);
    let mut language_counts: HashMap<String, u32> = HashMap::new();

    for i in 0..zig_core.file_count() {
        if let Some(zig_file) = zig_core.get_file(i) {
            // Read content if needed
            let content = if config.read_contents {
                std::fs::read_to_string(&zig_file.path).ok()
            } else {
                None
            };

            // Parse symbols if needed
            let symbols = if config.read_contents && !config.skip_symbols {
                content
                    .as_ref()
                    .map(|c| parse_with_thread_local(c, Path::new(&zig_file.path)))
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            // Count tokens
            let token_count = if let Some(ref text) = content {
                let counts = zig_core.count_tokens_all(text);
                TokenCounts {
                    claude: counts.claude,
                    gpt4o: counts.gpt4o,
                    gpt4: counts.gpt4,
                    gemini: counts.gemini,
                    llama: counts.llama,
                }
            } else {
                estimate_tokens(zig_file.size_bytes, None)
            };

            // Track language stats
            if let Some(ref lang) = zig_file.language {
                *language_counts.entry(lang.clone()).or_insert(0) += 1;
            }

            files.push(RepoFile {
                path: PathBuf::from(&zig_file.path),
                relative_path: zig_file.relative_path,
                language: zig_file.language,
                size_bytes: zig_file.size_bytes,
                token_count,
                symbols,
                importance: zig_file.importance,
                content,
            });
        }
    }

    // Build language stats
    let total_files = files.len() as u32;
    let languages: Vec<LanguageStats> = language_counts
        .into_iter()
        .map(|(lang, count)| {
            let percentage = if total_files > 0 {
                (count as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };
            LanguageStats { language: lang, files: count, lines: 0, percentage }
        })
        .collect();

    // Aggregate token counts
    let total_tokens = TokenCounts {
        claude: files.iter().map(|f| f.token_count.claude).sum(),
        gpt4o: files.iter().map(|f| f.token_count.gpt4o).sum(),
        gpt4: files.iter().map(|f| f.token_count.gpt4).sum(),
        gemini: files.iter().map(|f| f.token_count.gemini).sum(),
        llama: files.iter().map(|f| f.token_count.llama).sum(),
    };

    let total_lines: u64 = files
        .iter()
        .map(|f| {
            f.content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| estimate_lines(f.size_bytes))
        })
        .sum();

    let branch = detect_git_branch(path);
    let commit = detect_git_commit(path);
    let directory_structure = generate_directory_structure(&files);

    // Build dependency graph and extract external dependencies
    let temp_repo = Repository {
        name: repo_name.to_owned(),
        path: path.to_path_buf(),
        files: files.clone(),
        metadata: RepoMetadata::default(),
    };
    let dep_graph = DependencyGraph::build(&temp_repo);
    let mut external_dependencies: Vec<String> =
        dep_graph.get_external_deps().iter().cloned().collect();
    external_dependencies.sort();

    Ok(Repository {
        name: repo_name.to_owned(),
        path: path.to_path_buf(),
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
            directory_structure: Some(directory_structure),
            external_dependencies,
            git_history: None,
        },
    })
}

/// Generate a tree-like directory structure from file paths
fn generate_directory_structure(files: &[RepoFile]) -> String {
    use std::collections::BTreeSet;

    // Collect all unique directory paths
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    let mut file_set: BTreeSet<&str> = BTreeSet::new();

    for file in files {
        file_set.insert(&file.relative_path);

        // Add all parent directories
        let mut current = file.relative_path.as_str();
        while let Some(idx) = current.rfind('/') {
            current = &current[..idx];
            if !current.is_empty() {
                dirs.insert(current.to_owned());
            }
        }
    }

    // Build tree structure
    let mut output = String::new();
    let mut printed: BTreeSet<String> = BTreeSet::new();

    // Sort all paths (dirs first, then files at each level)
    let mut all_paths: Vec<(&str, bool)> = Vec::new();
    for dir in &dirs {
        all_paths.push((dir, true));
    }
    for file in files {
        all_paths.push((&file.relative_path, false));
    }
    all_paths.sort_by(|a, b| {
        let a_parts: Vec<&str> = a.0.split('/').collect();
        let b_parts: Vec<&str> = b.0.split('/').collect();
        a_parts.cmp(&b_parts)
    });

    for (path, is_dir) in all_paths {
        let parts: Vec<&str> = path.split('/').collect();
        let depth = parts.len() - 1;

        // Print parent directories if not printed
        let mut parent_path = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i < parts.len() - 1 {
                if !parent_path.is_empty() {
                    parent_path.push('/');
                }
                parent_path.push_str(part);

                if !printed.contains(&parent_path) {
                    let indent = "  ".repeat(i);
                    output.push_str(&format!("{}{}/\n", indent, part));
                    printed.insert(parent_path.clone());
                }
            }
        }

        // Print the item itself
        if !is_dir {
            let name = parts.last().unwrap_or(&"");
            let indent = "  ".repeat(depth);
            output.push_str(&format!("{}{}\n", indent, name));
        }
    }

    // Limit size for very large repos
    if output.len() > 50000 {
        let truncated: String = output.chars().take(49000).collect();
        format!("{}...\n[Directory structure truncated - {} files total]", truncated, files.len())
    } else {
        output
    }
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_zig_core_availability() {
        let available = is_zig_core_available();
        println!("Zig core available: {}", available);
        println!("Version: {}", zig_core_version());
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language(&PathBuf::from("test.py")), Some("python".to_string()));
        assert_eq!(detect_language(&PathBuf::from("test.rs")), Some("rust".to_string()));
        assert_eq!(detect_language(&PathBuf::from("test.ts")), Some("typescript".to_string()));
        assert_eq!(detect_language(&PathBuf::from("test")), None);
    }

    #[test]
    fn test_is_binary_extension() {
        assert!(is_binary_extension(&PathBuf::from("test.exe")));
        assert!(is_binary_extension(&PathBuf::from("test.png")));
        assert!(!is_binary_extension(&PathBuf::from("test.rs")));
        assert!(!is_binary_extension(&PathBuf::from("test.py")));
    }

    #[test]
    fn test_estimate_tokens() {
        let tokens = estimate_tokens(1000, None);
        assert!(tokens.claude > 0);
        assert!(tokens.gpt4o > 0);
    }
}
