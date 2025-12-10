const std = @import("std");
const fs = std.fs;
const mem = std.mem;
const posix = std.posix;
const Allocator = mem.Allocator;

/// Memory-mapped file reader for efficient large file access
/// Uses OS-level memory mapping to avoid copying file data into heap
pub const MappedFile = struct {
    /// Mapped memory region
    data: []align(std.heap.page_size_min) u8,
    /// File handle (kept open while mapped)
    file: fs.File,
    /// Actual file size
    size: u64,

    const Self = @This();

    /// Open and memory-map a file
    /// Returns null if file is too small or mapping fails
    pub fn open(path: []const u8) !Self {
        const file = try fs.cwd().openFile(path, .{ .mode = .read_only });
        errdefer file.close();

        const stat = try file.stat();
        const size = stat.size;

        if (size == 0) {
            return Self{
                .data = &[_]u8{},
                .file = file,
                .size = 0,
            };
        }

        // Memory map the file
        const mapped = try posix.mmap(
            null,
            size,
            posix.PROT.READ,
            .{ .TYPE = .SHARED },
            file.handle,
            0,
        );

        return Self{
            .data = mapped,
            .file = file,
            .size = size,
        };
    }

    /// Open file from directory handle
    pub fn openAt(dir: fs.Dir, name: []const u8) !Self {
        const file = try dir.openFile(name, .{ .mode = .read_only });
        errdefer file.close();

        const stat = try file.stat();
        const size = stat.size;

        if (size == 0) {
            return Self{
                .data = &[_]u8{},
                .file = file,
                .size = 0,
            };
        }

        // Memory map the file
        const mapped = try posix.mmap(
            null,
            size,
            posix.PROT.READ,
            .{ .TYPE = .SHARED },
            file.handle,
            0,
        );

        return Self{
            .data = mapped,
            .file = file,
            .size = size,
        };
    }

    /// Close the mapped file
    pub fn close(self: *Self) void {
        if (self.data.len > 0) {
            posix.munmap(self.data);
        }
        self.file.close();
    }

    /// Get the file contents as a slice
    pub fn contents(self: *const Self) []const u8 {
        if (self.size == 0) return "";
        return self.data[0..self.size];
    }

    /// Check if the file appears to be binary
    pub fn isBinary(self: *const Self) bool {
        const check_size = @min(self.size, 8192);
        if (check_size == 0) return false;

        const data = self.data[0..check_size];
        return isBinaryContent(data);
    }

    /// Advise the OS about access pattern (can improve performance)
    pub fn adviseSequential(self: *Self) void {
        if (self.data.len > 0) {
            posix.madvise(self.data, .SEQUENTIAL) catch {};
        }
    }

    /// Advise the OS we're done with a region
    pub fn adviseDontneed(self: *Self) void {
        if (self.data.len > 0) {
            posix.madvise(self.data, .DONTNEED) catch {};
        }
    }
};

/// Check if content appears to be binary
fn isBinaryContent(data: []const u8) bool {
    // Check for null bytes or high concentration of non-printable chars
    var non_printable: usize = 0;

    for (data) |byte| {
        if (byte == 0) return true; // Null byte = definitely binary

        // Count non-printable, non-whitespace bytes
        if (byte < 0x20 and byte != '\n' and byte != '\r' and byte != '\t') {
            non_printable += 1;
        }
    }

    // More than 10% non-printable = likely binary
    return non_printable * 10 > data.len;
}

/// File reader that automatically chooses between mmap and regular read
/// Uses mmap for large files, regular read for small files
pub const SmartReader = struct {
    /// Threshold for using mmap (default: 64KB)
    mmap_threshold: u64 = 64 * 1024,
    allocator: Allocator,

    const Self = @This();

    pub fn init(allocator: Allocator) Self {
        return Self{
            .allocator = allocator,
        };
    }

    /// Read file contents, using mmap for large files
    pub fn readFile(self: *Self, dir: fs.Dir, name: []const u8, max_size: u64) !FileContent {
        const file = try dir.openFile(name, .{ .mode = .read_only });
        defer file.close();

        const stat = try file.stat();
        const size = stat.size;

        if (size == 0) {
            return FileContent{
                .data = "",
                .owned = false,
                .mapped = null,
            };
        }

        if (size > max_size) {
            return error.FileTooLarge;
        }

        // Use mmap for large files
        if (size >= self.mmap_threshold) {
            const mapped = try MappedFile.openAt(dir, name);
            return FileContent{
                .data = mapped.contents(),
                .owned = false,
                .mapped = mapped,
            };
        }

        // Regular read for small files
        const data = try file.readToEndAlloc(self.allocator, max_size);
        return FileContent{
            .data = data,
            .owned = true,
            .mapped = null,
        };
    }

    pub const FileContent = struct {
        data: []const u8,
        owned: bool, // If true, data is allocated and needs freeing
        mapped: ?MappedFile, // If set, data is memory-mapped

        pub fn deinit(self: *FileContent, allocator: Allocator) void {
            if (self.mapped) |*m| {
                m.close();
            } else if (self.owned and self.data.len > 0) {
                allocator.free(@constCast(self.data));
            }
            self.data = "";
            self.owned = false;
            self.mapped = null;
        }
    };
};

// ============================================================================
// Tests
// ============================================================================

test "mmap basic" {
    // Create a test file
    const test_content = "Hello, World! This is a test file for memory mapping.";

    var tmp_dir = std.testing.tmpDir(.{});
    defer tmp_dir.cleanup();

    const file = try tmp_dir.dir.createFile("test.txt", .{});
    try file.writeAll(test_content);
    file.close();

    // Memory map and read
    var mapped = try MappedFile.openAt(tmp_dir.dir, "test.txt");
    defer mapped.close();

    try std.testing.expectEqualStrings(test_content, mapped.contents());
    try std.testing.expect(!mapped.isBinary());
}

test "smart reader small file" {
    const test_content = "Small file content";

    var tmp_dir = std.testing.tmpDir(.{});
    defer tmp_dir.cleanup();

    const file = try tmp_dir.dir.createFile("small.txt", .{});
    try file.writeAll(test_content);
    file.close();

    var reader = SmartReader.init(std.testing.allocator);
    var content = try reader.readFile(tmp_dir.dir, "small.txt", 1024 * 1024);
    defer content.deinit(std.testing.allocator);

    try std.testing.expectEqualStrings(test_content, content.data);
    try std.testing.expect(content.owned); // Small file should be owned
    try std.testing.expect(content.mapped == null);
}

test "binary detection" {
    const text_content = "This is plain text\nwith newlines\tand tabs.";
    const binary_content = "Binary\x00content\x01here";

    try std.testing.expect(!isBinaryContent(text_content));
    try std.testing.expect(isBinaryContent(binary_content));
}
