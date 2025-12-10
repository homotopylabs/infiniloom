const std = @import("std");

pub const scanner = @import("scanner/walker.zig");
pub const parallel_scanner = @import("scanner/parallel.zig");
pub const gitignore = @import("scanner/gitignore.zig");
pub const binary = @import("scanner/binary.zig");
pub const mmap_reader = @import("scanner/mmap.zig");
pub const tokenizer = @import("tokenizer/counter.zig");
pub const bpe = @import("tokenizer/bpe.zig");
pub const compressor = @import("compressor/rules.zig");

// Re-export key types
pub const Walker = scanner.Walker;
pub const ParallelWalker = parallel_scanner.ParallelWalker;
pub const WalkConfig = scanner.WalkConfig;
pub const FileEntry = scanner.FileEntry;
pub const GitignoreParser = gitignore.GitignoreParser;
pub const TokenizerModel = tokenizer.TokenizerModel;
pub const CompressionLevel = compressor.CompressionLevel;
pub const MappedFile = mmap_reader.MappedFile;
pub const BpeTokenizer = bpe.BpeTokenizer;

// C ABI exports for Rust/Python/Node integration
pub const exports = @import("exports.zig");

test {
    std.testing.refAllDecls(@This());
}
