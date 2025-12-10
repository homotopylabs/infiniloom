const std = @import("std");
const fs = std.fs;
const mem = std.mem;
const Allocator = mem.Allocator;
const ArrayList = std.ArrayList;
const Thread = std.Thread;

const gitignore = @import("gitignore.zig");
const binary = @import("binary.zig");
const mmap = @import("mmap.zig");

/// Configuration for directory walking
pub const WalkConfig = struct {
    /// Maximum file size to include (default: 50MB)
    max_file_size: u64 = 50 * 1024 * 1024,
    /// Whether to follow symbolic links
    follow_symlinks: bool = false,
    /// Whether to include hidden files (starting with .)
    include_hidden: bool = false,
    /// Whether to respect .gitignore files
    respect_gitignore: bool = true,
    /// Whether to read file contents (for output generation)
    read_contents: bool = false,
    /// Use memory-mapped files for large files (default: true)
    use_mmap: bool = true,
    /// Threshold for using mmap (default: 64KB)
    mmap_threshold: u64 = 64 * 1024,
    /// Additional patterns to ignore
    ignore_patterns: []const []const u8 = &[_][]const u8{},
    /// File extensions to include (empty = all)
    include_extensions: []const []const u8 = &[_][]const u8{},
    /// File extensions to exclude
    exclude_extensions: []const []const u8 = &[_][]const u8{
        ".exe", ".dll", ".so", ".dylib", ".a", ".o", ".obj",
        ".pyc", ".pyo", ".class", ".jar", ".war",
        ".zip", ".tar", ".gz", ".bz2", ".xz", ".7z", ".rar",
        ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ico", ".webp", ".svg",
        ".mp3", ".mp4", ".avi", ".mov", ".wav", ".flac",
        ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
        ".woff", ".woff2", ".ttf", ".eot", ".otf",
        ".min.js", ".min.css", ".map",
        ".lock", ".sum",
    },
};

/// Information about a scanned file
pub const FileEntry = struct {
    /// Full path to the file
    path: []const u8,
    /// Path relative to scan root
    relative_path: []const u8,
    /// File size in bytes
    size: u64,
    /// Whether the file appears to be binary
    is_binary: bool,
    /// Detected programming language (null if unknown)
    language: ?[]const u8,
    /// File extension
    extension: ?[]const u8,
    /// File content (null if not loaded)
    content: ?[]const u8,

    pub fn deinit(self: *FileEntry, allocator: Allocator) void {
        allocator.free(self.path);
        allocator.free(self.relative_path);
        if (self.content) |c| {
            allocator.free(c);
        }
    }
};

/// Statistics from a scan operation
pub const ScanStats = struct {
    total_files: u32 = 0,
    total_bytes: u64 = 0,
    skipped_binary: u32 = 0,
    skipped_size: u32 = 0,
    skipped_gitignore: u32 = 0,
    skipped_hidden: u32 = 0,
    scan_time_ms: i64 = 0,
};

/// File system walker with gitignore support
pub const Walker = struct {
    allocator: Allocator,
    config: WalkConfig,
    files: ArrayList(FileEntry),
    stats: ScanStats,
    gitignore_parser: ?gitignore.GitignoreParser,
    root_path: []const u8,
    mutex: Thread.Mutex,

    const Self = @This();

    pub fn init(allocator: Allocator, config: WalkConfig) Self {
        return Self{
            .allocator = allocator,
            .config = config,
            .files = .empty,
            .stats = ScanStats{},
            .gitignore_parser = null,
            .root_path = "",
            .mutex = .{},
        };
    }

    pub fn deinit(self: *Self) void {
        for (self.files.items) |*entry| {
            entry.deinit(self.allocator);
        }
        self.files.deinit(self.allocator);

        if (self.gitignore_parser) |*parser| {
            parser.deinit();
        }

        if (self.root_path.len > 0) {
            self.allocator.free(self.root_path);
        }
    }

    /// Scan a directory and collect file information
    pub fn walk(self: *Self, path: []const u8) !void {
        const start_time = std.time.milliTimestamp();

        // Store root path
        self.root_path = try self.allocator.dupe(u8, path);

        // Load .gitignore if present
        if (self.config.respect_gitignore) {
            self.loadGitignore(path);
        }

        // Open and walk directory
        var dir = fs.cwd().openDir(path, .{ .iterate = true }) catch |err| {
            std.debug.print("Failed to open directory {s}: {}\n", .{ path, err });
            return err;
        };
        defer dir.close();

        try self.walkDir(dir, path, "");

        self.stats.scan_time_ms = std.time.milliTimestamp() - start_time;
    }

    fn walkDir(self: *Self, dir: fs.Dir, base_path: []const u8, relative_prefix: []const u8) !void {
        var iter = dir.iterate();

        while (try iter.next()) |entry| {
            // Skip hidden files unless configured otherwise
            if (!self.config.include_hidden and entry.name[0] == '.') {
                self.stats.skipped_hidden += 1;
                continue;
            }

            // Build relative path - always allocate, always defer free
            const relative_path = if (relative_prefix.len > 0)
                try std.fmt.allocPrint(self.allocator, "{s}/{s}", .{ relative_prefix, entry.name })
            else
                try self.allocator.dupe(u8, entry.name);

            // Always free relative_path at end of iteration (it's either unused or copied into FileEntry)
            defer self.allocator.free(relative_path);

            // Check gitignore
            if (self.gitignore_parser) |*parser| {
                if (parser.matches(relative_path, entry.kind == .directory)) {
                    self.stats.skipped_gitignore += 1;
                    continue; // defer will free relative_path
                }
            }

            if (entry.kind == .directory) {
                // Recurse into subdirectory
                var sub_dir = dir.openDir(entry.name, .{ .iterate = true }) catch continue;
                defer sub_dir.close();

                // Pass relative_path to recursive call (it becomes the prefix)
                // The recursive call uses it read-only, we still own it
                try self.walkDir(sub_dir, base_path, relative_path);
            } else if (entry.kind == .file) {
                // processFile stores a COPY of relative_path in the file entry
                try self.processFile(dir, entry.name, base_path, relative_path);
            }
            // defer will free relative_path
        }
    }

    fn processFile(
        self: *Self,
        dir: fs.Dir,
        name: []const u8,
        base_path: []const u8,
        relative_path: []const u8,
    ) !void {
        // Get file stats
        const stat = dir.statFile(name) catch return;

        // Check file size
        if (stat.size > self.config.max_file_size) {
            self.stats.skipped_size += 1;
            return;
        }

        // Check extension
        const ext = getExtension(name);
        if (ext) |extension| {
            // Check exclude list
            for (self.config.exclude_extensions) |excluded| {
                if (mem.eql(u8, extension, excluded)) {
                    return;
                }
            }

            // Check include list (if specified)
            if (self.config.include_extensions.len > 0) {
                var found = false;
                for (self.config.include_extensions) |included| {
                    if (mem.eql(u8, extension, included)) {
                        found = true;
                        break;
                    }
                }
                if (!found) return;
            }
        }

        // Check if binary by reading first bytes
        var is_binary = false;
        if (stat.size > 0) {
            var file = dir.openFile(name, .{}) catch return;
            defer file.close();

            var header: [8192]u8 = undefined;
            const bytes_read = file.read(&header) catch 0;
            if (bytes_read > 0) {
                is_binary = binary.isBinary(header[0..bytes_read]);
            }
        }

        if (is_binary) {
            self.stats.skipped_binary += 1;
            return;
        }

        // Build full path
        const full_path = try std.fmt.allocPrint(
            self.allocator,
            "{s}/{s}",
            .{ base_path, relative_path },
        );

        // Detect language
        const language = detectLanguage(name);

        // Read file content if configured
        var content: ?[]const u8 = null;
        if (self.config.read_contents and stat.size > 0) {
            // Use mmap for large files to reduce memory copies
            if (self.config.use_mmap and stat.size >= self.config.mmap_threshold) {
                // For mmap, we need to copy since we can't store the MappedFile handle
                // But mmap is still faster for reading large files
                if (mmap.MappedFile.openAt(dir, name)) |mapped_file| {
                    var mapped = mapped_file;
                    defer mapped.close();
                    content = self.allocator.dupe(u8, mapped.contents()) catch null;
                } else |_| {
                    // Fallback to regular read
                    const file = dir.openFile(name, .{}) catch null;
                    if (file) |f| {
                        defer f.close();
                        content = f.readToEndAlloc(self.allocator, self.config.max_file_size) catch null;
                    }
                }
            } else {
                // Regular read for small files
                const file = dir.openFile(name, .{}) catch null;
                if (file) |f| {
                    defer f.close();
                    content = f.readToEndAlloc(self.allocator, self.config.max_file_size) catch null;
                }
            }
        }

        // Add file entry
        self.mutex.lock();
        defer self.mutex.unlock();

        try self.files.append(self.allocator, FileEntry{
            .path = full_path,
            .relative_path = try self.allocator.dupe(u8, relative_path),
            .size = stat.size,
            .is_binary = is_binary,
            .language = language,
            .extension = ext,
            .content = content,
        });

        self.stats.total_files += 1;
        self.stats.total_bytes += stat.size;
    }

    fn loadGitignore(self: *Self, path: []const u8) void {
        const gitignore_path = std.fmt.allocPrint(
            self.allocator,
            "{s}/.gitignore",
            .{path},
        ) catch return;
        defer self.allocator.free(gitignore_path);

        const file = fs.cwd().openFile(gitignore_path, .{}) catch return;
        defer file.close();

        const content = file.readToEndAlloc(self.allocator, 1024 * 1024) catch return;
        defer self.allocator.free(content);

        var parser = gitignore.GitignoreParser.init(self.allocator);
        parser.parse(content) catch return;

        // Add default ignores
        parser.addDefaults() catch return;

        self.gitignore_parser = parser;
    }

    /// Get list of scanned files
    pub fn getFiles(self: *const Self) []const FileEntry {
        return self.files.items;
    }

    /// Get scan statistics
    pub fn getStats(self: *const Self) ScanStats {
        return self.stats;
    }
};

/// Extract file extension
fn getExtension(filename: []const u8) ?[]const u8 {
    // Handle special cases like .min.js
    if (mem.endsWith(u8, filename, ".min.js")) return ".min.js";
    if (mem.endsWith(u8, filename, ".min.css")) return ".min.css";

    // Find last dot
    var i = filename.len;
    while (i > 0) {
        i -= 1;
        if (filename[i] == '.') {
            return filename[i..];
        }
        if (filename[i] == '/') break;
    }
    return null;
}

/// Detect programming language from filename
fn detectLanguage(filename: []const u8) ?[]const u8 {
    const ext = getExtension(filename) orelse return null;

    // Language mapping
    const mapping = std.StaticStringMap([]const u8).initComptime(.{
        // Python
        .{ ".py", "python" },
        .{ ".pyi", "python" },
        .{ ".pyx", "python" },

        // JavaScript/TypeScript
        .{ ".js", "javascript" },
        .{ ".jsx", "jsx" },
        .{ ".ts", "typescript" },
        .{ ".tsx", "tsx" },
        .{ ".mjs", "javascript" },
        .{ ".cjs", "javascript" },

        // Rust
        .{ ".rs", "rust" },

        // Go
        .{ ".go", "go" },

        // Java/JVM
        .{ ".java", "java" },
        .{ ".kt", "kotlin" },
        .{ ".kts", "kotlin" },
        .{ ".scala", "scala" },
        .{ ".groovy", "groovy" },

        // C/C++
        .{ ".c", "c" },
        .{ ".h", "c" },
        .{ ".cpp", "cpp" },
        .{ ".hpp", "cpp" },
        .{ ".cc", "cpp" },
        .{ ".cxx", "cpp" },
        .{ ".hxx", "cpp" },

        // C#
        .{ ".cs", "csharp" },

        // Ruby
        .{ ".rb", "ruby" },
        .{ ".rake", "ruby" },
        .{ ".gemspec", "ruby" },

        // PHP
        .{ ".php", "php" },

        // Swift
        .{ ".swift", "swift" },

        // Shell
        .{ ".sh", "bash" },
        .{ ".bash", "bash" },
        .{ ".zsh", "zsh" },
        .{ ".fish", "fish" },

        // Web
        .{ ".html", "html" },
        .{ ".htm", "html" },
        .{ ".css", "css" },
        .{ ".scss", "scss" },
        .{ ".sass", "sass" },
        .{ ".less", "less" },

        // Data/Config
        .{ ".json", "json" },
        .{ ".yaml", "yaml" },
        .{ ".yml", "yaml" },
        .{ ".toml", "toml" },
        .{ ".xml", "xml" },
        .{ ".ini", "ini" },

        // Documentation
        .{ ".md", "markdown" },
        .{ ".mdx", "mdx" },
        .{ ".rst", "rst" },
        .{ ".txt", "text" },

        // Zig
        .{ ".zig", "zig" },

        // Lua
        .{ ".lua", "lua" },

        // SQL
        .{ ".sql", "sql" },

        // Elixir/Erlang
        .{ ".ex", "elixir" },
        .{ ".exs", "elixir" },
        .{ ".erl", "erlang" },

        // Haskell
        .{ ".hs", "haskell" },

        // OCaml
        .{ ".ml", "ocaml" },
        .{ ".mli", "ocaml" },

        // Vue/Svelte
        .{ ".vue", "vue" },
        .{ ".svelte", "svelte" },

        // Docker
        .{ ".dockerfile", "dockerfile" },

        // Terraform
        .{ ".tf", "terraform" },
        .{ ".tfvars", "terraform" },
    });

    return mapping.get(ext);
}

test "walker basic" {
    const allocator = std.testing.allocator;

    var walker = Walker.init(allocator, .{});
    defer walker.deinit();

    // This test would need a test fixture directory
    // try walker.walk("test_fixtures");
}

test "detect language" {
    try std.testing.expectEqualStrings("python", detectLanguage("main.py").?);
    try std.testing.expectEqualStrings("typescript", detectLanguage("index.ts").?);
    try std.testing.expectEqualStrings("rust", detectLanguage("lib.rs").?);
    try std.testing.expect(detectLanguage("README") == null);
}

test "get extension" {
    try std.testing.expectEqualStrings(".py", getExtension("main.py").?);
    try std.testing.expectEqualStrings(".min.js", getExtension("bundle.min.js").?);
    try std.testing.expectEqualStrings(".ts", getExtension("src/index.ts").?);
}
