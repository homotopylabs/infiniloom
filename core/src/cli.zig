const std = @import("std");
const fs = std.fs;
const mem = std.mem;
const process = std.process;

const scanner = @import("scanner/walker.zig");
const tokenizer = @import("tokenizer/counter.zig");
const compressor = @import("compressor/rules.zig");

const version = "0.1.0";

/// Output format
const OutputFormat = enum {
    stats, // Default: just show statistics
    xml, // Claude-optimized XML
    markdown, // GPT-optimized Markdown
    json, // Generic JSON
    plain, // Plain text with file contents
};

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const args = try process.argsAlloc(allocator);
    defer process.argsFree(allocator, args);

    var path: []const u8 = ".";
    var show_help = false;
    var show_version = false;
    var verbose = false;
    var show_tokens = false;
    var include_hidden = false;
    var model: tokenizer.TokenizerModel = .claude;
    var format: OutputFormat = .stats;
    var output_file: ?[]const u8 = null;

    // Parse arguments
    var i: usize = 1;
    while (i < args.len) : (i += 1) {
        const arg = args[i];

        if (mem.eql(u8, arg, "-h") or mem.eql(u8, arg, "--help")) {
            show_help = true;
        } else if (mem.eql(u8, arg, "-v") or mem.eql(u8, arg, "--version")) {
            show_version = true;
        } else if (mem.eql(u8, arg, "--verbose")) {
            verbose = true;
        } else if (mem.eql(u8, arg, "--tokens")) {
            show_tokens = true;
        } else if (mem.eql(u8, arg, "--hidden")) {
            include_hidden = true;
        } else if (mem.eql(u8, arg, "--model")) {
            i += 1;
            if (i < args.len) {
                model = parseModel(args[i]);
            }
        } else if (mem.eql(u8, arg, "--format") or mem.eql(u8, arg, "-f")) {
            i += 1;
            if (i < args.len) {
                format = parseFormat(args[i]);
            }
        } else if (mem.eql(u8, arg, "-o") or mem.eql(u8, arg, "--output")) {
            i += 1;
            if (i < args.len) {
                output_file = args[i];
            }
        } else if (arg[0] != '-') {
            path = arg;
        }
    }

    const stdout = std.fs.File.stdout().deprecatedWriter();

    if (show_version) {
        try stdout.print("infiniloom-scan {s}\n", .{version});
        return;
    }

    if (show_help) {
        try printHelp(stdout);
        return;
    }

    // Determine if we need to read file contents
    const need_contents = format != .stats;

    // Perform scan
    if (format == .stats) {
        try stdout.print("Scanning: {s}\n", .{path});
        try stdout.print("Model: {s}\n\n", .{model.name()});
    }

    var walker = scanner.Walker.init(allocator, .{
        .include_hidden = include_hidden,
        .respect_gitignore = true,
        .read_contents = need_contents,
    });
    defer walker.deinit();

    walker.walk(path) catch |err| {
        try stdout.print("Error scanning directory: {}\n", .{err});
        return;
    };

    const stats = walker.getStats();
    const files = walker.getFiles();

    // Generate output based on format
    switch (format) {
        .stats => try outputStats(allocator, stdout, stats, files, model, verbose, show_tokens),
        .xml => try outputXml(allocator, path, files, stats, output_file),
        .markdown => try outputMarkdown(allocator, path, files, stats, output_file),
        .json => try outputJson(allocator, path, files, stats, output_file),
        .plain => try outputPlain(allocator, path, files, output_file),
    }
}

fn outputStats(
    allocator: mem.Allocator,
    stdout: anytype,
    stats: scanner.ScanStats,
    files: []const scanner.FileEntry,
    model: tokenizer.TokenizerModel,
    verbose: bool,
    show_tokens: bool,
) !void {
    // Print results
    try stdout.print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n", .{});
    try stdout.print("  Scan Results\n", .{});
    try stdout.print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n", .{});

    try stdout.print("  Files:        {d}\n", .{stats.total_files});
    try stdout.writeAll("  Total Size:   ");
    try writeFormattedBytes(stdout, stats.total_bytes);
    try stdout.writeAll("\n");
    try stdout.print("  Scan Time:    {d}ms\n", .{stats.scan_time_ms});
    try stdout.print("\n", .{});

    // Show skipped stats
    if (verbose) {
        try stdout.print("  Skipped:\n", .{});
        try stdout.print("    Binary:     {d}\n", .{stats.skipped_binary});
        try stdout.print("    Size:       {d}\n", .{stats.skipped_size});
        try stdout.print("    Gitignore:  {d}\n", .{stats.skipped_gitignore});
        try stdout.print("    Hidden:     {d}\n", .{stats.skipped_hidden});
        try stdout.print("\n", .{});
    }

    // Language breakdown
    var lang_counts = std.StringHashMap(u32).init(allocator);
    defer lang_counts.deinit();

    for (files) |file| {
        const lang = file.language orelse "unknown";
        const entry = lang_counts.getOrPut(lang) catch continue;
        if (!entry.found_existing) {
            entry.value_ptr.* = 0;
        }
        entry.value_ptr.* += 1;
    }

    try stdout.print("  Languages:\n", .{});
    var lang_iter = lang_counts.iterator();
    while (lang_iter.next()) |entry| {
        try stdout.print("    {s}: {d}\n", .{ entry.key_ptr.*, entry.value_ptr.* });
    }

    // Show token estimates
    if (show_tokens) {
        try stdout.print("\n  Token Estimates:\n", .{});

        var total_tokens: u64 = 0;
        for (files) |file| {
            const estimated = @as(u64, @intFromFloat(@as(f32, @floatFromInt(file.size)) / model.charsPerToken()));
            total_tokens += estimated;
        }

        try stdout.print("    {s}: ~{d}\n", .{ model.name(), total_tokens });
    }

    // List files if verbose
    if (verbose) {
        try stdout.print("\n  Files:\n", .{});
        for (files) |file| {
            try stdout.print("    {s} (", .{file.relative_path});
            try writeFormattedBytes(stdout, file.size);
            try stdout.writeAll(")\n");
        }
    }

    try stdout.print("\n", .{});
}

fn outputXml(
    allocator: mem.Allocator,
    path: []const u8,
    files: []const scanner.FileEntry,
    stats: scanner.ScanStats,
    output_file: ?[]const u8,
) !void {
    var output: std.ArrayList(u8) = .empty;
    defer output.deinit(allocator);

    const writer = output.writer(allocator);

    // XML header
    try writer.print("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", .{});
    try writer.print("<repository name=\"{s}\" version=\"1.0.0\">\n", .{getRepoName(path)});

    // Metadata
    try writer.print("  <metadata>\n", .{});
    try writer.print("    <stats>\n", .{});
    try writer.print("      <files>{d}</files>\n", .{stats.total_files});
    try writer.print("      <bytes>{d}</bytes>\n", .{stats.total_bytes});
    try writer.print("      <scan_time_ms>{d}</scan_time_ms>\n", .{stats.scan_time_ms});
    try writer.print("    </stats>\n", .{});

    // Language breakdown
    var lang_counts = std.StringHashMap(u32).init(allocator);
    defer lang_counts.deinit();

    for (files) |file| {
        const lang = file.language orelse "unknown";
        const entry = try lang_counts.getOrPut(lang);
        if (!entry.found_existing) {
            entry.value_ptr.* = 0;
        }
        entry.value_ptr.* += 1;
    }

    try writer.print("    <languages>\n", .{});
    var lang_iter = lang_counts.iterator();
    while (lang_iter.next()) |entry| {
        try writer.print("      <language name=\"{s}\" files=\"{d}\"/>\n", .{ entry.key_ptr.*, entry.value_ptr.* });
    }
    try writer.print("    </languages>\n", .{});
    try writer.print("  </metadata>\n\n", .{});

    // Directory structure
    try writer.print("  <directory_structure><![CDATA[\n", .{});
    try writeDirectoryTree(allocator, writer, files);
    try writer.print("  ]]></directory_structure>\n\n", .{});

    // Files with content
    try writer.print("  <files>\n", .{});
    for (files) |file| {
        try writer.print("    <file path=\"{s}\"", .{escapeXmlAttr(file.relative_path)});
        if (file.language) |lang| {
            try writer.print(" language=\"{s}\"", .{lang});
        }
        try writer.print(" size=\"{d}\">\n", .{file.size});

        if (file.content) |content| {
            try writer.print("      <content><![CDATA[\n", .{});
            // Write content with line numbers
            var line_num: u32 = 1;
            var line_start: usize = 0;
            for (content, 0..) |c, idx| {
                if (c == '\n' or idx == content.len - 1) {
                    const line_end = if (c == '\n') idx else idx + 1;
                    const line = content[line_start..line_end];
                    try writer.print("{d:4} | {s}\n", .{ line_num, line });
                    line_num += 1;
                    line_start = idx + 1;
                }
            }
            try writer.print("      ]]></content>\n", .{});
        }
        try writer.print("    </file>\n", .{});
    }
    try writer.print("  </files>\n", .{});

    try writer.print("</repository>\n", .{});

    // Output to file or stdout
    try writeOutput(output.items, output_file);
}

fn outputMarkdown(
    allocator: mem.Allocator,
    path: []const u8,
    files: []const scanner.FileEntry,
    stats: scanner.ScanStats,
    output_file: ?[]const u8,
) !void {
    var output: std.ArrayList(u8) = .empty;
    defer output.deinit(allocator);

    const writer = output.writer(allocator);

    // Header
    try writer.print("# Repository: {s}\n\n", .{getRepoName(path)});
    try writer.print("## Statistics\n\n", .{});
    try writer.print("- **Files**: {d}\n", .{stats.total_files});
    try writer.writeAll("- **Total Size**: ");
    try writeFormattedBytes(writer, stats.total_bytes);
    try writer.writeAll("\n");
    try writer.print("- **Scan Time**: {d}ms\n\n", .{stats.scan_time_ms});

    // Language breakdown
    var lang_counts = std.StringHashMap(u32).init(allocator);
    defer lang_counts.deinit();

    for (files) |file| {
        const lang = file.language orelse "unknown";
        const entry = try lang_counts.getOrPut(lang);
        if (!entry.found_existing) {
            entry.value_ptr.* = 0;
        }
        entry.value_ptr.* += 1;
    }

    try writer.print("## Languages\n\n", .{});
    var lang_iter = lang_counts.iterator();
    while (lang_iter.next()) |entry| {
        try writer.print("- {s}: {d} files\n", .{ entry.key_ptr.*, entry.value_ptr.* });
    }
    try writer.print("\n", .{});

    // Directory structure
    try writer.print("## Directory Structure\n\n```\n", .{});
    try writeDirectoryTree(allocator, writer, files);
    try writer.print("```\n\n", .{});

    // Files
    try writer.print("## Files\n\n", .{});
    for (files) |file| {
        try writer.print("### {s}\n\n", .{file.relative_path});

        if (file.content) |content| {
            const lang_hint = file.language orelse "";
            try writer.print("```{s}\n", .{lang_hint});
            try writer.print("{s}", .{content});
            if (content.len > 0 and content[content.len - 1] != '\n') {
                try writer.print("\n", .{});
            }
            try writer.print("```\n\n", .{});
        }
    }

    try writeOutput(output.items, output_file);
}

fn outputJson(
    allocator: mem.Allocator,
    path: []const u8,
    files: []const scanner.FileEntry,
    stats: scanner.ScanStats,
    output_file: ?[]const u8,
) !void {
    var output: std.ArrayList(u8) = .empty;
    defer output.deinit(allocator);

    const writer = output.writer(allocator);

    try writer.writeAll("{\n");
    try writer.writeAll("  \"repository\": \"");
    try writeJsonEscaped(writer, getRepoName(path));
    try writer.writeAll("\",\n");
    try writer.print("  \"statistics\": {{\n", .{});
    try writer.print("    \"files\": {d},\n", .{stats.total_files});
    try writer.print("    \"bytes\": {d},\n", .{stats.total_bytes});
    try writer.print("    \"scan_time_ms\": {d}\n", .{stats.scan_time_ms});
    try writer.writeAll("  },\n");

    try writer.writeAll("  \"files\": [\n");
    for (files, 0..) |file, idx| {
        try writer.writeAll("    {\n");
        try writer.writeAll("      \"path\": \"");
        try writeJsonEscaped(writer, file.relative_path);
        try writer.writeAll("\",\n");
        try writer.writeAll("      \"language\": ");
        if (file.language) |lang| {
            try writer.writeAll("\"");
            try writeJsonEscaped(writer, lang);
            try writer.writeAll("\",\n");
        } else {
            try writer.writeAll("null,\n");
        }
        try writer.print("      \"size\": {d},\n", .{file.size});
        try writer.writeAll("      \"content\": ");
        if (file.content) |content| {
            try writer.writeAll("\"");
            try writeJsonEscaped(writer, content);
            try writer.writeAll("\"\n");
        } else {
            try writer.writeAll("null\n");
        }
        try writer.writeAll("    }");
        if (idx < files.len - 1) {
            try writer.writeAll(",");
        }
        try writer.writeAll("\n");
    }
    try writer.writeAll("  ]\n");
    try writer.writeAll("}\n");

    try writeOutput(output.items, output_file);
}

fn outputPlain(
    allocator: mem.Allocator,
    path: []const u8,
    files: []const scanner.FileEntry,
    output_file: ?[]const u8,
) !void {
    var output: std.ArrayList(u8) = .empty;
    defer output.deinit(allocator);

    const writer = output.writer(allocator);

    try writer.print("Repository: {s}\n", .{getRepoName(path)});
    try writer.print("============================================================\n\n", .{});

    for (files) |file| {
        try writer.print("--- {s} ---\n", .{file.relative_path});
        if (file.content) |content| {
            try writer.print("{s}", .{content});
            if (content.len > 0 and content[content.len - 1] != '\n') {
                try writer.print("\n", .{});
            }
        }
        try writer.print("\n", .{});
    }

    try writeOutput(output.items, output_file);
}

fn writeDirectoryTree(allocator: mem.Allocator, writer: anytype, files: []const scanner.FileEntry) !void {
    // Collect unique directories
    var dirs = std.StringHashMap(void).init(allocator);
    defer dirs.deinit();

    for (files) |file| {
        // Add all parent directories
        var path_copy = file.relative_path;
        while (std.mem.lastIndexOf(u8, path_copy, "/")) |idx| {
            const dir = path_copy[0..idx];
            try dirs.put(dir, {});
            path_copy = dir;
        }
    }

    // Sort and print directories first
    var dir_list: std.ArrayList([]const u8) = .empty;
    defer dir_list.deinit(allocator);

    var dir_iter = dirs.keyIterator();
    while (dir_iter.next()) |key| {
        try dir_list.append(allocator, key.*);
    }

    std.mem.sort([]const u8, dir_list.items, {}, struct {
        fn lessThan(_: void, a: []const u8, b: []const u8) bool {
            return std.mem.lessThan(u8, a, b);
        }
    }.lessThan);

    for (dir_list.items) |dir| {
        const depth = std.mem.count(u8, dir, "/");
        var indent: usize = 0;
        while (indent < depth) : (indent += 1) {
            try writer.print("  ", .{});
        }
        // Get just the directory name
        const name = if (std.mem.lastIndexOf(u8, dir, "/")) |idx|
            dir[idx + 1 ..]
        else
            dir;
        try writer.print("{s}/\n", .{name});
    }

    // Print files
    for (files) |file| {
        const depth = std.mem.count(u8, file.relative_path, "/");
        var indent: usize = 0;
        while (indent < depth) : (indent += 1) {
            try writer.print("  ", .{});
        }
        // Get just the filename
        const name = if (std.mem.lastIndexOf(u8, file.relative_path, "/")) |idx|
            file.relative_path[idx + 1 ..]
        else
            file.relative_path;
        try writer.print("{s}\n", .{name});
    }
}

fn writeOutput(data: []const u8, output_file: ?[]const u8) !void {
    if (output_file) |path| {
        var file = try fs.cwd().createFile(path, .{});
        defer file.close();
        try file.writeAll(data);

        const msg_stdout = std.fs.File.stdout().deprecatedWriter();
        try msg_stdout.print("Output written to: {s} ({d} bytes)\n", .{ path, data.len });
    } else {
        const out_stdout = std.fs.File.stdout().deprecatedWriter();
        try out_stdout.writeAll(data);
    }
}

fn getRepoName(path: []const u8) []const u8 {
    // Get the last component of the path
    if (mem.lastIndexOf(u8, path, "/")) |idx| {
        return path[idx + 1 ..];
    }
    if (mem.eql(u8, path, ".")) {
        return "current";
    }
    return path;
}

fn escapeXmlAttr(s: []const u8) []const u8 {
    // For simplicity, just return as-is (proper impl would escape)
    return s;
}

fn escapeJsonString(s: []const u8) []const u8 {
    // Unused but kept for compatibility
    _ = s;
    return "";
}

fn writeJsonEscaped(writer: anytype, s: []const u8) !void {
    for (s) |c| {
        switch (c) {
            '"' => try writer.writeAll("\\\""),
            '\\' => try writer.writeAll("\\\\"),
            '\n' => try writer.writeAll("\\n"),
            '\r' => try writer.writeAll("\\r"),
            '\t' => try writer.writeAll("\\t"),
            0x08 => try writer.writeAll("\\b"), // backspace
            0x0C => try writer.writeAll("\\f"), // form feed
            else => {
                if (c < 0x20) {
                    // Other control characters as unicode escape
                    try writer.print("\\u{x:0>4}", .{c});
                } else {
                    try writer.writeByte(c);
                }
            },
        }
    }
}

fn parseModel(name: []const u8) tokenizer.TokenizerModel {
    if (mem.eql(u8, name, "claude")) return .claude;
    if (mem.eql(u8, name, "gpt4o") or mem.eql(u8, name, "gpt-4o")) return .gpt4o;
    if (mem.eql(u8, name, "gpt4") or mem.eql(u8, name, "gpt-4")) return .gpt4;
    if (mem.eql(u8, name, "gemini")) return .gemini;
    if (mem.eql(u8, name, "llama")) return .llama;
    if (mem.eql(u8, name, "codellama")) return .codellama;
    return .claude;
}

fn parseFormat(name: []const u8) OutputFormat {
    if (mem.eql(u8, name, "xml")) return .xml;
    if (mem.eql(u8, name, "markdown") or mem.eql(u8, name, "md")) return .markdown;
    if (mem.eql(u8, name, "json")) return .json;
    if (mem.eql(u8, name, "plain") or mem.eql(u8, name, "txt")) return .plain;
    return .stats;
}

fn writeFormattedBytes(writer: anytype, bytes: u64) !void {
    const units = [_][]const u8{ "B", "KB", "MB", "GB" };
    var size: f64 = @floatFromInt(bytes);
    var unit_idx: usize = 0;

    while (size >= 1024 and unit_idx < units.len - 1) {
        size /= 1024;
        unit_idx += 1;
    }

    if (unit_idx == 0) {
        try writer.print("{d} {s}", .{ bytes, units[unit_idx] });
    } else {
        try writer.print("{d:.1} {s}", .{ size, units[unit_idx] });
    }
}

fn printHelp(writer: anytype) !void {
    try writer.print(
        \\infiniloom-scan - Repository scanner for Infiniloom
        \\
        \\USAGE:
        \\    infiniloom-scan [OPTIONS] [PATH]
        \\
        \\ARGS:
        \\    PATH    Directory to scan (default: current directory)
        \\
        \\OPTIONS:
        \\    -h, --help           Show this help message
        \\    -v, --version        Show version
        \\    --verbose            Show detailed output
        \\    --tokens             Show token count estimates
        \\    --hidden             Include hidden files
        \\    --model <MODEL>      Target model for token counting
        \\                         (claude, gpt4o, gpt4, gemini, llama, codellama)
        \\    -f, --format <FMT>   Output format:
        \\                         stats (default), xml, markdown, json, plain
        \\    -o, --output <FILE>  Write output to file instead of stdout
        \\
        \\EXAMPLES:
        \\    infiniloom-scan                        # Show scan statistics
        \\    infiniloom-scan ./src --format xml     # Generate XML output
        \\    infiniloom-scan . -f xml -o repo.xml   # Save XML to file
        \\    infiniloom-scan --format markdown      # Generate Markdown
        \\    infiniloom-scan --tokens --verbose     # Detailed stats with tokens
        \\
        \\OUTPUT FORMATS:
        \\    stats     Show scan statistics only (default, fast)
        \\    xml       Claude-optimized XML with file contents
        \\    markdown  GPT-optimized Markdown with code blocks
        \\    json      Generic JSON format
        \\    plain     Plain text with file contents
        \\
    , .{});
}
