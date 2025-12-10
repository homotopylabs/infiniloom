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
pub mod types;
pub mod repomap;
pub mod ranking;
pub mod chunking;
pub mod output;
pub mod security;
pub mod parser;
pub mod default_ignores;

// New modules
pub mod tokenizer;
pub mod config;
pub mod dependencies;
pub mod remote;
pub mod mmap_scanner;
pub mod incremental;
pub mod git;

#[cfg(feature = "embeddings")]
pub mod semantic;

pub mod ffi;

// Re-exports from core modules
pub use types::*;
pub use ffi::{ZigCore, CompressionConfig, LanguageId, estimate_tokens, is_binary};
pub use repomap::{RepoMap, RepoMapGenerator};
pub use ranking::{SymbolRanker, rank_files, sort_files_by_importance};
pub use chunking::{Chunk, Chunker, ChunkStrategy};
pub use output::{OutputFormat, OutputFormatter};
pub use security::SecurityScanner;
pub use parser::{Parser, Language, ParserError};

// Re-exports from new modules
pub use tokenizer::{Tokenizer, TokenModel, TokenCounts as AccurateTokenCounts};
pub use config::{Config, ScanConfig, OutputConfig, SymbolConfig, SecurityConfig, PerformanceConfig};
pub use dependencies::{DependencyGraph, DependencyNode, DependencyEdge, ResolvedImport};
pub use remote::{RemoteRepo, GitProvider, RemoteError};
pub use mmap_scanner::{MmapScanner, MappedFile, ScannedFile as MmapScannedFile};
pub use incremental::{IncrementalScanner, RepoCache, CachedFile, FileChange};
pub use git::{GitRepo, Commit, ChangedFile, FileStatus, GitError};

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
