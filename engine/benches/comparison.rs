//! Benchmarks comparing Infiniloom performance with similar tools
//!
//! This benchmark suite measures:
//! - Repository scanning speed
//! - Symbol extraction performance
//! - Output generation time
//! - Memory usage patterns
//!
//! Run with: cargo bench
//!
//! For comparison with external tools (repomix, gitingest), use the
//! benchmark scripts in the scripts/ directory.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::fs;
use tempfile::TempDir;

// Mock imports - these would be real imports in the actual implementation
// use infiniloom_engine::{Repository, OutputFormatter, RepoMapGenerator, TokenizerModel};

/// Create a test repository with varying sizes
fn create_test_repo(num_files: usize, lines_per_file: usize) -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let base = temp_dir.path();

    // Create source directory
    fs::create_dir_all(base.join("src")).unwrap();

    // Generate Rust files
    for i in 0..num_files / 3 {
        let mut content = String::new();
        content.push_str(&format!("//! Module {}\n\n", i));

        for j in 0..lines_per_file / 10 {
            content.push_str(&format!(
                r#"
/// Function {} documentation
pub fn function_{}_{}(x: i32, y: i32) -> i32 {{
    let result = x + y;
    if result > 100 {{
        return result * 2;
    }}
    result
}}

"#,
                j, i, j
            ));
        }

        fs::write(base.join(format!("src/module_{}.rs", i)), content).unwrap();
    }

    // Generate Python files
    for i in 0..num_files / 3 {
        let mut content = String::new();
        content.push_str(&format!("\"\"\"Module {} for Python code\"\"\"\n\n", i));

        for j in 0..lines_per_file / 10 {
            content.push_str(&format!(
                r#"
def function_{}_{}(x: int, y: int) -> int:
    """Calculate result from x and y."""
    result = x + y
    if result > 100:
        return result * 2
    return result


class Class_{}_{}:
    """A sample class."""

    def __init__(self):
        self.value = 0

    def process(self):
        return self.value * 2

"#,
                i, j, i, j
            ));
        }

        fs::write(base.join(format!("src/module_{}.py", i)), content).unwrap();
    }

    // Generate JavaScript files
    for i in 0..num_files / 3 {
        let mut content = String::new();
        content.push_str(&format!("/**\n * Module {} for JavaScript code\n */\n\n", i));

        for j in 0..lines_per_file / 10 {
            content.push_str(&format!(
                r#"
/**
 * Function {}_{} documentation
 * @param {{{{number}}}} x - First number
 * @param {{{{number}}}} y - Second number
 * @returns {{{{number}}}} The result
 */
function function_{}_{}(x, y) {{{{
    const result = x + y;
    if (result > 100) {{{{
        return result * 2;
    }}}}
    return result;
}}}}

class Class_{}_{} {{{{
    constructor() {{{{
        this.value = 0;
    }}}}

    process() {{{{
        return this.value * 2;
    }}}}
}}}}

"#,
                i, j, i, j, i, j
            ));
        }

        fs::write(base.join(format!("src/module_{}.js", i)), content).unwrap();
    }

    // Create .gitignore
    fs::write(
        base.join(".gitignore"),
        "target/\nnode_modules/\n__pycache__/\n",
    )
    .unwrap();

    temp_dir
}

/// Benchmark file traversal speed
fn bench_file_traversal(c: &mut Criterion) {
    let sizes = [(10, "small"), (50, "medium"), (200, "large")];

    let mut group = c.benchmark_group("file_traversal");

    for (num_files, name) in sizes.iter() {
        let temp = create_test_repo(*num_files, 100);
        let path = temp.path().to_path_buf();

        group.throughput(Throughput::Elements(*num_files as u64));
        group.bench_with_input(
            BenchmarkId::new("walkdir", name),
            &path,
            |b, path| {
                b.iter(|| {
                    let mut count = 0;
                    for entry in walkdir::WalkDir::new(path)
                        .into_iter()
                        .filter_map(|e| e.ok())
                    {
                        if entry.file_type().is_file() {
                            count += 1;
                        }
                    }
                    black_box(count)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("ignore_crate", name),
            &path,
            |b, path| {
                b.iter(|| {
                    let mut count = 0;
                    for entry in ignore::WalkBuilder::new(path)
                        .hidden(false)
                        .git_ignore(true)
                        .build()
                        .filter_map(|e| e.ok())
                    {
                        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                            count += 1;
                        }
                    }
                    black_box(count)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark file reading speed
fn bench_file_reading(c: &mut Criterion) {
    let temp = create_test_repo(30, 500);
    let path = temp.path().to_path_buf();

    // Collect all file paths
    let files: Vec<_> = ignore::WalkBuilder::new(&path)
        .hidden(false)
        .git_ignore(true)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.path().to_path_buf())
        .collect();

    let mut group = c.benchmark_group("file_reading");

    // Benchmark sequential reading
    group.bench_function("sequential", |b| {
        b.iter(|| {
            let mut total_bytes = 0;
            for file in &files {
                if let Ok(content) = fs::read_to_string(file) {
                    total_bytes += content.len();
                }
            }
            black_box(total_bytes)
        })
    });

    // Benchmark parallel reading with rayon
    group.bench_function("parallel_rayon", |b| {
        use rayon::prelude::*;
        b.iter(|| {
            let total_bytes: usize = files
                .par_iter()
                .filter_map(|file| fs::read_to_string(file).ok())
                .map(|content| content.len())
                .sum();
            black_box(total_bytes)
        })
    });

    group.finish();
}

/// Benchmark line counting methods
fn bench_line_counting(c: &mut Criterion) {
    // Create a large file for benchmarking
    let temp = TempDir::new().unwrap();
    let large_file = temp.path().join("large.rs");

    let content: String = (0..10000)
        .map(|i| format!("fn function_{}() {{ /* code */ }}\n", i))
        .collect();
    fs::write(&large_file, &content).unwrap();

    let file_content = fs::read_to_string(&large_file).unwrap();

    let mut group = c.benchmark_group("line_counting");

    group.bench_function("lines_iterator", |b| {
        b.iter(|| black_box(file_content.lines().count()))
    });

    group.bench_function("matches_newline", |b| {
        b.iter(|| black_box(file_content.matches('\n').count() + 1))
    });

    group.bench_function("bytes_filter", |b| {
        b.iter(|| black_box(file_content.bytes().filter(|&b| b == b'\n').count() + 1))
    });

    group.finish();
}

/// Benchmark token estimation methods
fn bench_token_estimation(c: &mut Criterion) {
    // Sample code of varying sizes
    let small_code = "fn main() { println!(\"Hello\"); }";
    let medium_code: String = (0..100)
        .map(|i| format!("fn function_{}(x: i32) -> i32 {{ x * {} }}\n", i, i))
        .collect();
    let large_code: String = (0..1000)
        .map(|i| format!("fn function_{}(x: i32) -> i32 {{ x * {} }}\n", i, i))
        .collect();

    let mut group = c.benchmark_group("token_estimation");

    // Simple char-based estimation
    group.bench_function("char_ratio_small", |b| {
        b.iter(|| black_box((small_code.len() as f32 / 3.5) as u32))
    });

    group.bench_function("char_ratio_medium", |b| {
        b.iter(|| black_box((medium_code.len() as f32 / 3.5) as u32))
    });

    group.bench_function("char_ratio_large", |b| {
        b.iter(|| black_box((large_code.len() as f32 / 3.5) as u32))
    });

    // Word-based estimation (more accurate but slower)
    group.bench_function("word_based_small", |b| {
        b.iter(|| {
            let words = small_code.split_whitespace().count();
            let symbols = small_code.chars().filter(|c| !c.is_alphanumeric()).count();
            black_box(words + symbols / 2)
        })
    });

    group.bench_function("word_based_medium", |b| {
        b.iter(|| {
            let words = medium_code.split_whitespace().count();
            let symbols = medium_code.chars().filter(|c| !c.is_alphanumeric()).count();
            black_box(words + symbols / 2)
        })
    });

    group.bench_function("word_based_large", |b| {
        b.iter(|| {
            let words = large_code.split_whitespace().count();
            let symbols = large_code.chars().filter(|c| !c.is_alphanumeric()).count();
            black_box(words + symbols / 2)
        })
    });

    group.finish();
}

/// Benchmark output format generation
fn bench_output_generation(c: &mut Criterion) {
    // Simulated repository data
    let files: Vec<(String, String)> = (0..50)
        .map(|i| {
            let path = format!("src/module_{}.rs", i);
            let content: String = (0..20)
                .map(|j| format!("fn func_{}_{}_{}() {{ }}\n", i, j, j))
                .collect();
            (path, content)
        })
        .collect();

    let mut group = c.benchmark_group("output_generation");

    // XML generation
    group.bench_function("xml_format", |b| {
        b.iter(|| {
            let mut output = String::with_capacity(100_000);
            output.push_str("<repository>\n");
            output.push_str("  <files>\n");
            for (path, content) in &files {
                output.push_str(&format!("    <file path=\"{}\">\n", path));
                output.push_str("      <content><![CDATA[");
                output.push_str(content);
                output.push_str("]]></content>\n");
                output.push_str("    </file>\n");
            }
            output.push_str("  </files>\n");
            output.push_str("</repository>");
            black_box(output)
        })
    });

    // Markdown generation
    group.bench_function("markdown_format", |b| {
        b.iter(|| {
            let mut output = String::with_capacity(100_000);
            output.push_str("# Repository\n\n");
            for (path, content) in &files {
                output.push_str(&format!("## {}\n\n", path));
                output.push_str("```rust\n");
                output.push_str(content);
                output.push_str("```\n\n");
            }
            black_box(output)
        })
    });

    // JSON generation
    group.bench_function("json_format", |b| {
        b.iter(|| {
            let mut output = String::with_capacity(100_000);
            output.push_str("{\"files\":[");
            for (i, (path, content)) in files.iter().enumerate() {
                if i > 0 {
                    output.push(',');
                }
                output.push_str(&format!(
                    "{{\"path\":\"{}\",\"content\":{}}}",
                    path,
                    serde_json::to_string(content).unwrap()
                ));
            }
            output.push_str("]}");
            black_box(output)
        })
    });

    group.finish();
}

/// Benchmark language detection
fn bench_language_detection(c: &mut Criterion) {
    let extensions = [
        "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "c", "cpp", "h", "hpp", "rb", "php",
        "swift", "kt", "scala", "cs", "fs", "ml", "hs", "clj", "ex", "erl", "lua", "r", "jl",
        "zig", "nim", "cr", "d", "ada", "pas", "f90", "cob", "pl", "tcl", "sh", "bash", "zsh",
    ];

    let mut group = c.benchmark_group("language_detection");

    group.bench_function("match_extension", |b| {
        b.iter(|| {
            for ext in &extensions {
                let lang = match *ext {
                    "rs" => "rust",
                    "py" | "pyw" => "python",
                    "js" | "jsx" | "mjs" => "javascript",
                    "ts" | "tsx" => "typescript",
                    "go" => "go",
                    "java" => "java",
                    "c" | "h" => "c",
                    "cpp" | "cc" | "cxx" | "hpp" => "cpp",
                    _ => "unknown",
                };
                black_box(lang);
            }
        })
    });

    group.bench_function("hashmap_lookup", |b| {
        use std::collections::HashMap;
        let map: HashMap<&str, &str> = [
            ("rs", "rust"),
            ("py", "python"),
            ("pyw", "python"),
            ("js", "javascript"),
            ("jsx", "javascript"),
            ("mjs", "javascript"),
            ("ts", "typescript"),
            ("tsx", "typescript"),
            ("go", "go"),
            ("java", "java"),
            ("c", "c"),
            ("h", "c"),
            ("cpp", "cpp"),
            ("cc", "cpp"),
            ("cxx", "cpp"),
            ("hpp", "cpp"),
        ]
        .into_iter()
        .collect();

        b.iter(|| {
            for ext in &extensions {
                let lang = map.get(ext).copied().unwrap_or("unknown");
                black_box(lang);
            }
        })
    });

    group.finish();
}

/// Benchmark regex pattern matching for security scanning
fn bench_security_patterns(c: &mut Criterion) {
    use regex::Regex;

    let content = r#"
const AWS_KEY = "AKIAIOSFODNN7EXAMPLE";
const password = "mysecretpassword123";
const api_key = "sk-1234567890abcdef";
const token = "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
function connect() {
    const conn = "postgresql://user:pass@localhost/db";
}
"#
    .repeat(100);

    let mut group = c.benchmark_group("security_patterns");

    // Single pattern
    group.bench_function("single_regex", |b| {
        let re = Regex::new(r#"(?i)(password|secret|api_key|token)\s*[=:]\s*['"][^'"]+['"]"#)
            .unwrap();
        b.iter(|| black_box(re.find_iter(&content).count()))
    });

    // Multiple patterns compiled separately
    group.bench_function("multiple_regex", |b| {
        let patterns = vec![
            Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
            Regex::new(r#"(?i)password\s*[=:]\s*['"][^'"]+['"]"#).unwrap(),
            Regex::new(r#"(?i)api_key\s*[=:]\s*['"][^'"]+['"]"#).unwrap(),
            Regex::new(r"ghp_[a-zA-Z0-9]{36}").unwrap(),
        ];
        b.iter(|| {
            let mut count = 0;
            for re in &patterns {
                count += re.find_iter(&content).count();
            }
            black_box(count)
        })
    });

    // RegexSet for multiple patterns
    group.bench_function("regex_set", |b| {
        use regex::RegexSet;
        let set = RegexSet::new([
            r"AKIA[0-9A-Z]{16}",
            r#"(?i)password\s*[=:]\s*['"][^'"]+['"]"#,
            r#"(?i)api_key\s*[=:]\s*['"][^'"]+['"]"#,
            r"ghp_[a-zA-Z0-9]{36}",
        ])
        .unwrap();
        b.iter(|| black_box(set.matches(&content).iter().count()))
    });

    group.finish();
}

/// Benchmark string allocation strategies
fn bench_string_building(c: &mut Criterion) {
    let parts: Vec<String> = (0..1000)
        .map(|i| format!("Part {} of the content\n", i))
        .collect();

    let mut group = c.benchmark_group("string_building");

    // Push approach
    group.bench_function("push_string", |b| {
        b.iter(|| {
            let mut result = String::new();
            for part in &parts {
                result.push_str(part);
            }
            black_box(result)
        })
    });

    // Pre-allocated push
    group.bench_function("push_preallocated", |b| {
        b.iter(|| {
            let total_len: usize = parts.iter().map(|p| p.len()).sum();
            let mut result = String::with_capacity(total_len);
            for part in &parts {
                result.push_str(part);
            }
            black_box(result)
        })
    });

    // Collect approach
    group.bench_function("collect_join", |b| {
        b.iter(|| {
            let result: String = parts.iter().map(|s| s.as_str()).collect();
            black_box(result)
        })
    });

    // Collect with join
    group.bench_function("vec_join", |b| {
        b.iter(|| {
            let result = parts.join("");
            black_box(result)
        })
    });

    group.finish();
}

// Configure criterion
criterion_group!(
    benches,
    bench_file_traversal,
    bench_file_reading,
    bench_line_counting,
    bench_token_estimation,
    bench_output_generation,
    bench_language_detection,
    bench_security_patterns,
    bench_string_building,
);

criterion_main!(benches);
