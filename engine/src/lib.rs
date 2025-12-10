//! Infiniloom Engine - Repository context generation for LLMs
//!
//! This crate provides the core logic for generating optimized repository
//! context for large language models, including:
//!
//! - Repository mapping with PageRank-based symbol ranking
//! - Intelligent semantic chunking
//! - Model-specific output formatters (Claude, GPT, Gemini)
//! - Security scanning for secrets
//! - Accurate token counting (tiktoken for OpenAI, estimation for others)
//! - Full AST-based dependency resolution
//! - Memory-mapped file scanning for large repositories
//! - Incremental scanning with caching
//! - Remote Git repository support
//!
//! # Example
//!
//! ```rust,ignore
//! use infiniloom_engine::{Repository, RepoMapGenerator, OutputFormatter};
//!
//! let repo = Repository::scan("/path/to/repo")?;
//! let map = RepoMapGenerator::new(2000).generate(&repo);
//! let output = OutputFormatter::claude().format(&repo, &map);
//! ```

// Core modules
pub mod chunking;
pub mod default_ignores;
pub mod output;
pub mod parser;
pub mod ranking;
pub mod repomap;
pub mod security;
pub mod types;

// New modules
pub mod config;
pub mod dependencies;
pub mod git;
pub mod incremental;
pub mod mmap_scanner;
pub mod remote;
pub mod tokenizer;

#[cfg(feature = "embeddings")]
pub mod semantic;

pub mod ffi;

// Re-exports from core modules
pub use chunking::{Chunk, ChunkStrategy, Chunker};
pub use ffi::{estimate_tokens, is_binary, CompressionConfig, LanguageId, ZigCore};
pub use output::{OutputFormat, OutputFormatter};
pub use parser::{Language, Parser, ParserError};
pub use ranking::{rank_files, sort_files_by_importance, SymbolRanker};
pub use repomap::{RepoMap, RepoMapGenerator};
pub use security::SecurityScanner;
pub use types::*;

// Re-exports from new modules
pub use config::{
    Config, OutputConfig, PerformanceConfig, ScanConfig, SecurityConfig, SymbolConfig,
};
pub use dependencies::{DependencyEdge, DependencyGraph, DependencyNode, ResolvedImport};
pub use git::{ChangedFile, Commit, FileStatus, GitError, GitRepo};
pub use incremental::{CachedFile, FileChange, IncrementalScanner, RepoCache};
pub use mmap_scanner::{MappedFile, MmapScanner, ScannedFile as MmapScannedFile};
pub use remote::{GitProvider, RemoteError, RemoteRepo};
pub use tokenizer::{TokenCounts as AccurateTokenCounts, TokenModel, Tokenizer};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default token budget for repository maps
pub const DEFAULT_MAP_BUDGET: u32 = 2000;

/// Default chunk size in tokens
pub const DEFAULT_CHUNK_SIZE: u32 = 8000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        // Verify version follows semver format (at least has a number)
        assert!(VERSION.chars().any(|c| c.is_ascii_digit()));
    }
}
