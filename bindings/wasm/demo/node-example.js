#!/usr/bin/env node

/**
 * CodeLoom WASM - Node.js Example
 *
 * This example demonstrates using CodeLoom WASM in Node.js
 */

const {
    count_tokens_all,
    count_tokens,
    generate_context,
    detect_language,
    calculate_stats,
    OutputFormat,
    CompressionLevel,
} = require('../pkg-node/codeloom_wasm.js');

console.log('🧬 CodeLoom WASM - Node.js Example\n');

// Example 1: Token Counting
console.log('=== Token Counting ===');
const code = `
fn main() {
    println!("Hello, world!");

    let numbers = vec![1, 2, 3, 4, 5];
    let sum: i32 = numbers.iter().sum();

    println!("Sum: {}", sum);
}
`;

const tokens = count_tokens_all(code);
console.log('Token counts for all models:');
console.log('  Claude:  ', tokens.claude);
console.log('  GPT-4o:  ', tokens.gpt4o);
console.log('  GPT-4:   ', tokens.gpt4);
console.log('  Gemini:  ', tokens.gemini);
console.log('  Llama:   ', tokens.llama);
console.log();

// Example 2: Language Detection
console.log('=== Language Detection ===');
const filenames = ['main.rs', 'app.py', 'index.js', 'test.go', 'Main.java'];
filenames.forEach(filename => {
    const lang = detect_language(filename);
    console.log(`  ${filename.padEnd(12)} -> ${lang || 'unknown'}`);
});
console.log();

// Example 3: Context Generation (Claude XML)
console.log('=== Claude XML Context ===');
const files = [
    ['src/main.rs', code],
    ['src/lib.rs', 'pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}'],
];

const claudeContext = generate_context(
    files,
    OutputFormat.Claude,
    CompressionLevel.Balanced
);
console.log(claudeContext.substring(0, 500) + '...\n');

// Example 4: Context Generation (GPT Markdown)
console.log('=== GPT Markdown Context ===');
const gptContext = generate_context(
    files,
    OutputFormat.GPT,
    CompressionLevel.Minimal
);
console.log(gptContext.substring(0, 400) + '...\n');

// Example 5: Repository Statistics
console.log('=== Repository Statistics ===');
const moreFiles = [
    ['file1.js', 'console.log("hello");'.repeat(10)],
    ['file2.py', 'print("world")'.repeat(20)],
    ['file3.rs', 'fn main() {}'.repeat(15)],
];

const stats = calculate_stats(moreFiles);
console.log('Total files:  ', stats.total_files);
console.log('Total bytes:  ', stats.total_bytes);
console.log('Total lines:  ', stats.total_lines);
console.log('\nTokens by model:');
console.log('  Claude:  ', stats.tokens_claude);
console.log('  GPT-4o:  ', stats.tokens_gpt4o);
console.log('  GPT-4:   ', stats.tokens_gpt4);
console.log('  Gemini:  ', stats.tokens_gemini);
console.log('  Llama:   ', stats.tokens_llama);
console.log();

// Example 6: Compression Comparison
console.log('=== Compression Comparison ===');
const longCode = `
// This is a detailed comment explaining the function
// It spans multiple lines to show compression effects
fn calculate_fibonacci(n: u32) -> u64 {
    // Base cases
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return 1;
    }

    // Recursive case
    calculate_fibonacci(n - 1) + calculate_fibonacci(n - 2)
}

// Main function with detailed comments
fn main() {
    // Print header
    println!("Fibonacci Calculator");
    println!("====================");

    // Calculate and print first 10 numbers
    for i in 0..10 {
        let result = calculate_fibonacci(i);
        println!("F({}) = {}", i, result);
    }
}
`;

console.log('Original size: ', longCode.length, 'bytes');

const levels = [
    ['None', CompressionLevel.None],
    ['Minimal', CompressionLevel.Minimal],
    ['Balanced', CompressionLevel.Balanced],
    ['Aggressive', CompressionLevel.Aggressive],
];

levels.forEach(([name, level]) => {
    const files = [['test.rs', longCode]];
    const compressed = generate_context(files, OutputFormat.Plain, level);
    const reduction = ((longCode.length - compressed.length) / longCode.length * 100).toFixed(1);
    console.log(`  ${name.padEnd(12)}: ${compressed.length} bytes (-${reduction}%)`);
});

console.log('\n✅ All examples completed!');
