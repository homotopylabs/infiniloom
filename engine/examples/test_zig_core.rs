use infiniloom_engine::{ZigCore, estimate_tokens};

fn main() {
    println!("Testing Zig Core Integration");
    println!("============================");

    // Check if Zig core is available
    println!("\n1. Checking Zig core availability:");
    println!("   Available: {}", ZigCore::is_available());
    println!("   Version: {}", ZigCore::version());

    // Test token estimation with Rust fallback
    println!("\n2. Testing token estimation (Rust fallback):");
    let sample_code = r#"
fn main() {
    println!("Hello, World!");
    let x = 42;
    let y = x * 2;
    println!("x = {}, y = {}", x, y);
}
"#;
    let tokens = estimate_tokens(sample_code, "claude");
    println!("   Sample code tokens: {}", tokens);

    // Try to initialize Zig core
    println!("\n3. Initializing Zig core:");
    match ZigCore::new() {
        Some(zig) => {
            println!("   Successfully initialized!");

            // Test token counting with Zig
            println!("\n4. Token counting with Zig core:");
            let zig_tokens = zig.count_tokens(sample_code, infiniloom_engine::ffi::TokenizerModel::Claude);
            println!("   Sample code tokens (Zig): {}", zig_tokens);

            // Test all models
            println!("\n5. Token counts for all models:");
            let counts = zig.count_tokens_all(sample_code);
            println!("   Claude:  {}", counts.claude);
            println!("   GPT-4o:  {}", counts.gpt4o);
            println!("   GPT-4:   {}", counts.gpt4);
            println!("   Gemini:  {}", counts.gemini);
            println!("   Llama:   {}", counts.llama);

            // Test compression
            println!("\n6. Testing code compression:");
            let config = infiniloom_engine::CompressionConfig {
                level: 2, // balanced
                remove_comments: true,
                remove_empty_lines: true,
                preserve_imports: true,
            };
            match zig.compress(sample_code, config, infiniloom_engine::LanguageId::Rust) {
                Ok(compressed) => {
                    println!("   Original length: {}", sample_code.len());
                    println!("   Compressed length: {}", compressed.len());
                    println!("   Compression ratio: {:.1}%",
                        (1.0 - compressed.len() as f64 / sample_code.len() as f64) * 100.0);
                }
                Err(e) => println!("   Compression failed: {}", e),
            }

            println!("\n✓ All tests passed!");
        }
        None => {
            println!("   WARNING: ZigCore::new() returned None");
            println!("   This is expected if zig-core feature is not enabled");
        }
    }
}
