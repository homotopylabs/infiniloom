//! TOON (Token-Oriented Object Notation) output formatter
//!
//! TOON is a compact, human-readable format designed for LLM context.
//! It provides ~40% fewer tokens than JSON while maintaining readability.
//!
//! Format specification: https://github.com/toon-format/toon

use crate::output::Formatter;
use crate::repomap::RepoMap;
use crate::types::Repository;
use std::fmt::Write;

/// TOON formatter - most token-efficient format for LLMs
pub struct ToonFormatter {
    /// Include line numbers in code
    include_line_numbers: bool,
    /// Use tabular format for file metadata
    use_tabular: bool,
    /// Include file index/summary section
    show_file_index: bool,
}

impl ToonFormatter {
    /// Create a new TOON formatter with default settings
    pub fn new() -> Self {
        Self { include_line_numbers: true, use_tabular: true, show_file_index: true }
    }

    /// Set line numbers option
    pub fn with_line_numbers(mut self, enabled: bool) -> Self {
        self.include_line_numbers = enabled;
        self
    }

    /// Set tabular format option
    pub fn with_tabular(mut self, enabled: bool) -> Self {
        self.use_tabular = enabled;
        self
    }

    /// Set file index/summary option
    pub fn with_file_index(mut self, enabled: bool) -> Self {
        self.show_file_index = enabled;
        self
    }

    fn write_metadata(&self, output: &mut String, repo: &Repository) {
        writeln!(output, "metadata:").unwrap();
        writeln!(output, "  name: {}", repo.name).unwrap();
        writeln!(output, "  files: {}", repo.metadata.total_files).unwrap();
        writeln!(output, "  lines: {}", repo.metadata.total_lines).unwrap();
        writeln!(output, "  tokens: {}", repo.metadata.total_tokens.claude).unwrap();

        if let Some(ref desc) = repo.metadata.description {
            writeln!(output, "  description: {}", escape_toon(desc)).unwrap();
        }
        if let Some(ref branch) = repo.metadata.branch {
            writeln!(output, "  branch: {}", branch).unwrap();
        }
        if let Some(ref commit) = repo.metadata.commit {
            writeln!(output, "  commit: {}", commit).unwrap();
        }
        output.push('\n');
    }

    fn write_languages(&self, output: &mut String, repo: &Repository) {
        if repo.metadata.languages.is_empty() {
            return;
        }

        let count = repo.metadata.languages.len();
        writeln!(output, "languages[{}]{{name,files,percentage}}:", count).unwrap();
        for lang in &repo.metadata.languages {
            writeln!(output, "  {},{},{:.1}", lang.language, lang.files, lang.percentage).unwrap();
        }
        output.push('\n');
    }

    fn write_directory_structure(&self, output: &mut String, repo: &Repository) {
        if let Some(ref structure) = repo.metadata.directory_structure {
            writeln!(output, "directory_structure: |").unwrap();
            for line in structure.lines() {
                writeln!(output, "  {}", line).unwrap();
            }
            output.push('\n');
        }
    }

    fn write_dependencies(&self, output: &mut String, repo: &Repository) {
        if repo.metadata.external_dependencies.is_empty() {
            return;
        }

        let count = repo.metadata.external_dependencies.len();
        writeln!(output, "dependencies[{}]:", count).unwrap();
        for dep in &repo.metadata.external_dependencies {
            writeln!(output, "  {}", escape_toon(dep)).unwrap();
        }
        output.push('\n');
    }

    fn write_repomap(&self, output: &mut String, map: &RepoMap) {
        writeln!(output, "repository_map:").unwrap();
        writeln!(output, "  token_budget: {}", map.token_count).unwrap();
        writeln!(output, "  summary: |").unwrap();
        for line in map.summary.lines() {
            writeln!(output, "    {}", line).unwrap();
        }

        // Key symbols in tabular format
        if !map.key_symbols.is_empty() {
            let count = map.key_symbols.len();
            writeln!(output, "  symbols[{}]{{name,type,file,line,rank}}:", count).unwrap();
            for sym in &map.key_symbols {
                writeln!(
                    output,
                    "    {},{},{},{},{}",
                    escape_toon(&sym.name),
                    escape_toon(&sym.kind),
                    escape_toon(&sym.file),
                    sym.line,
                    sym.rank
                )
                .unwrap();
            }
        }

        // Modules in tabular format
        if !map.module_graph.nodes.is_empty() {
            let count = map.module_graph.nodes.len();
            writeln!(output, "  modules[{}]{{name,files,tokens}}:", count).unwrap();
            for module in &map.module_graph.nodes {
                writeln!(
                    output,
                    "    {},{},{}",
                    escape_toon(&module.name),
                    module.files,
                    module.tokens
                )
                .unwrap();
            }
        }
        output.push('\n');
    }

    fn write_file_index(&self, output: &mut String, repo: &Repository) {
        if repo.files.is_empty() {
            return;
        }

        let count = repo.files.len();
        writeln!(output, "file_index[{}]{{path,tokens,importance}}:", count).unwrap();
        for file in &repo.files {
            let importance = if file.importance > 0.8 {
                "critical"
            } else if file.importance > 0.6 {
                "high"
            } else if file.importance > 0.3 {
                "normal"
            } else {
                "low"
            };
            writeln!(
                output,
                "  {},{},{}",
                escape_toon(&file.relative_path),
                file.token_count.claude,
                importance
            )
            .unwrap();
        }
        output.push('\n');
    }

    fn write_files(&self, output: &mut String, repo: &Repository) {
        writeln!(output, "files:").unwrap();

        for file in &repo.files {
            if let Some(ref content) = file.content {
                // Compact file header: path|language|tokens
                let lang = file.language.as_deref().unwrap_or("?");
                writeln!(
                    output,
                    "- {}|{}|{}:",
                    escape_toon(&file.relative_path),
                    lang,
                    file.token_count.claude
                )
                .unwrap();

                // Content with minimal line numbers
                if self.include_line_numbers {
                    for (i, line) in content.lines().enumerate() {
                        // Use variable-width line numbers with single space after
                        writeln!(output, "  {}:{}", i + 1, line).unwrap();
                    }
                } else {
                    for line in content.lines() {
                        writeln!(output, "  {}", line).unwrap();
                    }
                }
            }
        }
    }
}

impl Default for ToonFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for ToonFormatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String {
        let mut output = String::new();

        // TOON header comment
        writeln!(output, "# Infiniloom Repository Context (TOON format)").unwrap();
        writeln!(output, "# Format: https://github.com/toon-format/toon").unwrap();
        output.push('\n');

        self.write_metadata(&mut output, repo);
        self.write_languages(&mut output, repo);
        self.write_directory_structure(&mut output, repo);
        self.write_dependencies(&mut output, repo);
        self.write_repomap(&mut output, map);
        if self.show_file_index {
            self.write_file_index(&mut output, repo);
        }
        self.write_files(&mut output, repo);

        output
    }

    fn format_repo(&self, repo: &Repository) -> String {
        let mut output = String::new();

        writeln!(output, "# Infiniloom Repository Context (TOON format)").unwrap();
        output.push('\n');

        self.write_metadata(&mut output, repo);
        self.write_languages(&mut output, repo);
        self.write_directory_structure(&mut output, repo);
        self.write_dependencies(&mut output, repo);
        if self.show_file_index {
            self.write_file_index(&mut output, repo);
        }
        self.write_files(&mut output, repo);

        output
    }

    fn name(&self) -> &'static str {
        "toon"
    }
}

/// Escape special characters for TOON format
/// Quotes are needed when:
/// - String is empty
/// - Contains leading/trailing whitespace
/// - Matches reserved literals (true, false, null)
/// - Matches numeric patterns
/// - Contains control characters or delimiters (comma, pipe, newline)
fn escape_toon(s: &str) -> String {
    // Check if quoting is needed
    let needs_quotes = s.is_empty()
        || s.starts_with(' ')
        || s.ends_with(' ')
        || s == "true"
        || s == "false"
        || s == "null"
        || s.parse::<f64>().is_ok()
        || s.contains(',')
        || s.contains('|')
        || s.contains('\n')
        || s.contains('\r')
        || s.contains('\t')
        || s.contains('"');

    if needs_quotes {
        // Escape backslashes and quotes
        let escaped = s
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        format!("\"{}\"", escaped)
    } else {
        s.to_owned()
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
                token_count: TokenCounts { claude: 50, gpt4o: 48, gpt4: 49, gemini: 47, llama: 46 },
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
                directory_structure: Some("main.py\n".to_string()),
                external_dependencies: vec!["requests".to_string(), "numpy".to_string()],
                git_history: None,
            },
        }
    }

    #[test]
    fn test_toon_output() {
        let repo = create_test_repo();
        let map = RepoMapGenerator::new(1000).generate(&repo);

        let formatter = ToonFormatter::new();
        let output = formatter.format(&repo, &map);

        assert!(output.contains("# Infiniloom Repository Context"));
        assert!(output.contains("metadata:"));
        assert!(output.contains("name: test"));
        assert!(output.contains("files: 1"));
        assert!(output.contains("languages[1]{name,files,percentage}:"));
        assert!(output.contains("directory_structure: |"));
        // Files are formatted as "- path|lang|tokens:"
        assert!(output.contains("main.py|python|50:"));
    }

    #[test]
    fn test_toon_escaping() {
        assert_eq!(escape_toon("hello"), "hello");
        assert_eq!(escape_toon(""), "\"\"");
        assert_eq!(escape_toon("true"), "\"true\"");
        assert_eq!(escape_toon("123"), "\"123\"");
        assert_eq!(escape_toon("a,b"), "\"a,b\"");
        assert_eq!(escape_toon("line\nbreak"), "\"line\\nbreak\"");
    }

    #[test]
    fn test_toon_tabular_format() {
        let repo = create_test_repo();
        let formatter = ToonFormatter::new();
        let output = formatter.format_repo(&repo);

        // Should use tabular format for languages and file_index
        assert!(output.contains("languages[1]{name,files,percentage}:"));
        assert!(output.contains("file_index[1]{path,tokens,importance}:"));
    }
}
