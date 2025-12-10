//! Claude-optimized XML output formatter
//!
//! This formatter is designed to maximize LLM comprehension of codebases by:
//! 1. Providing an executive summary for quick understanding
//! 2. Identifying entry points and key files
//! 3. Showing architecture and dependencies
//! 4. Prioritizing files by importance for code tasks

use crate::output::Formatter;
use crate::repomap::RepoMap;
use crate::types::Repository;
use std::fmt::Write;

/// XML formatter optimized for Claude
pub struct XmlFormatter {
    /// Include line numbers in code
    include_line_numbers: bool,
    /// Optimize for prompt caching
    cache_optimized: bool,
    /// Include CDATA sections for code
    use_cdata: bool,
    /// Include file index/summary section
    show_file_index: bool,
}

impl XmlFormatter {
    /// Create a new XML formatter
    pub fn new(cache_optimized: bool) -> Self {
        Self { include_line_numbers: true, cache_optimized, use_cdata: true, show_file_index: true }
    }

    /// Set line numbers option
    pub fn with_line_numbers(mut self, enabled: bool) -> Self {
        self.include_line_numbers = enabled;
        self
    }

    /// Set CDATA option
    pub fn with_cdata(mut self, enabled: bool) -> Self {
        self.use_cdata = enabled;
        self
    }

    /// Set file index/summary option
    pub fn with_file_index(mut self, enabled: bool) -> Self {
        self.show_file_index = enabled;
        self
    }

    fn write_llm_instructions(&self, output: &mut String, repo: &Repository) {
        writeln!(output, "  <llm_context_guide>").unwrap();
        writeln!(output, "    <purpose>This is a comprehensive code context for the {} repository, optimized for AI-assisted code understanding and generation.</purpose>", escape_xml(&repo.name)).unwrap();
        writeln!(output, "    <how_to_use>").unwrap();
        writeln!(output, "      <tip>Start with the <overview> section to understand the project's purpose and structure</tip>").unwrap();
        writeln!(output, "      <tip>Check <entry_points> to find main application files</tip>")
            .unwrap();
        writeln!(
            output,
            "      <tip>Use <repository_map> to understand relationships between modules</tip>"
        )
        .unwrap();
        writeln!(
            output,
            "      <tip>Files are ordered by importance - most critical files come first</tip>"
        )
        .unwrap();
        writeln!(output, "    </how_to_use>").unwrap();
        writeln!(output, "  </llm_context_guide>").unwrap();
    }

    fn write_overview(&self, output: &mut String, repo: &Repository) {
        writeln!(output, "  <overview>").unwrap();

        // Detect project type from languages and files
        let project_type = self.detect_project_type(repo);
        writeln!(output, "    <project_type>{}</project_type>", escape_xml(&project_type)).unwrap();

        // Primary language (find language with highest file count)
        if let Some(lang) = repo.metadata.languages.iter().max_by_key(|l| l.files) {
            writeln!(
                output,
                "    <primary_language>{}</primary_language>",
                escape_xml(&lang.language)
            )
            .unwrap();
        }

        // Framework detection
        if let Some(framework) = &repo.metadata.framework {
            writeln!(output, "    <framework>{}</framework>", escape_xml(framework)).unwrap();
        }

        // Auto-detect entry points (exclude empty __init__.py files)
        writeln!(output, "    <entry_points>").unwrap();
        let mut entry_count = 0;
        for file in &repo.files {
            if self.is_entry_point(&file.relative_path) {
                // Skip empty __init__.py files
                if file.relative_path.ends_with("__init__.py") && file.token_count.claude < 50 {
                    continue;
                }
                let entry_type = self.get_entry_type(&file.relative_path);
                writeln!(
                    output,
                    "      <entry path=\"{}\" type=\"{}\" tokens=\"{}\"/>",
                    escape_xml(&file.relative_path),
                    entry_type,
                    file.token_count.claude
                )
                .unwrap();
                entry_count += 1;
                // Limit to 10 most important entry points
                if entry_count >= 10 {
                    break;
                }
            }
        }
        writeln!(output, "    </entry_points>").unwrap();

        // Key configuration files
        writeln!(output, "    <config_files>").unwrap();
        for file in &repo.files {
            if self.is_config_file(&file.relative_path) {
                writeln!(
                    output,
                    "      <config path=\"{}\" tokens=\"{}\"/>",
                    escape_xml(&file.relative_path),
                    file.token_count.claude
                )
                .unwrap();
            }
        }
        writeln!(output, "    </config_files>").unwrap();

        writeln!(output, "  </overview>").unwrap();
    }

    fn detect_project_type(&self, repo: &Repository) -> String {
        // Check for common project indicators
        let has_cargo = repo.files.iter().any(|f| f.relative_path == "Cargo.toml");
        let has_package_json = repo.files.iter().any(|f| f.relative_path == "package.json");
        let has_pyproject = repo
            .files
            .iter()
            .any(|f| f.relative_path == "pyproject.toml" || f.relative_path == "setup.py");
        let has_go_mod = repo.files.iter().any(|f| f.relative_path == "go.mod");

        // Check for web framework indicators
        let has_routes = repo
            .files
            .iter()
            .any(|f| f.relative_path.contains("routes") || f.relative_path.contains("api/"));
        let has_components = repo
            .files
            .iter()
            .any(|f| f.relative_path.contains("components/") || f.relative_path.contains("views/"));

        if has_cargo {
            if repo
                .files
                .iter()
                .any(|f| f.relative_path.ends_with("lib.rs"))
            {
                "Rust Library"
            } else {
                "Rust Application"
            }
        } else if has_package_json {
            if has_components {
                "Frontend Application (JavaScript/TypeScript)"
            } else if has_routes {
                "Backend API (Node.js)"
            } else {
                "JavaScript/TypeScript Project"
            }
        } else if has_pyproject {
            if has_routes {
                "Python Web API"
            } else {
                "Python Package"
            }
        } else if has_go_mod {
            "Go Application"
        } else {
            "Software Project"
        }
        .to_owned()
    }

    fn is_entry_point(&self, path: &str) -> bool {
        let entry_patterns = [
            "main.rs",
            "main.go",
            "main.py",
            "main.ts",
            "main.js",
            "main.c",
            "main.cpp",
            "index.ts",
            "index.js",
            "index.tsx",
            "index.jsx",
            "index.py",
            "app.py",
            "app.ts",
            "app.js",
            "app.tsx",
            "app.jsx",
            "app.go",
            "server.py",
            "server.ts",
            "server.js",
            "server.go",
            "mod.rs",
            "lib.rs",
            "__main__.py",
            "__init__.py",
            "cmd/main.go",
        ];
        entry_patterns
            .iter()
            .any(|p| path.ends_with(p) || path.contains(&format!("/{}", p)))
    }

    fn get_entry_type(&self, path: &str) -> &'static str {
        if path.contains("main") {
            "main"
        } else if path.contains("index") {
            "index"
        } else if path.contains("app") {
            "app"
        } else if path.contains("server") {
            "server"
        } else if path.contains("lib") {
            "library"
        } else if path.contains("mod.rs") {
            "module"
        } else {
            "entry"
        }
    }

    fn is_config_file(&self, path: &str) -> bool {
        let config_files = [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "go.mod",
            "pom.xml",
            "build.gradle",
            "Gemfile",
            "requirements.txt",
            "setup.py",
            "setup.cfg",
            "tsconfig.json",
            "webpack.config",
            "vite.config",
            "next.config",
            "Makefile",
            "CMakeLists.txt",
            "Dockerfile",
            "docker-compose",
            ".env.example",
            "config.yaml",
            "config.yml",
            "config.json",
        ];
        // Only match root-level or well-known config paths
        let filename = path.rsplit('/').next().unwrap_or(path);
        config_files.iter().any(|c| filename.contains(c)) && path.matches('/').count() <= 1
    }

    fn write_metadata(&self, output: &mut String, repo: &Repository) {
        writeln!(output, "  <metadata>").unwrap();

        if let Some(desc) = &repo.metadata.description {
            writeln!(output, "    <description>{}</description>", escape_xml(desc)).unwrap();
        }

        writeln!(output, "    <stats>").unwrap();
        writeln!(output, "      <files>{}</files>", repo.metadata.total_files).unwrap();
        writeln!(output, "      <lines>{}</lines>", repo.metadata.total_lines).unwrap();
        writeln!(
            output,
            "      <tokens model=\"claude\">{}</tokens>",
            repo.metadata.total_tokens.claude
        )
        .unwrap();
        writeln!(output, "    </stats>").unwrap();

        if !repo.metadata.languages.is_empty() {
            writeln!(output, "    <languages>").unwrap();
            for lang in &repo.metadata.languages {
                writeln!(
                    output,
                    "      <language name=\"{}\" files=\"{}\" percentage=\"{:.1}\"/>",
                    escape_xml(&lang.language),
                    lang.files,
                    lang.percentage
                )
                .unwrap();
            }
            writeln!(output, "    </languages>").unwrap();
        }

        // Directory structure
        if let Some(ref structure) = repo.metadata.directory_structure {
            writeln!(output, "    <directory_structure><![CDATA[").unwrap();
            output.push_str(structure);
            writeln!(output, "]]></directory_structure>").unwrap();
        }

        // External dependencies
        if !repo.metadata.external_dependencies.is_empty() {
            writeln!(
                output,
                "    <dependencies count=\"{}\">",
                repo.metadata.external_dependencies.len()
            )
            .unwrap();
            for dep in &repo.metadata.external_dependencies {
                writeln!(output, "      <dependency name=\"{}\"/>", escape_xml(dep)).unwrap();
            }
            writeln!(output, "    </dependencies>").unwrap();
        }

        writeln!(output, "  </metadata>").unwrap();
    }

    fn write_git_history(&self, output: &mut String, repo: &Repository) {
        if let Some(ref git_history) = repo.metadata.git_history {
            writeln!(output, "  <git_history>").unwrap();

            // Write recent commits
            if !git_history.commits.is_empty() {
                writeln!(output, "    <recent_commits count=\"{}\">", git_history.commits.len())
                    .unwrap();
                for commit in &git_history.commits {
                    writeln!(
                        output,
                        "      <commit hash=\"{}\" author=\"{}\" date=\"{}\">",
                        escape_xml(&commit.short_hash),
                        escape_xml(&commit.author),
                        escape_xml(&commit.date)
                    )
                    .unwrap();
                    writeln!(output, "        <message><![CDATA[{}]]></message>", commit.message)
                        .unwrap();
                    writeln!(output, "      </commit>").unwrap();
                }
                writeln!(output, "    </recent_commits>").unwrap();
            }

            // Write uncommitted changes
            if !git_history.changed_files.is_empty() {
                writeln!(
                    output,
                    "    <uncommitted_changes count=\"{}\">",
                    git_history.changed_files.len()
                )
                .unwrap();
                for file in &git_history.changed_files {
                    writeln!(
                        output,
                        "      <change path=\"{}\" status=\"{}\"/>",
                        escape_xml(&file.path),
                        escape_xml(&file.status)
                    )
                    .unwrap();
                }
                writeln!(output, "    </uncommitted_changes>").unwrap();
            }

            writeln!(output, "  </git_history>").unwrap();
        }
    }

    fn write_repomap(&self, output: &mut String, map: &RepoMap) {
        writeln!(output, "  <repository_map token_budget=\"{}\">", map.token_count).unwrap();

        // Summary with CDATA
        writeln!(output, "    <summary><![CDATA[{}]]></summary>", map.summary).unwrap();

        // Key symbols
        writeln!(output, "    <key_symbols>").unwrap();
        for symbol in &map.key_symbols {
            writeln!(
                output,
                "      <symbol name=\"{}\" type=\"{}\" file=\"{}\" line=\"{}\" rank=\"{}\">",
                escape_xml(&symbol.name),
                escape_xml(&symbol.kind),
                escape_xml(&symbol.file),
                symbol.line,
                symbol.rank
            )
            .unwrap();

            if let Some(sig) = &symbol.signature {
                writeln!(output, "        <signature><![CDATA[{}]]></signature>", sig).unwrap();
            }

            writeln!(output, "      </symbol>").unwrap();
        }
        writeln!(output, "    </key_symbols>").unwrap();

        // Module graph
        if !map.module_graph.nodes.is_empty() {
            writeln!(output, "    <modules>").unwrap();
            for module in &map.module_graph.nodes {
                writeln!(
                    output,
                    "      <module name=\"{}\" files=\"{}\" tokens=\"{}\"/>",
                    escape_xml(&module.name),
                    module.files,
                    module.tokens
                )
                .unwrap();
            }
            writeln!(output, "    </modules>").unwrap();
        }

        writeln!(output, "  </repository_map>").unwrap();
    }

    fn write_file_index(&self, output: &mut String, repo: &Repository) {
        writeln!(output, "  <file_index entries=\"{}\">", repo.files.len()).unwrap();

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
                "    <file path=\"{}\" tokens=\"{}\" importance=\"{}\"/>",
                escape_xml(&file.relative_path),
                file.token_count.claude,
                importance
            )
            .unwrap();
        }

        writeln!(output, "  </file_index>").unwrap();
    }

    fn write_files(&self, output: &mut String, repo: &Repository) {
        writeln!(output, "  <files>").unwrap();

        for file in &repo.files {
            if let Some(content) = &file.content {
                writeln!(
                    output,
                    "    <file path=\"{}\" language=\"{}\" tokens=\"{}\">",
                    escape_xml(&file.relative_path),
                    file.language.as_deref().unwrap_or("unknown"),
                    file.token_count.claude
                )
                .unwrap();

                if self.include_line_numbers {
                    writeln!(output, "      <content line_numbers=\"true\"><![CDATA[").unwrap();
                    for (i, line) in content.lines().enumerate() {
                        writeln!(output, "{:4} | {}", i + 1, line).unwrap();
                    }
                    writeln!(output, "]]></content>").unwrap();
                } else if self.use_cdata {
                    writeln!(output, "      <content><![CDATA[{}]]></content>", content).unwrap();
                } else {
                    writeln!(output, "      <content>{}</content>", escape_xml(content)).unwrap();
                }

                writeln!(output, "    </file>").unwrap();
            }
        }

        writeln!(output, "  </files>").unwrap();
    }
}

impl Formatter for XmlFormatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String {
        let mut output = String::new();

        // XML declaration
        writeln!(output, r#"<?xml version="1.0" encoding="UTF-8"?>"#).unwrap();
        writeln!(output, r#"<repository name="{}" version="1.0.0">"#, escape_xml(&repo.name))
            .unwrap();

        // LLM context guide (helps LLMs understand how to use this context)
        self.write_llm_instructions(&mut output, repo);

        // Cacheable section (Claude prompt caching)
        if self.cache_optimized {
            writeln!(output, "  <!-- CACHEABLE_PREFIX_START -->").unwrap();
        }

        // Project overview with entry points and config (HIGH VALUE for LLM understanding)
        self.write_overview(&mut output, repo);

        self.write_metadata(&mut output, repo);

        // Git history (if available) - provides context on recent changes
        self.write_git_history(&mut output, repo);

        self.write_repomap(&mut output, map);
        if self.show_file_index {
            self.write_file_index(&mut output, repo);
        }

        if self.cache_optimized {
            writeln!(output, "  <!-- CACHEABLE_PREFIX_END -->").unwrap();
            writeln!(output, "  <!-- DYNAMIC_CONTENT_START -->").unwrap();
        }

        self.write_files(&mut output, repo);

        if self.cache_optimized {
            writeln!(output, "  <!-- DYNAMIC_CONTENT_END -->").unwrap();
        }

        writeln!(output, "</repository>").unwrap();

        output
    }

    fn format_repo(&self, repo: &Repository) -> String {
        let mut output = String::new();

        writeln!(output, r#"<?xml version="1.0" encoding="UTF-8"?>"#).unwrap();
        writeln!(output, r#"<repository name="{}">"#, escape_xml(&repo.name)).unwrap();

        self.write_metadata(&mut output, repo);
        if self.show_file_index {
            self.write_file_index(&mut output, repo);
        }
        self.write_files(&mut output, repo);

        writeln!(output, "</repository>").unwrap();

        output
    }

    fn name(&self) -> &'static str {
        "xml"
    }
}

/// Escape XML special characters (single-pass for performance)
fn escape_xml(s: &str) -> String {
    // Pre-allocate with some extra capacity for escapes
    let mut result = String::with_capacity(s.len() + s.len() / 10);

    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            _ => result.push(c),
        }
    }

    result
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
    fn test_xml_output() {
        let repo = create_test_repo();
        let map = RepoMapGenerator::new(1000).generate(&repo);

        let formatter = XmlFormatter::new(true);
        let output = formatter.format(&repo, &map);

        assert!(output.contains("<?xml version=\"1.0\""));
        assert!(output.contains("<repository name=\"test\""));
        assert!(output.contains("CACHEABLE_PREFIX_START"));
        assert!(output.contains("<file path=\"main.py\""));
    }

    #[test]
    fn test_xml_escaping() {
        assert_eq!(escape_xml("<test>"), "&lt;test&gt;");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
    }
}
