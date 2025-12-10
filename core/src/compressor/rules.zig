const std = @import("std");
const mem = std.mem;
const Allocator = mem.Allocator;
const ArrayList = std.ArrayList;

/// Compression level presets
pub const CompressionLevel = enum {
    /// No compression - raw output
    none,
    /// Remove trailing whitespace and empty lines
    minimal,
    /// Remove comments and normalize whitespace
    balanced,
    /// Remove docstrings, keep only signatures
    aggressive,
    /// Extract key symbols only
    extreme,

    pub fn description(self: CompressionLevel) []const u8 {
        return switch (self) {
            .none => "No compression",
            .minimal => "Remove empty lines, trim whitespace",
            .balanced => "Remove comments, normalize whitespace",
            .aggressive => "Signatures only, remove docstrings",
            .extreme => "Key symbols only",
        };
    }

    pub fn expectedReduction(self: CompressionLevel) u8 {
        return switch (self) {
            .none => 0,
            .minimal => 15,
            .balanced => 35,
            .aggressive => 60,
            .extreme => 80,
        };
    }
};

/// Configuration for compression
pub const CompressionConfig = struct {
    level: CompressionLevel = .balanced,
    /// Remove single-line comments
    remove_line_comments: bool = true,
    /// Remove block comments
    remove_block_comments: bool = true,
    /// Remove docstrings/doc comments
    remove_docstrings: bool = false,
    /// Remove empty lines
    remove_empty_lines: bool = true,
    /// Collapse multiple spaces to one
    normalize_whitespace: bool = true,
    /// Remove trailing whitespace
    trim_trailing: bool = true,
    /// Preserve import statements
    preserve_imports: bool = true,
    /// Preserve function/class signatures
    preserve_signatures: bool = true,
    /// Maximum consecutive newlines (0 = unlimited)
    max_consecutive_newlines: u8 = 2,
};

/// Programming language for syntax-aware compression
pub const Language = enum {
    python,
    javascript,
    typescript,
    rust,
    go,
    java,
    c,
    cpp,
    csharp,
    ruby,
    php,
    unknown,

    pub fn lineCommentPrefix(self: Language) ?[]const u8 {
        return switch (self) {
            .python, .ruby => "#",
            .javascript, .typescript, .rust, .go, .java, .c, .cpp, .csharp, .php => "//",
            .unknown => null,
        };
    }

    pub fn blockCommentStart(self: Language) ?[]const u8 {
        return switch (self) {
            .python => "\"\"\"",
            .javascript, .typescript, .rust, .go, .java, .c, .cpp, .csharp, .php => "/*",
            .ruby => "=begin",
            .unknown => null,
        };
    }

    pub fn blockCommentEnd(self: Language) ?[]const u8 {
        return switch (self) {
            .python => "\"\"\"",
            .javascript, .typescript, .rust, .go, .java, .c, .cpp, .csharp, .php => "*/",
            .ruby => "=end",
            .unknown => null,
        };
    }
};

/// Rule-based code compressor
pub const Compressor = struct {
    allocator: Allocator,
    config: CompressionConfig,

    const Self = @This();

    pub fn init(allocator: Allocator, config: CompressionConfig) Self {
        return Self{
            .allocator = allocator,
            .config = config,
        };
    }

    /// Compress source code based on configuration
    pub fn compress(self: *Self, source: []const u8, language: Language) ![]const u8 {
        return switch (self.config.level) {
            .none => try self.allocator.dupe(u8, source),
            .minimal => try self.compressMinimal(source),
            .balanced => try self.compressBalanced(source, language),
            .aggressive => try self.compressAggressive(source, language),
            .extreme => try self.compressExtreme(source, language),
        };
    }

    /// Minimal compression: trim whitespace, remove empty lines
    fn compressMinimal(self: *Self, source: []const u8) ![]const u8 {
        var result: ArrayList(u8) = .empty;
        errdefer result.deinit(self.allocator);

        var lines = mem.splitScalar(u8, source, '\n');
        var consecutive_empty: u8 = 0;

        while (lines.next()) |line| {
            // Trim trailing whitespace
            const trimmed = if (self.config.trim_trailing)
                mem.trimRight(u8, line, " \t\r")
            else
                line;

            // Handle empty lines
            if (trimmed.len == 0) {
                if (self.config.remove_empty_lines) {
                    consecutive_empty += 1;
                    if (self.config.max_consecutive_newlines > 0 and
                        consecutive_empty >= self.config.max_consecutive_newlines)
                    {
                        continue;
                    }
                }
                try result.append(self.allocator, '\n');
                continue;
            }

            consecutive_empty = 0;
            try result.appendSlice(self.allocator, trimmed);
            try result.append(self.allocator, '\n');
        }

        // Remove trailing newline if present
        if (result.items.len > 0 and result.items[result.items.len - 1] == '\n') {
            _ = result.pop();
        }

        return result.toOwnedSlice(self.allocator);
    }

    /// Balanced compression: remove comments
    fn compressBalanced(self: *Self, source: []const u8, language: Language) ![]const u8 {
        const intermediate = try self.removeComments(source, language);
        defer self.allocator.free(intermediate);

        return self.compressMinimal(intermediate);
    }

    /// Aggressive compression: remove docstrings
    fn compressAggressive(self: *Self, source: []const u8, language: Language) ![]const u8 {
        const without_comments = try self.removeComments(source, language);
        defer self.allocator.free(without_comments);

        const without_docstrings = try self.removeDocstrings(without_comments, language);
        defer self.allocator.free(without_docstrings);

        const minimal = try self.compressMinimal(without_docstrings);
        defer self.allocator.free(minimal);

        // Further compress whitespace
        return self.normalizeWhitespace(minimal);
    }

    /// Extreme compression: keep only key elements
    fn compressExtreme(self: *Self, source: []const u8, language: Language) ![]const u8 {
        // This would ideally use AST, but we do a best-effort line-based approach
        var result: ArrayList(u8) = .empty;
        errdefer result.deinit(self.allocator);

        var lines = mem.splitScalar(u8, source, '\n');

        while (lines.next()) |line| {
            const trimmed = mem.trim(u8, line, " \t\r");
            if (trimmed.len == 0) continue;

            // Keep import/include statements
            if (self.config.preserve_imports and isImportStatement(trimmed, language)) {
                try result.appendSlice(self.allocator, trimmed);
                try result.append(self.allocator, '\n');
                continue;
            }

            // Keep function/class definitions
            if (self.config.preserve_signatures and isDefinitionStart(trimmed, language)) {
                try result.appendSlice(self.allocator, trimmed);
                try result.append(self.allocator, '\n');
                continue;
            }
        }

        return result.toOwnedSlice(self.allocator);
    }

    /// Remove line and block comments
    fn removeComments(self: *Self, source: []const u8, language: Language) ![]const u8 {
        var result: ArrayList(u8) = .empty;
        errdefer result.deinit(self.allocator);

        const line_prefix = language.lineCommentPrefix();
        const block_start = language.blockCommentStart();
        const block_end = language.blockCommentEnd();

        var in_block_comment = false;
        var in_string = false;
        var string_char: u8 = 0;
        var i: usize = 0;

        while (i < source.len) {
            // Handle string literals (don't remove "comments" inside strings)
            if (!in_block_comment and (source[i] == '"' or source[i] == '\'')) {
                if (!in_string) {
                    in_string = true;
                    string_char = source[i];
                } else if (source[i] == string_char and (i == 0 or source[i - 1] != '\\')) {
                    in_string = false;
                }
                try result.append(self.allocator, source[i]);
                i += 1;
                continue;
            }

            if (in_string) {
                try result.append(self.allocator, source[i]);
                i += 1;
                continue;
            }

            // Check for block comment end
            if (in_block_comment) {
                if (block_end) |end| {
                    if (i + end.len <= source.len and mem.eql(u8, source[i .. i + end.len], end)) {
                        in_block_comment = false;
                        i += end.len;
                        continue;
                    }
                }
                i += 1;
                continue;
            }

            // Check for block comment start
            if (self.config.remove_block_comments) {
                if (block_start) |start| {
                    if (i + start.len <= source.len and mem.eql(u8, source[i .. i + start.len], start)) {
                        in_block_comment = true;
                        i += start.len;
                        continue;
                    }
                }
            }

            // Check for line comment
            if (self.config.remove_line_comments) {
                if (line_prefix) |prefix| {
                    if (i + prefix.len <= source.len and mem.eql(u8, source[i .. i + prefix.len], prefix)) {
                        // Skip to end of line
                        while (i < source.len and source[i] != '\n') {
                            i += 1;
                        }
                        continue;
                    }
                }
            }

            try result.append(self.allocator, source[i]);
            i += 1;
        }

        return result.toOwnedSlice(self.allocator);
    }

    /// Remove docstrings (Python-style triple quotes, etc.)
    fn removeDocstrings(self: *Self, source: []const u8, language: Language) ![]const u8 {
        _ = language;

        var result: ArrayList(u8) = .empty;
        errdefer result.deinit(self.allocator);

        var i: usize = 0;
        while (i < source.len) {
            // Check for triple quotes (Python docstring)
            if (i + 3 <= source.len) {
                const triple = source[i .. i + 3];
                if (mem.eql(u8, triple, "\"\"\"") or mem.eql(u8, triple, "'''")) {
                    // Skip to closing triple quote
                    i += 3;
                    while (i + 3 <= source.len) {
                        if (mem.eql(u8, source[i .. i + 3], triple)) {
                            i += 3;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                }
            }

            try result.append(self.allocator, source[i]);
            i += 1;
        }

        return result.toOwnedSlice(self.allocator);
    }

    /// Normalize whitespace (collapse multiple spaces)
    fn normalizeWhitespace(self: *Self, source: []const u8) ![]const u8 {
        var result: ArrayList(u8) = .empty;
        errdefer result.deinit(self.allocator);

        var prev_space = false;
        var line_start = true;

        for (source) |c| {
            if (c == '\n') {
                try result.append(self.allocator, c);
                prev_space = false;
                line_start = true;
                continue;
            }

            if (c == ' ' or c == '\t') {
                if (!prev_space and !line_start) {
                    try result.append(self.allocator, ' ');
                    prev_space = true;
                }
                continue;
            }

            prev_space = false;
            line_start = false;
            try result.append(self.allocator, c);
        }

        return result.toOwnedSlice(self.allocator);
    }
};

/// Check if line is an import statement
fn isImportStatement(line: []const u8, language: Language) bool {
    return switch (language) {
        .python => mem.startsWith(u8, line, "import ") or mem.startsWith(u8, line, "from "),
        .javascript, .typescript => mem.startsWith(u8, line, "import ") or mem.startsWith(u8, line, "export ") or mem.startsWith(u8, line, "require("),
        .rust => mem.startsWith(u8, line, "use ") or mem.startsWith(u8, line, "mod "),
        .go => mem.startsWith(u8, line, "import ") or mem.startsWith(u8, line, "package "),
        .java => mem.startsWith(u8, line, "import ") or mem.startsWith(u8, line, "package "),
        .c, .cpp => mem.startsWith(u8, line, "#include"),
        .csharp => mem.startsWith(u8, line, "using "),
        .ruby => mem.startsWith(u8, line, "require ") or mem.startsWith(u8, line, "require_relative "),
        .php => mem.startsWith(u8, line, "use ") or mem.startsWith(u8, line, "require ") or mem.startsWith(u8, line, "include "),
        .unknown => false,
    };
}

/// Check if line starts a function/class definition
fn isDefinitionStart(line: []const u8, language: Language) bool {
    return switch (language) {
        .python => mem.startsWith(u8, line, "def ") or mem.startsWith(u8, line, "class ") or mem.startsWith(u8, line, "async def "),
        .javascript, .typescript => mem.startsWith(u8, line, "function ") or mem.startsWith(u8, line, "class ") or mem.startsWith(u8, line, "const ") or mem.startsWith(u8, line, "let ") or mem.startsWith(u8, line, "var ") or mem.indexOf(u8, line, "=>") != null,
        .rust => mem.startsWith(u8, line, "fn ") or mem.startsWith(u8, line, "pub fn ") or mem.startsWith(u8, line, "struct ") or mem.startsWith(u8, line, "enum ") or mem.startsWith(u8, line, "impl ") or mem.startsWith(u8, line, "trait "),
        .go => mem.startsWith(u8, line, "func ") or mem.startsWith(u8, line, "type "),
        .java => mem.indexOf(u8, line, "class ") != null or mem.indexOf(u8, line, "interface ") != null or mem.indexOf(u8, line, "enum ") != null or (mem.indexOf(u8, line, "(") != null and mem.indexOf(u8, line, ")") != null and mem.indexOf(u8, line, "{") != null),
        .c, .cpp => mem.indexOf(u8, line, "(") != null and mem.indexOf(u8, line, ")") != null and (mem.indexOf(u8, line, "{") != null or mem.endsWith(u8, line, ")")),
        .csharp => mem.indexOf(u8, line, "class ") != null or mem.indexOf(u8, line, "interface ") != null or mem.indexOf(u8, line, "struct ") != null,
        .ruby => mem.startsWith(u8, line, "def ") or mem.startsWith(u8, line, "class ") or mem.startsWith(u8, line, "module "),
        .php => mem.startsWith(u8, line, "function ") or mem.startsWith(u8, line, "class ") or mem.indexOf(u8, line, "public function") != null,
        .unknown => false,
    };
}

test "minimal compression" {
    const allocator = std.testing.allocator;
    var compressor = Compressor.init(allocator, .{ .level = .minimal });

    const source =
        \\def hello():
        \\
        \\    print("hello")
        \\
        \\
        \\
        \\def world():
        \\    pass
    ;

    const compressed = try compressor.compress(source, .python);
    defer allocator.free(compressed);

    // Should remove excessive empty lines
    try std.testing.expect(compressed.len < source.len);
}

test "remove comments" {
    const allocator = std.testing.allocator;
    var compressor = Compressor.init(allocator, .{ .level = .balanced });

    const source =
        \\// This is a comment
        \\function hello() {
        \\    /* block comment */
        \\    console.log("hello"); // inline comment
        \\}
    ;

    const compressed = try compressor.compress(source, .javascript);
    defer allocator.free(compressed);

    // Comments should be removed
    try std.testing.expect(mem.indexOf(u8, compressed, "// This is") == null);
    try std.testing.expect(mem.indexOf(u8, compressed, "/* block") == null);
}

test "extreme compression" {
    const allocator = std.testing.allocator;
    var compressor = Compressor.init(allocator, .{ .level = .extreme });

    const source =
        \\import os
        \\
        \\def hello():
        \\    """This is a docstring"""
        \\    print("hello")
        \\
        \\class World:
        \\    def greet(self):
        \\        pass
    ;

    const compressed = try compressor.compress(source, .python);
    defer allocator.free(compressed);

    // Should keep imports and definitions
    try std.testing.expect(mem.indexOf(u8, compressed, "import os") != null);
    try std.testing.expect(mem.indexOf(u8, compressed, "def hello") != null);
    try std.testing.expect(mem.indexOf(u8, compressed, "class World") != null);
}
