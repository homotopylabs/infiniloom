const std = @import("std");
const tokenizer = @import("tokenizer/counter.zig");
const compressor = @import("compressor/rules.zig");

// WASM-specific allocator
var wasm_allocator = std.heap.wasm_allocator;

// ============================================================================
// Memory Management (exported for JavaScript)
// ============================================================================

/// Allocate memory from WASM heap
export fn wasm_alloc(size: usize) [*]u8 {
    const slice = wasm_allocator.alloc(u8, size) catch return @ptrFromInt(0);
    return slice.ptr;
}

/// Free memory back to WASM heap
export fn wasm_free(ptr: [*]u8, size: usize) void {
    const slice = ptr[0..size];
    wasm_allocator.free(slice);
}

// ============================================================================
// Token Counting
// ============================================================================

/// Count tokens for Claude model
export fn wasm_count_tokens_claude(text: [*]const u8, len: usize) u32 {
    return tokenizer.Tokenizer.quickEstimate(text[0..len], .claude);
}

/// Count tokens for GPT-4o model
export fn wasm_count_tokens_gpt4o(text: [*]const u8, len: usize) u32 {
    return tokenizer.Tokenizer.quickEstimate(text[0..len], .gpt4o);
}

/// Count tokens for GPT-4 model
export fn wasm_count_tokens_gpt4(text: [*]const u8, len: usize) u32 {
    return tokenizer.Tokenizer.quickEstimate(text[0..len], .gpt4);
}

/// Count tokens for Gemini model
export fn wasm_count_tokens_gemini(text: [*]const u8, len: usize) u32 {
    return tokenizer.Tokenizer.quickEstimate(text[0..len], .gemini);
}

/// Count tokens for all models, returns packed u32 array
/// Format: [claude, gpt4o, gpt4, gemini, llama]
export fn wasm_count_tokens_all(text: [*]const u8, len: usize, out: [*]u32) void {
    const text_slice = text[0..len];
    var tok = tokenizer.Tokenizer.init(wasm_allocator);
    const counts = tok.countAllModels(text_slice);

    out[0] = counts.claude;
    out[1] = counts.gpt4o;
    out[2] = counts.gpt4;
    out[3] = counts.gemini;
    out[4] = counts.llama;
}

// ============================================================================
// Compression
// ============================================================================

/// Compress code with minimal level
export fn wasm_compress_minimal(
    text: [*]const u8,
    text_len: usize,
    out: [*]u8,
    out_capacity: usize,
) i32 {
    return compressWithLevel(text, text_len, out, out_capacity, .minimal, .unknown);
}

/// Compress code with balanced level
export fn wasm_compress_balanced(
    text: [*]const u8,
    text_len: usize,
    lang: u8,
    out: [*]u8,
    out_capacity: usize,
) i32 {
    const language = langFromInt(lang);
    return compressWithLevel(text, text_len, out, out_capacity, .balanced, language);
}

/// Compress code with aggressive level
export fn wasm_compress_aggressive(
    text: [*]const u8,
    text_len: usize,
    lang: u8,
    out: [*]u8,
    out_capacity: usize,
) i32 {
    const language = langFromInt(lang);
    return compressWithLevel(text, text_len, out, out_capacity, .aggressive, language);
}

fn compressWithLevel(
    text: [*]const u8,
    text_len: usize,
    out: [*]u8,
    out_capacity: usize,
    level: compressor.CompressionLevel,
    language: compressor.Language,
) i32 {
    var comp = compressor.Compressor.init(wasm_allocator, .{ .level = level });

    const result = comp.compress(text[0..text_len], language) catch return -1;
    defer wasm_allocator.free(result);

    if (result.len > out_capacity) {
        return -2; // Buffer too small
    }

    @memcpy(out[0..result.len], result);
    return @intCast(result.len);
}

fn langFromInt(lang: u8) compressor.Language {
    return switch (lang) {
        0 => .python,
        1 => .javascript,
        2 => .typescript,
        3 => .rust,
        4 => .go,
        5 => .java,
        6 => .c,
        7 => .cpp,
        8 => .csharp,
        9 => .ruby,
        10 => .php,
        else => .unknown,
    };
}

// ============================================================================
// Language Detection
// ============================================================================

/// Detect language from filename extension
/// Returns language code (0=python, 1=js, 2=ts, etc.) or 255 for unknown
export fn wasm_detect_language(filename: [*]const u8, len: usize) u8 {
    const name = filename[0..len];

    // Find extension
    var ext_start: usize = len;
    while (ext_start > 0) {
        ext_start -= 1;
        if (name[ext_start] == '.') {
            break;
        }
    }

    if (ext_start == 0) return 255;

    const ext = name[ext_start..];

    if (std.mem.eql(u8, ext, ".py")) return 0;
    if (std.mem.eql(u8, ext, ".js")) return 1;
    if (std.mem.eql(u8, ext, ".ts")) return 2;
    if (std.mem.eql(u8, ext, ".rs")) return 3;
    if (std.mem.eql(u8, ext, ".go")) return 4;
    if (std.mem.eql(u8, ext, ".java")) return 5;
    if (std.mem.eql(u8, ext, ".c") or std.mem.eql(u8, ext, ".h")) return 6;
    if (std.mem.eql(u8, ext, ".cpp") or std.mem.eql(u8, ext, ".hpp")) return 7;
    if (std.mem.eql(u8, ext, ".cs")) return 8;
    if (std.mem.eql(u8, ext, ".rb")) return 9;
    if (std.mem.eql(u8, ext, ".php")) return 10;

    return 255;
}

// ============================================================================
// Utility
// ============================================================================

/// Get version string
export fn wasm_version() [*:0]const u8 {
    return "0.1.0";
}

/// Check if text appears to be binary
export fn wasm_is_binary(data: [*]const u8, len: usize) bool {
    const binary = @import("scanner/binary.zig");
    return binary.isBinary(data[0..len]);
}
