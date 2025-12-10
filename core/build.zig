const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Main library - use exports.zig as root to ensure C ABI symbols are exported
    const lib = b.addLibrary(.{
        .name = "infiniloom-core",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/exports.zig"),
            .target = target,
            .optimize = optimize,
        }),
        .linkage = .static,
    });

    // Link libc for tree-sitter
    lib.linkLibC();

    b.installArtifact(lib);

    // Shared library for FFI
    const shared_lib = b.addLibrary(.{
        .name = "infiniloom-core-shared",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/exports.zig"),
            .target = target,
            .optimize = optimize,
        }),
        .linkage = .dynamic,
    });

    shared_lib.linkLibC();
    b.installArtifact(shared_lib);

    // CLI executable for testing
    const exe = b.addExecutable(.{
        .name = "infiniloom-scan",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/cli.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    exe.linkLibC();
    b.installArtifact(exe);

    // Unit tests
    const unit_tests = b.addTest(.{
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    const run_unit_tests = b.addRunArtifact(unit_tests);
    const test_step = b.step("test", "Run unit tests");
    test_step.dependOn(&run_unit_tests.step);

    // WASM target
    const wasm_target = b.resolveTargetQuery(.{
        .cpu_arch = .wasm32,
        .os_tag = .freestanding,
    });

    const wasm_lib = b.addLibrary(.{
        .name = "infiniloom-wasm",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/wasm.zig"),
            .target = wasm_target,
            .optimize = .ReleaseSmall,
        }),
        .linkage = .static,
    });

    const wasm_step = b.step("wasm", "Build WASM library");
    wasm_step.dependOn(&b.addInstallArtifact(wasm_lib, .{}).step);
}
