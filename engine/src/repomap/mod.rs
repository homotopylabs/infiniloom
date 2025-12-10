//! Repository map generation with PageRank-based symbol ranking

mod graph;

use crate::types::{Repository, SymbolKind, TokenizerModel};
#[cfg(test)]
use crate::types::{RepoFile, Symbol};
use graph::SymbolGraph;
use serde::Serialize;
use std::collections::HashMap;

/// A repository map - a concise summary of the codebase
#[derive(Debug, Clone, Serialize)]
pub struct RepoMap {
    /// Text summary of the repository
    pub summary: String,
    /// Most important symbols ranked by PageRank
    pub key_symbols: Vec<RankedSymbol>,
    /// Module/directory dependency graph
    pub module_graph: ModuleGraph,
    /// Index of all files with metadata
    pub file_index: Vec<FileIndexEntry>,
    /// Total token count for this map
    pub token_count: u32,
}

/// A symbol with its computed rank
#[derive(Debug, Clone, Serialize)]
pub struct RankedSymbol {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: String,
    /// File containing the symbol
    pub file: String,
    /// Line number
    pub line: u32,
    /// Function/method signature
    pub signature: Option<String>,
    /// Number of references
    pub references: u32,
    /// Rank (1 = most important)
    pub rank: u32,
    /// Importance score (0.0 - 1.0)
    pub importance: f32,
}

/// Graph of module dependencies
#[derive(Debug, Clone, Serialize)]
pub struct ModuleGraph {
    /// Module nodes
    pub nodes: Vec<ModuleNode>,
    /// Dependency edges
    pub edges: Vec<ModuleEdge>,
}

/// A module/directory node
#[derive(Debug, Clone, Serialize)]
pub struct ModuleNode {
    /// Module name (usually directory name)
    pub name: String,
    /// Number of files in module
    pub files: u32,
    /// Total tokens in module
    pub tokens: u32,
}

/// A dependency edge between modules
#[derive(Debug, Clone, Serialize)]
pub struct ModuleEdge {
    /// Source module
    pub from: String,
    /// Target module
    pub to: String,
    /// Number of imports/references
    pub weight: u32,
}

/// File index entry
#[derive(Debug, Clone, Serialize)]
pub struct FileIndexEntry {
    /// Relative file path
    pub path: String,
    /// Token count
    pub tokens: u32,
    /// Importance level (critical/high/normal/low)
    pub importance: String,
    /// Brief summary (optional)
    pub summary: Option<String>,
}

/// Generator for repository maps
pub struct RepoMapGenerator {
    /// Token budget for the map
    #[allow(dead_code)]
    token_budget: u32,
    /// Maximum number of symbols to include
    max_symbols: usize,
    /// Target model for token counting
    model: TokenizerModel,
}

impl RepoMapGenerator {
    /// Create a new generator with token budget
    pub fn new(token_budget: u32) -> Self {
        Self {
            token_budget,
            max_symbols: 50,
            model: TokenizerModel::Claude,
        }
    }

    /// Set maximum symbols to include
    pub fn with_max_symbols(mut self, max: usize) -> Self {
        self.max_symbols = max;
        self
    }

    /// Set target model for token counting
    pub fn with_model(mut self, model: TokenizerModel) -> Self {
        self.model = model;
        self
    }

    /// Generate a repository map
    pub fn generate(&self, repo: &Repository) -> RepoMap {
        // Build symbol graph
        let mut graph = SymbolGraph::new();
        for file in &repo.files {
            graph.add_file(file);
        }

        // Build lookup index for fast import resolution
        let symbol_index = self.build_symbol_index(repo);

        // Extract references from symbols using index
        self.extract_references_fast(&mut graph, repo, &symbol_index);

        // Compute PageRank once
        let ranks = graph.compute_pagerank(0.85, 20); // Reduced iterations for speed

        // Get top symbols using pre-computed ranks
        let key_symbols = self.build_ranked_symbols_fast(&graph, &ranks);

        // Build module graph
        let module_graph = self.build_module_graph(repo);

        // Build file index
        let file_index = self.build_file_index(repo);

        // Generate summary
        let summary = self.generate_summary(repo, &key_symbols);

        // Estimate token count
        let token_count = self.estimate_tokens(&key_symbols, &file_index);

        RepoMap {
            summary,
            key_symbols,
            module_graph,
            file_index,
            token_count,
        }
    }

    /// Build an index of symbols for fast lookup
    fn build_symbol_index(&self, repo: &Repository) -> HashMap<String, String> {
        let mut index = HashMap::new();
        for file in &repo.files {
            // Index by file path (without extension)
            let path_key = file.relative_path
                .trim_end_matches(".rs")
                .trim_end_matches(".py")
                .trim_end_matches(".js")
                .trim_end_matches(".ts")
                .trim_end_matches(".go")
                .trim_end_matches(".java");

            for symbol in &file.symbols {
                // Index by symbol name
                index.insert(symbol.name.clone(), format!("{}:{}", file.relative_path, symbol.name));
                // Index by path component
                index.insert(path_key.to_owned(), format!("{}:{}", file.relative_path, symbol.name));
            }
        }
        index
    }

    /// Fast reference extraction using pre-built index
    fn extract_references_fast(&self, graph: &mut SymbolGraph, repo: &Repository, index: &HashMap<String, String>) {
        for file in &repo.files {
            for symbol in &file.symbols {
                if symbol.kind == SymbolKind::Import {
                    // Fast lookup using index
                    if let Some(target) = index.get(&symbol.name) {
                        let from_key = format!("{}:{}", file.relative_path, symbol.name);
                        graph.add_reference(&from_key, target, graph::EdgeType::Imports);
                    }
                }
            }
        }
    }

    /// Build ranked symbols using pre-computed ranks
    fn build_ranked_symbols_fast(
        &self,
        graph: &SymbolGraph,
        ranks: &HashMap<String, f64>,
    ) -> Vec<RankedSymbol> {
        let top_nodes = graph.get_top_symbols_with_ranks(ranks, self.max_symbols);

        top_nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let key = format!("{}:{}", node.file_path, node.symbol.name);
                let rank_score = ranks.get(&key).copied().unwrap_or(0.0);

                RankedSymbol {
                    name: node.symbol.name.clone(),
                    kind: node.symbol.kind.name().to_owned(),
                    file: node.file_path.clone(),
                    line: node.symbol.start_line,
                    signature: node.symbol.signature.clone(),
                    references: node.symbol.references,
                    rank: (i + 1) as u32,
                    importance: rank_score as f32,
                }
            })
            .collect()
    }

    #[allow(dead_code)]
    fn extract_references(&self, graph: &mut SymbolGraph, repo: &Repository) {
        // This is a simplified version - real implementation would use AST
        for file in &repo.files {
            for symbol in &file.symbols {
                // Add edges based on symbol references
                // In real implementation, parse imports and function calls
                if symbol.kind == SymbolKind::Import {
                    // Try to resolve import target
                    if let Some(target) = self.resolve_import(&symbol.name, repo) {
                        let from_key = format!("{}:{}", file.relative_path, symbol.name);
                        graph.add_reference(&from_key, &target, graph::EdgeType::Imports);
                    }
                }
            }
        }
    }

    #[allow(dead_code)]
    fn resolve_import(&self, name: &str, repo: &Repository) -> Option<String> {
        // Simple resolution - look for matching symbol
        for file in &repo.files {
            for symbol in &file.symbols {
                if symbol.name == name || file.relative_path.contains(name) {
                    return Some(format!("{}:{}", file.relative_path, symbol.name));
                }
            }
        }
        None
    }

    #[allow(dead_code)]
    fn build_ranked_symbols(
        &self,
        graph: &SymbolGraph,
        ranks: &HashMap<String, f64>,
    ) -> Vec<RankedSymbol> {
        let top_nodes = graph.get_top_symbols(self.max_symbols);

        top_nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let key = format!("{}:{}", node.file_path, node.symbol.name);
                let rank_score = ranks.get(&key).copied().unwrap_or(0.0);

                RankedSymbol {
                    name: node.symbol.name.clone(),
                    kind: node.symbol.kind.name().to_owned(),
                    file: node.file_path.clone(),
                    line: node.symbol.start_line,
                    signature: node.symbol.signature.clone(),
                    references: node.symbol.references,
                    rank: (i + 1) as u32,
                    importance: rank_score as f32,
                }
            })
            .collect()
    }

    fn build_module_graph(&self, repo: &Repository) -> ModuleGraph {
        let mut modules: HashMap<String, ModuleNode> = HashMap::new();

        // Build file index by module (first pass)
        for file in &repo.files {
            let module = file
                .relative_path
                .split('/')
                .next()
                .unwrap_or("root").to_owned();

            let entry = modules.entry(module.clone()).or_insert(ModuleNode {
                name: module.clone(),
                files: 0,
                tokens: 0,
            });

            entry.files += 1;
            entry.tokens += file.token_count.get(self.model);
        }

        // Skip edge computation for now (it's expensive and rarely needed)
        ModuleGraph {
            nodes: modules.into_values().collect(),
            edges: Vec::new(), // TODO: implement efficient edge computation if needed
        }
    }

    fn build_file_index(&self, repo: &Repository) -> Vec<FileIndexEntry> {
        let mut files: Vec<_> = repo
            .files
            .iter()
            .map(|f| {
                let importance = if f.importance > 0.8 {
                    "critical"
                } else if f.importance > 0.6 {
                    "high"
                } else if f.importance > 0.3 {
                    "normal"
                } else {
                    "low"
                };

                FileIndexEntry {
                    path: f.relative_path.clone(),
                    tokens: f.token_count.get(self.model),
                    importance: importance.to_owned(),
                    summary: None,
                }
            })
            .collect();

        // Sort by importance
        files.sort_by(|a, b| {
            let a_imp = match a.importance.as_str() {
                "critical" => 4,
                "high" => 3,
                "normal" => 2,
                _ => 1,
            };
            let b_imp = match b.importance.as_str() {
                "critical" => 4,
                "high" => 3,
                "normal" => 2,
                _ => 1,
            };
            b_imp.cmp(&a_imp)
        });

        files
    }

    fn generate_summary(&self, repo: &Repository, symbols: &[RankedSymbol]) -> String {
        let top_modules: Vec<_> = symbols
            .iter()
            .take(3)
            .filter_map(|s| s.file.split('/').next())
            .collect();

        let primary_lang = repo
            .metadata
            .languages
            .first()
            .map(|l| l.language.as_str())
            .unwrap_or("unknown");

        format!(
            "Repository: {} ({} files, {} lines)\n\
             Primary language: {}\n\
             Key modules: {}",
            repo.name,
            repo.metadata.total_files,
            repo.metadata.total_lines,
            primary_lang,
            top_modules.join(", ")
        )
    }

    fn estimate_tokens(&self, symbols: &[RankedSymbol], files: &[FileIndexEntry]) -> u32 {
        // Rough estimate: ~25 tokens per symbol entry, ~10 per file entry
        let symbol_tokens = symbols.len() as u32 * 25;
        let file_tokens = files.len() as u32 * 10;
        let overhead = 100; // Headers, summary, etc.

        symbol_tokens + file_tokens + overhead
    }
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use crate::types::{RepoMetadata, TokenCounts};

    fn create_test_repo() -> Repository {
        Repository {
            name: "test-repo".to_owned(),
            path: "/tmp/test".into(),
            files: vec![RepoFile {
                path: "/tmp/test/src/main.py".into(),
                relative_path: "src/main.py".to_string(),
                language: Some("python".to_string()),
                size_bytes: 1000,
                token_count: TokenCounts {
                    claude: 250,
                    gpt4o: 240,
                    gpt4: 245,
                    gemini: 230,
                    llama: 235,
                },
                symbols: vec![Symbol {
                    name: "main".to_string(),
                    kind: SymbolKind::Function,
                    signature: Some("def main() -> None".to_string()),
                    docstring: Some("Entry point".to_string()),
                    start_line: 10,
                    end_line: 25,
                    references: 5,
                    importance: 0.9,
                    parent: None,
                }],
                importance: 0.9,
                content: None,
            }],
            metadata: RepoMetadata {
                total_files: 1,
                total_lines: 100,
                total_tokens: TokenCounts {
                    claude: 250,
                    gpt4o: 240,
                    gpt4: 245,
                    gemini: 230,
                    llama: 235,
                },
                languages: vec![crate::types::LanguageStats {
                    language: "Python".to_string(),
                    files: 1,
                    lines: 100,
                    percentage: 100.0,
                }],
                framework: None,
                description: None,
                branch: None,
                commit: None,
                directory_structure: None,
                external_dependencies: vec![],
                git_history: None,
            },
        }
    }

    #[test]
    fn test_generate_repomap() {
        let repo = create_test_repo();
        let generator = RepoMapGenerator::new(2000);
        let map = generator.generate(&repo);

        assert!(!map.summary.is_empty());
        assert!(!map.file_index.is_empty());
    }
}
