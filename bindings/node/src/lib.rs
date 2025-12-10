#![deny(clippy::all)]

use infiniloom_engine::{
    CompressionLevel, OutputFormat, OutputFormatter, RepoMap, RepoMapGenerator, Repository,
    SecurityScanner, TokenizerModel,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::path::PathBuf;

mod scanner;
use scanner::{scan_repository as do_scan, ScanConfig};

/// Options for packing a repository
#[napi(object)]
pub struct PackOptions {
    /// Output format: "xml", "markdown", or "json"
    pub format: Option<String>,
    /// Target model: "claude", "gpt-4o", "gpt-4", "gemini", or "llama"
    pub model: Option<String>,
    /// Compression level: "none", "minimal", "balanced", "aggressive", "extreme", "semantic"
    pub compression: Option<String>,
    /// Token budget for repository map
    pub map_budget: Option<u32>,
    /// Maximum number of symbols in map
    pub max_symbols: Option<u32>,
    /// Skip security scanning
    pub skip_security: Option<bool>,
}

/// Statistics from scanning a repository
#[napi(object)]
pub struct ScanStats {
    /// Repository name
    pub name: String,
    /// Total number of files
    pub total_files: u32,
    /// Total lines of code
    pub total_lines: u32,
    /// Total tokens for target model
    pub total_tokens: u32,
    /// Primary language
    pub primary_language: Option<String>,
    /// Language breakdown
    pub languages: Vec<LanguageStat>,
    /// Number of security findings
    pub security_findings: u32,
}

/// Statistics for a single language
#[napi(object)]
pub struct LanguageStat {
    /// Language name
    pub language: String,
    /// Number of files
    pub files: u32,
    /// Total lines
    pub lines: u32,
    /// Percentage of codebase
    pub percentage: f64,
}

/// Pack a repository into optimized LLM context
///
/// # Arguments
/// * `path` - Path to repository root
/// * `options` - Optional packing options
///
/// # Returns
/// Formatted repository context as a string
///
/// # Example
/// ```javascript
/// const { pack } = require('@infiniloom/node');
///
/// const context = pack('./my-repo', {
///   format: 'xml',
///   model: 'claude',
///   compression: 'balanced',
///   map_budget: 2000
/// });
/// ```
#[napi]
pub fn pack(path: String, options: Option<PackOptions>) -> Result<String> {
    let opts = options.unwrap_or(PackOptions {
        format: None,
        model: None,
        compression: None,
        map_budget: None,
        max_symbols: None,
        skip_security: None,
    });

    // Parse options
    let format = parse_format(opts.format.as_deref())?;
    let model = parse_model(opts.model.as_deref())?;
    let compression = parse_compression(opts.compression.as_deref())?;
    let map_budget = opts.map_budget.unwrap_or(2000);
    let max_symbols = opts.max_symbols.unwrap_or(50);
    let skip_security = opts.skip_security.unwrap_or(false);

    // Scan repository (with contents for packing)
    let repo = scan_repository(&path, model, true)?;

    // Security check
    if !skip_security {
        let scanner = SecurityScanner::new();
        for file in &repo.files {
            if let Some(content) = &file.content {
                let findings = scanner.scan(content, &file.relative_path);
                if findings.iter().any(|f| {
                    matches!(
                        f.severity,
                        infiniloom_engine::security::Severity::Critical
                    )
                }) {
                    return Err(Error::new(
                        Status::GenericFailure,
                        format!(
                            "Critical security issues found in {}. Use skip_security: true to override.",
                            file.relative_path
                        ),
                    ));
                }
            }
        }
    }

    // Generate repository map
    let generator = RepoMapGenerator::new(map_budget)
        .with_max_symbols(max_symbols as usize)
        .with_model(model);
    let map = generator.generate(&repo);

    // Format output
    let formatter = OutputFormatter::by_format(format);
    let output = formatter.format(&repo, &map);

    Ok(output)
}

/// Scan a repository and return statistics
///
/// # Arguments
/// * `path` - Path to repository root
/// * `model` - Optional target model (default: "claude")
///
/// # Returns
/// Repository statistics
///
/// # Example
/// ```javascript
/// const { scan } = require('@infiniloom/node');
///
/// const stats = scan('./my-repo', 'claude');
/// console.log(`Total files: ${stats.total_files}`);
/// console.log(`Total tokens: ${stats.total_tokens}`);
/// ```
#[napi]
pub fn scan(path: String, model: Option<String>) -> Result<ScanStats> {
    let tokenizer_model = parse_model(model.as_deref())?;
    let repo = scan_repository(&path, tokenizer_model, false)?;

    // Security scan
    let scanner = SecurityScanner::new();
    let mut total_findings = 0;
    for file in &repo.files {
        if let Some(content) = &file.content {
            let findings = scanner.scan(content, &file.relative_path);
            total_findings += findings.len();
        }
    }

    Ok(ScanStats {
        name: repo.name.clone(),
        total_files: repo.metadata.total_files,
        total_lines: repo.metadata.total_lines as u32,
        total_tokens: repo.total_tokens(tokenizer_model),
        primary_language: repo
            .metadata
            .languages
            .first()
            .map(|l| l.language.clone()),
        languages: repo
            .metadata
            .languages
            .iter()
            .map(|l| LanguageStat {
                language: l.language.clone(),
                files: l.files,
                lines: l.lines as u32,
                percentage: l.percentage as f64,
            })
            .collect(),
        security_findings: total_findings as u32,
    })
}

/// Count tokens in text for a specific model
///
/// # Arguments
/// * `text` - Text to tokenize
/// * `model` - Optional model name (default: "claude")
///
/// # Returns
/// Token count
///
/// # Example
/// ```javascript
/// const { countTokens } = require('@infiniloom/node');
///
/// const count = countTokens('Hello, world!', 'claude');
/// console.log(`Tokens: ${count}`);
/// ```
#[napi]
pub fn count_tokens(text: String, model: Option<String>) -> Result<u32> {
    let tokenizer_model = parse_model(model.as_deref())?;

    // Simple approximation: ~4 chars per token
    // In a real implementation, use tiktoken or similar
    let approx_tokens = match tokenizer_model {
        TokenizerModel::Claude => (text.len() as f32 / 3.5) as u32,
        TokenizerModel::Gpt4o | TokenizerModel::Gpt4 => (text.len() as f32 / 4.0) as u32,
        TokenizerModel::Gemini => (text.len() as f32 / 4.2) as u32,
        TokenizerModel::Llama => (text.len() as f32 / 3.8) as u32,
    };

    Ok(approx_tokens)
}

/// Infiniloom class for advanced usage
#[napi]
pub struct Infiniloom {
    repo: Repository,
    model: TokenizerModel,
}

#[napi]
impl Infiniloom {
    /// Create a new Infiniloom instance
    ///
    /// # Arguments
    /// * `path` - Path to repository root
    /// * `model` - Optional model name (default: "claude")
    #[napi(constructor)]
    pub fn new(path: String, model: Option<String>) -> Result<Self> {
        let tokenizer_model = parse_model(model.as_deref())?;
        let repo = scan_repository(&path, tokenizer_model, true)?;

        Ok(Self {
            repo,
            model: tokenizer_model,
        })
    }

    /// Get repository statistics
    #[napi]
    pub fn get_stats(&self) -> ScanStats {
        let scanner = SecurityScanner::new();
        let mut total_findings = 0;
        for file in &self.repo.files {
            if let Some(content) = &file.content {
                let findings = scanner.scan(content, &file.relative_path);
                total_findings += findings.len();
            }
        }

        ScanStats {
            name: self.repo.name.clone(),
            total_files: self.repo.metadata.total_files,
            total_lines: self.repo.metadata.total_lines as u32,
            total_tokens: self.repo.total_tokens(self.model),
            primary_language: self
                .repo
                .metadata
                .languages
                .first()
                .map(|l| l.language.clone()),
            languages: self
                .repo
                .metadata
                .languages
                .iter()
                .map(|l| LanguageStat {
                    language: l.language.clone(),
                    files: l.files,
                    lines: l.lines as u32,
                    percentage: l.percentage as f64,
                })
                .collect(),
            security_findings: total_findings as u32,
        }
    }

    /// Generate a repository map
    ///
    /// # Arguments
    /// * `budget` - Token budget (default: 2000)
    /// * `max_symbols` - Maximum symbols (default: 50)
    #[napi]
    pub fn generate_map(&self, budget: Option<u32>, max_symbols: Option<u32>) -> Result<String> {
        let token_budget = budget.unwrap_or(2000);
        let max_syms = max_symbols.unwrap_or(50);

        let generator = RepoMapGenerator::new(token_budget)
            .with_max_symbols(max_syms as usize)
            .with_model(self.model);

        let map = generator.generate(&self.repo);

        serde_json::to_string_pretty(&map)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Pack repository with specific options
    #[napi]
    pub fn pack(&self, options: Option<PackOptions>) -> Result<String> {
        let opts = options.unwrap_or(PackOptions {
            format: None,
            model: None,
            compression: None,
            map_budget: None,
            max_symbols: None,
            skip_security: None,
        });

        let format = parse_format(opts.format.as_deref())?;
        let compression = parse_compression(opts.compression.as_deref())?;
        let map_budget = opts.map_budget.unwrap_or(2000);
        let max_symbols = opts.max_symbols.unwrap_or(50);

        let generator = RepoMapGenerator::new(map_budget)
            .with_max_symbols(max_symbols as usize)
            .with_model(self.model);

        let map = generator.generate(&self.repo);
        let formatter = OutputFormatter::by_format(format);

        Ok(formatter.format(&self.repo, &map))
    }

    /// Check for security issues
    #[napi]
    pub fn security_scan(&self) -> Result<Vec<String>> {
        let scanner = SecurityScanner::new();
        let mut findings = Vec::new();

        for file in &self.repo.files {
            if let Some(content) = &file.content {
                let file_findings = scanner.scan(content, &file.relative_path);
                for finding in file_findings {
                    findings.push(format!(
                        "{} in {} at line {}: {}",
                        finding.kind.name(),
                        finding.file,
                        finding.line,
                        finding.pattern
                    ));
                }
            }
        }

        Ok(findings)
    }
}

// Helper functions

fn parse_format(format: Option<&str>) -> Result<OutputFormat> {
    match format.unwrap_or("xml") {
        "xml" => Ok(OutputFormat::Xml),
        "markdown" | "md" => Ok(OutputFormat::Markdown),
        "json" => Ok(OutputFormat::Json),
        other => Err(Error::new(
            Status::InvalidArg,
            format!("Unknown format: {}. Use 'xml', 'markdown', or 'json'", other),
        )),
    }
}

fn parse_model(model: Option<&str>) -> Result<TokenizerModel> {
    match model.unwrap_or("claude") {
        "claude" => Ok(TokenizerModel::Claude),
        "gpt-4o" | "gpt4o" => Ok(TokenizerModel::Gpt4o),
        "gpt-4" | "gpt4" => Ok(TokenizerModel::Gpt4),
        "gemini" => Ok(TokenizerModel::Gemini),
        "llama" => Ok(TokenizerModel::Llama),
        other => Err(Error::new(
            Status::InvalidArg,
            format!(
                "Unknown model: {}. Use 'claude', 'gpt-4o', 'gpt-4', 'gemini', or 'llama'",
                other
            ),
        )),
    }
}

fn parse_compression(compression: Option<&str>) -> Result<CompressionLevel> {
    match compression.unwrap_or("balanced") {
        "none" => Ok(CompressionLevel::None),
        "minimal" => Ok(CompressionLevel::Minimal),
        "balanced" => Ok(CompressionLevel::Balanced),
        "aggressive" => Ok(CompressionLevel::Aggressive),
        "extreme" => Ok(CompressionLevel::Extreme),
        "semantic" => Ok(CompressionLevel::Semantic),
        other => Err(Error::new(
            Status::InvalidArg,
            format!(
                "Unknown compression: {}. Use 'none', 'minimal', 'balanced', 'aggressive', 'extreme', or 'semantic'",
                other
            ),
        )),
    }
}

fn scan_repository(path: &str, _model: TokenizerModel, read_contents: bool) -> Result<Repository> {
    let path_buf = PathBuf::from(path);

    if !path_buf.exists() {
        return Err(Error::new(
            Status::InvalidArg,
            format!("Path does not exist: {}", path),
        ));
    }

    let config = ScanConfig {
        include_hidden: false,
        respect_gitignore: true,
        read_contents,
        max_file_size: 50 * 1024 * 1024, // 50MB
    };

    do_scan(&path_buf, config).map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
}
