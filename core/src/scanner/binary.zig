const std = @import("std");

/// Check if data appears to be binary
/// Uses multiple heuristics to detect binary content
pub fn isBinary(data: []const u8) bool {
    if (data.len == 0) return false;

    // Check for common binary file signatures (magic numbers)
    if (hasBinarySignature(data)) return true;

    // Check for null bytes and control characters
    const check_len = @min(data.len, 8192);
    const sample = data[0..check_len];

    var null_count: usize = 0;
    var control_count: usize = 0;
    var high_byte_count: usize = 0;

    for (sample) |byte| {
        if (byte == 0) {
            null_count += 1;
        } else if (byte < 32 and !isAllowedControl(byte)) {
            control_count += 1;
        } else if (byte > 127) {
            high_byte_count += 1;
        }
    }

    // Any null byte typically indicates binary
    if (null_count > 0) return true;

    // More than 10% control characters indicates binary
    if (control_count * 10 > check_len) return true;

    // Check if it looks like valid UTF-8 with many high bytes
    // This allows UTF-8 text files with non-ASCII characters
    if (high_byte_count > 0) {
        if (!std.unicode.utf8ValidateSlice(sample)) {
            return true;
        }
    }

    return false;
}

/// Check for allowed control characters (tab, newline, carriage return)
fn isAllowedControl(byte: u8) bool {
    return byte == '\t' or byte == '\n' or byte == '\r';
}

/// Check for common binary file signatures
fn hasBinarySignature(data: []const u8) bool {
    if (data.len < 4) return false;

    const signatures = [_][]const u8{
        // Images
        &[_]u8{ 0xFF, 0xD8, 0xFF }, // JPEG
        &[_]u8{ 0x89, 0x50, 0x4E, 0x47 }, // PNG
        &[_]u8{ 0x47, 0x49, 0x46, 0x38 }, // GIF
        &[_]u8{ 0x42, 0x4D }, // BMP
        &[_]u8{ 0x00, 0x00, 0x01, 0x00 }, // ICO
        &[_]u8{ 0x52, 0x49, 0x46, 0x46 }, // WEBP (RIFF)

        // Archives
        &[_]u8{ 0x50, 0x4B, 0x03, 0x04 }, // ZIP/JAR/DOCX/etc
        &[_]u8{ 0x1F, 0x8B }, // GZIP
        &[_]u8{ 0x42, 0x5A, 0x68 }, // BZIP2
        &[_]u8{ 0xFD, 0x37, 0x7A, 0x58, 0x5A }, // XZ
        &[_]u8{ 0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C }, // 7Z
        &[_]u8{ 0x52, 0x61, 0x72, 0x21 }, // RAR

        // Executables
        &[_]u8{ 0x7F, 0x45, 0x4C, 0x46 }, // ELF
        &[_]u8{ 0x4D, 0x5A }, // DOS/PE (Windows EXE)
        &[_]u8{ 0xCF, 0xFA, 0xED, 0xFE }, // Mach-O 64-bit
        &[_]u8{ 0xCE, 0xFA, 0xED, 0xFE }, // Mach-O 32-bit
        &[_]u8{ 0xCA, 0xFE, 0xBA, 0xBE }, // Java class / Universal Mach-O

        // Audio/Video
        &[_]u8{ 0x49, 0x44, 0x33 }, // MP3 (ID3)
        &[_]u8{ 0xFF, 0xFB }, // MP3 (no ID3)
        &[_]u8{ 0x66, 0x4C, 0x61, 0x43 }, // FLAC
        &[_]u8{ 0x00, 0x00, 0x00 }, // MP4/MOV (partial, needs more context)

        // Documents
        &[_]u8{ 0x25, 0x50, 0x44, 0x46 }, // PDF
        &[_]u8{ 0xD0, 0xCF, 0x11, 0xE0 }, // MS Office (OLE)

        // Fonts
        &[_]u8{ 0x00, 0x01, 0x00, 0x00 }, // TrueType
        &[_]u8{ 0x4F, 0x54, 0x54, 0x4F }, // OpenType
        &[_]u8{ 0x77, 0x4F, 0x46, 0x46 }, // WOFF
        &[_]u8{ 0x77, 0x4F, 0x46, 0x32 }, // WOFF2

        // Databases
        &[_]u8{ 0x53, 0x51, 0x4C, 0x69, 0x74, 0x65 }, // SQLite
    };

    for (signatures) |sig| {
        if (data.len >= sig.len and std.mem.eql(u8, data[0..sig.len], sig)) {
            return true;
        }
    }

    return false;
}

/// Detect file encoding
pub const Encoding = enum {
    utf8,
    utf8_bom,
    utf16_le,
    utf16_be,
    utf32_le,
    utf32_be,
    ascii,
    latin1,
    unknown,
};

pub fn detectEncoding(data: []const u8) Encoding {
    if (data.len == 0) return .ascii;

    // Check BOM (Byte Order Mark)
    if (data.len >= 4) {
        // UTF-32 LE BOM
        if (data[0] == 0xFF and data[1] == 0xFE and data[2] == 0x00 and data[3] == 0x00) {
            return .utf32_le;
        }
        // UTF-32 BE BOM
        if (data[0] == 0x00 and data[1] == 0x00 and data[2] == 0xFE and data[3] == 0xFF) {
            return .utf32_be;
        }
    }

    if (data.len >= 3) {
        // UTF-8 BOM
        if (data[0] == 0xEF and data[1] == 0xBB and data[2] == 0xBF) {
            return .utf8_bom;
        }
    }

    if (data.len >= 2) {
        // UTF-16 LE BOM
        if (data[0] == 0xFF and data[1] == 0xFE) {
            return .utf16_le;
        }
        // UTF-16 BE BOM
        if (data[0] == 0xFE and data[1] == 0xFF) {
            return .utf16_be;
        }
    }

    // Check if valid UTF-8
    if (std.unicode.utf8ValidateSlice(data)) {
        // Check if pure ASCII
        var has_high_byte = false;
        for (data) |byte| {
            if (byte > 127) {
                has_high_byte = true;
                break;
            }
        }
        return if (has_high_byte) .utf8 else .ascii;
    }

    // Could be Latin-1 or other single-byte encoding
    return .latin1;
}

/// Get human-readable description of encoding
pub fn encodingName(encoding: Encoding) []const u8 {
    return switch (encoding) {
        .utf8 => "UTF-8",
        .utf8_bom => "UTF-8 with BOM",
        .utf16_le => "UTF-16 LE",
        .utf16_be => "UTF-16 BE",
        .utf32_le => "UTF-32 LE",
        .utf32_be => "UTF-32 BE",
        .ascii => "ASCII",
        .latin1 => "Latin-1",
        .unknown => "Unknown",
    };
}

test "binary detection" {
    // Text content
    try std.testing.expect(!isBinary("Hello, World!"));
    try std.testing.expect(!isBinary("def main():\n    print('hello')\n"));
    try std.testing.expect(!isBinary("const x = 42;\n"));

    // Binary content (null bytes)
    try std.testing.expect(isBinary(&[_]u8{ 0x00, 0x01, 0x02 }));
    try std.testing.expect(isBinary("Hello\x00World"));

    // Binary signatures
    try std.testing.expect(isBinary(&[_]u8{ 0x7F, 'E', 'L', 'F' })); // ELF
    try std.testing.expect(isBinary(&[_]u8{ 0x89, 'P', 'N', 'G' })); // PNG
    try std.testing.expect(isBinary(&[_]u8{ 0x50, 0x4B, 0x03, 0x04 })); // ZIP
}

test "encoding detection" {
    // ASCII
    try std.testing.expectEqual(Encoding.ascii, detectEncoding("Hello World"));

    // UTF-8 with BOM
    try std.testing.expectEqual(Encoding.utf8_bom, detectEncoding(&[_]u8{ 0xEF, 0xBB, 0xBF, 'H', 'i' }));

    // UTF-32 LE with BOM (FF FE 00 00)
    try std.testing.expectEqual(Encoding.utf32_le, detectEncoding(&[_]u8{ 0xFF, 0xFE, 0x00, 0x00 }));

    // UTF-8 (non-ASCII)
    const utf8_text = "Héllo Wörld";
    try std.testing.expectEqual(Encoding.utf8, detectEncoding(utf8_text));
}
