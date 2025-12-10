//! GPT-optimized Markdown output formatter

use crate::output::Formatter;
use crate::repomap::RepoMap;
use crate::types::Repository;
use std::fmt::Write;

/// Markdown formatter optimized for GPT
pub struct MarkdownFormatter {
    /// Include overview tables
    include_tables: bool,
    /// Include Mermaid diagrams
    include_mermaid: bool,
    /// Include file tree
    include_tree: bool,
    /// Include line numbers in code
    include_line_numbers: bool,
}

impl MarkdownFormatter {
    /// Create a new Markdown formatter
    pub fn new() -> Self {
        Self {
            include_tables: true,
            include_mermaid: true,
            include_tree: true,
            include_line_numbers: true,
        }
    }

    /// Set tables option
    pub fn with_tables(mut self, enabled: bool) -> Self {
        self.include_tables = enabled;
        self
    }

    /// Set Mermaid option
    pub fn with_mermaid(mut self, enabled: bool) -> Self {
        self.include_mermaid = enabled;
        self
    }

    /// Set line numbers option
    pub fn with_line_numbers(mut self, enabled: bool) -> Self {
        self.include_line_numbers = enabled;
        self
    }

    fn write_header(&self, output: &mut String, repo: &Repository) {
        writeln!(output, "# Repository: {}", repo.name).unwrap();
        writeln!(output).unwrap();

        // Quick stats
        writeln!(
            output,
            "> **Files**: {} | **Lines**: {} | **Tokens**: {}",
            repo.metadata.total_files, repo.metadata.total_lines, repo.metadata.total_tokens.gpt4o
        )
        .unwrap();
        writeln!(output).unwrap();
    }

    fn write_overview(&self, output: &mut String, repo: &Repository) {
        if !self.include_tables {
            return;
        }

        writeln!(output, "## Overview").unwrap();
        writeln!(output).unwrap();

        // Stats table
        writeln!(output, "| Metric | Value |").unwrap();
        writeln!(output, "|--------|-------|").unwrap();
        writeln!(output, "| Files | {} |", repo.metadata.total_files).unwrap();
        writeln!(output, "| Lines | {} |", repo.metadata.total_lines).unwrap();

        if let Some(lang) = repo.metadata.languages.first() {
            writeln!(output, "| Primary Language | {} |", lang.language).unwrap();
        }

        if let Some(framework) = &repo.metadata.framework {
            writeln!(output, "| Framework | {} |", framework).unwrap();
        }

        writeln!(output).unwrap();

        // Language breakdown
        if repo.metadata.languages.len() > 1 {
            writeln!(output, "### Languages").unwrap();
            writeln!(output).unwrap();
            writeln!(output, "| Language | Files | Percentage |").unwrap();
            writeln!(output, "|----------|-------|------------|").unwrap();

            for lang in &repo.metadata.languages {
                writeln!(
                    output,
                    "| {} | {} | {:.1}% |",
                    lang.language, lang.files, lang.percentage
                )
                .unwrap();
            }
            writeln!(output).unwrap();
        }
    }

    fn write_repomap(&self, output: &mut String, map: &RepoMap) {
        writeln!(output, "## Repository Map").unwrap();
        writeln!(output).unwrap();
        writeln!(output, "{}", map.summary).unwrap();
        writeln!(output).unwrap();

        // Key symbols table
        writeln!(output, "### Key Symbols").unwrap();
        writeln!(output).unwrap();
        writeln!(output, "| Rank | Symbol | Type | File | Line |").unwrap();
        writeln!(output, "|------|--------|------|------|------|").unwrap();

        for sym in map.key_symbols.iter().take(15) {
            writeln!(
                output,
                "| {} | `{}` | {} | {} | {} |",
                sym.rank, sym.name, sym.kind, sym.file, sym.line
            )
            .unwrap();
        }
        writeln!(output).unwrap();

        // Mermaid dependency graph
        if self.include_mermaid && !map.module_graph.edges.is_empty() {
            writeln!(output, "### Module Dependencies").unwrap();
            writeln!(output).unwrap();
            writeln!(output, "```mermaid").unwrap();
            writeln!(output, "graph LR").unwrap();

            for edge in &map.module_graph.edges {
                // Replace special chars with underscores for Mermaid IDs
                let sanitize_id = |s: &str| -> String {
                    s.chars().map(|c| if c == '-' || c == '.' { '_' } else { c }).collect()
                };
                let from_id = sanitize_id(&edge.from);
                let to_id = sanitize_id(&edge.to);
                writeln!(
                    output,
                    "    {}[\"{}\"] --> {}[\"{}\"]",
                    from_id, edge.from, to_id, edge.to
                )
                .unwrap();
            }

            writeln!(output, "```").unwrap();
            writeln!(output).unwrap();
        }
    }

    fn write_structure(&self, output: &mut String, repo: &Repository) {
        if !self.include_tree {
            return;
        }

        writeln!(output, "## Project Structure").unwrap();
        writeln!(output).unwrap();
        writeln!(output, "```").unwrap();

        // Build tree structure
        let mut paths: Vec<_> = repo.files.iter().map(|f| f.relative_path.as_str()).collect();
        paths.sort();

        // Simple tree rendering
        let mut prev_parts: Vec<&str> = Vec::new();
        for path in paths {
            let parts: Vec<_> = path.split('/').collect();

            // Find common prefix with previous path
            let mut common = 0;
            for (i, part) in parts.iter().enumerate() {
                if i < prev_parts.len() && prev_parts[i] == *part {
                    common = i + 1;
                } else {
                    break;
                }
            }

            // Print new parts
            for (i, part) in parts.iter().enumerate().skip(common) {
                let indent = "  ".repeat(i);
                let prefix = if i == parts.len() - 1 { "📄 " } else { "📁 " };
                writeln!(output, "{}{}{}", indent, prefix, part).unwrap();
            }

            prev_parts = parts;
        }

        writeln!(output, "```").unwrap();
        writeln!(output).unwrap();
    }

    fn write_files(&self, output: &mut String, repo: &Repository) {
        writeln!(output, "## Files").unwrap();
        writeln!(output).unwrap();

        for file in &repo.files {
            if let Some(content) = &file.content {
                writeln!(output, "### {}", file.relative_path).unwrap();
                writeln!(output).unwrap();

                // File metadata
                writeln!(
                    output,
                    "> **Tokens**: {} | **Language**: {}",
                    file.token_count.gpt4o,
                    file.language.as_deref().unwrap_or("unknown")
                )
                .unwrap();
                writeln!(output).unwrap();

                // Code block with language
                let lang = file.language.as_deref().unwrap_or("");
                writeln!(output, "```{}", lang).unwrap();
                if self.include_line_numbers {
                    for (i, line) in content.lines().enumerate() {
                        writeln!(output, "{:4} {}", i + 1, line).unwrap();
                    }
                } else {
                    writeln!(output, "{}", content).unwrap();
                }
                writeln!(output, "```").unwrap();
                writeln!(output).unwrap();
            }
        }
    }
}

impl Default for MarkdownFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for MarkdownFormatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String {
        let mut output = String::new();

        self.write_header(&mut output, repo);
        self.write_overview(&mut output, repo);
        self.write_repomap(&mut output, map);
        self.write_structure(&mut output, repo);
        self.write_files(&mut output, repo);

        output
    }

    fn format_repo(&self, repo: &Repository) -> String {
        let mut output = String::new();

        self.write_header(&mut output, repo);
        self.write_overview(&mut output, repo);
        self.write_structure(&mut output, repo);
        self.write_files(&mut output, repo);

        output
    }

    fn name(&self) -> &'static str {
        "markdown"
    }
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use crate::repomap::RepoMapGenerator;
    use crate::types::{LanguageStats, RepoFile, RepoMetadata, TokenCounts};

    fn create_test_repo() -> Repository {
        Repository {
            name: "test".to_string(),
            path: "/tmp/test".into(),
            files: vec![RepoFile {
                path: "/tmp/test/main.py".into(),
                relative_path: "main.py".to_string(),
                language: Some("python".to_string()),
                size_bytes: 100,
                token_count: TokenCounts {
                    claude: 50,
                    gpt4o: 48,
                    gpt4: 49,
                    gemini: 47,
                    llama: 46,
                },
                symbols: Vec::new(),
                importance: 0.8,
                content: Some("def main():\n    print('hello')".to_string()),
            }],
            metadata: RepoMetadata {
                total_files: 1,
                total_lines: 2,
                total_tokens: TokenCounts {
                    claude: 50,
                    gpt4o: 48,
                    gpt4: 49,
                    gemini: 47,
                    llama: 46,
                },
                languages: vec![LanguageStats {
                    language: "Python".to_string(),
                    files: 1,
                    lines: 2,
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
    fn test_markdown_output() {
        let repo = create_test_repo();
        let map = RepoMapGenerator::new(1000).generate(&repo);

        let formatter = MarkdownFormatter::new();
        let output = formatter.format(&repo, &map);

        assert!(output.contains("# Repository: test"));
        assert!(output.contains("## Overview"));
        assert!(output.contains("```python"));
    }
}
