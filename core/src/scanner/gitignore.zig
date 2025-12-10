const std = @import("std");
const mem = std.mem;
const Allocator = mem.Allocator;
const ArrayList = std.ArrayList;

/// A single gitignore pattern
pub const Pattern = struct {
    /// The pattern string
    pattern: []const u8,
    /// Whether this is a negation pattern (starts with !)
    is_negation: bool,
    /// Whether this pattern only matches directories (ends with /)
    is_directory: bool,
    /// Whether this pattern is rooted (starts with /)
    is_rooted: bool,
    /// Whether this pattern contains **
    has_double_star: bool,

    pub fn deinit(self: *Pattern, allocator: Allocator) void {
        allocator.free(self.pattern);
    }
};

/// Parser for .gitignore files
pub const GitignoreParser = struct {
    patterns: ArrayList(Pattern),
    allocator: Allocator,

    const Self = @This();

    pub fn init(allocator: Allocator) Self {
        return Self{
            .patterns = .empty,
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *Self) void {
        for (self.patterns.items) |*pattern| {
            pattern.deinit(self.allocator);
        }
        self.patterns.deinit(self.allocator);
    }

    /// Parse gitignore content
    pub fn parse(self: *Self, content: []const u8) !void {
        var lines = mem.splitScalar(u8, content, '\n');

        while (lines.next()) |line| {
            try self.parseLine(line);
        }
    }

    /// Parse a single gitignore line
    fn parseLine(self: *Self, line: []const u8) !void {
        var trimmed = mem.trim(u8, line, " \t\r");

        // Skip empty lines and comments
        if (trimmed.len == 0) return;
        if (trimmed[0] == '#') return;

        var is_negation = false;
        var is_directory = false;
        var is_rooted = false;

        // Check for negation
        if (trimmed[0] == '!') {
            is_negation = true;
            trimmed = trimmed[1..];
            if (trimmed.len == 0) return;
        }

        // Check for directory-only match
        if (trimmed[trimmed.len - 1] == '/') {
            is_directory = true;
            trimmed = trimmed[0 .. trimmed.len - 1];
            if (trimmed.len == 0) return;
        }

        // Check if pattern is rooted
        if (trimmed[0] == '/') {
            is_rooted = true;
            trimmed = trimmed[1..];
            if (trimmed.len == 0) return;
        } else if (mem.indexOf(u8, trimmed, "/") != null) {
            // Patterns with a slash in the middle are also rooted
            is_rooted = true;
        }

        // Check for double star
        const has_double_star = mem.indexOf(u8, trimmed, "**") != null;

        // Store the pattern
        try self.patterns.append(self.allocator, Pattern{
            .pattern = try self.allocator.dupe(u8, trimmed),
            .is_negation = is_negation,
            .is_directory = is_directory,
            .is_rooted = is_rooted,
            .has_double_star = has_double_star,
        });
    }

    /// Add default ignore patterns
    pub fn addDefaults(self: *Self) !void {
        const defaults = [_][]const u8{
            // Version control
            ".git",
            ".svn",
            ".hg",

            // Dependencies
            "node_modules",
            "vendor",
            "__pycache__",
            ".venv",
            "venv",
            "env",
            ".env",
            "target", // Rust
            "zig-out",
            "zig-cache",

            // Build outputs
            "dist",
            "build",
            "out",
            ".next",
            ".nuxt",

            // IDE
            ".idea",
            ".vscode",
            "*.swp",
            "*.swo",
            "*~",

            // OS files
            ".DS_Store",
            "Thumbs.db",

            // Logs
            "*.log",
            "logs",

            // Coverage
            "coverage",
            ".coverage",
            "htmlcov",
            ".nyc_output",
        };

        for (defaults) |pattern| {
            try self.patterns.append(self.allocator, Pattern{
                .pattern = try self.allocator.dupe(u8, pattern),
                .is_negation = false,
                .is_directory = false,
                .is_rooted = false,
                .has_double_star = false,
            });
        }
    }

    /// Check if a path matches any ignore pattern
    pub fn matches(self: *const Self, path: []const u8, is_dir: bool) bool {
        var matched = false;

        for (self.patterns.items) |pattern| {
            // Directory-only patterns don't match files
            if (pattern.is_directory and !is_dir) continue;

            if (matchPattern(pattern, path)) {
                matched = !pattern.is_negation;
            }
        }

        return matched;
    }

    /// Match a single pattern against a path
    fn matchPattern(pattern: Pattern, path: []const u8) bool {
        if (pattern.has_double_star) {
            return matchDoubleStarPattern(pattern.pattern, path);
        }

        if (pattern.is_rooted) {
            return globMatch(pattern.pattern, path);
        }

        // Non-rooted patterns match any component
        // e.g., "*.pyc" matches "foo/bar.pyc"
        const basename = getBasename(path);

        // Try matching against full path
        if (globMatch(pattern.pattern, path)) return true;

        // Try matching against basename
        if (globMatch(pattern.pattern, basename)) return true;

        return false;
    }

    /// Get basename from path
    fn getBasename(path: []const u8) []const u8 {
        var i = path.len;
        while (i > 0) {
            i -= 1;
            if (path[i] == '/') {
                return path[i + 1 ..];
            }
        }
        return path;
    }

    /// Simple glob matching (supports * and ?)
    fn globMatch(pattern: []const u8, text: []const u8) bool {
        var p: usize = 0;
        var t: usize = 0;
        var star_p: ?usize = null;
        var star_t: usize = 0;

        while (t < text.len) {
            if (p < pattern.len and (pattern[p] == '?' or pattern[p] == text[t])) {
                p += 1;
                t += 1;
            } else if (p < pattern.len and pattern[p] == '*') {
                star_p = p;
                star_t = t;
                p += 1;
            } else if (star_p) |sp| {
                p = sp + 1;
                star_t += 1;
                t = star_t;
            } else {
                return false;
            }
        }

        // Check remaining pattern characters
        while (p < pattern.len and pattern[p] == '*') {
            p += 1;
        }

        return p == pattern.len;
    }

    /// Match patterns containing **
    fn matchDoubleStarPattern(pattern: []const u8, path: []const u8) bool {
        // Handle **/ at start (matches any directory prefix)
        if (mem.startsWith(u8, pattern, "**/")) {
            const rest = pattern[3..];
            // Match against path and all subdirectories
            if (globMatch(rest, path)) return true;

            var i: usize = 0;
            while (i < path.len) {
                if (path[i] == '/') {
                    if (globMatch(rest, path[i + 1 ..])) return true;
                }
                i += 1;
            }
            return false;
        }

        // Handle /** at end (matches everything inside)
        if (mem.endsWith(u8, pattern, "/**")) {
            const prefix = pattern[0 .. pattern.len - 3];
            return mem.startsWith(u8, path, prefix);
        }

        // Handle /**/ in middle
        if (mem.indexOf(u8, pattern, "/**/")) |idx| {
            const prefix = pattern[0..idx];
            const suffix = pattern[idx + 4 ..];

            if (!mem.startsWith(u8, path, prefix)) return false;

            // Try matching suffix at any depth
            const rest = path[prefix.len..];
            if (globMatch(suffix, rest)) return true;

            var i: usize = 0;
            while (i < rest.len) {
                if (rest[i] == '/') {
                    if (globMatch(suffix, rest[i + 1 ..])) return true;
                }
                i += 1;
            }
            return false;
        }

        // Fallback to simple glob
        return globMatch(pattern, path);
    }
};

test "parse gitignore" {
    const allocator = std.testing.allocator;

    var parser = GitignoreParser.init(allocator);
    defer parser.deinit();

    try parser.parse(
        \\# Comment
        \\*.pyc
        \\__pycache__/
        \\/build
        \\!important.pyc
        \\node_modules
    );

    try std.testing.expectEqual(@as(usize, 5), parser.patterns.items.len);

    // Check first pattern
    try std.testing.expectEqualStrings("*.pyc", parser.patterns.items[0].pattern);
    try std.testing.expect(!parser.patterns.items[0].is_negation);

    // Check negation pattern
    try std.testing.expectEqualStrings("important.pyc", parser.patterns.items[3].pattern);
    try std.testing.expect(parser.patterns.items[3].is_negation);
}

test "glob match" {
    const parser = GitignoreParser.init(std.testing.allocator);
    _ = parser;

    try std.testing.expect(GitignoreParser.globMatch("*.py", "main.py"));
    try std.testing.expect(GitignoreParser.globMatch("*.py", "test.py"));
    try std.testing.expect(!GitignoreParser.globMatch("*.py", "main.js"));
    try std.testing.expect(GitignoreParser.globMatch("test_*", "test_main"));
    try std.testing.expect(GitignoreParser.globMatch("*.min.js", "bundle.min.js"));
}

test "matches" {
    const allocator = std.testing.allocator;

    var parser = GitignoreParser.init(allocator);
    defer parser.deinit();

    try parser.parse(
        \\*.pyc
        \\node_modules/
        \\build
    );

    try std.testing.expect(parser.matches("main.pyc", false));
    try std.testing.expect(parser.matches("src/test.pyc", false));
    try std.testing.expect(parser.matches("node_modules", true));
    try std.testing.expect(!parser.matches("node_modules", false)); // dir-only pattern
    try std.testing.expect(parser.matches("build", false));
    try std.testing.expect(parser.matches("build", true));
}
