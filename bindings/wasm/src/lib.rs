//! WebAssembly bindings for Infiniloom
//!
//! This module exposes Infiniloom functionality to JavaScript environments.

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use std::panic;

// ============================================================================
// Initialization
// ============================================================================

#[wasm_bindgen(start)]
pub fn init() {
    // Set panic hook for better error messages
    panic::set_hook(Box::new(console_error_panic_hook::hook));

    // Initialize logger
    wasm_logger::init(wasm_logger::Config::default());
}

#[wasm_bindgen]
pub fn version() -> String {
    infiniloom_engine::VERSION.to_string()
}

// ============================================================================
// Token Counting
// ============================================================================

/// Token count result for all models
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCounts {
    pub claude: u32,
    pub gpt4o: u32,
    pub gpt4: u32,
    pub gemini: u32,
    pub llama: u32,
}

#[wasm_bindgen]
impl TokenCounts {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            claude: 0,
            gpt4o: 0,
            gpt4: 0,
            gemini: 0,
            llama: 0,
        }
    }
}

/// Count tokens for a specific model
#[wasm_bindgen]
pub fn count_tokens(text: &str, model: &str) -> Result<u32, JsValue> {
    // This would call into the Zig WASM module
    // For now, we'll use a simple heuristic
    let count = match model {
        "claude" => estimate_tokens_claude(text),
        "gpt4o" => estimate_tokens_gpt4o(text),
        "gpt4" => estimate_tokens_gpt4(text),
        "gemini" => estimate_tokens_gemini(text),
        "llama" => estimate_tokens_llama(text),
        _ => return Err(JsValue::from_str(&format!("Unknown model: {}", model))),
    };

    Ok(count)
}

/// Count tokens for all models at once
#[wasm_bindgen]
pub fn count_tokens_all(text: &str) -> TokenCounts {
    TokenCounts {
        claude: estimate_tokens_claude(text),
        gpt4o: estimate_tokens_gpt4o(text),
        gpt4: estimate_tokens_gpt4(text),
        gemini: estimate_tokens_gemini(text),
        llama: estimate_tokens_llama(text),
    }
}

// Quick estimation algorithms (simplified versions)
fn estimate_tokens_claude(text: &str) -> u32 {
    // Claude's tokenizer is similar to GPT-4
    // Roughly 4 chars per token for English
    (text.len() as f32 / 4.0).ceil() as u32
}

fn estimate_tokens_gpt4o(text: &str) -> u32 {
    // GPT-4o is slightly more efficient
    (text.len() as f32 / 4.2).ceil() as u32
}

fn estimate_tokens_gpt4(text: &str) -> u32 {
    // GPT-4 baseline
    (text.len() as f32 / 4.0).ceil() as u32
}

fn estimate_tokens_gemini(text: &str) -> u32 {
    // Gemini similar to GPT-4
    (text.len() as f32 / 4.0).ceil() as u32
}

fn estimate_tokens_llama(text: &str) -> u32 {
    // Llama tends to use more tokens
    (text.len() as f32 / 3.5).ceil() as u32
}

// ============================================================================
// File Processing
// ============================================================================

/// File information
#[wasm_bindgen]
#[derive(Clone)]
pub struct FileInfo {
    path: String,
    language: Option<String>,
    tokens: TokenCounts,
    size_bytes: usize,
}

#[wasm_bindgen]
impl FileInfo {
    #[wasm_bindgen(getter)]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn language(&self) -> Option<String> {
        self.language.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn tokens(&self) -> TokenCounts {
        self.tokens.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn size_bytes(&self) -> usize {
        self.size_bytes
    }
}

/// Detect programming language from filename
#[wasm_bindgen]
pub fn detect_language(filename: &str) -> Option<String> {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())?;

    let language = match ext {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "jsx" => "javascript",
        "tsx" => "typescript",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "h" | "hpp" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "zig" => "zig",
        "md" => "markdown",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "html" => "html",
        "css" => "css",
        "scss" | "sass" => "scss",
        _ => return None,
    };

    Some(language.to_string())
}

/// Process a file and get its information
#[wasm_bindgen]
pub fn process_file(filename: &str, content: &str) -> FileInfo {
    let language = detect_language(filename);
    let tokens = count_tokens_all(content);
    let size_bytes = content.len();

    FileInfo {
        path: filename.to_string(),
        language,
        tokens,
        size_bytes,
    }
}

// ============================================================================
// Compression
// ============================================================================

/// Compression level
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub enum CompressionLevel {
    None,
    Minimal,
    Balanced,
    Aggressive,
}

/// Compress code with specified level
#[wasm_bindgen]
pub fn compress(content: &str, level: CompressionLevel, language: Option<String>) -> String {
    match level {
        CompressionLevel::None => content.to_string(),
        CompressionLevel::Minimal => compress_minimal(content),
        CompressionLevel::Balanced => compress_balanced(content, language.as_deref()),
        CompressionLevel::Aggressive => compress_aggressive(content, language.as_deref()),
    }
}

fn compress_minimal(content: &str) -> String {
    // Remove trailing whitespace and excessive blank lines
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut blank_count = 0;

    for line in lines {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                result.push("");
            }
        } else {
            blank_count = 0;
            result.push(trimmed);
        }
    }

    result.join("\n")
}

fn compress_balanced(content: &str, language: Option<&str>) -> String {
    let mut result = Vec::new();
    let comment_prefix = match language {
        Some("python") | Some("ruby") => "#",
        Some("javascript") | Some("typescript") | Some("rust") | Some("go") | Some("java") => "//",
        _ => "//",
    };

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip comment lines
        if trimmed.starts_with(comment_prefix) || trimmed.starts_with("/*") || trimmed.starts_with("*") {
            continue;
        }

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        result.push(line.trim_end());
    }

    result.join("\n")
}

fn compress_aggressive(content: &str, language: Option<&str>) -> String {
    let balanced = compress_balanced(content, language);

    // Further compress by removing docstrings and simplifying
    let mut result = Vec::new();
    let mut in_docstring = false;

    for line in balanced.lines() {
        let trimmed = line.trim();

        // Detect docstrings (Python triple quotes)
        if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
            in_docstring = !in_docstring;
            continue;
        }

        if in_docstring {
            continue;
        }

        result.push(line);
    }

    result.join("\n")
}

// ============================================================================
// Context Generation
// ============================================================================

/// Output format
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Claude,    // XML
    GPT,       // Markdown
    Gemini,    // YAML-like
    Plain,     // Plain text
}

/// Generate context output from files
#[wasm_bindgen]
pub fn generate_context(
    files: JsValue,
    format: OutputFormat,
    compression: CompressionLevel,
) -> Result<String, JsValue> {
    // Deserialize the files array
    let files: Vec<(String, String)> = serde_wasm_bindgen::from_value(files)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse files: {}", e)))?;

    let mut output = String::new();

    match format {
        OutputFormat::Claude => {
            output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
            output.push_str("<repository>\n");
            output.push_str("  <context>\n");

            for (filename, content) in &files {
                let language = detect_language(filename).unwrap_or_else(|| "text".to_string());
                let compressed = compress(content, compression, Some(language.clone()));

                output.push_str(&format!("    <file path=\"{}\" language=\"{}\">\n", filename, language));
                output.push_str("      <content>\n");

                for line in compressed.lines() {
                    output.push_str(&format!("        {}\n", escape_xml(line)));
                }

                output.push_str("      </content>\n");
                output.push_str("    </file>\n");
            }

            output.push_str("  </context>\n");
            output.push_str("</repository>");
        }

        OutputFormat::GPT => {
            output.push_str("# Repository Context\n\n");

            for (filename, content) in &files {
                let language = detect_language(filename).unwrap_or_else(|| "text".to_string());
                let compressed = compress(content, compression, Some(language.clone()));

                output.push_str(&format!("## {}\n\n", filename));
                output.push_str(&format!("```{}\n", language));
                output.push_str(&compressed);
                output.push_str("\n```\n\n");
            }
        }

        OutputFormat::Gemini => {
            output.push_str("repository:\n");
            output.push_str("  files:\n");

            for (filename, content) in &files {
                let language = detect_language(filename).unwrap_or_else(|| "text".to_string());
                let compressed = compress(content, compression, Some(language.clone()));

                output.push_str(&format!("    - path: {}\n", filename));
                output.push_str(&format!("      language: {}\n", language));
                output.push_str("      content: |\n");

                for line in compressed.lines() {
                    output.push_str(&format!("        {}\n", line));
                }
            }
        }

        OutputFormat::Plain => {
            for (filename, content) in &files {
                let language = detect_language(filename).unwrap_or_else(|| "text".to_string());
                let compressed = compress(content, compression, Some(language));

                output.push_str(&format!("=== {} ===\n", filename));
                output.push_str(&compressed);
                output.push_str("\n\n");
            }
        }
    }

    Ok(output)
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ============================================================================
// Statistics
// ============================================================================

/// Repository statistics
#[wasm_bindgen]
#[derive(Clone)]
pub struct RepoStats {
    total_files: usize,
    total_bytes: usize,
    total_lines: usize,
    tokens_claude: u32,
    tokens_gpt4o: u32,
    tokens_gpt4: u32,
    tokens_gemini: u32,
    tokens_llama: u32,
}

#[wasm_bindgen]
impl RepoStats {
    #[wasm_bindgen(getter)]
    pub fn total_files(&self) -> usize {
        self.total_files
    }

    #[wasm_bindgen(getter)]
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    #[wasm_bindgen(getter)]
    pub fn total_lines(&self) -> usize {
        self.total_lines
    }

    #[wasm_bindgen(getter)]
    pub fn tokens_claude(&self) -> u32 {
        self.tokens_claude
    }

    #[wasm_bindgen(getter)]
    pub fn tokens_gpt4o(&self) -> u32 {
        self.tokens_gpt4o
    }

    #[wasm_bindgen(getter)]
    pub fn tokens_gpt4(&self) -> u32 {
        self.tokens_gpt4
    }

    #[wasm_bindgen(getter)]
    pub fn tokens_gemini(&self) -> u32 {
        self.tokens_gemini
    }

    #[wasm_bindgen(getter)]
    pub fn tokens_llama(&self) -> u32 {
        self.tokens_llama
    }
}

/// Calculate statistics for multiple files
#[wasm_bindgen]
pub fn calculate_stats(files: JsValue) -> Result<RepoStats, JsValue> {
    let files: Vec<(String, String)> = serde_wasm_bindgen::from_value(files)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse files: {}", e)))?;

    let mut stats = RepoStats {
        total_files: files.len(),
        total_bytes: 0,
        total_lines: 0,
        tokens_claude: 0,
        tokens_gpt4o: 0,
        tokens_gpt4: 0,
        tokens_gemini: 0,
        tokens_llama: 0,
    };

    for (_filename, content) in &files {
        stats.total_bytes += content.len();
        stats.total_lines += content.lines().count();

        let tokens = count_tokens_all(content);
        stats.tokens_claude += tokens.claude;
        stats.tokens_gpt4o += tokens.gpt4o;
        stats.tokens_gpt4 += tokens.gpt4;
        stats.tokens_gemini += tokens.gemini;
        stats.tokens_llama += tokens.llama;
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_estimation() {
        let text = "Hello, world!";
        let claude_count = estimate_tokens_claude(text);
        assert!(claude_count > 0);
        assert!(claude_count < 10);
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(detect_language("main.rs"), Some("rust".to_string()));
        assert_eq!(detect_language("app.py"), Some("python".to_string()));
        assert_eq!(detect_language("index.js"), Some("javascript".to_string()));
    }

    #[test]
    fn test_compression() {
        let code = "fn main() {\n    // Comment\n    println!(\"hello\");\n}\n";
        let compressed = compress_balanced(code, Some("rust"));
        assert!(!compressed.contains("// Comment"));
    }
}
