//! Intelligent code chunking for LLM context windows

use crate::types::{RepoFile, Repository, TokenizerModel};
use serde::Serialize;

/// A chunk of repository content
#[derive(Debug, Clone, Serialize)]
pub struct Chunk {
    /// Chunk index (0-based)
    pub index: usize,
    /// Total number of chunks
    pub total: usize,
    /// Focus/theme of this chunk
    pub focus: String,
    /// Token count for this chunk
    pub tokens: u32,
    /// Files included in this chunk
    pub files: Vec<ChunkFile>,
    /// Context information
    pub context: ChunkContext,
}

/// A file within a chunk
#[derive(Debug, Clone, Serialize)]
pub struct ChunkFile {
    /// Relative file path
    pub path: String,
    /// File content (may be compressed)
    pub content: String,
    /// Token count
    pub tokens: u32,
    /// Whether content is truncated
    pub truncated: bool,
}

/// Context for chunk continuity
#[derive(Debug, Clone, Serialize)]
pub struct ChunkContext {
    /// Summary of previous chunks
    pub previous_summary: Option<String>,
    /// Current focus description
    pub current_focus: String,
    /// Preview of next chunk
    pub next_preview: Option<String>,
    /// Cross-references to other chunks
    pub cross_references: Vec<CrossReference>,
}

/// Reference to symbol in another chunk
#[derive(Debug, Clone, Serialize)]
pub struct CrossReference {
    /// Symbol name
    pub symbol: String,
    /// Chunk containing the symbol
    pub chunk_index: usize,
    /// File containing the symbol
    pub file: String,
}

/// Chunking strategy
#[derive(Debug, Clone, Copy, Default)]
pub enum ChunkStrategy {
    /// Fixed token size chunks
    Fixed {
        /// Maximum tokens per chunk
        size: u32,
    },
    /// One file per chunk
    File,
    /// Group by module/directory
    Module,
    /// Group by semantic similarity
    #[default]
    Semantic,
    /// Group by dependency order
    Dependency,
}

/// Chunker for splitting repositories
pub struct Chunker {
    /// Chunking strategy
    strategy: ChunkStrategy,
    /// Maximum tokens per chunk
    max_tokens: u32,
    /// Overlap tokens between chunks
    overlap_tokens: u32,
    /// Target model for token counting
    model: TokenizerModel,
}

impl Chunker {
    /// Create a new chunker
    pub fn new(strategy: ChunkStrategy, max_tokens: u32) -> Self {
        Self {
            strategy,
            max_tokens,
            overlap_tokens: 200,
            model: TokenizerModel::Claude,
        }
    }

    /// Set overlap tokens
    pub fn with_overlap(mut self, tokens: u32) -> Self {
        self.overlap_tokens = tokens;
        self
    }

    /// Set target model
    pub fn with_model(mut self, model: TokenizerModel) -> Self {
        self.model = model;
        self
    }

    /// Chunk a repository
    pub fn chunk(&self, repo: &Repository) -> Vec<Chunk> {
        match self.strategy {
            ChunkStrategy::Fixed { size } => self.fixed_chunk(repo, size),
            ChunkStrategy::File => self.file_chunk(repo),
            ChunkStrategy::Module => self.module_chunk(repo),
            ChunkStrategy::Semantic => self.semantic_chunk(repo),
            ChunkStrategy::Dependency => self.dependency_chunk(repo),
        }
    }

    /// Fixed-size chunking
    fn fixed_chunk(&self, repo: &Repository, size: u32) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut current_files = Vec::new();
        let mut current_tokens = 0u32;

        for file in &repo.files {
            let file_tokens = file.token_count.get(self.model);

            if current_tokens + file_tokens > size && !current_files.is_empty() {
                chunks.push(self.create_chunk(chunks.len(), &current_files, current_tokens));
                current_files.clear();
                current_tokens = 0;
            }

            current_files.push(file.clone());
            current_tokens += file_tokens;
        }

        if !current_files.is_empty() {
            chunks.push(self.create_chunk(chunks.len(), &current_files, current_tokens));
        }

        self.finalize_chunks(chunks)
    }

    /// One file per chunk
    fn file_chunk(&self, repo: &Repository) -> Vec<Chunk> {
        let chunks: Vec<_> = repo
            .files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                self.create_chunk(i, std::slice::from_ref(file), file.token_count.get(self.model))
            })
            .collect();

        self.finalize_chunks(chunks)
    }

    /// Group by module/directory
    fn module_chunk(&self, repo: &Repository) -> Vec<Chunk> {
        use std::collections::HashMap;

        let mut modules: HashMap<String, Vec<RepoFile>> = HashMap::new();

        for file in &repo.files {
            let module = file
                .relative_path
                .split('/')
                .next()
                .unwrap_or("root").to_owned();

            modules.entry(module).or_default().push(file.clone());
        }

        let chunks: Vec<_> = modules
            .into_iter()
            .enumerate()
            .map(|(i, (_, files))| {
                let tokens = files.iter().map(|f| f.token_count.get(self.model)).sum();
                self.create_chunk(i, &files, tokens)
            })
            .collect();

        self.finalize_chunks(chunks)
    }

    /// Semantic chunking (group related files)
    fn semantic_chunk(&self, repo: &Repository) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut current_files = Vec::new();
        let mut current_tokens = 0u32;
        let mut current_module: Option<String> = None;

        // Sort files by path for better grouping
        let mut sorted_files: Vec<_> = repo.files.iter().collect();
        sorted_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        for file in sorted_files {
            let file_tokens = file.token_count.get(self.model);
            let file_module = file.relative_path.split('/').next().map(String::from);

            // Check if we should start a new chunk
            let should_split = current_tokens + file_tokens > self.max_tokens
                || (current_module.is_some()
                    && file_module.is_some()
                    && current_module != file_module
                    && current_tokens > self.max_tokens / 2);

            if should_split && !current_files.is_empty() {
                chunks.push(self.create_chunk(chunks.len(), &current_files, current_tokens));

                // Keep some overlap for context
                current_files = self.get_overlap_files(&current_files);
                current_tokens = current_files
                    .iter()
                    .map(|f| f.token_count.get(self.model))
                    .sum();
            }

            current_files.push(file.clone());
            current_tokens += file_tokens;
            current_module = file_module;
        }

        if !current_files.is_empty() {
            chunks.push(self.create_chunk(chunks.len(), &current_files, current_tokens));
        }

        self.finalize_chunks(chunks)
    }

    /// Dependency-based chunking
    fn dependency_chunk(&self, repo: &Repository) -> Vec<Chunk> {
        // For now, fall back to semantic chunking
        // Full implementation would build dependency graph and topological sort
        self.semantic_chunk(repo)
    }

    fn create_chunk(&self, index: usize, files: &[RepoFile], tokens: u32) -> Chunk {
        let focus = self.determine_focus(files);

        Chunk {
            index,
            total: 0, // Updated in finalize
            focus: focus.clone(),
            tokens,
            files: files
                .iter()
                .map(|f| ChunkFile {
                    path: f.relative_path.clone(),
                    content: f.content.clone().unwrap_or_default(),
                    tokens: f.token_count.get(self.model),
                    truncated: false,
                })
                .collect(),
            context: ChunkContext {
                previous_summary: None,
                current_focus: focus,
                next_preview: None,
                cross_references: Vec::new(),
            },
        }
    }

    fn determine_focus(&self, files: &[RepoFile]) -> String {
        if files.is_empty() {
            return "Empty".to_owned();
        }

        // Try to find common directory
        let first_path = &files[0].relative_path;
        if let Some(module) = first_path.split('/').next() {
            if files.iter().all(|f| f.relative_path.starts_with(module)) {
                return format!("{} module", module);
            }
        }

        // Try to find common language
        if let Some(lang) = &files[0].language {
            if files.iter().all(|f| f.language.as_ref() == Some(lang)) {
                return format!("{} files", lang);
            }
        }

        "Mixed content".to_owned()
    }

    fn get_overlap_files(&self, files: &[RepoFile]) -> Vec<RepoFile> {
        // Keep files that might be needed for context
        // For now, just keep the last file if it's small enough
        files
            .last()
            .filter(|f| f.token_count.get(self.model) < self.overlap_tokens)
            .cloned()
            .into_iter()
            .collect()
    }

    fn finalize_chunks(&self, mut chunks: Vec<Chunk>) -> Vec<Chunk> {
        let total = chunks.len();

        // First pass: collect the focus strings we need
        let focus_strs: Vec<String> = chunks.iter().map(|c| c.focus.clone()).collect();

        for (i, chunk) in chunks.iter_mut().enumerate() {
            chunk.total = total;

            // Add previous summary
            if i > 0 {
                chunk.context.previous_summary =
                    Some(format!("Previous: {}", focus_strs[i - 1]));
            }

            // Add next preview
            if i + 1 < total {
                chunk.context.next_preview = Some(format!("Next: Chunk {}", i + 2));
            }
        }

        chunks
    }
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use crate::types::TokenCounts;

    fn create_test_repo() -> Repository {
        let mut repo = Repository::new("test", "/tmp/test");

        for i in 0..5 {
            repo.files.push(RepoFile {
                path: format!("/tmp/test/src/file{}.py", i).into(),
                relative_path: format!("src/file{}.py", i),
                language: Some("python".to_string()),
                size_bytes: 1000,
                token_count: TokenCounts {
                    claude: 500,
                    gpt4o: 480,
                    gpt4: 490,
                    gemini: 470,
                    llama: 460,
                },
                symbols: Vec::new(),
                importance: 0.5,
                content: Some(format!("# File {}\ndef func{}(): pass", i, i)),
            });
        }

        repo
    }

    #[test]
    fn test_fixed_chunking() {
        let repo = create_test_repo();
        let chunker = Chunker::new(ChunkStrategy::Fixed { size: 1000 }, 1000);
        let chunks = chunker.chunk(&repo);

        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|c| c.tokens <= 1000 || c.files.len() == 1));
    }

    #[test]
    fn test_file_chunking() {
        let repo = create_test_repo();
        let chunker = Chunker::new(ChunkStrategy::File, 8000);
        let chunks = chunker.chunk(&repo);

        assert_eq!(chunks.len(), repo.files.len());
    }

    #[test]
    fn test_semantic_chunking() {
        let repo = create_test_repo();
        let chunker = Chunker::new(ChunkStrategy::Semantic, 2000);
        let chunks = chunker.chunk(&repo);

        assert!(!chunks.is_empty());
        // All chunks should have correct total
        assert!(chunks.iter().all(|c| c.total == chunks.len()));
    }
}
