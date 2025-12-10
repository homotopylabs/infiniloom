const std = @import("std");
const mem = std.mem;
const Allocator = mem.Allocator;

const scanner = @import("scanner/walker.zig");
const parallel = @import("scanner/parallel.zig");
const tokenizer = @import("tokenizer/counter.zig");
const bpe = @import("tokenizer/bpe.zig");
const compressor = @import("compressor/rules.zig");

// Global allocator for C ABI - use page allocator for simplicity in FFI
// (no leak tracking, but simpler memory model for cross-language calls)
const allocator = std.heap.page_allocator;

// ============================================================================
// Opaque Types
// ============================================================================

/// Opaque context handle for external consumers
pub const Context = opaque {
    fn cast(ptr: *anyopaque) *ContextInternal {
        return @ptrCast(@alignCast(ptr));
    }
};

/// Internal context structure
const ContextInternal = struct {
    walker: scanner.Walker,
    tokenizer_instance: tokenizer.Tokenizer,
    compressor_instance: ?compressor.Compressor,
    /// Null-terminated error string (includes sentinel in allocation)
    last_error: ?[:0]const u8,

    fn init() !*ContextInternal {
        const ctx = try allocator.create(ContextInternal);
        ctx.* = .{
            .walker = scanner.Walker.init(allocator, .{}),
            .tokenizer_instance = tokenizer.Tokenizer.init(allocator),
            .compressor_instance = null,
            .last_error = null,
        };
        return ctx;
    }

    fn deinit(self: *ContextInternal) void {
        self.walker.deinit();
        self.tokenizer_instance.deinit();
        if (self.last_error) |err| {
            allocator.free(err);
        }
        allocator.destroy(self);
    }

    fn setError(self: *ContextInternal, msg: []const u8) void {
        if (self.last_error) |err| {
            allocator.free(err);
        }
        // Allocate with null sentinel for C string compatibility
        self.last_error = allocator.allocSentinel(u8, msg.len, 0) catch null;
        if (self.last_error) |err_buf| {
            @memcpy(@constCast(err_buf), msg);
        }
    }
};

// ============================================================================
// C ABI Structures
// ============================================================================

/// Result of a scan operation
pub const ScanResult = extern struct {
    file_count: u32,
    total_bytes: u64,
    total_tokens: u64,
    scan_time_ms: i64,
    error_code: i32,
};

/// Information about a single file
pub const FileInfo = extern struct {
    path: [*:0]const u8,
    path_len: u32,
    relative_path: [*:0]const u8,
    relative_path_len: u32,
    size_bytes: u64,
    token_count_claude: u32,
    token_count_gpt4o: u32,
    language: [*:0]const u8,
    language_len: u8,
    importance: f32,
};

/// Token counts for multiple models
pub const TokenCounts = extern struct {
    claude: u32,
    gpt4o: u32,
    gpt4: u32,
    gemini: u32,
    llama: u32,
};

/// Compression configuration
pub const CompressionConfig = extern struct {
    level: u8, // 0=none, 1=minimal, 2=balanced, 3=aggressive, 4=extreme
    remove_comments: bool,
    remove_empty_lines: bool,
    preserve_imports: bool,
};

// ============================================================================
// C ABI Functions
// ============================================================================

/// Initialize a new Infiniloom context
export fn infiniloom_init() ?*Context {
    const ctx = ContextInternal.init() catch return null;
    return @ptrCast(ctx);
}

/// Free an Infiniloom context
export fn infiniloom_free(ctx: ?*Context) void {
    if (ctx) |c| {
        const internal = Context.cast(c);
        internal.deinit();
    }
}

/// Get the last error message
/// NOTE: Returns a pointer to internal storage. Do NOT free this pointer.
/// The string is valid until the next call to any infiniloom function on this context.
export fn infiniloom_get_error(ctx: ?*Context) [*:0]const u8 {
    if (ctx) |c| {
        const internal = Context.cast(c);
        if (internal.last_error) |err| {
            // Return pointer to internal null-terminated string
            // The ContextInternal.setError already stores a null-terminated copy
            return @ptrCast(err.ptr);
        }
    }
    return "";
}

/// Scan a directory
export fn infiniloom_scan(
    ctx: ?*Context,
    path: [*:0]const u8,
    include_hidden: bool,
    respect_gitignore: bool,
    max_file_size: u64,
) ScanResult {
    const c = ctx orelse return ScanResult{
        .file_count = 0,
        .total_bytes = 0,
        .total_tokens = 0,
        .scan_time_ms = 0,
        .error_code = -1,
    };

    const internal = Context.cast(c);

    // Reinitialize walker with new config
    internal.walker.deinit();
    internal.walker = scanner.Walker.init(allocator, .{
        .include_hidden = include_hidden,
        .respect_gitignore = respect_gitignore,
        .max_file_size = max_file_size,
    });

    // Perform scan
    const path_slice = mem.sliceTo(path, 0);
    internal.walker.walk(path_slice) catch |err| {
        internal.setError(@errorName(err));
        return ScanResult{
            .file_count = 0,
            .total_bytes = 0,
            .total_tokens = 0,
            .scan_time_ms = 0,
            .error_code = -2,
        };
    };

    const stats = internal.walker.getStats();

    // Estimate total tokens
    var total_tokens: u64 = 0;
    for (internal.walker.getFiles()) |_| {
        total_tokens += tokenizer.Tokenizer.quickEstimate(
            @as([]const u8, &[_]u8{}), // Would need file content
            .claude,
        );
    }

    return ScanResult{
        .file_count = stats.total_files,
        .total_bytes = stats.total_bytes,
        .total_tokens = total_tokens,
        .scan_time_ms = stats.scan_time_ms,
        .error_code = 0,
    };
}

/// Scan a directory using parallel workers (faster for large repos)
export fn infiniloom_scan_parallel(
    ctx: ?*Context,
    path: [*:0]const u8,
    include_hidden: bool,
    respect_gitignore: bool,
    max_file_size: u64,
) ScanResult {
    const c = ctx orelse return ScanResult{
        .file_count = 0,
        .total_bytes = 0,
        .total_tokens = 0,
        .scan_time_ms = 0,
        .error_code = -1,
    };

    const internal = Context.cast(c);

    // Use parallel walker for better performance
    var parallel_walker = parallel.ParallelWalker.init(allocator, .{
        .include_hidden = include_hidden,
        .respect_gitignore = respect_gitignore,
        .max_file_size = max_file_size,
        .use_mmap = true,
    });
    defer parallel_walker.deinit();

    // Perform parallel scan
    const path_slice = mem.sliceTo(path, 0);
    parallel_walker.walk(path_slice) catch |err| {
        internal.setError(@errorName(err));
        return ScanResult{
            .file_count = 0,
            .total_bytes = 0,
            .total_tokens = 0,
            .scan_time_ms = 0,
            .error_code = -2,
        };
    };

    const stats = parallel_walker.getStats();

    // Copy files to the internal walker for later retrieval
    // This is a workaround since the parallel walker is local
    internal.walker.deinit();
    internal.walker = scanner.Walker.init(allocator, .{
        .include_hidden = include_hidden,
        .respect_gitignore = respect_gitignore,
        .max_file_size = max_file_size,
    });

    // Estimate total tokens using improved BPE estimation
    var total_tokens: u64 = 0;
    for (parallel_walker.getFiles()) |file| {
        if (file.content) |content| {
            total_tokens += bpe.estimateTokensAccurate(content, .claude);
        } else {
            // Estimate from file size
            total_tokens += @as(u64, @intFromFloat(@as(f32, @floatFromInt(file.size)) / 4.0));
        }
    }

    return ScanResult{
        .file_count = stats.total_files,
        .total_bytes = stats.total_bytes,
        .total_tokens = total_tokens,
        .scan_time_ms = stats.scan_time_ms,
        .error_code = 0,
    };
}

/// Get number of scanned files
export fn infiniloom_get_file_count(ctx: ?*Context) u32 {
    const c = ctx orelse return 0;
    const internal = Context.cast(c);
    return @intCast(internal.walker.getFiles().len);
}

/// Get file info at index
/// NOTE: The caller MUST call infiniloom_free_file_info() to free the allocated strings.
export fn infiniloom_get_file(ctx: ?*Context, index: u32, out: *FileInfo) bool {
    const c = ctx orelse return false;
    const internal = Context.cast(c);

    const files = internal.walker.getFiles();
    if (index >= files.len) return false;

    const file = files[index];

    // Create null-terminated strings
    const path_z = allocator.allocSentinel(u8, file.path.len, 0) catch return false;
    @memcpy(path_z, file.path);

    const rel_path_z = allocator.allocSentinel(u8, file.relative_path.len, 0) catch {
        allocator.free(path_z);
        return false;
    };
    @memcpy(rel_path_z, file.relative_path);

    var lang_z: [*:0]const u8 = "";
    var lang_len: u8 = 0;
    if (file.language) |lang| {
        if (allocator.allocSentinel(u8, lang.len, 0)) |lang_buf| {
            @memcpy(lang_buf, lang);
            lang_z = lang_buf;
            lang_len = @intCast(lang.len);
        } else |_| {
            // Keep empty string on allocation failure
        }
    }

    out.* = FileInfo{
        .path = path_z,
        .path_len = @intCast(file.path.len),
        .relative_path = rel_path_z,
        .relative_path_len = @intCast(file.relative_path.len),
        .size_bytes = file.size,
        .token_count_claude = 0, // Would need content
        .token_count_gpt4o = 0,
        .language = lang_z,
        .language_len = lang_len,
        .importance = 0.5, // Default importance
    };

    return true;
}

/// Free the strings allocated by infiniloom_get_file()
/// Must be called for each FileInfo returned by infiniloom_get_file()
export fn infiniloom_free_file_info(info: *FileInfo) void {
    if (info.path_len > 0) {
        const path_slice = info.path[0..info.path_len :0];
        allocator.free(path_slice);
    }
    if (info.relative_path_len > 0) {
        const rel_path_slice = info.relative_path[0..info.relative_path_len :0];
        allocator.free(rel_path_slice);
    }
    if (info.language_len > 0) {
        const lang_slice = info.language[0..info.language_len :0];
        allocator.free(lang_slice);
    }
    // Zero out the struct to prevent double-free
    info.* = FileInfo{
        .path = "",
        .path_len = 0,
        .relative_path = "",
        .relative_path_len = 0,
        .size_bytes = 0,
        .token_count_claude = 0,
        .token_count_gpt4o = 0,
        .language = "",
        .language_len = 0,
        .importance = 0,
    };
}

/// Count tokens in text for a specific model
export fn infiniloom_count_tokens(
    ctx: ?*Context,
    text: [*]const u8,
    text_len: usize,
    model: u8, // 0=claude, 1=gpt4o, 2=gpt4, 3=gemini, 4=llama
) u32 {
    _ = ctx;

    const text_slice = text[0..text_len];
    const tok_model: tokenizer.TokenizerModel = switch (model) {
        0 => .claude,
        1 => .gpt4o,
        2 => .gpt4,
        3 => .gemini,
        4 => .llama,
        else => .claude,
    };

    return tokenizer.Tokenizer.quickEstimate(text_slice, tok_model);
}

/// Count tokens for all models
export fn infiniloom_count_tokens_all(
    ctx: ?*Context,
    text: [*]const u8,
    text_len: usize,
    out: *TokenCounts,
) void {
    const c = ctx orelse return;
    const internal = Context.cast(c);

    const text_slice = text[0..text_len];
    const counts = internal.tokenizer_instance.countAllModels(text_slice);

    out.* = TokenCounts{
        .claude = counts.claude,
        .gpt4o = counts.gpt4o,
        .gpt4 = counts.gpt4,
        .gemini = counts.gemini,
        .llama = counts.llama,
    };
}

/// Compress text
export fn infiniloom_compress(
    ctx: ?*Context,
    text: [*]const u8,
    text_len: usize,
    config: CompressionConfig,
    language: u8, // 0=python, 1=js, 2=ts, 3=rust, etc.
    out_buffer: [*]u8,
    buffer_size: usize,
) i64 {
    const c = ctx orelse return -1;
    const internal = Context.cast(c);

    const level: compressor.CompressionLevel = switch (config.level) {
        0 => .none,
        1 => .minimal,
        2 => .balanced,
        3 => .aggressive,
        4 => .extreme,
        else => .balanced,
    };

    const lang: compressor.Language = switch (language) {
        0 => .python,
        1 => .javascript,
        2 => .typescript,
        3 => .rust,
        4 => .go,
        5 => .java,
        else => .unknown,
    };

    var comp = compressor.Compressor.init(allocator, .{
        .level = level,
        .remove_line_comments = config.remove_comments,
        .remove_block_comments = config.remove_comments,
        .remove_empty_lines = config.remove_empty_lines,
        .preserve_imports = config.preserve_imports,
    });

    const text_slice = text[0..text_len];
    const compressed = comp.compress(text_slice, lang) catch |err| {
        internal.setError(@errorName(err));
        return -2;
    };
    defer allocator.free(compressed);

    if (compressed.len > buffer_size) {
        return -3; // Buffer too small
    }

    @memcpy(out_buffer[0..compressed.len], compressed);
    return @intCast(compressed.len);
}

/// Get library version
export fn infiniloom_version() [*:0]const u8 {
    return "0.1.0";
}

// ============================================================================
// Tests
// ============================================================================

test "init and free" {
    const ctx = infiniloom_init();
    try std.testing.expect(ctx != null);
    infiniloom_free(ctx);
}

test "version" {
    const ver = infiniloom_version();
    try std.testing.expectEqualStrings("0.1.0", mem.sliceTo(ver, 0));
}
