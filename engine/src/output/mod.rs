//! Output formatters for different LLM models

mod markdown;
mod toon;
mod xml;

use crate::repomap::RepoMap;
use crate::types::Repository;

pub use markdown::MarkdownFormatter;
pub use toon::ToonFormatter;
pub use xml::XmlFormatter;

/// Output format type
#[derive(Debug, Clone, Copy, Default)]
pub enum OutputFormat {
    /// Claude-optimized XML
    #[default]
    Xml,
    /// GPT-optimized Markdown
    Markdown,
    /// JSON (generic)
    Json,
    /// YAML (Gemini)
    Yaml,
    /// TOON (Token-Oriented Object Notation) - most token-efficient
    Toon,
    /// Plain text (simple, no formatting)
    Plain,
}

/// Output formatter trait
pub trait Formatter {
    /// Format repository with map
    fn format(&self, repo: &Repository, map: &RepoMap) -> String;

    /// Format repository only
    fn format_repo(&self, repo: &Repository) -> String;

    /// Get format name
    fn name(&self) -> &'static str;
}

/// Output formatter factory
pub struct OutputFormatter;

impl OutputFormatter {
    /// Create Claude-optimized XML formatter
    pub fn claude() -> XmlFormatter {
        XmlFormatter::new(true)
    }

    /// Create GPT-optimized Markdown formatter
    pub fn gpt() -> MarkdownFormatter {
        MarkdownFormatter::new()
    }

    /// Create JSON formatter
    pub fn json() -> JsonFormatter {
        JsonFormatter
    }

    /// Create YAML formatter (Gemini)
    pub fn gemini() -> YamlFormatter {
        YamlFormatter
    }

    /// Create formatter by format type
    pub fn by_format(format: OutputFormat) -> Box<dyn Formatter> {
        Self::by_format_with_options(format, true)
    }

    /// Create formatter by format type with line numbers option
    pub fn by_format_with_options(format: OutputFormat, line_numbers: bool) -> Box<dyn Formatter> {
        Self::by_format_with_all_options(format, line_numbers, true)
    }

    /// Create formatter by format type with all options
    pub fn by_format_with_all_options(
        format: OutputFormat,
        line_numbers: bool,
        show_file_index: bool,
    ) -> Box<dyn Formatter> {
        match format {
            OutputFormat::Xml => Box::new(
                XmlFormatter::new(true)
                    .with_line_numbers(line_numbers)
                    .with_file_index(show_file_index),
            ),
            OutputFormat::Markdown => {
                Box::new(MarkdownFormatter::new().with_line_numbers(line_numbers))
            },
            OutputFormat::Json => Box::new(JsonFormatter),
            OutputFormat::Yaml => Box::new(YamlFormatter),
            OutputFormat::Toon => Box::new(
                ToonFormatter::new()
                    .with_line_numbers(line_numbers)
                    .with_file_index(show_file_index),
            ),
            OutputFormat::Plain => Box::new(PlainFormatter::new().with_line_numbers(line_numbers)),
        }
    }

    /// Create TOON formatter (most token-efficient)
    pub fn toon() -> ToonFormatter {
        ToonFormatter::new()
    }
}

/// JSON formatter
pub struct JsonFormatter;

impl Formatter for JsonFormatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String {
        #[derive(serde::Serialize)]
        struct Output<'a> {
            repository: &'a Repository,
            map: &'a RepoMap,
        }

        serde_json::to_string_pretty(&Output { repository: repo, map }).unwrap_or_default()
    }

    fn format_repo(&self, repo: &Repository) -> String {
        serde_json::to_string_pretty(repo).unwrap_or_default()
    }

    fn name(&self) -> &'static str {
        "json"
    }
}

/// Plain text formatter (simple, no markup)
pub struct PlainFormatter {
    /// Include line numbers in code
    include_line_numbers: bool,
}

impl PlainFormatter {
    /// Create a new plain formatter
    pub fn new() -> Self {
        Self { include_line_numbers: true }
    }

    /// Set line numbers option
    pub fn with_line_numbers(mut self, enabled: bool) -> Self {
        self.include_line_numbers = enabled;
        self
    }
}

impl Default for PlainFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for PlainFormatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&format!("Repository: {}\n", repo.name));
        output.push_str(&format!(
            "Files: {} | Lines: {} | Tokens: {}\n",
            repo.metadata.total_files, repo.metadata.total_lines, repo.metadata.total_tokens.claude
        ));
        output.push_str(&"=".repeat(60));
        output.push('\n');
        output.push('\n');

        // Repository map summary
        output.push_str("REPOSITORY MAP\n");
        output.push_str(&"-".repeat(40));
        output.push('\n');
        output.push_str(&map.summary);
        output.push_str("\n\n");

        // Directory structure
        if let Some(structure) = &repo.metadata.directory_structure {
            output.push_str("DIRECTORY STRUCTURE\n");
            output.push_str(&"-".repeat(40));
            output.push('\n');
            output.push_str(structure);
            output.push_str("\n\n");
        }

        // Files
        output.push_str("FILES\n");
        output.push_str(&"=".repeat(60));
        output.push('\n');

        for file in &repo.files {
            output.push('\n');
            output.push_str(&format!("File: {}\n", file.relative_path));
            if let Some(lang) = &file.language {
                output.push_str(&format!("Language: {}\n", lang));
            }
            output.push_str(&format!("Tokens: {}\n", file.token_count.claude));
            output.push_str(&"-".repeat(40));
            output.push('\n');

            if let Some(content) = &file.content {
                if self.include_line_numbers {
                    for (i, line) in content.lines().enumerate() {
                        output.push_str(&format!("{:4} {}\n", i + 1, line));
                    }
                } else {
                    output.push_str(content);
                    if !content.ends_with('\n') {
                        output.push('\n');
                    }
                }
            }
            output.push_str(&"-".repeat(40));
            output.push('\n');
        }

        output
    }

    fn format_repo(&self, repo: &Repository) -> String {
        let mut output = String::new();
        for file in &repo.files {
            output.push_str(&format!("=== {} ===\n", file.relative_path));
            if let Some(content) = &file.content {
                if self.include_line_numbers {
                    for (i, line) in content.lines().enumerate() {
                        output.push_str(&format!("{:4} {}\n", i + 1, line));
                    }
                } else {
                    output.push_str(content);
                    if !content.ends_with('\n') {
                        output.push('\n');
                    }
                }
            }
            output.push('\n');
        }
        output
    }

    fn name(&self) -> &'static str {
        "plain"
    }
}

/// YAML formatter (Gemini-optimized)
pub struct YamlFormatter;

impl Formatter for YamlFormatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String {
        let mut output = String::new();

        // YAML header
        output.push_str("---\n");
        output.push_str("# Repository Context for Gemini\n");
        output.push_str("# Note: Query should be at the END of this context\n\n");

        // Metadata
        output.push_str("metadata:\n");
        output.push_str(&format!("  name: {}\n", repo.name));
        output.push_str(&format!("  files: {}\n", repo.metadata.total_files));
        output.push_str(&format!("  lines: {}\n", repo.metadata.total_lines));
        output.push_str(&format!("  tokens: {}\n", repo.metadata.total_tokens.gemini));
        output.push('\n');

        // Languages
        output.push_str("languages:\n");
        for lang in &repo.metadata.languages {
            output.push_str(&format!(
                "  - name: {}\n    files: {}\n    percentage: {:.1}%\n",
                lang.language, lang.files, lang.percentage
            ));
        }
        output.push('\n');

        // Repository map
        output.push_str("repository_map:\n");
        output.push_str(&format!("  summary: |\n    {}\n", map.summary.replace('\n', "\n    ")));
        output.push_str("  key_symbols:\n");
        for sym in &map.key_symbols {
            output.push_str(&format!(
                "    - name: {}\n      type: {}\n      file: {}\n      rank: {}\n",
                sym.name, sym.kind, sym.file, sym.rank
            ));
        }
        output.push('\n');

        // Files
        output.push_str("files:\n");
        for file in &repo.files {
            output.push_str(&format!("  - path: {}\n", file.relative_path));
            if let Some(lang) = &file.language {
                output.push_str(&format!("    language: {}\n", lang));
            }
            output.push_str(&format!("    tokens: {}\n", file.token_count.gemini));

            if let Some(content) = &file.content {
                output.push_str("    content: |\n");
                for line in content.lines() {
                    output.push_str(&format!("      {}\n", line));
                }
            }
        }

        // Query placeholder at end (Gemini best practice)
        output.push_str("\n# --- INSERT YOUR QUERY BELOW THIS LINE ---\n");
        output.push_str("query: |\n");
        output.push_str("  [Your question about this repository]\n");

        output
    }

    fn format_repo(&self, repo: &Repository) -> String {
        serde_yaml::to_string(repo).unwrap_or_default()
    }

    fn name(&self) -> &'static str {
        "yaml"
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
                external_dependencies: vec!["requests".to_string()],
                git_history: None,
            },
        }
    }

    #[test]
    fn test_json_formatter() {
        let repo = create_test_repo();
        let map = RepoMapGenerator::new(1000).generate(&repo);

        let formatter = OutputFormatter::json();
        let output = formatter.format(&repo, &map);

        assert!(output.contains("\"name\": \"test\""));
        assert!(output.contains("\"files\""));
    }

    #[test]
    fn test_yaml_formatter() {
        let repo = create_test_repo();
        let map = RepoMapGenerator::new(1000).generate(&repo);

        let formatter = OutputFormatter::gemini();
        let output = formatter.format(&repo, &map);

        assert!(output.contains("name: test"));
        assert!(output.contains("# --- INSERT YOUR QUERY"));
    }
}
