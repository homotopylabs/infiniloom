//! Core type definitions for Infiniloom

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A scanned repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    /// Repository name (usually directory name)
    pub name: String,
    /// Absolute path to repository root
    pub path: PathBuf,
    /// List of files in the repository
    pub files: Vec<RepoFile>,
    /// Repository metadata and statistics
    pub metadata: RepoMetadata,
}

impl Repository {
    /// Create a new empty repository
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            files: Vec::new(),
            metadata: RepoMetadata::default(),
        }
    }

    /// Get total token count for a specific model
    pub fn total_tokens(&self, model: TokenizerModel) -> u32 {
        self.files.iter().map(|f| f.token_count.get(model)).sum()
    }

    /// Get files filtered by language
    pub fn files_by_language(&self, language: &str) -> Vec<&RepoFile> {
        self.files
            .iter()
            .filter(|f| f.language.as_deref() == Some(language))
            .collect()
    }

    /// Get files sorted by importance
    pub fn files_by_importance(&self) -> Vec<&RepoFile> {
        let mut files: Vec<_> = self.files.iter().collect();
        files.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap());
        files
    }
}

/// A single file in the repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoFile {
    /// Absolute path to file
    pub path: PathBuf,
    /// Path relative to repository root
    pub relative_path: String,
    /// Detected programming language
    pub language: Option<String>,
    /// File size in bytes
    pub size_bytes: u64,
    /// Token counts for different models
    pub token_count: TokenCounts,
    /// Extracted symbols (functions, classes, etc.)
    pub symbols: Vec<Symbol>,
    /// Calculated importance score (0.0 - 1.0)
    pub importance: f32,
    /// File content (may be None to save memory)
    pub content: Option<String>,
}

impl RepoFile {
    /// Create a new file entry
    pub fn new(path: impl Into<PathBuf>, relative_path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            relative_path: relative_path.into(),
            language: None,
            size_bytes: 0,
            token_count: TokenCounts::default(),
            symbols: Vec::new(),
            importance: 0.5,
            content: None,
        }
    }

    /// Get file extension
    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(|e| e.to_str())
    }

    /// Get filename without path
    pub fn filename(&self) -> &str {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
    }
}

/// Token counts for multiple models
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TokenCounts {
    pub claude: u32,
    pub gpt4o: u32,
    pub gpt4: u32,
    pub gemini: u32,
    pub llama: u32,
}

impl TokenCounts {
    /// Get token count for a specific model
    pub fn get(&self, model: TokenizerModel) -> u32 {
        match model {
            TokenizerModel::Claude => self.claude,
            TokenizerModel::Gpt4o => self.gpt4o,
            TokenizerModel::Gpt4 => self.gpt4,
            TokenizerModel::Gemini => self.gemini,
            TokenizerModel::Llama => self.llama,
        }
    }

    /// Set token count for a specific model
    pub fn set(&mut self, model: TokenizerModel, count: u32) {
        match model {
            TokenizerModel::Claude => self.claude = count,
            TokenizerModel::Gpt4o => self.gpt4o = count,
            TokenizerModel::Gpt4 => self.gpt4 = count,
            TokenizerModel::Gemini => self.gemini = count,
            TokenizerModel::Llama => self.llama = count,
        }
    }
}

/// Supported tokenizer models
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenizerModel {
    Claude,
    Gpt4o,
    Gpt4,
    Gemini,
    Llama,
}

impl TokenizerModel {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Gpt4o => "gpt-4o",
            Self::Gpt4 => "gpt-4",
            Self::Gemini => "gemini",
            Self::Llama => "llama",
        }
    }
}

/// A code symbol (function, class, variable, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: SymbolKind,
    /// Function/method signature (if applicable)
    pub signature: Option<String>,
    /// Documentation string
    pub docstring: Option<String>,
    /// Starting line number (1-indexed)
    pub start_line: u32,
    /// Ending line number (1-indexed)
    pub end_line: u32,
    /// Number of references to this symbol
    pub references: u32,
    /// Calculated importance (0.0 - 1.0)
    pub importance: f32,
    /// Parent symbol name (for methods inside classes)
    pub parent: Option<String>,
}

impl Symbol {
    /// Create a new symbol
    pub fn new(name: impl Into<String>, kind: SymbolKind) -> Self {
        Self {
            name: name.into(),
            kind,
            signature: None,
            docstring: None,
            start_line: 0,
            end_line: 0,
            references: 0,
            importance: 0.5,
            parent: None,
        }
    }

    /// Get line count
    pub fn line_count(&self) -> u32 {
        if self.end_line >= self.start_line {
            self.end_line - self.start_line + 1
        } else {
            1
        }
    }
}

/// Kind of code symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Interface,
    Struct,
    Enum,
    Constant,
    Variable,
    Import,
    Export,
    TypeAlias,
    Module,
    Trait,
    Macro,
}

impl SymbolKind {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Constant => "constant",
            Self::Variable => "variable",
            Self::Import => "import",
            Self::Export => "export",
            Self::TypeAlias => "type",
            Self::Module => "module",
            Self::Trait => "trait",
            Self::Macro => "macro",
        }
    }
}

/// Repository metadata and statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoMetadata {
    /// Total number of files
    pub total_files: u32,
    /// Total lines of code
    pub total_lines: u64,
    /// Aggregate token counts
    pub total_tokens: TokenCounts,
    /// Language breakdown
    pub languages: Vec<LanguageStats>,
    /// Detected framework (e.g., "React", "Django")
    pub framework: Option<String>,
    /// Repository description
    pub description: Option<String>,
    /// Git branch (if in git repo)
    pub branch: Option<String>,
    /// Git commit hash (if in git repo)
    pub commit: Option<String>,
    /// Directory structure tree
    pub directory_structure: Option<String>,
    /// External dependencies (packages/libraries)
    pub external_dependencies: Vec<String>,
    /// Git history (commits and changes) - for structured output
    pub git_history: Option<GitHistory>,
}

/// Statistics for a single language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageStats {
    /// Language name
    pub language: String,
    /// Number of files
    pub files: u32,
    /// Total lines in this language
    pub lines: u64,
    /// Percentage of total codebase
    pub percentage: f32,
}

/// A git commit entry for structured output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommitInfo {
    /// Full commit hash
    pub hash: String,
    /// Short commit hash (7 chars)
    pub short_hash: String,
    /// Author name
    pub author: String,
    /// Commit date (YYYY-MM-DD)
    pub date: String,
    /// Commit message
    pub message: String,
}

/// Git history information for structured output
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitHistory {
    /// Recent commits
    pub commits: Vec<GitCommitInfo>,
    /// Files with uncommitted changes
    pub changed_files: Vec<GitChangedFile>,
}

/// A file with uncommitted changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitChangedFile {
    /// File path relative to repo root
    pub path: String,
    /// Change status (A=Added, M=Modified, D=Deleted, R=Renamed)
    pub status: String,
}

/// Compression level for output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CompressionLevel {
    /// No compression
    None,
    /// Remove empty lines, trim whitespace
    Minimal,
    /// Remove comments, normalize whitespace
    #[default]
    Balanced,
    /// Remove docstrings, keep signatures only
    Aggressive,
    /// Key symbols only
    Extreme,
    /// AI-powered semantic compression
    Semantic,
}

impl CompressionLevel {
    /// Expected reduction percentage
    pub fn expected_reduction(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::Minimal => 15,
            Self::Balanced => 35,
            Self::Aggressive => 60,
            Self::Extreme => 80,
            Self::Semantic => 90,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_new() {
        let repo = Repository::new("test", "/tmp/test");
        assert_eq!(repo.name, "test");
        assert!(repo.files.is_empty());
    }

    #[test]
    fn test_token_counts() {
        let mut counts = TokenCounts::default();
        counts.set(TokenizerModel::Claude, 100);
        assert_eq!(counts.get(TokenizerModel::Claude), 100);
    }

    #[test]
    fn test_symbol_line_count() {
        let mut sym = Symbol::new("test", SymbolKind::Function);
        sym.start_line = 10;
        sym.end_line = 20;
        assert_eq!(sym.line_count(), 11);
    }
}
