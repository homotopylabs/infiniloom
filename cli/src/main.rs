//! Infiniloom CLI - Repository context generator for LLMs
//!
//! This CLI tool generates optimized repository context for AI assistants.

// CLI tools legitimately use print macros for user output
#![allow(clippy::print_stdout, clippy::print_stderr)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use humansize::{format_size, BINARY};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Instant;

mod scanner;

use infiniloom_engine::{
    git::GitRepo,
    output::{OutputFormat, OutputFormatter},
    remote::RemoteRepo,
    repomap::RepoMapGenerator,
    security::SecurityScanner,
    types::{CompressionLevel, TokenizerModel},
};
use std::io::{self, BufRead};

/// Infiniloom - Repository context generator for LLMs
#[derive(Parser)]
#[command(
    name = "infiniloom",
    version,
    about = "Generate optimized repository context for LLMs",
    long_about = "Infiniloom transforms codebases into LLM-friendly formats with intelligent\ncompression, symbol ranking, and model-specific optimizations."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Pack a repository into LLM-friendly format
    Pack {
        /// Path to repository (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, value_enum, default_value = "xml")]
        format: Format,

        /// Target model for optimization
        #[arg(short, long, value_enum, default_value = "claude")]
        model: Model,

        /// Compression level
        #[arg(short, long, value_enum, default_value = "balanced")]
        compression: Compression,

        /// Maximum output tokens (0 = no limit)
        #[arg(short = 't', long, default_value = "100000")]
        max_tokens: u32,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Include hidden files
        #[arg(long)]
        hidden: bool,

        /// Don't respect .gitignore
        #[arg(long)]
        no_gitignore: bool,

        /// Enable symbol extraction (slower, but provides better repo map)
        #[arg(long)]
        symbols: bool,

        /// Enable full analysis mode (symbols + repo map + PageRank ranking)
        #[arg(long)]
        full: bool,

        /// Include test files (excluded by default)
        #[arg(long)]
        include_tests: bool,

        /// Include documentation files (excluded by default)
        #[arg(long)]
        include_docs: bool,

        /// Disable default ignore patterns (node_modules, dist, etc.)
        #[arg(long)]
        no_default_ignores: bool,

        /// Verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Custom header text to include at the top
        #[arg(long)]
        header_text: Option<String>,

        /// Path to file containing custom instructions
        #[arg(long)]
        instruction_file: Option<PathBuf>,

        /// Copy output to clipboard
        #[arg(long)]
        copy_to_clipboard: bool,

        /// Show token count breakdown by file
        #[arg(long)]
        token_tree: bool,

        /// Hide directory structure from output
        #[arg(long)]
        no_directory_structure: bool,

        /// Hide file summary from output
        #[arg(long)]
        no_file_summary: bool,

        /// Remove empty lines from code
        #[arg(long)]
        remove_empty_lines: bool,

        /// Remove comments from code
        #[arg(long)]
        remove_comments: bool,

        /// Limit number of files in summary (0 = all)
        #[arg(long, default_value = "0")]
        top_files: usize,

        /// Include git commit history in output
        #[arg(long)]
        include_logs: bool,

        /// Number of git log entries to include
        #[arg(long, default_value = "50")]
        logs_count: usize,

        /// Include git diffs in output
        #[arg(long)]
        include_diffs: bool,

        /// Sort files by git change frequency
        #[arg(long)]
        sort_by_changes: bool,

        /// Read file paths from stdin (one per line)
        #[arg(long)]
        stdin: bool,

        /// Truncate base64 encoded content
        #[arg(long)]
        truncate_base64: bool,

        /// Include only files matching glob pattern (can be repeated)
        #[arg(long = "include", short = 'i')]
        include_patterns: Vec<String>,

        /// Exclude files matching glob pattern (can be repeated)
        #[arg(long = "exclude", short = 'e')]
        exclude_patterns: Vec<String>,

        /// Scan for security issues (secrets, API keys)
        #[arg(long)]
        security_check: bool,

        /// Branch to checkout for remote repositories
        #[arg(long)]
        remote_branch: Option<String>,

        /// Disable line numbers in output
        #[arg(long)]
        no_line_numbers: bool,

        /// Path to config file (default: .infiniloom.yaml)
        #[arg(long)]
        config: Option<PathBuf>,

        /// Watch for file changes and regenerate output
        #[arg(long)]
        watch: bool,
    },

    /// Scan a repository and show statistics
    Scan {
        /// Path to repository (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Target model for token counting
        #[arg(short, long, value_enum, default_value = "claude")]
        model: Model,

        /// Include hidden files
        #[arg(long)]
        hidden: bool,

        /// Show detailed file list
        #[arg(short, long)]
        verbose: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Generate a repository map (symbol index)
    Map {
        /// Path to repository (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Token budget for map
        #[arg(short, long, default_value = "2000")]
        budget: u32,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Show version and configuration info
    Info,

    /// Initialize a new configuration file
    Init {
        /// Configuration format
        #[arg(short, long, value_enum, default_value = "yaml")]
        format: ConfigFormat,

        /// Output path (default: .infiniloom.yaml in current directory)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Overwrite existing config file
        #[arg(long)]
        force: bool,
    },
}

#[derive(ValueEnum, Clone, Copy)]
enum ConfigFormat {
    /// YAML format
    Yaml,
    /// TOML format
    Toml,
    /// JSON format
    Json,
}

#[derive(ValueEnum, Clone, Copy)]
enum Format {
    /// XML format (Claude-optimized)
    Xml,
    /// Markdown format (GPT-optimized)
    Markdown,
    /// JSON format (generic)
    Json,
    /// YAML format (Gemini-optimized)
    Yaml,
    /// TOON format (most token-efficient, 40% smaller)
    Toon,
    /// Plain text format (simple, no formatting)
    Plain,
}

impl From<Format> for OutputFormat {
    fn from(f: Format) -> Self {
        match f {
            Format::Xml => OutputFormat::Xml,
            Format::Markdown => OutputFormat::Markdown,
            Format::Json => OutputFormat::Json,
            Format::Yaml => OutputFormat::Yaml,
            Format::Toon => OutputFormat::Toon,
            Format::Plain => OutputFormat::Plain,
        }
    }
}

#[derive(ValueEnum, Clone, Copy)]
enum Model {
    Claude,
    Gpt4o,
    Gpt4,
    Gemini,
    Llama,
}

impl From<Model> for TokenizerModel {
    fn from(m: Model) -> Self {
        match m {
            Model::Claude => TokenizerModel::Claude,
            Model::Gpt4o => TokenizerModel::Gpt4o,
            Model::Gpt4 => TokenizerModel::Gpt4,
            Model::Gemini => TokenizerModel::Gemini,
            Model::Llama => TokenizerModel::Llama,
        }
    }
}

#[derive(ValueEnum, Clone, Copy)]
enum Compression {
    /// No compression
    None,
    /// Minimal: remove empty lines
    Minimal,
    /// Balanced: remove comments
    Balanced,
    /// Aggressive: signatures only
    Aggressive,
    /// Extreme: key symbols only
    Extreme,
}

impl From<Compression> for CompressionLevel {
    fn from(c: Compression) -> Self {
        match c {
            Compression::None => CompressionLevel::None,
            Compression::Minimal => CompressionLevel::Minimal,
            Compression::Balanced => CompressionLevel::Balanced,
            Compression::Aggressive => CompressionLevel::Aggressive,
            Compression::Extreme => CompressionLevel::Extreme,
        }
    }
}

fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn")
    ).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Pack {
            path,
            format,
            model,
            compression,
            max_tokens,
            output,
            hidden,
            no_gitignore,
            symbols,
            full,
            include_tests,
            include_docs,
            no_default_ignores,
            verbose,
            header_text,
            instruction_file,
            copy_to_clipboard,
            token_tree,
            no_directory_structure,
            no_file_summary,
            remove_empty_lines,
            remove_comments,
            top_files,
            include_logs,
            logs_count,
            include_diffs,
            sort_by_changes,
            stdin,
            truncate_base64,
            include_patterns,
            exclude_patterns,
            security_check,
            remote_branch,
            no_line_numbers,
            config,
            watch,
        } => cmd_pack(
            path,
            format.into(),
            model.into(),
            compression.into(),
            max_tokens,
            output,
            hidden,
            !no_gitignore,
            symbols || full,  // Enable symbols if --symbols or --full
            full,             // Full mode for PageRank ranking
            include_tests,
            include_docs,
            !no_default_ignores,
            verbose,
            header_text,
            instruction_file,
            copy_to_clipboard,
            token_tree,
            !no_directory_structure,
            !no_file_summary,
            remove_empty_lines,
            remove_comments,
            top_files,
            include_logs,
            logs_count,
            include_diffs,
            sort_by_changes,
            stdin,
            truncate_base64,
            include_patterns,
            exclude_patterns,
            security_check,
            remote_branch,
            !no_line_numbers,
            config,
            watch,
        ),
        Commands::Scan {
            path,
            model,
            hidden,
            verbose,
            json,
        } => cmd_scan(path, model.into(), hidden, verbose, json),
        Commands::Map {
            path,
            budget,
            output,
        } => cmd_map(path, budget, output),
        Commands::Info => cmd_info(),
        Commands::Init {
            format,
            output,
            force,
        } => cmd_init(format, output, force),
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_pack(
    path: PathBuf,
    format: OutputFormat,
    model: TokenizerModel,
    compression: CompressionLevel,
    max_tokens: u32,
    output: Option<PathBuf>,
    include_hidden: bool,
    respect_gitignore: bool,
    enable_symbols: bool,
    full_mode: bool,
    include_tests: bool,
    include_docs: bool,
    use_default_ignores: bool,
    verbose: bool,
    header_text: Option<String>,
    instruction_file: Option<PathBuf>,
    copy_to_clipboard: bool,
    token_tree: bool,
    show_directory_structure: bool,
    show_file_summary: bool,
    remove_empty_lines: bool,
    remove_comments: bool,
    top_files: usize,
    include_logs: bool,
    logs_count: usize,
    include_diffs: bool,
    sort_by_changes: bool,
    stdin: bool,
    truncate_base64: bool,
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
    security_check: bool,
    remote_branch: Option<String>,
    show_line_numbers: bool,
    config_path: Option<PathBuf>,
    watch_mode: bool,
) -> Result<()> {
    let start = Instant::now();

    // Handle stdin mode - read file paths from stdin
    let stdin_paths: Option<Vec<String>> = if stdin {
        let stdin_handle = io::stdin();
        let paths: Vec<String> = stdin_handle
            .lock()
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.trim().is_empty())
            .collect();
        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    } else {
        None
    };

    if verbose {
        eprintln!("{}", "Infiniloom - Repository Context Generator".cyan().bold());
        eprintln!();
    }

    // Create progress bar with better formatting
    let pb = if verbose {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed_precise}] {msg}")
                .unwrap(),
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        pb.set_message("Scanning repository...");
        Some(pb)
    } else {
        None
    };

    // Load config file if specified or look for default
    let loaded_config = load_config_file(config_path.as_ref(), &path);

    // Handle remote URL - clone if needed
    let (repo_path, _temp_dir) = if RemoteRepo::is_remote_url(path.to_string_lossy().as_ref()) {
        if let Some(pb) = &pb {
            pb.set_message("Cloning remote repository...");
        }
        let mut remote = RemoteRepo::parse(path.to_string_lossy().as_ref())
            .map_err(|e| anyhow::anyhow!("Invalid remote URL: {}", e))?;

        // Override branch if specified via CLI
        if let Some(ref branch) = remote_branch {
            remote.branch = Some(branch.clone());
        }

        if verbose {
            let branch_info = remote.branch.as_deref().unwrap_or("default");
            eprintln!("  Cloning {} from {:?} (branch: {})...", remote.name, remote.provider, branch_info);
        }

        let cloned_path = remote.clone(None)
            .map_err(|e| anyhow::anyhow!("Failed to clone repository: {}", e))?;

        // Keep temp dir alive by returning it
        (cloned_path, Some(()))
    } else {
        (path.clone(), None)
    };

    // Scan repository
    // Fast mode (default): skip symbols for speed
    // Full mode: enable symbols for better ranking and repo map
    let config = scanner::ScanConfig {
        include_hidden,
        respect_gitignore,
        read_contents: true,
        max_file_size: 50 * 1024 * 1024, // 50MB
        skip_symbols: !enable_symbols,  // Skip by default unless --symbols or --full
        ..Default::default()
    };

    let mut repo = scanner::scan_repository(&repo_path, config)
        .context("Failed to scan repository")?;

    // Apply default ignores (test files, docs, node_modules, etc.)
    if use_default_ignores {
        use infiniloom_engine::default_ignores::{DEFAULT_IGNORES, TEST_IGNORES, DOC_IGNORES, matches_any};

        let before_count = repo.files.len();
        repo.files.retain(|f| {
            // Always apply default ignores
            if matches_any(&f.relative_path, DEFAULT_IGNORES) {
                return false;
            }
            // Optionally filter tests
            if !include_tests && matches_any(&f.relative_path, TEST_IGNORES) {
                return false;
            }
            // Optionally filter docs
            if !include_docs && matches_any(&f.relative_path, DOC_IGNORES) {
                return false;
            }
            true
        });

        if verbose && repo.files.len() < before_count {
            if let Some(pb) = &pb {
                pb.set_message(format!("Filtered {} -> {} files (default ignores)", before_count, repo.files.len()));
            }
        }
    }

    // Filter to stdin paths if provided
    if let Some(ref paths) = stdin_paths {
        repo.files.retain(|f| paths.iter().any(|p| f.relative_path == *p || f.relative_path.ends_with(p)));
        if verbose {
            if let Some(pb) = &pb {
                pb.set_message(format!("Filtered to {} files from stdin", repo.files.len()));
            }
        }
    }

    // Apply include patterns
    if !include_patterns.is_empty() {
        let patterns: Vec<glob::Pattern> = include_patterns
            .iter()
            .filter_map(|p| glob::Pattern::new(p).ok())
            .collect();
        if !patterns.is_empty() {
            repo.files.retain(|f| patterns.iter().any(|p| p.matches(&f.relative_path)));
            if verbose {
                if let Some(pb) = &pb {
                    pb.set_message(format!("Included {} files matching patterns", repo.files.len()));
                }
            }
        }
    }

    // Apply exclude patterns (combine CLI args with config file patterns)
    let all_exclude_patterns: Vec<String> = exclude_patterns
        .into_iter()
        .chain(loaded_config.exclude_patterns)
        .collect();

    if !all_exclude_patterns.is_empty() {
        let patterns: Vec<glob::Pattern> = all_exclude_patterns
            .iter()
            .filter_map(|p| glob::Pattern::new(p).ok())
            .collect();
        if !patterns.is_empty() {
            repo.files.retain(|f| !patterns.iter().any(|p| p.matches(&f.relative_path)));
            if verbose {
                if let Some(pb) = &pb {
                    pb.set_message(format!("After exclusions: {} files", repo.files.len()));
                }
            }
        }
    }

    // Limit to top N files if specified
    if top_files > 0 && repo.files.len() > top_files {
        repo.files.truncate(top_files);
        if verbose {
            if let Some(pb) = &pb {
                pb.set_message(format!("Limited to top {} files", top_files));
            }
        }
    }

    if let Some(pb) = &pb {
        pb.set_message(format!("Found {} files", repo.files.len()));
    }

    // Sort by git change frequency if requested
    if sort_by_changes {
        if let Ok(git_repo) = GitRepo::open(&path) {
            // Calculate change frequency for each file (commits in last 90 days)
            let mut file_changes: Vec<(String, u32)> = repo.files
                .iter()
                .map(|f| {
                    let freq = git_repo.file_change_frequency(&f.relative_path, 90).unwrap_or(0);
                    (f.relative_path.clone(), freq)
                })
                .collect();

            // Sort by frequency descending
            file_changes.sort_by(|a, b| b.1.cmp(&a.1));

            // Reorder files based on change frequency
            let order_map: std::collections::HashMap<String, usize> = file_changes
                .iter()
                .enumerate()
                .map(|(i, (path, _))| (path.clone(), i))
                .collect();

            repo.files.sort_by_key(|f| order_map.get(&f.relative_path).copied().unwrap_or(usize::MAX));

            if verbose {
                if let Some(pb) = &pb {
                    pb.set_message("Sorted files by git change frequency");
                }
            }
        }
    } else if full_mode {
        // Full mode: use PageRank-based ranking (slower, better quality)
        infiniloom_engine::rank_files(&mut repo);
        infiniloom_engine::sort_files_by_importance(&mut repo);
    } else {
        // Fast mode (default): use heuristic-based ranking
        rank_files_fast(&mut repo);
    }

    // Apply content transformations based on compression level and flags
    let should_remove_comments = remove_comments || matches!(compression, CompressionLevel::Balanced | CompressionLevel::Aggressive | CompressionLevel::Extreme);
    let should_remove_empty = remove_empty_lines || matches!(compression, CompressionLevel::Minimal | CompressionLevel::Balanced | CompressionLevel::Aggressive | CompressionLevel::Extreme);

    for file in &mut repo.files {
        if let Some(ref mut content) = file.content {
            // Remove empty lines if requested
            if should_remove_empty {
                *content = remove_empty_lines_from_content(content);
            }
            // Remove comments if requested
            if should_remove_comments {
                if let Some(lang) = &file.language {
                    *content = remove_comments_from_content(content, lang);
                }
            }
            // Truncate base64 content if requested
            if truncate_base64 {
                *content = truncate_base64_content(content);
            }
        }
    }

    // Run security scan if requested
    let security_issues = if security_check {
        if let Some(pb) = &pb {
            pb.set_message("Scanning for security issues...");
        }
        let scanner = SecurityScanner::new();
        let mut issues = Vec::new();
        for file in &repo.files {
            if let Some(content) = &file.content {
                let file_issues = scanner.scan(content, &file.relative_path);
                issues.extend(file_issues);
            }
        }
        Some(issues)
    } else {
        None
    };

    // Populate git history in Repository struct (for structured output in formatters)
    if include_logs || include_diffs {
        if let Ok(git_repo) = GitRepo::open(&repo_path) {
            use infiniloom_engine::types::{GitHistory, GitCommitInfo, GitChangedFile};

            let mut git_history = GitHistory::default();

            // Get recent commits if requested
            if include_logs {
                if let Ok(commits) = git_repo.log(logs_count) {
                    git_history.commits = commits.iter().map(|c| GitCommitInfo {
                        hash: c.hash.clone(),
                        short_hash: c.short_hash.clone(),
                        author: c.author.clone(),
                        date: c.date.clone(),
                        message: c.message.clone(),
                    }).collect();
                }
            }

            // Get uncommitted changes if requested
            if include_diffs {
                if let Ok(changed_files) = git_repo.status() {
                    git_history.changed_files = changed_files.iter().map(|f| {
                        let status = match f.status {
                            infiniloom_engine::git::FileStatus::Added => "A",
                            infiniloom_engine::git::FileStatus::Modified => "M",
                            infiniloom_engine::git::FileStatus::Deleted => "D",
                            infiniloom_engine::git::FileStatus::Renamed => "R",
                            infiniloom_engine::git::FileStatus::Copied => "C",
                            infiniloom_engine::git::FileStatus::Unknown => "?",
                        };
                        GitChangedFile {
                            path: f.path.clone(),
                            status: status.to_owned(),
                        }
                    }).collect();
                }
            }

            // Set git history on repo metadata
            repo.metadata.git_history = Some(git_history);

            if verbose {
                if let Some(pb) = &pb {
                    pb.set_message(format!("Loaded {} commits, {} changes",
                        repo.metadata.git_history.as_ref().map(|h| h.commits.len()).unwrap_or(0),
                        repo.metadata.git_history.as_ref().map(|h| h.changed_files.len()).unwrap_or(0)));
                }
            }
        } else if verbose {
            eprintln!("{} Not a git repository, skipping git history", "⚠".yellow());
        }
    }

    // Clear directory structure if --no-directory-structure was passed
    if !show_directory_structure {
        repo.metadata.directory_structure = None;
    }

    // Generate repo map
    let map = RepoMapGenerator::new(2000).generate(&repo);

    if let Some(pb) = &pb {
        pb.set_message("Generating output...");
    }

    // Format output with options
    let formatter = OutputFormatter::by_format_with_all_options(format, show_line_numbers, show_file_summary);
    let mut output_text = formatter.format(&repo, &map);

    // Prepend custom header if specified
    if let Some(header) = header_text {
        output_text = format!("{}\n\n{}", header, output_text);
    }

    // Include custom instructions from file
    if let Some(instr_path) = instruction_file {
        let instructions = std::fs::read_to_string(&instr_path)
            .with_context(|| format!("Failed to read instruction file: {}", instr_path.display()))?;
        output_text = format!("{}\n\n<!-- Custom Instructions -->\n{}\n\n", output_text, instructions);
    }

    // Add token tree if requested
    if token_tree {
        let mut tree = String::from("\n\n<!-- Token Count by File -->\n");
        tree.push_str("| File | Tokens |\n|------|--------|\n");
        for file in &repo.files {
            tree.push_str(&format!("| {} | {} |\n", file.relative_path, file.token_count.claude));
        }
        output_text.push_str(&tree);
    }

    // Add security issues if found
    if let Some(ref issues) = security_issues {
        if !issues.is_empty() {
            let mut sec_output = String::from("\n\n<!-- Security Scan Results -->\n");
            sec_output.push_str(&format!("⚠️ Found {} potential security issues:\n\n", issues.len()));
            for issue in issues {
                sec_output.push_str(&format!(
                    "- [{:?}] {} in {} (line {})\n",
                    issue.severity, issue.kind.name(), issue.file, issue.line
                ));
            }
            output_text.push_str(&sec_output);

            if verbose {
                eprintln!("{} Found {} security issues", "⚠".yellow(), issues.len());
            }
        } else if verbose {
            eprintln!("{} No security issues found", "✓".green());
        }
    }

    // Enforce max tokens limit
    if max_tokens > 0 {
        let current_tokens = estimate_tokens(&output_text, model);
        if current_tokens > max_tokens as usize {
            if verbose {
                eprintln!("{} Output exceeds token limit ({} > {}), truncating...",
                    "⚠".yellow(), current_tokens, max_tokens);
            }
            output_text = truncate_to_tokens(&output_text, max_tokens as usize, model);
        }
    }

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    // Copy to clipboard if requested
    if copy_to_clipboard {
        #[cfg(feature = "clipboard")]
        {
            use clipboard::{ClipboardContext, ClipboardProvider};
            if let Ok(mut ctx) = ClipboardContext::new() {
                let _ = ctx.set_contents(output_text.clone());
                if verbose {
                    eprintln!("{} Copied to clipboard", "✓".green());
                }
            }
        }
        #[cfg(not(feature = "clipboard"))]
        {
            eprintln!("{} Clipboard support not enabled. Build with --features clipboard", "⚠".yellow());
        }
    }

    // Write output
    if let Some(ref output_path) = output {
        std::fs::write(output_path, &output_text)
            .context("Failed to write output file")?;

        if verbose {
            let elapsed = start.elapsed();
            let total_lines: usize = repo.files.iter()
                .filter_map(|f| f.content.as_ref())
                .map(|c| c.lines().count())
                .sum();

            eprintln!();
            eprintln!("{}", "━".repeat(50).dimmed());
            eprintln!("{} Output written to: {}", "✓".green(), output_path.display());
            eprintln!("{}", "━".repeat(50).dimmed());
            eprintln!("  {} {} files", "📁".dimmed(), repo.files.len());
            eprintln!("  {} {} lines", "📄".dimmed(), total_lines);
            eprintln!("  {} {}", "📦".dimmed(), format_size(output_text.len() as u64, BINARY));
            eprintln!("  {} ~{} tokens ({})", "🔢".dimmed(), repo.total_tokens(model), model.name());
            eprintln!("  {} {:?}", "⏱️ ".dimmed(), elapsed);

            // Show language breakdown if available
            if !repo.metadata.languages.is_empty() {
                eprintln!();
                eprintln!("  {}:", "Languages".cyan());
                for lang in repo.metadata.languages.iter().take(5) {
                    eprintln!("    {} {}: {} files ({:.1}%)",
                        "•".dimmed(), lang.language, lang.files, lang.percentage);
                }
            }
            eprintln!();
        }
    } else {
        print!("{}", output_text);
    }

    // Handle watch mode
    if watch_mode {
        if output.is_none() {
            eprintln!("{} Watch mode requires --output to be specified", "Error:".red().bold());
            std::process::exit(1);
        }

        let output_path = output.as_ref().unwrap().clone();
        eprintln!();
        eprintln!("{} Watching for file changes... (Ctrl+C to stop)", "👀".cyan());

        use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher, Event};
        use std::sync::mpsc::channel;
        use std::time::Duration;

        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                        let _ = tx.send(());
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        ).context("Failed to create file watcher")?;

        watcher.watch(&repo_path, RecursiveMode::Recursive)
            .context("Failed to watch directory")?;

        // Debounce: wait for changes to settle
        let debounce_duration = Duration::from_millis(500);
        let mut last_rebuild = Instant::now();

        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(()) => {
                    // Debounce - wait for changes to settle
                    if last_rebuild.elapsed() < debounce_duration {
                        continue;
                    }

                    eprintln!("{} Change detected, regenerating...", "🔄".yellow());

                    // Re-run the pack logic
                    let rebuild_start = Instant::now();

                    // Re-scan repository
                    let scan_config = scanner::ScanConfig {
                        include_hidden,
                        respect_gitignore,
                        read_contents: true,
                        max_file_size: 50 * 1024 * 1024,
                        skip_symbols: !enable_symbols,
                        ..Default::default()
                    };

                    if let Ok(mut new_repo) = scanner::scan_repository(&repo_path, scan_config) {
                        // Re-apply transformations
                        if full_mode {
                            infiniloom_engine::rank_files(&mut new_repo);
                            infiniloom_engine::sort_files_by_importance(&mut new_repo);
                        } else {
                            rank_files_fast(&mut new_repo);
                        }

                        let new_map = RepoMapGenerator::new(2000).generate(&new_repo);
                        let new_formatter = OutputFormatter::by_format_with_options(format, show_line_numbers);
                        let new_output = new_formatter.format(&new_repo, &new_map);

                        if let Err(e) = std::fs::write(&output_path, &new_output) {
                            eprintln!("{} Failed to write output: {}", "Error:".red(), e);
                        } else {
                            eprintln!("{} Regenerated in {:?} ({} files, ~{} tokens)",
                                "✓".green(),
                                rebuild_start.elapsed(),
                                new_repo.files.len(),
                                new_repo.total_tokens(model)
                            );
                        }
                    }

                    last_rebuild = Instant::now();
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Just keep watching
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }
    }

    Ok(())
}

fn cmd_scan(
    path: PathBuf,
    model: TokenizerModel,
    include_hidden: bool,
    verbose: bool,
    json_output: bool,
) -> Result<()> {
    let start = Instant::now();

    let config = scanner::ScanConfig {
        include_hidden,
        respect_gitignore: true,
        read_contents: false, // Don't need content for stats
        max_file_size: 50 * 1024 * 1024,
        skip_symbols: true, // No need for symbols in scan mode
        ..Default::default()
    };

    let repo = scanner::scan_repository(&path, config)
        .context("Failed to scan repository")?;

    let elapsed = start.elapsed();

    if json_output {
        // JSON output
        let stats = serde_json::json!({
            "repository": repo.name,
            "files": repo.files.len(),
            "total_bytes": repo.files.iter().map(|f| f.size_bytes).sum::<u64>(),
            "total_tokens": {
                "claude": repo.total_tokens(TokenizerModel::Claude),
                "gpt4o": repo.total_tokens(TokenizerModel::Gpt4o),
                "gemini": repo.total_tokens(TokenizerModel::Gemini),
            },
            "languages": repo.metadata.languages,
            "scan_time_ms": elapsed.as_millis(),
        });
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        // Human-readable output
        println!();
        println!("{}", "━".repeat(50).dimmed());
        println!("  {}", "Scan Results".cyan().bold());
        println!("{}", "━".repeat(50).dimmed());
        println!();

        println!("  Repository:   {}", repo.name.yellow());
        println!("  Path:         {}", path.display());
        println!("  Files:        {}", repo.files.len());

        let total_bytes: u64 = repo.files.iter().map(|f| f.size_bytes).sum();
        println!("  Total Size:   {}", format_size(total_bytes, BINARY));
        println!("  Scan Time:    {:?}", elapsed);
        println!();

        // Language breakdown
        if !repo.metadata.languages.is_empty() {
            println!("  {}:", "Languages".cyan());
            for lang in &repo.metadata.languages {
                println!(
                    "    {}: {} files ({:.1}%)",
                    lang.language, lang.files, lang.percentage
                );
            }
            println!();
        }

        // Token estimates
        println!("  {} ({}):", "Token Estimates".cyan(), model.name());
        println!("    Total: ~{}", repo.total_tokens(model));
        println!();

        // Verbose file list
        if verbose {
            println!("  {}:", "Files".cyan());
            let mut files: Vec<_> = repo.files.iter().collect();
            files.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

            for file in files.iter().take(20) {
                let lang = file.language.as_deref().unwrap_or("?");
                println!(
                    "    {} ({}) - {}",
                    file.relative_path,
                    lang,
                    format_size(file.size_bytes, BINARY)
                );
            }

            if files.len() > 20 {
                println!("    ... and {} more files", files.len() - 20);
            }
            println!();
        }
    }

    Ok(())
}

fn cmd_map(path: PathBuf, budget: u32, output: Option<PathBuf>) -> Result<()> {
    let config = scanner::ScanConfig {
        include_hidden: false,
        respect_gitignore: true,
        read_contents: true,
        max_file_size: 50 * 1024 * 1024,
        skip_symbols: false, // Map command needs symbols for ranking
        ..Default::default()
    };

    let mut repo = scanner::scan_repository(&path, config)
        .context("Failed to scan repository")?;

    // Rank files by importance
    infiniloom_engine::rank_files(&mut repo);
    infiniloom_engine::sort_files_by_importance(&mut repo);

    let map = RepoMapGenerator::new(budget).generate(&repo);

    let output_text = map.summary.clone();

    if let Some(output_path) = output {
        std::fs::write(&output_path, &output_text)
            .context("Failed to write output file")?;
        eprintln!("Repository map written to: {}", output_path.display());
    } else {
        println!("{}", output_text);
    }

    Ok(())
}

fn cmd_info() -> Result<()> {
    println!();
    println!("{}", "Infiniloom - Repository Context Generator".cyan().bold());
    println!("{}", "━".repeat(50).dimmed());
    println!();
    println!("  Version:      {}", env!("CARGO_PKG_VERSION"));
    println!("  Engine:       {}", infiniloom_engine::VERSION);
    println!();
    println!("  {}:", "Supported Formats".yellow());
    println!("    xml       - Claude-optimized (with cache hints)");
    println!("    markdown  - GPT-optimized (with code blocks)");
    println!("    json      - Generic structured format");
    println!("    yaml      - Gemini-optimized (query at end)");
    println!("    toon      - Most token-efficient (~40% smaller)");
    println!("    plain     - Simple plain text (no markup)");
    println!();
    println!("  {}:", "Supported Models".yellow());
    println!("    claude    - Anthropic Claude (default)");
    println!("    gpt4o     - OpenAI GPT-4o");
    println!("    gpt4      - OpenAI GPT-4");
    println!("    gemini    - Google Gemini");
    println!("    llama     - Meta Llama");
    println!();
    println!("  {}:", "Compression Levels".yellow());
    println!("    none      - No compression (0%)");
    println!("    minimal   - Whitespace only (~15%)");
    println!("    balanced  - Remove comments (~35%)");
    println!("    aggressive - Signatures only (~60%)");
    println!("    extreme   - Key symbols only (~80%)");
    println!();

    Ok(())
}

fn cmd_init(format: ConfigFormat, output: Option<PathBuf>, force: bool) -> Result<()> {
    let (ext, format_name) = match format {
        ConfigFormat::Yaml => ("yaml", "yaml"),
        ConfigFormat::Toml => ("toml", "toml"),
        ConfigFormat::Json => ("json", "json"),
    };

    let output_path = output.unwrap_or_else(|| PathBuf::from(format!(".infiniloom.{}", ext)));

    // Check if file exists
    if output_path.exists() && !force {
        eprintln!(
            "{} Configuration file already exists: {}",
            "Error:".red().bold(),
            output_path.display()
        );
        eprintln!("Use --force to overwrite");
        std::process::exit(1);
    }

    // Generate default config
    let config_content = infiniloom_engine::Config::generate_default(format_name);

    // Write config file
    std::fs::write(&output_path, &config_content)
        .with_context(|| format!("Failed to write config file: {}", output_path.display()))?;

    println!("{} Created configuration file: {}", "✓".green(), output_path.display());
    println!();
    println!("Edit this file to customize Infiniloom behavior.");
    println!("See https://github.com/homotopylabs/infiniloom#configuration for options.");

    Ok(())
}

/// Truncate base64 encoded content in a string
/// This helps reduce token count when files contain embedded binary data
fn truncate_base64_content(content: &str) -> String {
    // Common base64 patterns (data URIs, embedded content)
    let base64_pattern = regex::Regex::new(
        r"(?:data:[^;]+;base64,|[A-Za-z0-9+/]{100,}={0,2})"
    ).ok();

    if let Some(re) = base64_pattern {
        re.replace_all(content, |caps: &regex::Captures<'_>| {
            let matched = caps.get(0).map_or("", |m| m.as_str());
            if matched.starts_with("data:") {
                // Data URI - keep prefix, truncate data
                if let Some(comma_idx) = matched.find(',') {
                    let prefix = &matched[..comma_idx + 1];
                    format!("{}[BASE64_TRUNCATED]", prefix)
                } else {
                    "[BASE64_TRUNCATED]".to_owned()
                }
            } else if matched.len() > 100 {
                // Long base64 string
                format!("{}...[BASE64_TRUNCATED]", &matched[..50])
            } else {
                matched.to_owned()
            }
        }).to_string()
    } else {
        // Fallback: simple pattern matching without regex
        let mut result = String::new();
        let mut in_base64 = false;

        for line in content.lines() {
            // Check if line looks like base64 (only valid chars, long)
            let is_base64_line = line.len() > 76 &&
                line.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=');

            if is_base64_line {
                if !in_base64 {
                    result.push_str(&line[..50.min(line.len())]);
                    result.push_str("...[BASE64_TRUNCATED]\n");
                    in_base64 = true;
                }
            } else {
                in_base64 = false;
                result.push_str(line);
                result.push('\n');
            }
        }
        result
    }
}

/// Remove empty lines from content
fn remove_empty_lines_from_content(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove comments from code based on language
fn remove_comments_from_content(content: &str, language: &str) -> String {
    let (line_comment, block_start, block_end) = match language.to_lowercase().as_str() {
        "python" | "ruby" | "shell" | "bash" | "sh" | "yaml" | "yml" => ("#", "", ""),
        "javascript" | "typescript" | "java" | "c" | "cpp" | "c++" | "rust" | "go" | "swift" | "kotlin" | "scala" => ("//", "/*", "*/"),
        "html" | "xml" => ("", "<!--", "-->"),
        "css" | "scss" | "sass" => ("", "/*", "*/"),
        "sql" => ("--", "/*", "*/"),
        "lua" => ("--", "--[[", "]]"),
        _ => ("//", "/*", "*/"), // Default to C-style
    };

    let mut result = String::new();
    let mut in_block_comment = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Handle block comments
        if !block_start.is_empty() && !block_end.is_empty() {
            if in_block_comment {
                if let Some(idx) = line.find(block_end) {
                    in_block_comment = false;
                    let after_block = &line[idx + block_end.len()..];
                    if !after_block.trim().is_empty() {
                        result.push_str(after_block);
                        result.push('\n');
                    }
                }
                continue;
            }

            if let Some(idx) = line.find(block_start) {
                // Check if block comment ends on same line
                if let Some(end_idx) = line[idx + block_start.len()..].find(block_end) {
                    let before = &line[..idx];
                    let after = &line[idx + block_start.len() + end_idx + block_end.len()..];
                    let combined = format!("{}{}", before.trim_end(), after);
                    if !combined.trim().is_empty() {
                        result.push_str(&combined);
                        result.push('\n');
                    }
                    continue;
                } else {
                    in_block_comment = true;
                    let before = &line[..idx];
                    if !before.trim().is_empty() {
                        result.push_str(before.trim_end());
                        result.push('\n');
                    }
                    continue;
                }
            }
        }

        // Handle line comments (simple approach - may not handle strings perfectly)
        if !line_comment.is_empty() && trimmed.starts_with(line_comment) {
            continue;
        }

        // Try to remove trailing line comments
        if !line_comment.is_empty() {
            if let Some(idx) = line.find(line_comment) {
                // Simple heuristic: skip if inside a string
                let before = &line[..idx];
                let quote_count = before.matches('"').count() + before.matches('\'').count();
                if quote_count % 2 == 0 {
                    let cleaned = before.trim_end();
                    if !cleaned.is_empty() {
                        result.push_str(cleaned);
                        result.push('\n');
                    }
                    continue;
                }
            }
        }

        result.push_str(line);
        result.push('\n');
    }

    result
}

/// Estimate token count for text using model-specific estimation
fn estimate_tokens(text: &str, model: TokenizerModel) -> usize {
    // Use model-specific ratio (approximate)
    let char_ratio = match model {
        TokenizerModel::Claude => 4.0,
        TokenizerModel::Gpt4o => 4.0,
        TokenizerModel::Gpt4 => 4.0,
        TokenizerModel::Gemini => 4.2,
        TokenizerModel::Llama => 3.8,
    };
    (text.len() as f64 / char_ratio) as usize
}

/// Truncate text to fit within token limit
fn truncate_to_tokens(text: &str, max_tokens: usize, model: TokenizerModel) -> String {
    let current = estimate_tokens(text, model);
    if current <= max_tokens {
        return text.to_owned();
    }

    // Estimate how many characters we need
    let ratio = max_tokens as f64 / current as f64;
    let target_chars = (text.len() as f64 * ratio * 0.95) as usize; // 5% buffer

    // Try to truncate at a sensible boundary (file boundary in output)
    let truncated = &text[..target_chars.min(text.len())];

    // Find last complete file section (look for file markers)
    let markers = ["</file>", "```\n\n", "----------------------------------------\n", "\n---\n"];
    let mut best_end = truncated.len();

    for marker in markers {
        if let Some(pos) = truncated.rfind(marker) {
            let end_pos = pos + marker.len();
            if end_pos > truncated.len() / 2 {
                best_end = end_pos;
                break;
            }
        }
    }

    let mut result = truncated[..best_end].to_string();
    result.push_str("\n\n<!-- Output truncated to fit token limit -->\n");
    result
}

/// Fast heuristic-based file ranking (no symbol extraction needed)
/// This is the default mode - much faster than PageRank-based ranking
fn rank_files_fast(repo: &mut infiniloom_engine::Repository) {
    repo.files.sort_by_key(|f| {
        let path = &f.relative_path;
        let mut score: i32 = 1000; // Base score

        // === CRITICAL: Entry points (highest priority) ===
        let entry_point_patterns = [
            "main.rs", "main.go", "main.py", "main.ts", "main.js", "main.c", "main.cpp",
            "index.ts", "index.js", "index.tsx", "index.jsx", "index.py",
            "app.py", "app.ts", "app.js", "app.tsx", "app.jsx", "app.go",
            "server.py", "server.ts", "server.js", "server.go",
            "mod.rs", "lib.rs", "lib.py",
            "__main__.py", "__init__.py",
        ];
        if entry_point_patterns.iter().any(|p| path.ends_with(p)) {
            score -= 5000;
        }

        // === HIGH: Config and manifest files ===
        let config_patterns = [
            "Cargo.toml", "package.json", "pyproject.toml", "go.mod", "pom.xml",
            "build.gradle", "Gemfile", "requirements.txt", "setup.py", "setup.cfg",
            "tsconfig.json", "webpack.config", "vite.config", "next.config",
            "Makefile", "CMakeLists.txt", "Dockerfile", "docker-compose",
            ".env.example",
        ];
        if config_patterns.iter().any(|p| path.contains(p)) {
            score -= 3000;
        }

        // === MEDIUM-HIGH: Source directories ===
        if path.starts_with("src/") || path.starts_with("lib/") || path.starts_with("pkg/") {
            score -= 1000;
        }

        // === MEDIUM: API/Routes/Models ===
        let important_patterns = ["api/", "routes/", "models/", "controllers/", "services/", "handlers/"];
        if important_patterns.iter().any(|p| path.contains(p)) {
            score -= 500;
        }

        // === LOW: Tests (if included) ===
        let test_patterns = ["/test", "_test.", ".test.", ".spec.", "tests/", "__tests__/"];
        if test_patterns.iter().any(|p| path.contains(p)) {
            score += 2000;
        }

        // === LOWER: Examples, benchmarks, scripts ===
        let auxiliary_patterns = ["examples/", "example/", "benchmarks/", "bench/", "scripts/", "tools/"];
        if auxiliary_patterns.iter().any(|p| path.contains(p)) {
            score += 1500;
        }

        // === LOWEST: Vendored, generated, docs ===
        let low_priority_patterns = ["vendor/", "third_party/", "generated/", "docs/", "doc/"];
        if low_priority_patterns.iter().any(|p| path.contains(p)) {
            score += 3000;
        }

        // Prefer shallower paths (fewer slashes = more important)
        score += (path.matches('/').count() as i32) * 50;

        // Prefer shorter filenames
        if let Some(name) = path.rsplit('/').next() {
            score += (name.len() as i32) / 5;
        }

        score
    });

    // Update importance field based on new order
    let total = repo.files.len() as f32;
    for (i, file) in repo.files.iter_mut().enumerate() {
        file.importance = 1.0 - (i as f32 / total);
    }
}

/// Loaded configuration from file
#[derive(Default)]
struct LoadedConfig {
    /// Additional exclude patterns from config
    exclude_patterns: Vec<String>,
    /// Additional include patterns from config
    #[allow(dead_code)]
    include_patterns: Vec<String>,
}

/// Load config file (.infiniloom.yaml, .infiniloom.toml, .infiniloom.json)
fn load_config_file(config_path: Option<&PathBuf>, repo_path: &std::path::Path) -> LoadedConfig {
    let mut config = LoadedConfig::default();

    // Try to load specified config file
    if let Some(path) = config_path {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                parse_config_content(&content, path, &mut config);
            }
        }
        return config;
    }

    // Look for default config files
    let config_files = [".infiniloom.yaml", ".infiniloom.yml", ".infiniloom.toml", ".infiniloom.json"];
    for name in config_files {
        let path = repo_path.join(name);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                parse_config_content(&content, &path, &mut config);
                break;
            }
        }
    }

    // Also load .infiniloomignore patterns
    let ignore_path = repo_path.join(".infiniloomignore");
    if ignore_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&ignore_path) {
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    config.exclude_patterns.push(line.to_owned());
                }
            }
        }
    }

    config
}

/// Parse config content based on file extension
fn parse_config_content(content: &str, path: &std::path::Path, config: &mut LoadedConfig) {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "yaml" | "yml" => {
            // Simple YAML parsing for ignore patterns
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("- ") && !line.contains(':') {
                    // This is likely an array item in ignore section
                    let pattern = line.trim_start_matches("- ").trim();
                    if !pattern.is_empty() {
                        config.exclude_patterns.push(pattern.to_owned());
                    }
                }
            }
        }
        "toml" => {
            // Simple TOML parsing
            let mut in_ignore_section = false;
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("[ignore]") || line.starts_with("[exclude]") {
                    in_ignore_section = true;
                } else if line.starts_with('[') {
                    in_ignore_section = false;
                } else if in_ignore_section && line.starts_with('"') {
                    let pattern = line.trim_matches(|c| c == '"' || c == ',' || c == ' ');
                    if !pattern.is_empty() {
                        config.exclude_patterns.push(pattern.to_owned());
                    }
                }
            }
        }
        "json" => {
            // Simple JSON parsing for ignore array
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(content) {
                if let Some(ignore) = value.get("ignore").or_else(|| value.get("exclude")) {
                    if let Some(arr) = ignore.as_array() {
                        for item in arr {
                            if let Some(s) = item.as_str() {
                                config.exclude_patterns.push(s.to_owned());
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}
