//! Symbol graph with PageRank computation

use crate::types::{RepoFile, Symbol};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

/// A node in the symbol graph
#[derive(Debug, Clone)]
pub(super) struct SymbolNode {
    /// The symbol
    pub symbol: Symbol,
    /// File containing this symbol
    pub file_path: String,
}

/// Type of edge between symbols
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(super) enum EdgeType {
    /// Function calls another function
    Calls,
    /// File imports symbol
    Imports,
    /// Class inherits from another
    Inherits,
    /// Class implements interface
    Implements,
    /// Generic reference
    References,
}

/// Graph of symbols with reference relationships
pub(super) struct SymbolGraph {
    /// The underlying directed graph
    graph: DiGraph<SymbolNode, EdgeType>,
    /// Map from symbol key to node index
    symbol_indices: HashMap<String, NodeIndex>,
}

impl SymbolGraph {
    /// Create a new empty graph
    pub(super) fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            symbol_indices: HashMap::new(),
        }
    }

    /// Add all symbols from a file
    pub(super) fn add_file(&mut self, file: &RepoFile) {
        for symbol in &file.symbols {
            let node = SymbolNode {
                symbol: symbol.clone(),
                file_path: file.relative_path.clone(),
            };

            let idx = self.graph.add_node(node);
            let key = format!("{}:{}", file.relative_path, symbol.name);
            self.symbol_indices.insert(key, idx);
        }
    }

    /// Add a reference edge between symbols
    pub(super) fn add_reference(&mut self, from: &str, to: &str, edge_type: EdgeType) {
        if let (Some(&from_idx), Some(&to_idx)) =
            (self.symbol_indices.get(from), self.symbol_indices.get(to))
        {
            self.graph.add_edge(from_idx, to_idx, edge_type);
        }
    }

    /// Compute PageRank scores for all symbols
    pub(super) fn compute_pagerank(&self, damping: f64, iterations: usize) -> HashMap<String, f64> {
        let node_count = self.graph.node_count();
        if node_count == 0 {
            return HashMap::new();
        }

        // Initialize ranks
        let initial_rank = 1.0 / node_count as f64;
        let mut ranks: Vec<f64> = vec![initial_rank; node_count];
        let mut new_ranks: Vec<f64> = vec![0.0; node_count];

        // Iterative PageRank computation (optimized dangling node handling)
        for _ in 0..iterations {
            // Reset new ranks with teleportation probability
            let teleport = (1.0 - damping) / node_count as f64;
            new_ranks.fill(teleport);

            // First pass: accumulate dangling node contribution
            let mut dangling_sum = 0.0;
            for node_idx in self.graph.node_indices() {
                let out_degree = self.graph.neighbors(node_idx).count();
                if out_degree == 0 {
                    dangling_sum += ranks[node_idx.index()];
                }
            }
            // Distribute dangling mass to all nodes at once (O(n) instead of O(n²))
            let dangling_contribution = damping * dangling_sum / node_count as f64;
            for rank in &mut new_ranks {
                *rank += dangling_contribution;
            }

            // Second pass: distribute rank from non-dangling nodes to neighbors
            for node_idx in self.graph.node_indices() {
                let out_degree = self.graph.neighbors(node_idx).count();
                if out_degree > 0 {
                    let rank_contribution = damping * ranks[node_idx.index()] / out_degree as f64;
                    for neighbor in self.graph.neighbors(node_idx) {
                        new_ranks[neighbor.index()] += rank_contribution;
                    }
                }
            }

            // Swap ranks
            std::mem::swap(&mut ranks, &mut new_ranks);
        }

        // Build result map
        let mut result = HashMap::new();
        for (key, &idx) in &self.symbol_indices {
            result.insert(key.clone(), ranks[idx.index()]);
        }
        result
    }

    /// Get top N symbols using pre-computed ranks
    pub(super) fn get_top_symbols_with_ranks(&self, ranks: &HashMap<String, f64>, n: usize) -> Vec<&SymbolNode> {
        let mut ranked: Vec<_> = self
            .graph
            .node_indices()
            .filter_map(|idx| {
                let node = self.graph.node_weight(idx)?;
                let key = format!("{}:{}", node.file_path, node.symbol.name);
                let rank = ranks.get(&key).copied().unwrap_or(0.0);
                Some((node, rank))
            })
            .collect();

        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        ranked.into_iter().take(n).map(|(node, _)| node).collect()
    }

    /// Get top N symbols by PageRank (computes ranks internally - use get_top_symbols_with_ranks if ranks already computed)
    #[allow(dead_code)]
    pub(super) fn get_top_symbols(&self, n: usize) -> Vec<&SymbolNode> {
        let ranks = self.compute_pagerank(0.85, 100);
        self.get_top_symbols_with_ranks(&ranks, n)
    }

    /// Get number of nodes
    #[allow(dead_code)]
    pub(super) fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Get number of edges
    #[allow(dead_code)]
    pub(super) fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }
}

impl Default for SymbolGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SymbolKind;

    #[test]
    fn test_empty_graph() {
        let graph = SymbolGraph::new();
        assert_eq!(graph.node_count(), 0);
        assert!(graph.get_top_symbols(10).is_empty());
    }

    #[test]
    fn test_add_symbols() {
        let mut graph = SymbolGraph::new();

        let file = RepoFile {
            path: "/test/main.py".into(),
            relative_path: "main.py".to_owned(),
            language: Some("python".to_owned()),
            size_bytes: 100,
            token_count: Default::default(),
            symbols: vec![
                Symbol::new("main", SymbolKind::Function),
                Symbol::new("helper", SymbolKind::Function),
            ],
            importance: 0.5,
            content: None,
        };

        graph.add_file(&file);
        assert_eq!(graph.node_count(), 2);
    }

    #[test]
    fn test_pagerank() {
        let mut graph = SymbolGraph::new();

        // Create a simple graph: A -> B -> C, A -> C
        let file = RepoFile {
            path: "/test/main.py".into(),
            relative_path: "main.py".to_owned(),
            language: Some("python".to_owned()),
            size_bytes: 100,
            token_count: Default::default(),
            symbols: vec![
                Symbol::new("a", SymbolKind::Function),
                Symbol::new("b", SymbolKind::Function),
                Symbol::new("c", SymbolKind::Function),
            ],
            importance: 0.5,
            content: None,
        };

        graph.add_file(&file);
        graph.add_reference("main.py:a", "main.py:b", EdgeType::Calls);
        graph.add_reference("main.py:b", "main.py:c", EdgeType::Calls);
        graph.add_reference("main.py:a", "main.py:c", EdgeType::Calls);

        let ranks = graph.compute_pagerank(0.85, 100);

        // C should have highest rank (most incoming edges)
        let rank_a = ranks.get("main.py:a").unwrap();
        let rank_c = ranks.get("main.py:c").unwrap();
        assert!(rank_c > rank_a);
    }
}
