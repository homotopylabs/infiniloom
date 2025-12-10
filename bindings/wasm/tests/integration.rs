//! Integration tests for Infiniloom WASM bindings

use infiniloom_wasm::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_version() {
    let ver = version();
    assert!(!ver.is_empty());
    assert!(ver.starts_with("0."));
}

#[wasm_bindgen_test]
fn test_token_counting() {
    let text = "fn main() { println!(\"Hello, world!\"); }";

    // Test individual model counting
    let claude_count = count_tokens(text, "claude").unwrap();
    assert!(claude_count > 0);
    assert!(claude_count < 50);

    let gpt4o_count = count_tokens(text, "gpt4o").unwrap();
    assert!(gpt4o_count > 0);
    assert!(gpt4o_count < 50);

    // Test all models at once
    let all_tokens = count_tokens_all(text);
    assert!(all_tokens.claude > 0);
    assert!(all_tokens.gpt4o > 0);
    assert!(all_tokens.gpt4 > 0);
    assert!(all_tokens.gemini > 0);
    assert!(all_tokens.llama > 0);
}

#[wasm_bindgen_test]
fn test_token_count_consistency() {
    let text = "Hello, world!";
    let tokens1 = count_tokens_all(text);
    let tokens2 = count_tokens_all(text);

    // Same input should produce same output
    assert_eq!(tokens1.claude, tokens2.claude);
    assert_eq!(tokens1.gpt4o, tokens2.gpt4o);
}

#[wasm_bindgen_test]
fn test_language_detection() {
    assert_eq!(detect_language("main.rs"), Some("rust".to_string()));
    assert_eq!(detect_language("app.py"), Some("python".to_string()));
    assert_eq!(detect_language("index.js"), Some("javascript".to_string()));
    assert_eq!(detect_language("test.ts"), Some("typescript".to_string()));
    assert_eq!(detect_language("Main.java"), Some("java".to_string()));
    assert_eq!(detect_language("main.go"), Some("go".to_string()));
    assert_eq!(detect_language("test.c"), Some("c".to_string()));
    assert_eq!(detect_language("test.cpp"), Some("cpp".to_string()));

    // Unknown extension
    assert_eq!(detect_language("file.xyz"), None);

    // No extension
    assert_eq!(detect_language("Makefile"), None);
}

#[wasm_bindgen_test]
fn test_process_file() {
    let filename = "test.rs";
    let content = "fn main() { println!(\"test\"); }";

    let info = process_file(filename, content);

    assert_eq!(info.path(), filename);
    assert_eq!(info.language(), Some("rust".to_string()));
    assert_eq!(info.size_bytes(), content.len());
    assert!(info.tokens().claude > 0);
}

#[wasm_bindgen_test]
fn test_compression_minimal() {
    let code = "fn main() {  \n\n\n    println!(\"hello\");  \n\n}";

    let compressed = compress(code, CompressionLevel::Minimal, Some("rust".to_string()));

    // Should remove trailing whitespace
    assert!(!compressed.contains("  \n"));

    // Should reduce excessive blank lines
    assert!(!compressed.contains("\n\n\n"));

    // Should preserve content
    assert!(compressed.contains("fn main()"));
    assert!(compressed.contains("println!"));
}

#[wasm_bindgen_test]
fn test_compression_balanced() {
    let code = r#"
// This is a comment
fn main() {
    // Another comment
    println!("hello");
}
"#;

    let compressed = compress(code, CompressionLevel::Balanced, Some("rust".to_string()));

    // Should remove comments
    assert!(!compressed.contains("// This is a comment"));
    assert!(!compressed.contains("// Another comment"));

    // Should preserve code
    assert!(compressed.contains("fn main()"));
    assert!(compressed.contains("println!"));
}

#[wasm_bindgen_test]
fn test_compression_aggressive() {
    let code = r#"
/// Documentation comment
fn main() {
    // Regular comment
    println!("hello");
}
"#;

    let compressed = compress(code, CompressionLevel::Aggressive, Some("rust".to_string()));

    // Should be significantly smaller
    assert!(compressed.len() < code.len());

    // Should preserve essential structure
    assert!(compressed.contains("fn main()"));
}

#[wasm_bindgen_test]
fn test_generate_context_claude() {
    use wasm_bindgen::JsValue;
    use serde_json::json;

    let files = json!([
        ["test.rs", "fn main() {}"],
        ["lib.rs", "pub fn hello() {}"]
    ]);

    let context = generate_context(
        JsValue::from_serde(&files).unwrap(),
        OutputFormat::Claude,
        CompressionLevel::None,
    ).unwrap();

    // Should be XML
    assert!(context.contains("<?xml"));
    assert!(context.contains("<repository>"));
    assert!(context.contains("<file path=\"test.rs\""));
    assert!(context.contains("<file path=\"lib.rs\""));
    assert!(context.contains("fn main()"));
    assert!(context.contains("pub fn hello()"));
}

#[wasm_bindgen_test]
fn test_generate_context_gpt() {
    use wasm_bindgen::JsValue;
    use serde_json::json;

    let files = json!([
        ["test.py", "print('hello')"]
    ]);

    let context = generate_context(
        JsValue::from_serde(&files).unwrap(),
        OutputFormat::GPT,
        CompressionLevel::None,
    ).unwrap();

    // Should be Markdown
    assert!(context.contains("# Repository Context"));
    assert!(context.contains("## test.py"));
    assert!(context.contains("```python"));
    assert!(context.contains("print('hello')"));
}

#[wasm_bindgen_test]
fn test_generate_context_gemini() {
    use wasm_bindgen::JsValue;
    use serde_json::json;

    let files = json!([
        ["test.js", "console.log('test');"]
    ]);

    let context = generate_context(
        JsValue::from_serde(&files).unwrap(),
        OutputFormat::Gemini,
        CompressionLevel::None,
    ).unwrap();

    // Should be YAML-like
    assert!(context.contains("repository:"));
    assert!(context.contains("files:"));
    assert!(context.contains("path: test.js"));
    assert!(context.contains("console.log"));
}

#[wasm_bindgen_test]
fn test_calculate_stats() {
    use wasm_bindgen::JsValue;
    use serde_json::json;

    let files = json!([
        ["file1.js", "console.log('hello');"],
        ["file2.py", "print('world')"],
        ["file3.rs", "fn main() {}"]
    ]);

    let stats = calculate_stats(JsValue::from_serde(&files).unwrap()).unwrap();

    assert_eq!(stats.total_files(), 3);
    assert!(stats.total_bytes() > 0);
    assert!(stats.total_lines() > 0);
    assert!(stats.tokens_claude() > 0);
    assert!(stats.tokens_gpt4o() > 0);
}

#[wasm_bindgen_test]
fn test_empty_input() {
    let empty = "";

    let tokens = count_tokens_all(empty);
    assert_eq!(tokens.claude, 0);
    assert_eq!(tokens.gpt4o, 0);

    let compressed = compress(empty, CompressionLevel::Minimal, None);
    assert_eq!(compressed, "");
}

#[wasm_bindgen_test]
fn test_large_input() {
    // Test with larger input
    let large_code = "fn test() {}\n".repeat(1000);

    let tokens = count_tokens_all(&large_code);
    assert!(tokens.claude > 100);
    assert!(tokens.claude < 10000);

    let compressed = compress(&large_code, CompressionLevel::Balanced, Some("rust".to_string()));
    assert!(compressed.len() < large_code.len());
}

#[wasm_bindgen_test]
fn test_unicode_handling() {
    let unicode = "fn main() { println!(\"Hello 世界 🌍\"); }";

    let tokens = count_tokens_all(unicode);
    assert!(tokens.claude > 0);

    let compressed = compress(unicode, CompressionLevel::Minimal, Some("rust".to_string()));
    assert!(compressed.contains("世界"));
    assert!(compressed.contains("🌍"));
}

#[wasm_bindgen_test]
fn test_special_characters() {
    let code = r#"fn main() { let s = "quotes \"and\" escapes\n"; }"#;

    let tokens = count_tokens_all(code);
    assert!(tokens.claude > 0);

    let compressed = compress(code, CompressionLevel::Minimal, Some("rust".to_string()));
    assert!(compressed.contains(r#"\"and\""#));
}

#[wasm_bindgen_test]
fn test_xml_escaping() {
    use wasm_bindgen::JsValue;
    use serde_json::json;

    let files = json!([
        ["test.rs", "fn main() { let x = 5 < 10 && 15 > 10; }"]
    ]);

    let context = generate_context(
        JsValue::from_serde(&files).unwrap(),
        OutputFormat::Claude,
        CompressionLevel::None,
    ).unwrap();

    // Should escape XML special characters
    assert!(context.contains("&lt;"));
    assert!(context.contains("&gt;"));
    assert!(context.contains("&amp;"));
}

#[wasm_bindgen_test]
fn test_compression_levels() {
    let code = r#"
// Comment
fn main() {
    // Another comment
    println!("hello");
}
"#;

    let none = compress(code, CompressionLevel::None, Some("rust".to_string()));
    let minimal = compress(code, CompressionLevel::Minimal, Some("rust".to_string()));
    let balanced = compress(code, CompressionLevel::Balanced, Some("rust".to_string()));
    let aggressive = compress(code, CompressionLevel::Aggressive, Some("rust".to_string()));

    // Each level should compress more
    assert_eq!(none.len(), code.len());
    assert!(minimal.len() < none.len());
    assert!(balanced.len() < minimal.len());
    assert!(aggressive.len() <= balanced.len());
}

#[wasm_bindgen_test]
fn test_multiple_files_context() {
    use wasm_bindgen::JsValue;
    use serde_json::json;

    let files = json!([
        ["main.rs", "fn main() {}"],
        ["lib.rs", "pub fn lib() {}"],
        ["utils.rs", "pub fn util() {}"],
        ["config.rs", "pub struct Config {}"]
    ]);

    let context = generate_context(
        JsValue::from_serde(&files).unwrap(),
        OutputFormat::Claude,
        CompressionLevel::Balanced,
    ).unwrap();

    // Should include all files
    assert!(context.contains("main.rs"));
    assert!(context.contains("lib.rs"));
    assert!(context.contains("utils.rs"));
    assert!(context.contains("config.rs"));
}
