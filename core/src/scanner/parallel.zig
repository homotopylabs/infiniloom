const std = @import("std");
const fs = std.fs;
const mem = std.mem;
const Allocator = mem.Allocator;
const ArrayList = std.ArrayList;
const Thread = std.Thread;

const gitignore = @import("gitignore.zig");
const binary = @import("binary.zig");
const mmap = @import("mmap.zig");
const walker = @import("walker.zig");

/// Parallel directory scanner using a thread pool
/// Significantly faster for large repositories with many files
pub const ParallelWalker = struct {
    allocator: Allocator,
    config: walker.WalkConfig,
    /// Thread-safe file list
    files: ArrayList(walker.FileEntry),
    stats: walker.ScanStats,
    gitignore_parser: ?gitignore.GitignoreParser,
    root_path: []const u8,
    /// Mutex for shared state
    mutex: Thread.Mutex,
    /// Work queue for directories to process
    work_queue: WorkQueue,
    /// Number of worker threads
    num_threads: usize,
    /// Error flag
    has_error: std.atomic.Value(bool),

    const Self = @This();

    /// Work item: a directory to scan
    const WorkItem = struct {
        dir_path: []const u8,
        relative_prefix: []const u8,
    };

    /// Thread-safe work queue
    const WorkQueue = struct {
        items: ArrayList(WorkItem),
        allocator: Allocator,
        mutex: Thread.Mutex,
        cond: Thread.Condition,
        done: std.atomic.Value(bool),
        active_workers: std.atomic.Value(u32),

        fn init(alloc: Allocator) WorkQueue {
            return WorkQueue{
                .items = .empty,
                .allocator = alloc,
                .mutex = .{},
                .cond = .{},
                .done = std.atomic.Value(bool).init(false),
                .active_workers = std.atomic.Value(u32).init(0),
            };
        }

        fn deinit(self: *WorkQueue, alloc: Allocator) void {
            for (self.items.items) |item| {
                alloc.free(item.dir_path);
                if (item.relative_prefix.len > 0) {
                    alloc.free(item.relative_prefix);
                }
            }
            self.items.deinit(alloc);
        }

        fn push(self: *WorkQueue, item: WorkItem) !void {
            self.mutex.lock();
            defer self.mutex.unlock();
            try self.items.append(self.allocator, item);
            self.cond.signal();
        }

        fn pop(self: *WorkQueue) ?WorkItem {
            self.mutex.lock();
            defer self.mutex.unlock();

            while (self.items.items.len == 0) {
                if (self.done.load(.monotonic)) {
                    return null;
                }
                // Check if all workers are idle and queue is empty
                if (self.active_workers.load(.monotonic) == 0) {
                    self.done.store(true, .monotonic);
                    self.cond.broadcast();
                    return null;
                }
                self.cond.wait(&self.mutex);
            }

            return self.items.pop();
        }

        fn markDone(self: *WorkQueue) void {
            self.done.store(true, .monotonic);
            self.cond.broadcast();
        }
    };

    pub fn init(allocator: Allocator, config: walker.WalkConfig) Self {
        // Use available CPU cores, but cap at 8 for I/O bound work
        const cpu_count = Thread.getCpuCount() catch 4;
        const num_threads = @min(cpu_count, 8);

        return Self{
            .allocator = allocator,
            .config = config,
            .files = .empty,
            .stats = walker.ScanStats{},
            .gitignore_parser = null,
            .root_path = "",
            .mutex = .{},
            .work_queue = WorkQueue.init(allocator),
            .num_threads = num_threads,
            .has_error = std.atomic.Value(bool).init(false),
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

        self.work_queue.deinit(self.allocator);
    }

    /// Scan a directory using parallel workers
    pub fn walk(self: *Self, path: []const u8) !void {
        const start_time = std.time.milliTimestamp();

        // Store root path
        self.root_path = try self.allocator.dupe(u8, path);

        // Load .gitignore if present
        if (self.config.respect_gitignore) {
            self.loadGitignore(path);
        }

        // Add root directory to work queue
        const root_item = WorkItem{
            .dir_path = try self.allocator.dupe(u8, path),
            .relative_prefix = "",
        };
        try self.work_queue.push(root_item);

        // Start worker threads
        var threads: [8]?Thread = [_]?Thread{null} ** 8;
        const actual_threads = @min(self.num_threads, 8);

        for (0..actual_threads) |i| {
            threads[i] = Thread.spawn(.{}, workerThread, .{self}) catch null;
        }

        // Wait for all threads to complete
        for (0..actual_threads) |i| {
            if (threads[i]) |t| {
                t.join();
            }
        }

        self.stats.scan_time_ms = std.time.milliTimestamp() - start_time;
    }

    fn workerThread(self: *Self) void {
        _ = self.work_queue.active_workers.fetchAdd(1, .monotonic);
        defer _ = self.work_queue.active_workers.fetchSub(1, .monotonic);

        while (self.work_queue.pop()) |work_item| {
            defer {
                self.allocator.free(work_item.dir_path);
                if (work_item.relative_prefix.len > 0) {
                    self.allocator.free(work_item.relative_prefix);
                }
            }

            self.processDirectory(work_item.dir_path, work_item.relative_prefix) catch {
                self.has_error.store(true, .monotonic);
            };
        }
    }

    fn processDirectory(self: *Self, dir_path: []const u8, relative_prefix: []const u8) !void {
        var dir = fs.cwd().openDir(dir_path, .{ .iterate = true }) catch return;
        defer dir.close();

        var iter = dir.iterate();

        while (try iter.next()) |entry| {
            // Skip hidden files unless configured
            if (!self.config.include_hidden and entry.name[0] == '.') {
                _ = @atomicRmw(u32, &self.stats.skipped_hidden, .Add, 1, .monotonic);
                continue;
            }

            // Build relative path
            const relative_path = if (relative_prefix.len > 0)
                try std.fmt.allocPrint(self.allocator, "{s}/{s}", .{ relative_prefix, entry.name })
            else
                try self.allocator.dupe(u8, entry.name);

            // Check gitignore
            var should_skip = false;
            if (self.gitignore_parser) |*parser| {
                self.mutex.lock();
                const matches = parser.matches(relative_path, entry.kind == .directory);
                self.mutex.unlock();

                if (matches) {
                    _ = @atomicRmw(u32, &self.stats.skipped_gitignore, .Add, 1, .monotonic);
                    should_skip = true;
                }
            }

            if (should_skip) {
                self.allocator.free(relative_path);
                continue;
            }

            if (entry.kind == .directory) {
                // Add subdirectory to work queue
                const sub_path = try std.fmt.allocPrint(self.allocator, "{s}/{s}", .{ dir_path, entry.name });
                const work_item = WorkItem{
                    .dir_path = sub_path,
                    .relative_prefix = relative_path,
                };
                self.work_queue.push(work_item) catch {
                    self.allocator.free(sub_path);
                    self.allocator.free(relative_path);
                };
            } else if (entry.kind == .file) {
                // Process file
                self.processFile(dir, entry.name, dir_path, relative_path) catch {
                    self.allocator.free(relative_path);
                };
            } else {
                self.allocator.free(relative_path);
            }
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
        const stat = dir.statFile(name) catch {
            self.allocator.free(relative_path);
            return;
        };

        // Check file size
        if (stat.size > self.config.max_file_size) {
            _ = @atomicRmw(u32, &self.stats.skipped_size, .Add, 1, .monotonic);
            self.allocator.free(relative_path);
            return;
        }

        // Check extension
        const ext = getExtension(name);
        if (ext) |extension| {
            for (self.config.exclude_extensions) |excluded| {
                if (mem.eql(u8, extension, excluded)) {
                    self.allocator.free(relative_path);
                    return;
                }
            }

            if (self.config.include_extensions.len > 0) {
                var found = false;
                for (self.config.include_extensions) |included| {
                    if (mem.eql(u8, extension, included)) {
                        found = true;
                        break;
                    }
                }
                if (!found) {
                    self.allocator.free(relative_path);
                    return;
                }
            }
        }

        // Check if binary
        var is_binary = false;
        if (stat.size > 0) {
            var file = dir.openFile(name, .{}) catch {
                self.allocator.free(relative_path);
                return;
            };
            defer file.close();

            var header: [8192]u8 = undefined;
            const bytes_read = file.read(&header) catch 0;
            if (bytes_read > 0) {
                is_binary = binary.isBinary(header[0..bytes_read]);
            }
        }

        if (is_binary) {
            _ = @atomicRmw(u32, &self.stats.skipped_binary, .Add, 1, .monotonic);
            self.allocator.free(relative_path);
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
            if (self.config.use_mmap and stat.size >= self.config.mmap_threshold) {
                if (mmap.MappedFile.openAt(dir, name)) |mapped_file| {
                    var mapped = mapped_file;
                    defer mapped.close();
                    content = self.allocator.dupe(u8, mapped.contents()) catch null;
                } else |_| {
                    const file = dir.openFile(name, .{}) catch null;
                    if (file) |f| {
                        defer f.close();
                        content = f.readToEndAlloc(self.allocator, self.config.max_file_size) catch null;
                    }
                }
            } else {
                const file = dir.openFile(name, .{}) catch null;
                if (file) |f| {
                    defer f.close();
                    content = f.readToEndAlloc(self.allocator, self.config.max_file_size) catch null;
                }
            }
        }

        // Add file entry (thread-safe)
        self.mutex.lock();
        defer self.mutex.unlock();

        self.files.append(self.allocator, walker.FileEntry{
            .path = full_path,
            .relative_path = relative_path,
            .size = stat.size,
            .is_binary = is_binary,
            .language = language,
            .extension = ext,
            .content = content,
        }) catch {
            self.allocator.free(full_path);
            self.allocator.free(relative_path);
            if (content) |c| self.allocator.free(c);
            return;
        };

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
        parser.addDefaults() catch return;

        self.gitignore_parser = parser;
    }

    /// Get list of scanned files
    pub fn getFiles(self: *const Self) []const walker.FileEntry {
        return self.files.items;
    }

    /// Get scan statistics
    pub fn getStats(self: *const Self) walker.ScanStats {
        return self.stats;
    }
};

// Reuse helper functions from walker module
fn getExtension(filename: []const u8) ?[]const u8 {
    if (mem.endsWith(u8, filename, ".min.js")) return ".min.js";
    if (mem.endsWith(u8, filename, ".min.css")) return ".min.css";

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

fn detectLanguage(filename: []const u8) ?[]const u8 {
    const ext = getExtension(filename) orelse return null;

    const mapping = std.StaticStringMap([]const u8).initComptime(.{
        .{ ".py", "python" },
        .{ ".pyi", "python" },
        .{ ".js", "javascript" },
        .{ ".jsx", "jsx" },
        .{ ".ts", "typescript" },
        .{ ".tsx", "tsx" },
        .{ ".rs", "rust" },
        .{ ".go", "go" },
        .{ ".java", "java" },
        .{ ".c", "c" },
        .{ ".h", "c" },
        .{ ".cpp", "cpp" },
        .{ ".hpp", "cpp" },
        .{ ".cs", "csharp" },
        .{ ".rb", "ruby" },
        .{ ".php", "php" },
        .{ ".swift", "swift" },
        .{ ".kt", "kotlin" },
        .{ ".scala", "scala" },
        .{ ".sh", "bash" },
        .{ ".bash", "bash" },
        .{ ".html", "html" },
        .{ ".css", "css" },
        .{ ".scss", "scss" },
        .{ ".json", "json" },
        .{ ".yaml", "yaml" },
        .{ ".yml", "yaml" },
        .{ ".toml", "toml" },
        .{ ".xml", "xml" },
        .{ ".md", "markdown" },
        .{ ".zig", "zig" },
        .{ ".lua", "lua" },
        .{ ".sql", "sql" },
    });

    return mapping.get(ext);
}

// ============================================================================
// Tests
// ============================================================================

test "parallel walker init" {
    var pw = ParallelWalker.init(std.testing.allocator, .{});
    defer pw.deinit();

    try std.testing.expect(pw.num_threads > 0);
    try std.testing.expect(pw.num_threads <= 8);
}

test "work queue" {
    var queue = ParallelWalker.WorkQueue.init(std.testing.allocator);
    defer queue.deinit(std.testing.allocator);

    const item = ParallelWalker.WorkItem{
        .dir_path = try std.testing.allocator.dupe(u8, "/test/path"),
        .relative_prefix = "",
    };
    try queue.push(item);

    const popped = queue.pop();
    try std.testing.expect(popped != null);
    std.testing.allocator.free(popped.?.dir_path);
}
